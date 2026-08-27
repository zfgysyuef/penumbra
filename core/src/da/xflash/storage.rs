/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use log::debug;

use crate::da::xflash::{Cmd, XFlash};
use crate::port::MtkPort;
use crate::storage::{EmmcStorage, StorageKind, UfsStorage};
use crate::traits::FromBytes;

// TODO: Avoid repeated logic
pub fn detect_storage<P: MtkPort>(xflash: &mut XFlash, port: &mut P) -> Option<StorageKind> {
    let emmc_response = xflash.devctrl(port, Cmd::GetEmmcInfo, None);
    let ufs_response = xflash.devctrl(port, Cmd::GetUfsInfo, None);

    debug!("EMMC response: {:?}", emmc_response);
    debug!("UFS response: {:?}", ufs_response);
    if let Ok(resp) = emmc_response
        && let Some(storage) = EmmcStorage::from_bytes(&resp)
    {
        debug!("eMMC storage detected.");
        return Some(StorageKind::Emmc(storage));
    }

    if let Ok(resp) = ufs_response
        && let Some(storage) = UfsStorage::from_bytes(&resp)
    {
        debug!("UFS storage detected.");
        return Some(StorageKind::Ufs(storage));
    }

    None
}
