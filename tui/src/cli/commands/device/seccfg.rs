/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use anyhow::Result;
use clap::{Args, ValueEnum};
use log::info;
use penumbra::hacc::LockState;
use penumbra::{Device, MtkPort};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Debug, ValueEnum, Clone)]
pub enum SeccfgAction {
    Unlock,
    Lock,
}

#[derive(Args, Debug)]
pub struct SeccfgArgs {
    pub action: SeccfgAction,
}

impl CommandMetadata for SeccfgArgs {
    fn about() -> &'static str {
        "Lock or unlock the seccfg partition on the device."
    }

    fn long_about() -> &'static str {
        "Lock or unlock the seccfg partition on the device.
        This command only work when the device is in DA mode and vulnerable to an exploit or unfused,
        because it requires DA extensions to be loaded."
    }
}

impl DeviceCommand for SeccfgArgs {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        match self.action {
            SeccfgAction::Unlock => {
                info!("Unlocking seccfg...");
                if dev.set_seccfg_lock_state(LockState::Unlock).is_err() {
                    info!("Failed to unlock seccfg or already unlocked.");
                    return Ok(());
                }
                info!("Unlocked seccfg!");
            }
            SeccfgAction::Lock => {
                info!("Locking seccfg partition...");
                if dev.set_seccfg_lock_state(LockState::Lock).is_err() {
                    info!("Failed to lock seccfg or already locked.");
                    return Ok(());
                }
                info!("Locked seccfg!");
            }
        }

        Ok(())
    }
}
