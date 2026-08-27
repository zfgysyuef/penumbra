/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::io::{BufReader, Cursor, Read, Write};

use log::{debug, error, info};

use crate::connection::Connection;
use crate::connection::port::ConnectionType;
use crate::core::devinfo::DeviceInfo;
use crate::core::seccfg::LockFlag;
use crate::core::storage::{
    Gpt,
    Partition,
    PartitionKind,
    RpmbRegion,
    Storage,
    StorageKind,
    StorageType,
};
use crate::da::protocol::{BootMode, DownloadProtocol};
use crate::da::xml::cmds::{
    BootTo,
    HostSupportedCommands,
    NotifyInitHw,
    Reboot,
    SetBootMode,
    XmlCmdLifetime,
};
use crate::da::xml::flash;
#[cfg(not(feature = "no_exploits"))]
use crate::da::xml::sec::{parse_seccfg, write_seccfg};
#[cfg(not(feature = "no_exploits"))]
use crate::da::xml::{exts, patch};
use crate::da::{DA, DAEntryRegion, Xml};
use crate::error::{Error, Result};
use crate::exploit;
#[cfg(not(feature = "no_exploits"))]
use crate::exploit::{Carbonara, Exploit, HeapBait};

impl DownloadProtocol for Xml {
    fn upload_da(&mut self) -> Result<bool> {
        let da1 = self.da.get_da1().ok_or_else(|| Error::penumbra("DA1 region not found"))?;

        self.upload_stage1(da1.addr, da1.length, da1.data.clone(), da1.sig_len)
            .map_err(|e| Error::proto(format!("Failed to upload XML DA1: {e}")))?;

        let carbonara_succeeded = if self.force_heapb8 {
            info!("[Exploit] --force-heapb8 set, keeping HeapBait enabled after DA2 hash setup");
            let patch_before_carbonara = self.patch;
            exploit!(Carbonara, self);
            let succeeded = patch_before_carbonara && !self.patch;

            if succeeded {
                info!("[Exploit] Carbonara DA2 hash helper succeeded; forcing HeapBait next");
                self.patch = true;
            } else {
                info!(
                    "[Exploit] Carbonara DA2 hash helper unavailable; trying signed DA2 for HeapBait"
                );
            }

            succeeded
        } else {
            exploit!(Carbonara, self);
            false
        };

        let (da2_addr, da2_data) = {
            let da2 = self.da.get_da2().ok_or_else(|| Error::penumbra("DA2 region not found"))?;
            let data = if self.force_heapb8 && !carbonara_succeeded {
                info!("[Exploit] --force-heapb8 set, uploading signed XML DA2 before HeapBait");
                da2.data.clone()
            } else {
                let sig_len = da2.sig_len as usize;
                da2.data[..da2.data.len().saturating_sub(sig_len)].to_vec()
            };
            (da2.addr, data)
        };

        info!("Uploading and booting to XML DA2...");
        if let Err(e) = self.boot_to(da2_addr, &da2_data) {
            self.reboot(BootMode::Normal).ok();
            return Err(Error::proto(format!("Failed to upload XML DA2: {e}")));
        }

        info!("Successfully uploaded and booted to XML DA2");
        exploit!(HeapBait, self);

        // These may fail on some devices
        xmlcmd_e!(self, HostSupportedCommands).ok();

        xmlcmd!(self, NotifyInitHw)?;
        let mock_progress = |_, _| {};
        self.progress_report(mock_progress)?;
        self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;

        self.handle_sla()?;

        #[cfg(not(feature = "no_exploits"))]
        self.boot_extensions()?;

        Ok(true)
    }

    fn boot_to(&mut self, addr: u32, data: &[u8]) -> Result<bool> {
        xmlcmd!(self, BootTo, addr, addr, data.len())?;

        let reader = BufReader::new(Cursor::new(data));
        let progress = |_, _| {};
        self.download_file(data.len(), reader, progress)?;

        self.lifetime_ack(XmlCmdLifetime::CmdEnd)?;
        Ok(true)
    }

    fn send(&mut self, data: &[u8]) -> Result<bool> {
        self.send_data(&[data])
    }

    fn send_data(&mut self, data: &[&[u8]]) -> Result<bool> {
        let max_chunk_size = self.write_packet_length.unwrap_or(0x8000);

        for param in data {
            let hdr = self.generate_header(param);
            self.conn.write(&hdr)?;

            let mut pos = 0;
            while pos < param.len() {
                let end = (pos + max_chunk_size).min(param.len());
                let chunk = &param[pos..end];
                debug!("[TX] Sending chunk (0x{:X} bytes)", chunk.len());
                self.conn.write(chunk)?;
                pos = end;
            }

            debug!("[TX] Completed sending 0x{:X} bytes", param.len());
        }

        Ok(true)
    }

    /// We don't need it for XML DA
    fn get_status(&mut self) -> Result<u32> {
        Ok(0)
    }

    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down device...");

        xmlcmd_e!(self, Reboot, "IMMEDIATE")
            .map(|_| ())
            .map_err(|e| Error::proto(format!("Failed to shutdown device: {e}")))
    }

    fn reboot(&mut self, bootmode: BootMode) -> Result<()> {
        info!("Rebooting device into {:?} mode...", bootmode);
        match bootmode {
            BootMode::Normal | BootMode::HomeScreen => self.shutdown()?,
            mode => {
                let xml_mode = mode.to_text().unwrap();
                xmlcmd_e!(self, SetBootMode, xml_mode, "USB", "ON", "ON")?;
            }
        }

        Ok(())
    }

    fn read_flash<W, F>(
        &mut self,
        addr: u64,
        size: usize,
        section: PartitionKind,
        writer: W,
        progress: F,
    ) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send,
    {
        flash::read_flash(self, addr, size, section, writer, progress)
    }

    fn write_flash<R, F>(
        &mut self,
        addr: u64,
        size: usize,
        section: PartitionKind,
        reader: R,
        progress: F,
    ) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send,
    {
        flash::write_flash(self, addr, size, section, reader, progress)
    }

    fn erase_flash<F>(
        &mut self,
        addr: u64,
        size: usize,
        section: PartitionKind,
        progress: F,
    ) -> Result<()>
    where
        F: FnMut(usize, usize) + Send,
    {
        flash::erase_flash(self, addr, size, section, progress)
    }

    fn download<R, F>(&mut self, part_name: &str, size: usize, reader: R, progress: F) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send,
    {
        flash::download(self, part_name, size, reader, progress)
    }

    fn upload<W, F>(&mut self, part_name: &str, writer: W, progress: F) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send,
    {
        flash::upload(self, part_name, writer, progress)
    }

    fn format<F>(&mut self, part_name: &str, progress: F) -> Result<()>
    where
        F: FnMut(usize, usize) + Send,
    {
        flash::format(self, part_name, progress)
    }

    fn read32(&mut self, _addr: u32) -> Result<u32> {
        todo!()
    }

    fn write32(&mut self, _addr: u32, _value: u32) -> Result<()> {
        todo!()
    }

    fn get_usb_speed(&mut self) -> Result<u32> {
        todo!()
    }

    fn get_connection(&mut self) -> &mut Connection {
        &mut self.conn
    }

    fn set_connection_type(&mut self, conn_type: ConnectionType) -> Result<()> {
        self.conn.connection_type = conn_type;
        Ok(())
    }

    fn get_storage(&mut self) -> Option<StorageKind> {
        self.get_or_detect_storage()
    }

    fn get_storage_type(&mut self) -> StorageType {
        self.get_or_detect_storage().map_or(StorageType::Unknown, |s| s.kind())
    }

    fn get_partitions(&mut self) -> Vec<Partition> {
        let Some(storage) = self.get_storage() else {
            error!("[Penumbra] Failed to get storage for partition parsing");
            return Vec::new();
        };

        let pl1_size = storage.get_pl1_size() as usize;
        let pl1_part = storage.get_pl_part1();
        let pl2_size = storage.get_pl2_size() as usize;
        let pl2_part = storage.get_pl_part2();

        let mut partitions = vec![
            Partition::new("preloader", pl1_size, 0, pl1_part),
            Partition::new("preloader_backup", pl2_size, 0, pl2_part),
        ];

        let mut gpt_parts = Vec::new();

        for gpt_name in ["PGPT", "SGPT"] {
            let mut data = Vec::new();

            if self.upload(gpt_name, Cursor::new(&mut data), |_, _| {}).is_ok()
                && let Ok(gpt) = Gpt::parse(&data)
            {
                let parsed = Gpt::to_partitions(Some(&gpt), &storage);
                if !parsed.is_empty() {
                    gpt_parts = parsed;
                    break;
                }
            }
        }

        if gpt_parts.is_empty() {
            gpt_parts = Gpt::to_partitions(None, &storage);
        }

        partitions.append(&mut gpt_parts);
        partitions
    }

    #[cfg(not(feature = "no_exploits"))]
    fn set_seccfg_lock_state(&mut self, locked: LockFlag) -> Option<[u8; 512]> {
        let mut seccfg = match parse_seccfg(self) {
            Some(s) => s,
            None => {
                error!("[Penumbra] Failed to parse seccfg, cannot set lock state");
                return None;
            }
        };

        seccfg.set_lock_state(locked);
        write_seccfg(self, &mut seccfg)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn peek<W, F>(&mut self, addr: u32, length: usize, writer: W, progress: F) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send,
    {
        exts::peek(self, addr, length, writer, progress)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn poke<R, F>(&mut self, addr: u32, length: usize, reader: R, progress: F) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send,
    {
        exts::poke(self, addr, length, reader, progress)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn read_rpmb<W, F>(
        &mut self,
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
        exts::read_rpmb(self, region, start_sector, sectors_count, writer, progress)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn write_rpmb<R, F>(
        &mut self,
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
        exts::write_rpmb(self, region, start_sector, sectors_count, reader, progress)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn auth_rpmb(&mut self, region: RpmbRegion, key: &[u8]) -> Result<()> {
        exts::auth_rpmb(self, region, key)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn verify_derived_rpmb_key(&mut self, region: RpmbRegion) -> Result<()> {
        exts::verify_derived_rpmb_key(self, region)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn get_rpmb_sector_count(&mut self, region: RpmbRegion) -> Result<u32> {
        exts::get_rpmb_sector_count(self, region)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn get_rpmb_region_info(&mut self, region: RpmbRegion) -> Result<(bool, u32)> {
        exts::get_rpmb_region_info(self, region)
    }

    #[cfg(not(feature = "no_exploits"))]
    fn patch_da(&mut self) -> Option<DA> {
        patch::patch_da(self).ok()
    }

    #[cfg(not(feature = "no_exploits"))]
    fn patch_da1(&mut self) -> Option<DAEntryRegion> {
        patch::patch_da1(self).ok()
    }

    #[cfg(not(feature = "no_exploits"))]
    fn patch_da2(&mut self) -> Option<DAEntryRegion> {
        patch::patch_da2(self).ok()
    }

    fn get_devinfo(&self) -> DeviceInfo {
        self.dev_info.clone()
    }

    fn get_da(&self) -> &DA {
        &self.da
    }
}
