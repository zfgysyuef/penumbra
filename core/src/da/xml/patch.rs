/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use log::{info, warn};

use crate::da::{DA, DAEntryRegion, Xml};
use crate::error::{Error, Result};
use crate::exploit::get_v6_payload;
use crate::utilities::analysis::{Arch, ArchAnalyzer, create_analyzer};
use crate::utilities::arm::{encode_bl_arm, force_return as arm_force_return};
use crate::utilities::arm64::{encode_bl as arm64_encode_bl, force_return as arm64_force_return};
use crate::utilities::patching::*;

const EXTLOADER: &[u8] = include_bytes!("../../../payloads/extloader_v6.bin");

pub const fn to_arch(is_arm64: bool) -> Arch {
    if is_arm64 { Arch::Aarch64 } else { Arch::Arm }
}

pub fn patch_da(_xml: &mut Xml) -> Result<DA> {
    todo!()
}

pub fn patch_da1(_xml: &mut Xml) -> Result<DAEntryRegion> {
    todo!()
}

pub fn patch_da2(xml: &Xml) -> Result<DAEntryRegion> {
    let mut da2 = xml
        .da
        .get_da2()
        .cloned()
        .ok_or_else(|| Error::penumbra("DA2 region not found for patching"))?;

    let is_arm64 = xml.da.is_arm64();
    let analyzer = create_analyzer(da2.data.clone(), da2.addr as u64, to_arch(is_arm64));

    patch_security(&mut da2, analyzer.as_ref(), is_arm64, !xml.force_heapb8)?;
    patch_boot_to(&mut da2, analyzer.as_ref(), is_arm64)?;

    Ok(da2)
}

pub fn patch_boot_to(
    da: &mut DAEntryRegion,
    analyzer: &dyn ArchAnalyzer,
    is_arm64: bool,
) -> Result<bool> {
    if find_pattern(&da.data, "434D443A424F4F542D544F00", 0) != HEX_NOT_FOUND {
        return Ok(true);
    }

    let mut extloader = get_v6_payload(EXTLOADER, is_arm64).to_vec();

    let Some(download_function_off) = analyzer.find_function_from_string("Download host file:%s")
    else {
        warn!("Could not find download function to patch Ext-Loader!");
        return Ok(false);
    };

    let payload_pointer = find_pattern(&extloader, "11111111", 0);
    if payload_pointer == HEX_NOT_FOUND {
        warn!("Could not prepare Ext-Loader!");
        return Ok(false);
    }

    let download_addr: u32 = (download_function_off as u32) + da.addr;
    patch(&mut extloader, payload_pointer, &bytes_to_hex(&download_addr.to_le_bytes()))?;

    let Some(rsc_func_off) = analyzer.find_function_from_string("RSC file") else {
        warn!("Could not find RSC function to inject Ext-Loader!");
        return Ok(false);
    };

    patch(&mut da.data, rsc_func_off, &bytes_to_hex(&extloader))?;
    patch_string(&mut da.data, "CMD:SET-RSC", "CMD:BOOT-TO");

    info!("Injected Ext-Loader to DA2 successfully.");
    Ok(true)
}

fn patch_security(
    da: &mut DAEntryRegion,
    analyzer: &dyn ArchAnalyzer,
    is_arm64: bool,
    patch_sla_registration: bool,
) -> Result<bool> {
    patch_lock_state(da, analyzer, is_arm64)?;
    patch_sec_policy(da, analyzer, is_arm64)?;
    if patch_sla_registration {
        patch_da_sla(da, analyzer, is_arm64)
    } else {
        // Forced HeapBait patches the live verifier and completes the dummy
        // SLA handshake after DA2 starts. Preserve the original GET-DEV-FW
        // registration so HeapBait can fetch the required device context.
        info!("Preserving DA SLA command registration for forced HeapBait");
        Ok(true)
    }
}

fn patch_lock_state(
    da: &mut DAEntryRegion,
    analyzer: &dyn ArchAnalyzer,
    is_arm64: bool,
) -> Result<bool> {
    let lks_patch: Vec<u8> = if is_arm64 {
        #[rustfmt::skip]
        let p = vec![
            0x1F, 0x00, 0x00, 0xB9, // str xzr, [x0]
            0x00, 0x00, 0x80, 0xD2, // mov x0, #0
            0xC0, 0x03, 0x5F, 0xD6, // ret
        ];
        p
    } else {
        #[rustfmt::skip]
        let p = vec![
            0x00, 0x20, 0xA0, 0xE3, // mov r2, #0
            0x04, 0x00, 0x80, 0xE8, // stmia r0, {r2}
            0x00, 0x00, 0xA0, 0xE3, // mov r0, #0
            0x1E, 0xFF, 0x2F, 0xE1, // bx lr
        ];
        p
    };

    let Some(off) = analyzer.find_function_from_string("[%s] sec_get_seccfg") else {
        warn!("Could not patch lock state!");
        return Ok(false);
    };

    patch(&mut da.data, off, &bytes_to_hex(&lks_patch))?;
    info!("Patched DA2 to always report unlocked state.");
    Ok(true)
}

fn patch_sec_policy(
    da: &mut DAEntryRegion,
    analyzer: &dyn ArchAnalyzer,
    is_arm64: bool,
) -> Result<bool> {
    const POLICY_FUNC: &str = "==========security policy==========";

    let Some(part_sec_pol_off) = analyzer.find_function_from_string(POLICY_FUNC) else {
        warn!("Could not find security policy function!");
        return Ok(false);
    };

    // BL policy_index
    // BL hash_binding
    // BL verify_policy
    // BL download_policy
    let Some(policy_idx_bl) = analyzer.get_next_bl_from_off(part_sec_pol_off) else {
        warn!("Could not find policy_idx call");
        return Ok(false);
    };
    let Some(hash_binding_bl) = analyzer.get_next_bl_from_off(policy_idx_bl + 4) else {
        warn!("Could not find hash_binding call");
        return Ok(false);
    };
    let Some(verify_bl) = analyzer.get_next_bl_from_off(hash_binding_bl + 4) else {
        warn!("Could not find verify_policy call");
        return Ok(false);
    };
    let Some(download_bl) = analyzer.get_next_bl_from_off(verify_bl + 4) else {
        warn!("Could not find download_policy call");
        return Ok(false);
    };

    let targets =
        [(hash_binding_bl, "Hash Binding"), (verify_bl, "Verification"), (download_bl, "Download")];

    let mut patched_any = false;

    for (bl_offset, desc) in targets {
        if let Some(func_offset) = analyzer.get_bl_target_offset(bl_offset) {
            if is_arm64 {
                arm64_force_return(&mut da.data, func_offset, 0)?;
            } else {
                arm_force_return(&mut da.data, func_offset, 0, false)?;
            }
            info!("Patched DA2 to skip security policy ({desc})");
            patched_any = true;
        } else {
            warn!("Failed to resolve target for {desc}");
        }
    }

    if !patched_any {
        warn!("Could not patch security policy!");
    }

    Ok(patched_any)
}

fn patch_da_sla(
    da: &mut DAEntryRegion,
    analyzer: &dyn ArchAnalyzer,
    is_arm64: bool,
) -> Result<bool> {
    let sla_str_offset = find_pattern(&da.data, "44412E534C4100454E41424C454400", 0);
    if sla_str_offset == HEX_NOT_FOUND {
        return Ok(true);
    }

    let Some(register_all_cmds_off) = analyzer.find_function_from_string("CMD:REBOOT") else {
        warn!("Could not patch DA SLA!");
        return Ok(false);
    };

    let Some(cmd_offset) = analyzer.find_string_xref("CMD:SECURITY-GET-DEV-FW-INFO") else {
        warn!("Could not patch DA SLA!");
        return Ok(false);
    };

    let Some(bl_to_patch_off) = analyzer.get_next_bl_from_off(cmd_offset) else {
        warn!("Could not find BL instruction to patch for DA SLA!");
        return Ok(false);
    };

    let bl_patch = if is_arm64 {
        arm64_encode_bl(bl_to_patch_off as u32 + da.addr, register_all_cmds_off as u32 + da.addr)?
    } else {
        encode_bl_arm(bl_to_patch_off as u32 + da.addr, register_all_cmds_off as u32 + da.addr)?
    };

    patch(&mut da.data, bl_to_patch_off, &bytes_to_hex(&bl_patch.to_le_bytes()))?;
    info!("Patched DA2 SLA to be disabled.");
    Ok(true)
}
