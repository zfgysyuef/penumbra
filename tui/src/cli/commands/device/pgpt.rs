/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use anyhow::Result;
use clap::Args;
use human_bytes::human_bytes;
use log::info;
use penumbra::{Device, MtkPort};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Args, Debug)]
pub struct PgptArgs;

impl CommandMetadata for PgptArgs {
    fn visible_aliases() -> &'static [&'static str] {
        &["gpt"]
    }

    fn about() -> &'static str {
        "Display the partition table of the connected device."
    }

    fn long_about() -> &'static str {
        Self::about()
    }
}

impl DeviceCommand for PgptArgs {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        let partitions = dev.partitions();

        info!("Partition Table:");
        for p in partitions {
            info!(
                "Name: {:<25} \t Addr: 0x{:016X} \t Size: 0x{:016X} {:<12} \t Section: {}",
                p.name,
                p.address,
                p.size,
                format!("({})", human_bytes(p.size as f64)),
                Into::<&str>::into(p.kind)
            );
        }

        Ok(())
    }
}
