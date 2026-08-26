/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::io::{BufWriter, Read, Write};
use std::time::Duration;

use log::{debug, error, info, trace, warn};

use crate::VERSION;
use crate::connection::Connection;
use crate::core::auth::{AuthManager, SignData, SignPurpose, SignRequest};
use crate::core::devinfo::DeviceInfo;
use crate::core::log_buffer::DeviceLog;
use crate::core::storage::StorageKind;
use crate::core::traits::ToBytes;
use crate::da::protocol::{DAProtocolParams, DataType, PacketHeader};
use crate::da::xml::cmds::{
    CMD_END,
    CMD_START,
    FileSystemOp,
    GetSysProperty,
    HostSupportedCommands,
    NotifyInitHw,
    SecurityGetDevFwInfo,
    SecuritySetFlashPolicy,
    SetHostInfo,
    SetRuntimeParameter,
    XmlCmdLifetime,
    XmlCommand,
    create_cmd,
};
#[cfg(not(feature = "no_exploits"))]
use crate::da::xml::exts::boot_extensions;
use crate::da::xml::storage::detect_storage;
use crate::da::{DA, DownloadProtocol};
use crate::error::{Error, Result, XmlError, XmlErrorKind};
use crate::utilities::xml::{get_tag, get_tag_usize};

pub struct Xml {
    pub conn: Connection,
    pub da: DA,
    pub dev_info: DeviceInfo,
    #[allow(dead_code)]
    pub(super) using_exts: bool,
    #[allow(dead_code)]
    pub(super) read_packet_length: Option<usize>,
    pub(super) write_packet_length: Option<usize>,
    pub(super) rpmb_authenticated_regions: u8,
    pub(super) patch: bool,
    pub(super) verbose: bool,
    pub(super) force_heapb8: bool,
    pub(super) usb_log_channel: bool,
    pub(super) device_log: DeviceLog,
}

impl Xml {
    pub fn new(conn: Connection, params: DAProtocolParams) -> Self {
        Self {
            conn,
            da: params.da,
            dev_info: params.devinfo,
            using_exts: false,
            read_packet_length: None,
            write_packet_length: None,
            rpmb_authenticated_regions: 0,
            patch: true,
            verbose: params.verbose,
            force_heapb8: params.force_heapb8,
            usb_log_channel: params.usb_log_channel,
            device_log: params.device_log,
        }
    }

    fn read_next_flow_header(&mut self) -> Result<PacketHeader> {
        loop {
            let mut buf = [0u8; PacketHeader::SIZE];
            self.conn.read(&mut buf)?;

            let hdr = PacketHeader::from_bytes(&buf).ok_or_else(|| {
                debug!("[RX] Invalid packet header bytes: {:02X?}", buf);
                Error::io(format!("Invalid packet header: {:02X?}", buf))
            })?;

            match hdr.data_type {
                DataType::Flow => return Ok(hdr),
                DataType::Message => self.drain_message(hdr.length)?,
            }
        }
    }

    fn drain_message(&mut self, length: u32) -> Result<()> {
        let mut payload = vec![0u8; length as usize];
        self.conn.read(&mut payload)?;

        let body = String::from_utf8_lossy(&payload[4..]).into_owned();

        trace!("[DA Message] {}", body);

        if self.usb_log_channel {
            self.device_log.push(body);
        }

        Ok(())
    }

    /// Reads data of arbitrary length from the device.
    pub fn read_data(&mut self) -> Result<Vec<u8>> {
        let hdr = self.read_next_flow_header()?;

        debug!("[RX] Packet header received: 0x{:X} bytes", hdr.length);

        let mut data = vec![0u8; hdr.length as usize];
        self.conn.read(&mut data)?;
        Ok(data)
    }

    pub(super) fn generate_header(&self, data: &[u8]) -> [u8; PacketHeader::SIZE] {
        let hdr = PacketHeader::new(data.len() as u32);
        debug!("[TX] Packet header sent: 0x{:X} bytes", data.len());
        hdr.to_bytes()
    }

    /// Checks for the lifetime acknowledgment (CMD:START or CMD:END).
    fn check_lifetime(&mut self, lifetime: XmlCmdLifetime) -> Result<bool> {
        let data = match self.read_data() {
            Ok(d) => d,
            Err(Error::Timeout) => {
                return Ok(true);
            }
            Err(e) => return Err(e),
        };

        let pattern: &[u8] = match lifetime {
            XmlCmdLifetime::CmdStart => CMD_START,
            XmlCmdLifetime::CmdEnd => CMD_END,
        };

        if data.windows(20).any(|window| window == b"<result>ERR</result>") {
            // We need to ack before returning, or the device will hang.
            self.ack(None)?;
            return Err(Error::proto("Device command reported ERR"));
        }

        Ok(data.windows(pattern.len()).any(|window| window == pattern))
    }

    /// Sends an acknowledgment to the device.
    /// By default, it sends "OK\0".
    /// If a value is provided, it sends "OK@{value}\0".
    pub fn ack(&mut self, value: Option<usize>) -> Result<bool> {
        if let Some(v) = value {
            self.send(format!("OK@{v}\0").as_bytes())
        } else {
            self.send(b"OK\0")
        }
    }

    /// Reads an acknowledgment from the device.
    pub fn read_ack(&mut self) -> Result<()> {
        let resp = self.read_data()?;
        let s = String::from_utf8_lossy(&resp);
        let ack = s.trim_end_matches('\0').trim();

        if ack == "OK" {
            return Ok(());
        }

        if let Some(status) = ack.strip_prefix("OK@0x") {
            let status = u32::from_str_radix(status, 16)
                .map_err(|_| Error::proto(format!("Invalid status acknowledgment: {ack}")))?;

            if status == 0 {
                return Ok(());
            }

            return Err(Error::proto(format!("Device returned status 0x{status:08X}")));
        }

        if s.contains("ERR!UNSUPPORTED") {
            return Err(Error::Xml(XmlError::from_message(&resp)));
        }

        if ack.starts_with("ERR") {
            return Err(Error::proto(format!("Device returned error acknowledgment: {ack}")));
        }

        Err(Error::proto(format!("Invalid acknowledgment: {ack}")))
    }

    /// Acknowledges the lifetime of an XML command (CMD:START or CMD:END).
    pub fn lifetime_ack(&mut self, lifetime: XmlCmdLifetime) -> Result<bool> {
        let is_valid = self.check_lifetime(lifetime)?;
        if !is_valid {
            return Err(Error::io("Invalid lifetime acknowledgment"));
        }
        self.ack(None)
    }

    /// Sends an XML command to the device.
    pub fn send_cmd<C: XmlCommand>(&mut self, cmd: &C) -> Result<bool> {
        let xml_str = create_cmd(cmd);
        let xml_bytes = xml_str.as_bytes();

        self.lifetime_ack(XmlCmdLifetime::CmdStart)?;
        self.send(xml_bytes)?;

        debug!("Sent XML Command: {}", cmd);

        // Read the ack back.
        // We don't wait for CMD:END here, because each CMD might
        // perform different actions in between.
        match self.read_ack() {
            Ok(_) => Ok(true),
            Err(Error::Xml(err)) if err.kind == XmlErrorKind::UnsupportedCmd => {
                self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;
                Err(Error::Xml(err))
            }
            Err(e) => Err(e),
        }
    }

    /// Sends a file to the device.
    pub fn download_file<R, F>(&mut self, size: usize, mut reader: R, mut progress: F) -> Result<()>
    where
        R: Read,
        F: FnMut(usize, usize) + Send,
    {
        let resp = self.read_data()?;
        let resp_string = String::from_utf8_lossy(&resp);

        let cmd: String = get_tag(&resp_string, "command")?;
        if cmd != "CMD:DOWNLOAD-FILE" {
            debug!("Invalid xml response for CMD:DOWNLOAD-FILE: {}", resp_string);
            return Err(Error::proto("Expected CMD:DOWNLOAD-FILE"));
        }

        let info: String = get_tag(&resp_string, "arg/info").unwrap_or_default();
        debug!("Received CMD:DOWNLOAD-FILE command.");
        debug!("  Info: {info}");

        // Acknowledge we received the command
        self.ack(None)?;

        // Tell the device the size we want to send
        self.ack(Some(size))?;
        // Read the response
        self.read_ack()?;

        let packet_length: usize = get_tag_usize(&resp_string, "arg/packet_length")?;

        let mut chunk = vec![0u8; packet_length];
        let mut bytes_sent = 0;

        while bytes_sent < size {
            let to_read = packet_length.min(size - bytes_sent);
            reader.read_exact(&mut chunk[..to_read])?;

            // Status
            self.ack(Some(0))?;
            self.read_ack()?;

            self.send(&chunk[..to_read])?;
            self.read_ack()?;

            bytes_sent += to_read;
            progress(bytes_sent, size);
        }

        debug!("File download completed, 0x{:X} bytes sent.", size);
        Ok(())
    }

    /// Receives a file from the device.
    pub fn upload_file<W, F>(&mut self, mut writer: W, mut progress: F) -> Result<usize>
    where
        F: FnMut(usize, usize) + Send,
        W: Write,
    {
        let resp = self.read_data()?;
        let resp_string = String::from_utf8_lossy(&resp);

        let cmd: String = get_tag(&resp_string, "command")?;
        if cmd != "CMD:UPLOAD-FILE" {
            debug!("Invalid xml response for CMD:UPLOAD-FILE: {}", resp_string);
            return Err(Error::proto("Expected CMD:UPLOAD-FILE"));
        }

        let info: String = get_tag(&resp_string, "arg/info").unwrap_or_default();
        debug!("Received CMD:UPLOAD-FILE command.");
        debug!("  Info: {info}");

        self.ack(None)?;

        let length_resp = self.read_data()?;
        let length_str = String::from_utf8_lossy(&length_resp);

        let size = {
            let trimmed = length_str.trim_end_matches('\0').trim();
            let hex = trimmed
                .strip_prefix("OK@0x")
                .ok_or_else(|| Error::proto("Invalid response format, expected OK@0x<hex>\\0"))?;

            usize::from_str_radix(hex, 16)
                .map_err(|_| Error::proto("Invalid hex number in OK@0x<...>\\0"))?
        };

        self.ack(None)?;

        let packet_length: usize = get_tag_usize(&resp_string, "arg/packet_length")?;
        let mut bytes_received = 0;

        while bytes_received < size {
            let to_read = packet_length.min(size - bytes_received);
            self.read_ack()?;
            self.ack(None)?;
            let data = self.read_data()?;
            writer.write_all(&data)?;
            self.ack(None)?;

            bytes_received += to_read;
            progress(bytes_received, size);
        }

        debug!("File upload completed, 0x{:X} bytes received.", size);

        Ok(bytes_received)
    }

    /// Waits for the device to finish a certain operation, reporting progress.
    pub fn progress_report<F>(&mut self, mut progress: F) -> Result<bool>
    where
        F: FnMut(usize, usize) + Send,
    {
        let resp = self.read_data()?;
        let resp_string = String::from_utf8_lossy(&resp);

        let cmd: String = get_tag(&resp_string, "command")?;
        if cmd != "CMD:PROGRESS-REPORT" {
            return Err(Error::proto("Expected CMD:PROGRESS-REPORT"));
        }

        let msg: String = get_tag(&resp_string, "arg/message")?;
        debug!("Received progress report command. Message: {msg}");

        self.ack(None)?;

        // Progress report might make the device delay a bit during USB
        // transfers. As a solution, we increase the port timeout
        // while we're waiting for the progress report, and restore it afterwards.
        self.conn.port.set_timeout(Some(Duration::from_secs(3)))?;

        let mut resp: Vec<u8> = Vec::new();
        while resp != b"OK!EOT\0" {
            resp = self.read_data()?;
            self.ack(None)?;

            let resp_string = String::from_utf8_lossy(&resp);

            if !resp_string.starts_with("OK!PROGRESS@") {
                continue;
            }

            let prog = resp_string
                .trim_end_matches('\0')
                .split('@')
                .nth(1)
                .ok_or_else(|| Error::proto("Invalid progress format"))?;

            let progress_value: usize =
                prog.parse().map_err(|_| Error::proto("Invalid progress value"))?;

            progress(progress_value, 100);
        }

        progress(100, 100);
        self.conn.port.set_timeout(None)?;

        Ok(true)
    }

    /// Perform a (fake) file system operation
    /// This is used in SPFT for asking the tool to do stuff like creating directories,
    /// checking file existence, etc.
    /// We don't need it.
    pub fn file_system_op(&mut self, op: FileSystemOp) -> Result<bool> {
        let resp = self.read_data()?;
        let resp_string = String::from_utf8_lossy(&resp);

        let cmd: String = get_tag(&resp_string, "command")?;
        if cmd != "CMD:FILE-SYS-OPERATION" {
            return Err(Error::proto("Expected CMD:FILE-SYS-OPERATION"));
        }

        debug!("Received file system operation command: {cmd}");
        self.ack(None)?;
        self.send(format!("OK@{}\0", op.default()).as_bytes())?;

        Ok(true)
    }

    pub(super) fn upload_stage1(
        &mut self,
        addr: u32,
        length: u32,
        data: Vec<u8>,
        sig_len: u32,
    ) -> Result<bool> {
        info!(
            "[Penumbra] Uploading XML DA1 region to address 0x{:08X} with length 0x{:X}",
            addr, length
        );

        self.conn.send_da(&data, length, addr, sig_len)?;
        info!("[Penumbra] Sent XML DA1, jumping to address 0x{:08X}...", addr);
        self.conn.jump_da(addr)?;

        let log_level = if self.verbose { "DEBUG" } else { "INFO" };
        let channel = if self.usb_log_channel { "USB" } else { "UART" };

        xmlcmd_e!(self, SetRuntimeParameter, log_level, channel,)?;
        xmlcmd_e!(self, HostSupportedCommands)?;
        // Wait for the device to initialize DRAM
        xmlcmd!(self, NotifyInitHw)?;
        let mock_progress = |_, _| {};
        self.progress_report(mock_progress)?;
        self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

        xmlcmd_e!(self, SetHostInfo, format!("Penumbra v{}", VERSION))?;

        Ok(true)
    }

    pub(super) fn get_or_detect_storage(&mut self) -> Option<StorageKind> {
        if self.dev_info.storage().is_none() {
            let detected = detect_storage(self)?;
            self.dev_info.set_storage(detected);
        }

        self.dev_info.storage()
    }

    pub fn get_upload_file_resp(&mut self) -> Result<String> {
        let mut buffer = Vec::new();
        let mut writer = BufWriter::new(&mut buffer);
        let progress = |_, _| {};

        self.upload_file(&mut writer, progress)?;
        writer.flush()?;
        drop(writer);

        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    pub(super) fn handle_sla(&mut self) -> Result<bool> {
        xmlcmd!(self, GetSysProperty, "DA.SLA")?;

        let response = self.get_upload_file_resp()?;
        self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

        let sla_enabled = response.contains("ENABLED");
        if !sla_enabled {
            return Ok(true);
        }

        info!("DA SLA is enabled");

        let da2_data = self.da.get_da2().map_or_else(Vec::new, |da2| da2.data.clone());

        let auth = AuthManager::get();
        let progress = |_, _| {};

        if !auth.can_sign(&da2_data) {
            #[cfg(not(feature = "no_exploits"))]
            {
                info!("No available signers for DA SLA, trying dummy signature...");
                let dummy_sig = [0u8; 256];
                xmlcmd!(self, SecuritySetFlashPolicy, "Penumbra Dummy SLA challenge")?;
                self.download_file(dummy_sig.len(), dummy_sig.as_slice(), progress)?;
                if self.lifetime_ack(XmlCmdLifetime::CmdEnd).is_ok() {
                    info!("DA SLA signature accepted (dummy)!");
                    return Ok(true);
                }
            }

            error!("No signer available for DA SLA! Can't proceed.");
            return Err(Error::penumbra(
                "DA SLA is enabled, but no signer is available. Can't continue.",
            ));
        }

        xmlcmd!(self, SecurityGetDevFwInfo)?;
        let fw_info = self.get_upload_file_resp()?;
        self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

        debug!("Firmware info: {}", fw_info);
        let rnd_str = get_tag::<String>(&fw_info, "rnd")?;
        let hrid_str = get_tag::<String>(&fw_info, "hrid")?;
        let socid_str = get_tag::<String>(&fw_info, "socid")?;
        let rnd = hex::decode(rnd_str).map_err(|_| Error::proto("Invalid rnd response"))?;
        let hrid = hex::decode(hrid_str).map_err(|_| Error::proto("Invalid hrid response"))?;
        let soc_id = hex::decode(socid_str).map_err(|_| Error::proto("Invalid socid response"))?;

        let sign_data = SignData { rnd, hrid, soc_id, raw: fw_info.into() };
        let sign_req =
            SignRequest { data: sign_data, purpose: SignPurpose::DaSla, pubk_mod: da2_data };

        info!("Found signer for DA SLA!");
        let signed_rnd = auth.sign(&sign_req)?;
        info!("Signed DA SLA challenge. Uploading to device...");

        xmlcmd!(self, SecuritySetFlashPolicy, "Penumbra SLA challenge")?;
        self.download_file(signed_rnd.len(), signed_rnd.as_slice(), progress)?;
        self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;
        info!("DA SLA signature accepted!");
        Ok(true)
    }

    #[cfg(not(feature = "no_exploits"))]
    pub(super) fn boot_extensions(&mut self) -> Result<bool> {
        if self.using_exts {
            warn!("DA extensions already in use, skipping re-upload");
            return Ok(true);
        }
        info!("Booting DA extensions...");
        self.using_exts = boot_extensions(self)?;
        Ok(true)
    }
}
