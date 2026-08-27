/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use anyhow::Result;
use clap::Args;
use log::{error, info};
use penumbra::{Device, MtkPort};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Args, Debug)]
pub struct GetActiveSlotArgs;

impl CommandMetadata for GetActiveSlotArgs {
    fn visible_aliases() -> &'static [&'static str] {
        &["getslot"]
    }

    fn about() -> &'static str {
        "Display the active boot slot for AB devices."
    }

    fn long_about() -> &'static str {
        Self::about()
    }
}

impl DeviceCommand for GetActiveSlotArgs {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        match dev.get_bootctrl() {
            Ok(bootctrl) => info!("Active slot: {:?}", bootctrl.get_active_slot()),
            Err(_) => {
                error!("Couldn't retrieve boot control info. The device may not support AB slots.")
            }
        }

        Ok(())
    }
}
