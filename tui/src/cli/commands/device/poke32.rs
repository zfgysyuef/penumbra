/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use anyhow::Result;
use clap::Args;
use clap_num::maybe_hex;
use log::info;
use penumbra::{Device, MtkPort};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Args, Debug)]
pub struct Poke32Args {
    /// The register to read from.
    #[clap(value_parser=maybe_hex::<u64>)]
    pub address: u64,
    /// The value to write to the register.
    #[clap(value_parser=maybe_hex::<u32>)]
    pub value: u32,
}

impl CommandMetadata for Poke32Args {
    fn visible_aliases() -> &'static [&'static str] {
        &["wr", "ww"]
    }

    fn about() -> &'static str {
        "Write the provided value in a register."
    }

    fn long_about() -> &'static str {
        "Write the provided from to the specified register. If the address is not 4 bytes aligned,
        it will be automatically rounded down. DA Extensions must be loaded for this command to work."
    }
}

impl DeviceCommand for Poke32Args {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        dev.write_register(self.address, self.value)?;

        info!("Wrote value 0x{:08X} to register 0x{:08X}", self.value, self.address);

        Ok(())
    }
}
