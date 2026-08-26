/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::io::{Cursor, Read, Write};

use log::{debug, info};
use wincode::SchemaWrite;
use xmlcmd_derive::XmlCommand;

use crate::core::ToBytes;
use crate::core::storage::{RPMB_FRAME_DATA_SZ, RpmbRegion, Storage, StorageType};
use crate::da::DownloadProtocol;
use crate::da::xml::Xml;
use crate::da::xml::cmds::{XmlCmdLifetime, XmlCommand};
use crate::da::xml::patch::to_arch;
use crate::error::{Error, Result};
use crate::exploit::get_v6_payload;
use crate::utilities::analysis::{ArchAnalyzer, create_analyzer};
use crate::utilities::patching::bytes_to_hex;
use crate::utilities::xml::get_tag;

const DA_EXT: &[u8] = include_bytes!("../../../payloads/da_xml.bin");
const POINTER_TABLE_MAGIC: u32 = 0x54525450;

#[derive(SchemaWrite, ToBytes)]
#[repr(C)]
struct ExtPointerTableLegacy {
    magic: u32,
    uart_base: u32,
    reg_cmd: u32,
    malloc: u32,
    free: u32,
    mmc_get_card: u32,
}

#[derive(SchemaWrite, ToBytes)]
#[repr(C)]
struct ExtPointerTable {
    magic: u32,
    uart_base: u32,
    reg_cmd: u32,
    malloc: u32,
    free: u32,
    mmc_get_card: u32,
    ufs_get_lu: u32,
    ufs_get_tag: u32,
    ufs_queuecommand: u32,
    ufs_put_tag: u32,
    da_key_derive: u32,
    ufs_read_desc: u32,
}

#[derive(XmlCommand)]
pub struct ExtAck;

#[derive(XmlCommand)]
pub struct ExtDaCtx {
    #[xml(tag = "sej_base", fmt = "0x{sej_base:X}")]
    sej_base: u32,
    #[xml(tag = "tzcc_base", fmt = "0x{tzcc_base:X}")]
    tzcc_base: u32,
    #[xml(tag = "ssr_base", fmt = "0x{ssr_base:X}")]
    ssr_base: u32,
    #[xml(tag = "da2_base", fmt = "0x{da2_base:X}")]
    da2_base: u32,
    #[xml(tag = "da2_size", fmt = "0x{da2_size:X}")]
    da2_size: u32,
    #[xml(tag = "storage")]
    storage: String,
    #[xml(tag = "usb_log")]
    usb_log: String,
}

#[derive(XmlCommand)]
pub struct ExtReadMem {
    #[xml(tag = "address", fmt = "0x{address:X}")]
    address: u32,
    #[xml(tag = "length", fmt = "0x{length:X}")]
    length: usize,
}

#[derive(XmlCommand)]
pub struct ExtWriteMem {
    #[xml(tag = "address", fmt = "0x{address:X}")]
    address: u32,
    #[xml(tag = "length", fmt = "0x{length:X}")]
    length: usize,
}

#[derive(XmlCommand)]
pub struct ExtKeyDerive {
    #[xml(tag = "key_type")]
    key_type: String,
}

#[derive(XmlCommand)]
pub struct ExtSej {
    #[xml(tag = "encrypt")]
    encrypt: String,
    #[xml(tag = "ac")]
    anti_clone: String,
    #[xml(tag = "length", fmt = "0x{length:X}")]
    length: u32,
}

#[derive(XmlCommand)]
pub struct ExtRpmbInit {
    #[xml(tag = "partition", fmt = "{partition}")]
    partition: u32,
    #[xml(tag = "key")]
    key: String,
}

#[derive(XmlCommand)]
pub struct ExtRpmbRead {
    #[xml(tag = "partition", fmt = "{partition}")]
    partition: u32,
    #[xml(tag = "start_sector", fmt = "{start_sector}")]
    start_sector: u32,
    #[xml(tag = "sectors_count", fmt = "{sectors_count}")]
    sectors_count: u32,
}

#[derive(XmlCommand)]
pub struct ExtRpmbWrite {
    #[xml(tag = "partition", fmt = "{partition}")]
    partition: u32,
    #[xml(tag = "start_sector", fmt = "{start_sector}")]
    start_sector: u32,
    #[xml(tag = "sectors_count", fmt = "{sectors_count}")]
    sectors_count: u32,
}

#[derive(XmlCommand)]
pub struct ExtRpmbInfo {
    #[xml(tag = "partition", fmt = "{partition}")]
    partition: u32,
}

fn ack_extensions(xml: &mut Xml) -> Result<bool> {
    if xmlcmd!(xml, ExtAck).is_err() {
        debug!("Extensions did not reply to EXT-ACK");
        return Ok(false);
    }

    let response = match xml.get_upload_file_resp() {
        Ok(resp) => resp,
        Err(_) => {
            xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;
            debug!("Failed to get extension ack response");
            return Ok(false);
        }
    };

    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    let ack: String = get_tag(&response, "status")?;
    if ack != "OK" {
        debug!("DA extensions returned non-OK ack: {}", ack);
        return Ok(false);
    }

    Ok(true)
}

fn configure_extensions(xml: &mut Xml) -> Result<()> {
    let sej_base = xml.chip().sej_base();
    let tzcc_base = xml.chip().tzcc_base();
    let ssr_base = xml.chip().ssr_base();
    let da2_base = xml.da.get_da2().map(|da2| da2.addr).unwrap_or(0);
    let da2_size = xml.da.get_da2().map(|da2| da2.data.len() as u32).unwrap_or(0);
    let storage = xml.get_storage().map_or("Unknown", |s| s.as_str());
    let usb_log = if xml.usb_log_channel { "yes" } else { "no" };

    xmlcmd_e!(xml, ExtDaCtx, sej_base, tzcc_base, ssr_base, da2_base, da2_size, storage, usb_log)?;

    Ok(())
}

pub fn boot_extensions(xml: &mut Xml) -> Result<bool> {
    let ext_data = match prepare_extensions(xml) {
        Some(data) => data,
        None => {
            info!(
                "Failed to prepare XML extensions. This DA may not expose the eMMC symbols required by the bundled extensions."
            );
            return Ok(false);
        }
    };

    debug!("Trying booting XML extensions...");

    let ext_addr = 0x68000000;
    let ext_size = DA_EXT.len() as u32;

    info!("Uploading XML extensions to 0x{:08X} (0x{:X} bytes)", ext_addr, ext_size);

    let boot_to_resp = xml.boot_to(ext_addr, &ext_data).unwrap_or(false);
    if !boot_to_resp {
        info!("Failed to upload XML extensions, continuing without extensions");
        return Ok(false);
    }

    if !ack_extensions(xml)? {
        info!("Extensions did not reply, continuing without extensions");
        return Ok(false);
    }

    configure_extensions(xml)?;

    info!("Successfully booted XML extensions");

    Ok(true)
}

fn prepare_extensions(xml: &Xml) -> Option<Vec<u8>> {
    let da2address = xml.da.get_da2()?.addr;
    let da2data = &xml.da.get_da2()?.data;

    let is_arm64 = xml.da.is_arm64();
    let mut da_ext_data = get_v6_payload(DA_EXT, is_arm64).to_vec();

    let analyzer = create_analyzer(da2data.clone(), da2address as u64, to_arch(is_arm64));

    let off = analyzer.find_string_xref("CMD:REBOOT")?;
    let bl_off = analyzer.get_next_bl_from_off(off)?;
    let reg_cmd_addr = analyzer.get_bl_target(bl_off)? as u32;

    debug!("Reg CMD function at VA 0x{:X}", reg_cmd_addr);

    let off = analyzer.va_to_offset(reg_cmd_addr as u64)?;
    let bl_off = analyzer.get_next_bl_from_off(off)?;
    let malloc_addr = analyzer.get_bl_target(bl_off)? as u32;

    debug!("Malloc function at VA 0x{:X}", malloc_addr);

    let off = analyzer.find_string_xref("Bad %s")?;
    let bl1 = analyzer.get_next_bl_from_off(off)?;
    let bl2 = analyzer.get_next_bl_from_off(bl1 + 4)?;
    let free_addr = analyzer.get_bl_target(bl2)? as u32;

    debug!("Free function at VA 0x{:X}", free_addr);

    let mmc_get_card = analyzer
        .find_function_from_string("mmc_switch_part")
        .and_then(|off| analyzer.get_next_bl_from_off(off))
        .and_then(|bl_off| analyzer.get_bl_target(bl_off))
        .map_or(0, |addr| addr as u32);

    if mmc_get_card == 0 {
        debug!("Could not locate mmc_get_card function, continuing with a null MMC pointer");
    } else {
        debug!("mmc_get_card function at VA 0x{:X}", mmc_get_card);
    }

    let (ufs_get_lu, ufs_get_tag, ufs_queuecommand, ufs_put_tag) = if is_arm64 {
        (0, 0, 0, 0)
    } else {
        find_ufs_rpmb_helpers(analyzer.as_ref()).unwrap_or((0, 0, 0, 0))
    };

    if ufs_get_lu != 0 && ufs_get_tag != 0 && ufs_queuecommand != 0 && ufs_put_tag != 0 {
        debug!(
            "UFS RPMB helpers at get_lu=0x{:X}, get_tag=0x{:X}, queuecommand=0x{:X}, put_tag=0x{:X}",
            ufs_get_lu, ufs_get_tag, ufs_queuecommand, ufs_put_tag
        );
    } else {
        debug!("Could not locate UFS RPMB helper functions");
    }

    let da_key_derive = if is_arm64 {
        0
    } else {
        find_native_da_key_derive(analyzer.as_ref()).unwrap_or(0)
    };

    if da_key_derive != 0 {
        info!("Using native DA key derivation helper at 0x{da_key_derive:08X}");
    } else {
        debug!("Could not locate native DA key derivation helper");
    }

    let ufs_read_desc = if is_arm64 {
        0
    } else {
        find_ufs_read_desc(analyzer.as_ref()).unwrap_or(0)
    };

    if ufs_read_desc != 0 {
        info!("Using native UFS descriptor reader at 0x{ufs_read_desc:08X}");
    } else {
        debug!("Could not locate native UFS descriptor query helper");
    }

    let uart_base = xml.chip().uart();

    debug!("UART base address at 0x{:X}", uart_base);

    if is_arm64 {
        // The bundled ARM64 payload still uses the original six-field table.
        // Keep its ABI intact instead of overwriting the preceding payload data.
        let table = ExtPointerTableLegacy {
            magic: POINTER_TABLE_MAGIC,
            uart_base,
            reg_cmd: reg_cmd_addr,
            malloc: malloc_addr,
            free: free_addr,
            mmc_get_card,
        };
        let off = da_ext_data.len().checked_sub(ExtPointerTableLegacy::SIZE)?;
        da_ext_data[off..off + ExtPointerTableLegacy::SIZE].copy_from_slice(&table.to_bytes());
    } else {
        let table = ExtPointerTable {
            magic: POINTER_TABLE_MAGIC,
            uart_base,
            reg_cmd: reg_cmd_addr,
            malloc: malloc_addr,
            free: free_addr,
            mmc_get_card,
            ufs_get_lu,
            ufs_get_tag,
            ufs_queuecommand,
            ufs_put_tag,
            da_key_derive,
            ufs_read_desc,
        };
        let off = da_ext_data.len().checked_sub(ExtPointerTable::SIZE)?;
        da_ext_data[off..off + ExtPointerTable::SIZE].copy_from_slice(&table.to_bytes());
    }

    Some(da_ext_data)
}

fn find_ufs_rpmb_helpers(analyzer: &dyn ArchAnalyzer) -> Option<(u32, u32, u32, u32)> {
    let read_counter_off = analyzer.find_function_from_string("rpmb_authen_read_counter")?;

    let get_lu_bl = analyzer.get_next_bl_from_off(read_counter_off)?;
    let ufs_get_lu = analyzer.get_bl_target(get_lu_bl)? as u32;

    let get_tag_bl = analyzer.get_next_bl_from_off(get_lu_bl + 4)?;
    let ufs_get_tag = analyzer.get_bl_target(get_tag_bl)? as u32;

    // Oplus/MTK ARM32 DA layout observed around rpmb_authen_read_counter:
    // +0x150: first ufshcd_queuecommand call, +0x2D4: tag release helper.
    let ufs_queuecommand = analyzer.get_bl_target(read_counter_off + 0x150)? as u32;
    let ufs_put_tag = analyzer.get_bl_target(read_counter_off + 0x2D4)? as u32;

    Some((ufs_get_lu, ufs_get_tag, ufs_queuecommand, ufs_put_tag))
}

fn find_native_da_key_derive(analyzer: &dyn ArchAnalyzer) -> Option<u32> {
    let offset = analyzer.find_function_from_string("key_derive fails")?;

    // Guard the dynamic string-based match with the ARM32 prologue used by
    // the three-argument native wrapper in the rubens/MT6895 DA. Calling an
    // unrelated function as a key helper would be unsafe.
    let expected = [0xE92D_48F0, 0xE28D_B010, 0xE24D_D030];
    for (index, instruction) in expected.into_iter().enumerate() {
        if analyzer.read_u32(offset + index * 4)? != instruction {
            return None;
        }
    }

    let address = analyzer.offset_to_va(offset)?;
    if address > u32::MAX as u64 || address & 3 != 0 {
        return None;
    }

    Some(address as u32)
}

fn find_ufs_read_desc(analyzer: &dyn ArchAnalyzer) -> Option<u32> {
    let offset = analyzer.find_function_from_string(
        "[UFS] failed reading descriptor. desc_id %d desc_len %d ret %d",
    )?;

    // MT6895 checked descriptor reader. ABI:
    // int fn(ufs, desc_id, index, selector, buffer, length).
    let expected = [0xE92D_48F0, 0xE28D_B010, 0xE24D_D010, 0xE1A0_4001];
    for (index, instruction) in expected.into_iter().enumerate() {
        if analyzer.read_u32(offset + index * 4)? != instruction {
            return None;
        }
    }

    let address = analyzer.offset_to_va(offset)?;
    if address > u32::MAX as u64 || address & 3 != 0 {
        return None;
    }
    Some(address as u32)
}

pub fn peek<W, F>(xml: &mut Xml, addr: u32, length: usize, writer: W, progress: F) -> Result<()>
where
    W: Write,
    F: FnMut(usize, usize) + Send,
{
    xmlcmd!(xml, ExtReadMem, addr, length)?;

    xml.upload_file(writer, progress)?;

    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    Ok(())
}

pub fn poke<R, F>(xml: &mut Xml, addr: u32, length: usize, reader: R, progress: F) -> Result<()>
where
    R: Read,
    F: FnMut(usize, usize) + Send,
{
    xmlcmd!(xml, ExtWriteMem, addr, length)?;

    xml.download_file(length, reader, progress)?;

    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    Ok(())
}

pub fn sej(xml: &mut Xml, data: &[u8], encrypt: bool, anti_clone: bool) -> Result<Vec<u8>> {
    let length = data.len() as u32;

    let encrypt_str = if encrypt { "yes" } else { "no" };
    let anti_clone_str = if anti_clone { "yes" } else { "no" };
    xmlcmd!(xml, ExtSej, encrypt_str, anti_clone_str, length)?;

    let mut buf = data.to_vec();
    let mut cursor = Cursor::new(&mut buf);
    let progress = |_: usize, _: usize| {};

    xml.download_file(length as usize, &mut cursor, progress)?;
    cursor.set_position(0);
    xml.upload_file(&mut cursor, progress)?;

    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    Ok(buf)
}

fn ensure_rpmb_extensions(xml: &mut Xml) -> Result<()> {
    if xml.using_exts {
        return Ok(());
    }

    info!("XML DA extensions are not marked as loaded; probing existing extension state");
    if ack_extensions(xml)? {
        configure_extensions(xml)?;
        xml.using_exts = true;
        info!("Reattached to already-loaded XML extensions");
        return Ok(());
    }

    info!("Existing XML extensions did not respond; attempting to boot extensions now");
    xml.boot_extensions()?;
    if xml.using_exts {
        return Ok(());
    }

    let storage = xml.get_storage().map_or("Unknown", |s| s.as_str());
    Err(Error::penumbra(format!(
        "XML DA extensions are not loaded; RPMB commands are unavailable for storage type {storage}."
    )))
}

fn init_rpmb(xml: &mut Xml, region: RpmbRegion) -> Result<()> {
    ensure_rpmb_extensions(xml)?;

    // Derive RPMB key (0 = RPMB)
    xmlcmd!(xml, ExtKeyDerive, "RPMB")?;
    let resp = xml.get_upload_file_resp()?;

    if let Ok(status) = get_tag::<String>(&resp, "status")
        && status != "OK"
    {
        let error = get_tag::<String>(&resp, "error").unwrap_or_else(|_| "unknown".to_string());
        xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;
        return Err(Error::penumbra(format!("RPMB key derivation failed: {error}")));
    }

    let key: String = get_tag(&resp, "result")?;
    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    if key.len() != 64 || !key.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::penumbra(format!(
            "RPMB key derivation returned an invalid key ({} hex chars)",
            key.len()
        )));
    }

    // The extension installs the key and authenticates it against the RPMB
    // write-counter response before allowing a write.
    xmlcmd_e!(xml, ExtRpmbInit, region as u32, key)?;
    xml.rpmb_authenticated_regions |= 1 << (region as u8);

    Ok(())
}

pub fn verify_derived_rpmb_key(xml: &mut Xml, region: RpmbRegion) -> Result<()> {
    init_rpmb(xml, region)
}

pub fn get_rpmb_sector_count(xml: &mut Xml, region: RpmbRegion) -> Result<u32> {
    ensure_rpmb_extensions(xml)?;

    xmlcmd!(xml, ExtRpmbInfo, region as u32)?;
    let response = xml.get_upload_file_resp()?;
    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    get_tag::<u32>(&response, "sector_count")
}

pub fn read_rpmb<W, F>(
    xml: &mut Xml,
    region: RpmbRegion,
    start_sector: u32,
    sectors_count: u32,
    writer: W,
    progress: F,
) -> Result<()>
where
    W: Write + Send,
    F: FnMut(usize, usize) + Send,
{
    let storage = match xml.get_storage() {
        Some(s) => s,
        None => {
            return Err(Error::penumbra("Failed to get storage information for RPMB read"));
        }
    };

    if storage.kind() == StorageType::Ufs {
        ensure_rpmb_extensions(xml)?;
        info!("Skipping RPMB key derivation/init for UFS RPMB read");
    } else {
        init_rpmb(xml, region)?;
    }

    if sectors_count == 0 {
        return Err(Error::penumbra("RPMB sector count must be greater than 0"));
    }

    let rpmb_size = storage.get_rpmb_size();
    if rpmb_size != 0 {
        let max_sectors = (rpmb_size / RPMB_FRAME_DATA_SZ as u64) as u32;
        if start_sector.checked_add(sectors_count).is_none_or(|end| end > max_sectors) {
            return Err(Error::penumbra("Requested RPMB read range is out of bounds"));
        }
    } else {
        info!("Device reports unknown RPMB size; skipping RPMB bounds check");
    }

    xmlcmd!(xml, ExtRpmbRead, region as u32, start_sector, sectors_count)?;
    if let Err(error) = xml.upload_file(writer, progress) {
        // A stream callback error is sent as ERR\0 before the command handler
        // returns. Consume CMD:END as well so the next command stays in sync.
        let _ = xml.lifetime_ack(XmlCmdLifetime::CmdEnd);
        return Err(error);
    }
    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    Ok(())
}

pub fn write_rpmb<R, F>(
    xml: &mut Xml,
    region: RpmbRegion,
    start_sector: u32,
    sectors_count: u32,
    reader: R,
    progress: F,
) -> Result<()>
where
    R: Read + Send,
    F: FnMut(usize, usize) + Send,
{
    let storage = match xml.get_storage() {
        Some(s) => s,
        None => {
            return Err(Error::penumbra("Failed to get storage information for RPMB write"));
        }
    };

    if storage.kind() == StorageType::Ufs {
        if xml.rpmb_authenticated_regions & (1 << (region as u8)) != 0 {
            ensure_rpmb_extensions(xml)?;
            info!("Using provided RPMB key for UFS RPMB write");
        } else {
            info!("No RPMB key was provided; attempting device-side UFS RPMB key derivation");
            init_rpmb(xml, region)?;
        }
    } else {
        init_rpmb(xml, region)?;
    }

    if sectors_count == 0 {
        return Err(Error::penumbra("RPMB sector count must be greater than 0"));
    }

    let rpmb_size = storage.get_rpmb_size();
    if rpmb_size != 0 {
        let max_sectors = (rpmb_size / RPMB_FRAME_DATA_SZ as u64) as u32;
        if start_sector.checked_add(sectors_count).is_none_or(|end| end > max_sectors) {
            return Err(Error::penumbra("Requested RPMB write range is out of bounds"));
        }
    } else {
        info!("Device reports unknown RPMB size; skipping RPMB bounds check");
    }

    let data_len = sectors_count as usize * RPMB_FRAME_DATA_SZ;

    xmlcmd!(xml, ExtRpmbWrite, region as u32, start_sector, sectors_count)?;
    if let Err(error) = xml.download_file(data_len, reader, progress) {
        // See the read path above: preserve the useful stream error while
        // draining the command lifetime packet.
        let _ = xml.lifetime_ack(XmlCmdLifetime::CmdEnd);
        return Err(error);
    }
    xml.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

    Ok(())
}

pub fn auth_rpmb(xml: &mut Xml, region: RpmbRegion, key: &[u8]) -> Result<()> {
    ensure_rpmb_extensions(xml)?;

    if key.len() != 32 {
        return Err(Error::penumbra("RPMB key must be exactly 32 bytes"));
    }

    let key = bytes_to_hex(key);
    xmlcmd_e!(xml, ExtRpmbInit, region as u32, key)?;
    xml.rpmb_authenticated_regions |= 1 << (region as u8);

    Ok(())
}
