/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use anyhow::Result;
use clap::{Args, ValueEnum};
use penumbra::da::BootMode;
use penumbra::{Device, MtkPort};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Debug, ValueEnum, Clone)]
pub enum RebootAction {
    Normal,
    HomeScreen,
    Fastboot,
    Meta,
    Test,
}

impl CommandMetadata for RebootArgs {
    fn about() -> &'static str {
        "Reboot the device into a specified mode."
    }

    fn long_about() -> &'static str {
        "Reboot the device into a specified mode. On XFlash and Legacy, only Normal, HomeScreen
        and Fastboot modes are supported, the rest will default to Normal.
        On XML, also the Meta and Test modes are available."
    }
}

impl From<RebootAction> for BootMode {
    fn from(action: RebootAction) -> Self {
        match action {
            RebootAction::Normal => Self::Normal,
            RebootAction::HomeScreen => Self::HomeScreen,
            RebootAction::Fastboot => Self::Fastboot,
            RebootAction::Test => Self::Test,
            RebootAction::Meta => Self::Meta,
        }
    }
}

#[derive(Args, Debug)]
pub struct RebootArgs {
    #[arg(value_enum, default_value_t = RebootAction::Normal)]
    pub action: RebootAction,
}

impl DeviceCommand for RebootArgs {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        let bootmode: BootMode = self.action.clone().into();
        dev.reboot(bootmode)?;

        Ok(())
    }
}
