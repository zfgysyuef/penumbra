/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use log::info;
use penumbra::connection::BROM_SLA_MAX_DATA_SIZE;
use penumbra::connection::port::ConnectionType;
use penumbra::core::devinfo::DevInfoData;
use penumbra::error::{Error as PenumbraError, Result as PenumbraResult};
use penumbra::{Device, DeviceBuilder, find_mtk_port};
use tokio::fs::read;

use crate::cli::CliArgs;
use crate::cli::helpers::logging::setup_file_logger;
use crate::cli::state::PersistedDeviceState;

const DA_LOG_FILE: &str = "da.log";
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
// Exact fixed envelope used by the modified SPFlashV6 Auth.exe before its
// 16-byte one-time BROM challenge. The embedded ASCII field is part of that
// tool's signing API contract and must not be replaced with the SoC ID.
const MI_AUTH_BLOB_PREFIX_HEX: &str =
    "020000000134022030323131343442303342313642414236423835363530353631373344364641390310";
const MI_AUTH_SIGNATURE_SIZE: usize = 0x100;

fn encode_base64(data: &[u8]) -> String {
    let mut encoded = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let value = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        encoded.push(BASE64_ALPHABET[((value >> 18) & 0x3F) as usize] as char);
        encoded.push(BASE64_ALPHABET[((value >> 12) & 0x3F) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64_ALPHABET[((value >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(value & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn decode_base64(encoded: &str) -> std::result::Result<Vec<u8>, String> {
    let encoded: Vec<u8> = encoded.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect();
    if encoded.is_empty() || !encoded.len().is_multiple_of(4) {
        return Err("Base64 length must be a non-zero multiple of 4".into());
    }

    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let mut decoded = Vec::with_capacity(encoded.len() / 4 * 3);
    let chunk_count = encoded.len() / 4;
    for (index, chunk) in encoded.chunks_exact(4).enumerate() {
        let last = index + 1 == chunk_count;
        let a = value(chunk[0]).ok_or_else(|| "invalid Base64 character".to_string())?;
        let b = value(chunk[1]).ok_or_else(|| "invalid Base64 character".to_string())?;
        let c = if chunk[2] == b'=' {
            if !last || chunk[3] != b'=' || b & 0x0F != 0 {
                return Err("invalid Base64 padding".into());
            }
            None
        } else {
            Some(value(chunk[2]).ok_or_else(|| "invalid Base64 character".to_string())?)
        };
        let d = match (c, chunk[3]) {
            (None, b'=') => None,
            (None, _) => return Err("invalid Base64 padding".into()),
            (Some(c), b'=') => {
                if !last || c & 0x03 != 0 {
                    return Err("invalid Base64 padding".into());
                }
                None
            }
            (Some(_), byte) => {
                Some(value(byte).ok_or_else(|| "invalid Base64 character".to_string())?)
            }
        };

        decoded.push((a << 2) | (b >> 4));
        if let Some(c) = c {
            decoded.push((b << 4) | (c >> 2));
            if let Some(d) = d {
                decoded.push((c << 6) | d);
            }
        }
    }
    Ok(decoded)
}

fn decode_mi_auth_signature(input: &str) -> std::result::Result<Vec<u8>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("SIGN cannot be empty".into());
    }

    let (format, encoded) = if let Some(value) = input.strip_prefix("hex:") {
        (Some("hex"), value)
    } else if let Some(value) = input.strip_prefix("base64:") {
        (Some("base64"), value)
    } else {
        (None, input)
    };
    let encoded: String = encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect();

    let looks_like_hex = !encoded.is_empty()
        && encoded.len().is_multiple_of(2)
        && encoded.bytes().all(|byte| byte.is_ascii_hexdigit());
    let signature = match format {
        Some("hex") => hex::decode(&encoded).map_err(|error| format!("invalid hex: {error}"))?,
        Some("base64") => decode_base64(&encoded)?,
        _ if looks_like_hex => {
            hex::decode(&encoded).map_err(|error| format!("invalid hex: {error}"))?
        }
        _ => decode_base64(&encoded).map_err(|error| format!("expected hex or Base64: {error}"))?,
    };

    if signature.is_empty() {
        return Err("SIGN cannot be empty".into());
    }
    if signature.len() > BROM_SLA_MAX_DATA_SIZE {
        return Err(format!(
            "SIGN is too large ({} bytes; maximum is {} bytes)",
            signature.len(),
            BROM_SLA_MAX_DATA_SIZE
        ));
    }
    if !signature.len().is_multiple_of(2) {
        return Err(format!("SIGN length must be even, got {} bytes", signature.len()));
    }

    Ok(signature)
}

fn swap_u16_bytes(data: &mut [u8]) {
    for word in data.chunks_exact_mut(2) {
        word.swap(0, 1);
    }
}

fn build_mi_auth_signing_blob(callback_blob: &[u8]) -> std::result::Result<Vec<u8>, String> {
    // SP Flash Tool gives its callback SOC_ID[32] || swap16(challenge[16]).
    // The modified Auth.exe ignores that SOC_ID, restores the challenge's
    // device byte order, prepends its own fixed TLV envelope, then Base64
    // encodes the resulting 58 bytes. That representation starts with AgAA.
    if callback_blob.len() != 48 {
        return Err(format!(
            "modified SPFlashV6 MI auth expects a 48-byte callback BLOB, got {} bytes",
            callback_blob.len()
        ));
    }

    let mut challenge = callback_blob[32..].to_vec();
    swap_u16_bytes(&mut challenge);

    let mut signing_blob =
        hex::decode(MI_AUTH_BLOB_PREFIX_HEX).expect("hardcoded MI auth BLOB prefix is valid hex");
    signing_blob.extend_from_slice(&challenge);
    Ok(signing_blob)
}

fn read_mi_auth_signature<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    blob: &[u8],
) -> PenumbraResult<Vec<u8>> {
    let signing_blob = build_mi_auth_signing_blob(blob).map_err(PenumbraError::penumbra)?;
    writeln!(
        writer,
        "\nMI-AUTH BLOB (modified SPFlashV6 Base64 / flashToken, {} bytes):",
        signing_blob.len()
    )?;
    writeln!(writer, "{}", encode_base64(&signing_blob))?;
    writeln!(writer, "MI-AUTH BLOB (HEX):")?;
    writeln!(writer, "{}", hex::encode_upper(&signing_blob))?;
    writeln!(
        writer,
        "This BLOB is valid only for the current connection. Paste the matching SIGN below."
    )?;

    loop {
        write!(writer, "SIGN (hex or Base64): ")?;
        writer.flush()?;

        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(PenumbraError::penumbra(
                "End of input while waiting for the MI authentication SIGN",
            ));
        }

        match decode_mi_auth_signature(&line) {
            Ok(mut signature) => {
                if signature.len() != MI_AUTH_SIGNATURE_SIZE {
                    writeln!(
                        writer,
                        "Invalid SIGN: modified SPFlashV6 requires exactly {} bytes, got {}. Please try again.",
                        MI_AUTH_SIGNATURE_SIZE,
                        signature.len()
                    )?;
                    continue;
                }
                // Auth.exe swaps each 16-bit word before returning SIGN to
                // SP Flash Tool. The core transport performs SPFT's second
                // swap before USB transmission, restoring signer byte order.
                swap_u16_bytes(&mut signature);
                return Ok(signature);
            }
            Err(error) => {
                writeln!(writer, "Invalid SIGN: {error}. Please try again.")?;
            }
        }
    }
}

fn prompt_mi_auth_signature(blob: &[u8]) -> PenumbraResult<Vec<u8>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    read_mi_auth_signature(&mut reader, &mut writer, blob)
}

fn should_run_mi_auth(requested: bool, connection_type: ConnectionType) -> bool {
    requested && connection_type != ConnectionType::Da
}

pub async fn setup_device(args: &CliArgs, state: &mut PersistedDeviceState) -> Result<Device> {
    let usb_log_channel = state.usb_log || args.usb_log;

    let da_data = if let Some(da_path) = &args.da_file {
        let data = read(da_path).await?;
        state.da_file_path = Some(da_path.to_string_lossy().to_string());
        Some(data)
    } else {
        None
    };

    let pl_data = if let Some(pl_path) = &args.preloader_file {
        let data = read(pl_path).await?;
        Some(data)
    } else {
        None
    };

    let auth_data = if let Some(auth_path) = &args.auth_file {
        let data = read(auth_path).await?;
        Some(data)
    } else {
        None
    };

    let mut last_seen = Instant::now();
    let timeout = Duration::from_millis(500);

    info!("Waiting for MTK device...");
    let mtk_port = loop {
        if let Some(port) = find_mtk_port() {
            info!("Found MTK port: {}", port.get_port_name());
            break port;
        } else if last_seen.elapsed() > timeout {
            state.reset().await?;
            last_seen = Instant::now();
        }
    };
    let port_connection_type = mtk_port.get_connection_type();
    let needs_mi_auth = should_run_mi_auth(args.mi_auth, port_connection_type);
    if args.mi_auth && port_connection_type == ConnectionType::Da {
        return Err(anyhow!(
            "Device is already in DA mode; reconnect it in Preloader/BROM mode to perform --mi-auth"
        ));
    }
    if needs_mi_auth && state.flash_mode != 0 {
        info!(
            "--mi-auth was requested; ignoring the cached DA session and requesting a fresh one-time challenge"
        );
    }

    let mut builder = DeviceBuilder::default()
        .with_mtk_port(mtk_port)
        .with_verbose(args.verbose)
        .with_usb_log_channel(usb_log_channel)
        .with_force_heapb8(args.force_heapb8);

    if usb_log_channel {
        if let Some(device_log) = setup_file_logger(DA_LOG_FILE).await {
            builder = builder.with_device_log(device_log);
        }
    }

    builder = if let Some(da) = da_data {
        builder.with_da_data(da)
    } else if let Some(da_path_str) = &state.da_file_path {
        let da_path = Path::new(da_path_str);
        let data = read(da_path).await?;
        builder.with_da_data(data)
    } else {
        builder
    };

    builder = if let Some(pl) = pl_data { builder.with_preloader(pl) } else { builder };
    builder = if let Some(auth) = auth_data { builder.with_auth(auth) } else { builder };

    let mut dev = builder.build()?;

    if state.hw_code != 0 && !needs_mi_auth {
        let dev_info = DevInfoData {
            soc_id: state.soc_id,
            meid: state.meid,
            hw_code: state.hw_code,
            partitions: vec![],
            target_config: state.target_config,
            bootctrl: None,
        };

        if state.flash_mode != 0 {
            dev.set_connection_type(ConnectionType::Da)?;
        }

        dev.dev_info.set_chip(penumbra::core::chip::chip_from_hw_code(state.hw_code));
        dev.reinit(dev_info)?;
    } else {
        info!("Initializing device...");
        if needs_mi_auth {
            dev.init_with_brom_sla(prompt_mi_auth_signature)?;
        } else {
            dev.init()?;
        }

        state.soc_id = dev.dev_info.soc_id();
        state.meid = dev.dev_info.meid();
        state.hw_code = dev.dev_info.hw_code();
        state.target_config = dev.dev_info.target_config();
    }

    info!("=====================================");
    info!("SBC: {}", (state.target_config & 0x1) != 0);
    info!("SLA: {}", (state.target_config & 0x2) != 0);
    info!("DAA: {}", (state.target_config & 0x4) != 0);
    info!("=====================================");

    Ok(dev)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn decodes_hex_and_base64_signatures() {
        assert_eq!(decode_mi_auth_signature("00112233").unwrap(), [0x00, 0x11, 0x22, 0x33]);
        assert_eq!(decode_mi_auth_signature("base64:ABEiMw==").unwrap(), [0x00, 0x11, 0x22, 0x33]);
    }

    #[test]
    fn rejects_empty_or_odd_length_signatures() {
        assert!(decode_mi_auth_signature("").is_err());
        assert!(decode_mi_auth_signature("hex:001122").is_err());
    }

    #[test]
    fn reproduces_modified_spflashv6_agaa_blob() {
        let mut callback_blob = [0xAA; 48];
        callback_blob[32..].copy_from_slice(&[
            0xBB, 0x22, 0xA8, 0x6C, 0xE3, 0xA9, 0x32, 0x5D, 0xF7, 0xE3, 0x29, 0x10, 0x3D, 0x62,
            0x23, 0x7B,
        ]);

        let signing_blob = build_mi_auth_signing_blob(&callback_blob).unwrap();

        assert_eq!(signing_blob.len(), 58);
        assert_eq!(
            encode_base64(&signing_blob),
            "AgAAAAE0AiAwMjExNDRCMDNCMTZCQUI2Qjg1NjUwNTYxNzNENkZBOQMQIrtsqKnjXTLj9xApYj17Iw=="
        );
    }

    #[test]
    fn prints_blob_once_and_retries_invalid_sign() {
        let mut blob = [0xAA; 48];
        blob[32..].copy_from_slice(&[
            0xBB, 0x22, 0xA8, 0x6C, 0xE3, 0xA9, 0x32, 0x5D, 0xF7, 0xE3, 0x29, 0x10, 0x3D, 0x62,
            0x23, 0x7B,
        ]);
        let valid_sign = "0011".repeat(128);
        let mut input = Cursor::new(format!("not-a-sign\n{valid_sign}\n").into_bytes());
        let mut output = Vec::new();

        let sign = read_mi_auth_signature(&mut input, &mut output, &blob).unwrap();
        let output = String::from_utf8(output).unwrap();

        assert_eq!(sign.len(), 0x100);
        assert_eq!(&sign[..4], [0x11, 0x00, 0x11, 0x00]);
        assert_eq!(output.matches("AgAAAAE0").count(), 1);
        assert!(output.contains("Invalid SIGN"));
    }

    #[test]
    fn runs_mi_auth_only_in_preloader_or_brom_mode() {
        assert!(should_run_mi_auth(true, ConnectionType::Brom));
        assert!(should_run_mi_auth(true, ConnectionType::Preloader));
        assert!(!should_run_mi_auth(true, ConnectionType::Da));
        assert!(!should_run_mi_auth(false, ConnectionType::Brom));
    }
}
