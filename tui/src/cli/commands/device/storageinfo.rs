/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use anyhow::{Result, anyhow};
use clap::Args;
use human_bytes::human_bytes;
use log::info;
use penumbra::storage::UfsInfo;
use penumbra::{Device, MtkPort, Storage, StorageKind};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Args, Debug)]
pub struct StorageInfoArgs;

impl CommandMetadata for StorageInfoArgs {
    fn about() -> &'static str {
        "Show detailed information about the device's storage."
    }

    fn long_about() -> &'static str {
        Self::about()
    }
}

impl DeviceCommand for StorageInfoArgs {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        let storage = dev
            .get_storage()
            .ok_or_else(|| anyhow!("Cannot retrieve storage information from the device."))?;

        let total_size = storage.total_size();
        let block_size = storage.block_size();
        let pl1_size = storage.get_pl1_size();
        let pl2_size = storage.get_pl2_size();
        let user_size = storage.get_user_size();
        let rpmb_size = storage.get_rpmb_size();
        let partition_count = dev.partitions_iter().count();

        info!("Storage Information:");
        info!("  Storage Type: {}", storage.as_str());
        info!("  Number of Partitions: {}", partition_count);
        info!("  Total Size: 0x{:X?} bytes ({})", total_size, human_bytes(total_size as f64));
        info!("  Block Size: 0x{:X?} bytes ({})", block_size, human_bytes(block_size as f64));
        info!("  Boot 1 size: 0x{:X?} bytes ({})", pl1_size, human_bytes(pl1_size as f64));
        info!("  Boot 2 size: 0x{:X?} bytes ({})", pl2_size, human_bytes(pl2_size as f64));
        info!("  User size: 0x{:X?} bytes ({})", user_size, human_bytes(user_size as f64));
        info!("  RPMB size: 0x{:X?} bytes ({})", rpmb_size, human_bytes(rpmb_size as f64));

        match storage {
            StorageKind::Emmc(emmc) => {
                let gp1 = emmc.info.gp1_size;
                let gp2 = emmc.info.gp2_size;
                let gp3 = emmc.info.gp3_size;
                let gp4 = emmc.info.gp4_size;
                let cid = emmc.info.cid;
                info!("  EMMC GP1 size: 0x{:X?} bytes ({})", gp1, human_bytes(gp1 as f64));
                info!("  EMMC GP2 size: 0x{:X?} bytes ({})", gp2, human_bytes(gp2 as f64));
                info!("  EMMC GP3 size: 0x{:X?} bytes ({})", gp3, human_bytes(gp3 as f64));
                info!("  EMMC GP4 size: 0x{:X?} bytes ({})", gp4, human_bytes(gp4 as f64));
                info!("  EMMC CID: {}", hex::encode(cid));
            }
            StorageKind::Ufs(ufs) => match ufs.info {
                UfsInfo::V1(v1) => {
                    info!("  UFS CID: {:?}", hex::encode(v1.cid));
                }
                UfsInfo::V2(v2) => {
                    info!("  UFS CID: {:?}", hex::encode(v2.cid));
                }
            },
        }

        Ok(())
    }
}
