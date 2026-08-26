/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::io::{Cursor, Read, Write};

use log::{debug, info};

use super::structs::{DACtx, ExtPointerTable, SejParams};
use crate::core::ToBytes;
use crate::core::storage::{RPMB_FRAME_DATA_SZ, RpmbRegion, Storage};
use crate::da::DownloadProtocol;
use crate::da::xflash::{Cmd, XFlash};
use crate::error::{Error, Result};
use crate::le_u32;
use crate::utilities::analysis::{Arch, create_analyzer};

const DA_EXT: &[u8] = include_bytes!("../../../payloads/da_x.bin");
// Won't go faster, and bigger packets makes the device hang
const RPMB_WRITE_PKT_LEN: usize = 32 * 1024;
const POINTER_TABLE_MAGIC: u32 = 0x54525450;

pub fn boot_extensions(xflash: &mut XFlash) -> Result<bool> {
    debug!("Trying booting XFlash extensions...");

    let ext_data = match prepare_extensions(xflash) {
        Some(data) => data,
        None => {
            debug!("Failed to prepare DA extensions");
            return Ok(false);
        }
    };

    let ext_addr = 0x68000000;
    let ext_size = ext_data.len() as u32;

    info!("Uploading DA extensions to 0x{:08X} (0x{:X} bytes)", ext_addr, ext_size);
    if xflash.boot_to(ext_addr, &ext_data).is_err() {
        // If DA extensions fail to upload, we just return false, not a fatal error
        info!("Failed to upload DA extensions, continuing without extensions");
        return Ok(false);
    }

    info!("DA extensions uploaded");

    let ack = xflash.devctrl(Cmd::ExtAck, None)?;
    if ack.len() < 4 || le_u32!(ack, 0) != 0 {
        info!("DA extensions ACK failed, continuing without extensions");
        return Ok(false);
    }

    let sej_base = xflash.chip().sej_base();
    let tzcc_base = xflash.chip().tzcc_base();
    let da2_base = xflash.da.get_da2().map(|da2| da2.addr).unwrap_or(0);
    let da2_size = xflash.da.get_da2().map(|da2| da2.data.len() as u32).unwrap_or(0);
    let storage_type = xflash.get_storage_type() as u32;
    let read_pkt_len = xflash.read_packet_length.unwrap_or(0x100) as u32;
    let write_pkt_len = xflash.write_packet_length.unwrap_or(0x100) as u32;
    let usb_log = xflash.usb_log_channel as u32;

    let ctx = DACtx {
        sej_base,
        tzcc_base,
        da2_base,
        da2_size,
        write_pkt_len,
        read_pkt_len,
        storage_type,
        usb_log,
    };

    xflash.devctrl(Cmd::ExtSetupDaCtx, Some(&[&ctx.to_bytes()]))?;

    Ok(true)
}

fn prepare_extensions(xflash: &XFlash) -> Option<Vec<u8>> {
    let da2 = &xflash.da.get_da2()?.data;
    let da2address = xflash.da.get_da2()?.addr as u64;

    let mut da_ext_data = DA_EXT.to_vec();

    let analyzer = create_analyzer(da2.clone(), da2address, Arch::Thumb2);

    let off = analyzer.find_function_from_string("allocation was %zd bytes long at ptr %p\n")?;
    let free = analyzer.offset_to_va(off)? as u32;

    debug!("Found free at 0x{:08X}", free);

    // kernel main
    let off = analyzer.find_string_xref("\n***10.dagent_register_commands.\n")?;
    let off = analyzer.get_next_bl_from_off(off + 6)?; // Skip dprintf
    let off = analyzer.get_bl_target(off)?;
    let off = analyzer.va_to_offset(off)?;
    // + 0x20 to account of the extloader just in case
    let off = analyzer.get_next_bl_from_off(off)?;
    let reg_devc = analyzer.get_bl_target(off)? as u32 | 1;

    debug!("Found register_device_ctrl at 0x{:08X}", reg_devc);

    let off = analyzer.va_to_offset(reg_devc as u64)?;
    let off = analyzer.get_next_bl_from_off(off)?;
    let malloc = analyzer.get_bl_target(off)? as u32 | 1;

    debug!("Found malloc at 0x{:08X}", malloc);

    let off = analyzer.find_function_from_string("%s, mmc_set_part_config done!!\n")?;
    let off = analyzer.get_next_bl_from_off(off)?; // Skip dprintf

    let off = analyzer.get_bl_target(off)?;
    let mmc_get_card = off as u32 | 1;

    debug!("Found mmc_get_card at 0x{:08X}", mmc_get_card);

    let uart_base = xflash.chip().uart();

    debug!("UART base address at 0x{:X}", uart_base);

    let table = ExtPointerTable {
        magic: POINTER_TABLE_MAGIC,
        uart_base,
        reg_devc,
        malloc,
        free,
        mmc_get_card,
    };

    let off = da_ext_data.len() - ExtPointerTable::SIZE;

    da_ext_data[off..].copy_from_slice(&table.to_bytes());

    Some(da_ext_data)
}

pub fn read32_ext(xflash: &mut XFlash, addr: u32) -> Result<u32> {
    xflash.devctrl(Cmd::ExtReadRegister, Some(&[&addr.to_le_bytes()]))?;

    let payload = xflash.read_data()?;
    status_ok!(xflash);

    Ok(le_u32!(payload, 0))
}

pub fn write32_ext(xflash: &mut XFlash, addr: u32, value: u32) -> Result<()> {
    let addr_bytes = addr.to_le_bytes();
    let value_bytes = value.to_le_bytes();

    xflash.devctrl(Cmd::ExtWriteRegister, Some(&[&addr_bytes, &value_bytes]))?;

    Ok(())
}

pub fn peek<W, F>(
    xflash: &mut XFlash,
    addr: u32,
    length: usize,
    writer: W,
    progress: F,
) -> Result<()>
where
    W: Write + Send,
    F: FnMut(usize, usize) + Send,
{
    let mut range = [0u8; 16];
    range[0..8].copy_from_slice(&(addr as u64).to_le_bytes());
    range[8..16].copy_from_slice(&(length as u64).to_le_bytes());

    xflash.devctrl(Cmd::ExtReadMem, Some(&[&range]))?;
    xflash.upload_data(length, writer, progress)?;

    status_ok!(xflash);

    Ok(())
}

pub fn poke<R, F>(
    xflash: &mut XFlash,
    addr: u32,
    length: usize,
    reader: R,
    progress: F,
) -> Result<()>
where
    R: Read + Send,
    F: FnMut(usize, usize) + Send,
{
    let mut range = [0u8; 16];
    range[0..8].copy_from_slice(&(addr as u64).to_le_bytes());
    range[8..16].copy_from_slice(&(length as u64).to_le_bytes());

    xflash.devctrl(Cmd::ExtWriteMem, Some(&[&range]))?;
    xflash.download_data(length, reader, progress)?;

    status_ok!(xflash);

    Ok(())
}

pub fn sej(
    xflash: &mut XFlash,
    data: &[u8],
    encrypt: bool,
    legacy: bool,
    anti_clone: bool,
    xor: bool,
) -> Result<Vec<u8>> {
    let params = SejParams {
        encrypt,
        legacy,
        anti_clone,
        xor,
        length: data.len() as u32,
        ..Default::default()
    };

    xflash.devctrl(Cmd::ExtSej, Some(&[&params.to_bytes()]))?;

    let mut reader = Cursor::new(data);
    let mut payload = vec![0u8; data.len()];
    let mut writer = Cursor::new(&mut payload);

    xflash.download_data(data.len(), &mut reader, |_, _| {})?;
    xflash.upload_data(data.len(), &mut writer, |_, _| {})?;

    status_ok!(xflash);

    Ok(payload)
}

fn init_rpmb(xflash: &mut XFlash, region: RpmbRegion) -> Result<()> {
    // Derive RPMB key (0 = RPMB)
    xflash.devctrl(Cmd::ExtKeyDerive, Some(&[&0u32.to_le_bytes()]))?;
    let rpmb_key = xflash.read_data()?;
    status_ok!(xflash);

    // If the RPMB is already initialized (even with another key), this will succeed
    // without actually changing the key.
    auth_rpmb(xflash, region, &rpmb_key)?;

    Ok(())
}

pub fn verify_derived_rpmb_key(xflash: &mut XFlash, region: RpmbRegion) -> Result<()> {
    init_rpmb(xflash, region)
}

pub fn read_rpmb<W, F>(
    xflash: &mut XFlash,
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
    init_rpmb(xflash, region)?;

    let storage = match xflash.get_storage() {
        Some(s) => s,
        None => {
            return Err(Error::penumbra("Failed to get storage information for RPMB read"));
        }
    };

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

    let mut sector_range = [0u8; 8];
    sector_range[0..4].copy_from_slice(&start_sector.to_le_bytes());
    sector_range[4..8].copy_from_slice(&sectors_count.to_le_bytes());

    let region = (region as u32).to_le_bytes();
    let data_len = sectors_count as usize * RPMB_FRAME_DATA_SZ;

    xflash.devctrl(Cmd::ExtRpmbRead, Some(&[&region, &sector_range]))?;
    xflash.upload_data(data_len, writer, progress)?;
    status_ok!(xflash);

    Ok(())
}

pub fn write_rpmb<R, F>(
    xflash: &mut XFlash,
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
    init_rpmb(xflash, region)?;

    let storage = match xflash.get_storage() {
        Some(s) => s,
        None => {
            return Err(Error::penumbra("Failed to get storage information for RPMB write"));
        }
    };

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

    let mut sector_range = [0u8; 8];
    sector_range[0..4].copy_from_slice(&start_sector.to_le_bytes());
    sector_range[4..8].copy_from_slice(&sectors_count.to_le_bytes());

    let region = (region as u32).to_le_bytes();
    let data_len = sectors_count as usize * RPMB_FRAME_DATA_SZ;

    xflash.devctrl(Cmd::ExtRpmbWrite, Some(&[&region, &sector_range]))?;
    xflash.download_data_with(data_len, RPMB_WRITE_PKT_LEN, reader, progress)?;
    status_ok!(xflash);

    Ok(())
}

pub fn auth_rpmb(xflash: &mut XFlash, region: RpmbRegion, key: &[u8]) -> Result<()> {
    xflash.devctrl(Cmd::ExtRpmbInit, Some(&[&(region as u32).to_le_bytes(), key]))?;
    status_ok!(xflash);

    Ok(())
}
