/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

const EXT_LOADER: &[u8] = include_bytes!("../../../payloads/extloader_v5.bin");

use hacc::DaEntry;
use log::{debug, info, warn};

use crate::da::xflash::Cmd;
use crate::error::{Result, XFlashErrorKind};
use crate::exploit::DaEntryExt;
use crate::utils::analysis::{Analyzer, ArchAnalyzer, Thumb2Analyzer};
use crate::utils::hash::hash;
use crate::utils::patching::*;

const FORCE_RETURN_PATCH: &[u8] = &[0x00, 0x20, 0x70, 0x47]; // movs r0, #0; bx lr

pub fn patch_da(da: &mut DaEntry) -> Result<()> {
    patch_da2(da)?;
    patch_da1(da)?;
    Ok(())
}

pub fn patch_da1(da: &mut DaEntry) -> Result<()> {
    let Some(hash_pos) = da.hash_offset() else {
        warn!("Could not find DA1 hash position, skipping patching");
        return Ok(());
    };

    let hash_type = da.get_hash_type();
    let da2_code = da.da2_code();
    let hash_result = hash(hash_type, da2_code);

    debug!("New DA1 hash: {:X?}", hash_result);

    let da1_data = da.da1_data_mut();

    patch(da1_data, hash_pos, &hash_result)?;
    patch_u32(da1_data, XFlashErrorKind::DaHashMismatch as u32, 0)
        .map(|_| info!("Patched DA1 hash check"))
        .ok();
    patch_anti_rollback(da1_data).map(|_| info!("Patched DA1 anti-rollback.")).ok();

    Ok(())
}

pub fn patch_da2(da: &mut DaEntry) -> Result<()> {
    let da2_addr = da.da2().addr();
    let da2_data = da.da2_data();

    debug!("Patching DA2 with VA: 0x{:08X}", da2_addr);

    let analyzer = Analyzer::Thumb2(Thumb2Analyzer::new(da2_data.into(), da2_addr as u64));
    let data = da.da2_data_mut();

    patch_boot_to(data, &analyzer)?;
    patch_anti_rollback(data).map(|_| info!("Patched DA2 anti-rollback.")).ok();
    patch_da_sla(data, &analyzer)?;
    patch_security(data, &analyzer)?;
    patch_u32(data, 0x4340F003, 0x300F003)
        .map(|_| info!("Patched DA2 cmd loop error handling."))
        .ok();

    Ok(())
}

/// Disables the DA version anti-rollback check by overwriting the
/// 0xC0020053 error constant in the DA's literal pool with 0, so the
/// error-return path returns success and older DA versions are accepted.
fn patch_anti_rollback(data: &mut [u8]) -> Result<bool> {
    patch_u32(data, XFlashErrorKind::DaVersionAntiRollbackError as u32, 0)?;
    Ok(true)
}

/// Disables security checks in DA2 in `cmd_download`, `cmd_format`, `cmd_write_data`
fn patch_security(da: &mut [u8], analyzer: &Analyzer) -> Result<bool> {
    let Some(cmd_download_log) = analyzer.str_xref("cmd_download") else {
        warn!("Could not patch security!");
        return Ok(false);
    };

    let Some(security_enabled_bl) = analyzer
        .next_bl_from_off(cmd_download_log)
        .and_then(|off| analyzer.next_bl_from_off(off + 4))
    else {
        warn!("Could not patch security!");
        return Ok(false);
    };

    let Some(security_func) =
        analyzer.bl_target(security_enabled_bl).and_then(|va| analyzer.va_to_off(va))
    else {
        warn!("Could not patch security!");
        return Ok(false);
    };

    // movs r3, #0x1 -> movs r3, #0x0
    patch(da, security_func, &0x2300_u16.to_le_bytes())?;

    info!("Patched DA2 to skip security check.");

    Ok(true)
}

/// Adds back the boot_to command to da2, allowing to load extensions.
/// This is needed only on DAs which build date is >= late 2023
/// On DA1, this allows to skip DA2 verification.
fn patch_boot_to(da: &mut [u8], analyzer: &Analyzer) -> Result<bool> {
    if let Some(boot_to_fn) = analyzer.fn_from_str("cmd_boot_to") {
        patch(da, boot_to_fn, EXT_LOADER)?;

        info!("Patched DA2 boot_to!");

        return Ok(true);
    }

    let Some(bootstrap) = analyzer.str_xref("\n***10.dagent_register_commands.\n") else {
        warn!("Can't patch cmd_boot_to!");
        return Ok(false);
    };

    // after this, there's bl + another bl, we want the second one, so we skip the first one
    let da_reg_bl = bootstrap + 4;
    let Some(da_reg_cmds) = analyzer.bl_target(da_reg_bl) else {
        warn!("Can't patch cmd_boot_to!");
        return Ok(false);
    };

    let Some(injection_off) = analyzer.fn_from_str("devc_set_all_in_one_signature") else {
        warn!("Can't patch cmd_boot_to!");
        return Ok(false);
    };

    let Some(cmd_code) =
        find_pattern(da, &(Cmd::SetAllInOneSig as u32).to_le_bytes(), da_reg_cmds as usize)
    else {
        warn!("Can't patch cmd_boot_to!");
        return Ok(false);
    };

    patch(da, cmd_code, &(Cmd::BootTo as u32).to_le_bytes())?;
    patch(da, injection_off, EXT_LOADER)?;

    info!("Patched DA to add cmd_boot_to");

    Ok(true)
}

fn patch_da_sla(da: &mut [u8], analyzer: &Analyzer) -> Result<bool> {
    let Some(devc_sla_status) = analyzer.str_xref("devc_get_sla_enabled_status") else {
        // If the DA doesn't have this string, it likely doesn't have SLA to begin with
        return Ok(true);
    };

    // dprintf
    let Some(first_bl) = analyzer.next_bl_from_off(devc_sla_status) else {
        warn!("Could not patch DA SLA!");
        return Ok(false);
    };

    let Some(off) = analyzer.next_bl_from_off(first_bl + 4) else {
        warn!("Could not patch DA SLA!");
        return Ok(false);
    };

    let target = analyzer.bl_target(off).unwrap_or(0);

    if let Some(target_off) = analyzer.va_to_off(target) {
        patch(da, target_off, FORCE_RETURN_PATCH)?;
        info!("Patched DA2 SLA to be disabled.");

        Ok(true)
    } else {
        warn!("Could not patch DA SLA!");
        Ok(false)
    }
}
