/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clap::Args;
use log::{error, info};
use penumbra::{Device, MtkPort, PlProtocol};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::state::PersistedDeviceState;

#[derive(Args, Debug)]
pub struct CrashArgs {}

impl CommandMetadata for CrashArgs {
    fn about() -> &'static str {
        "Crash the device to bootrom."
    }

    fn long_about() -> &'static str {
        "Crash the device into bootrom by triggering an assertion."
    }
}

impl DeviceCommand for CrashArgs {
    fn run<P: MtkPort>(&self, dev: &mut Device<P>, state: &mut PersistedDeviceState) -> Result<()> {
        if state.connection_type == CONN_DA {
            info!("The device can't be crashed while in DA mode.");
            info!("Please reboot the device into Preloader mode and try again.");
            return Ok(());
        };

        let dummy_data = [0u8; 0x100];
        let data_len = dummy_data.len() as u32;

        info!("Crashing device...");

        let port = dev.port_mut();
        let mut pl = PlProtocol::new(port);

        pl.send_da(&dummy_data, data_len, 0, data_len).ok();

        let _last_seen = Instant::now();
        let _sleep_timeout = Duration::from_millis(500);
        let _timeout = Duration::from_secs(5);
        let _start = Instant::now();

        info!("Waiting for MTK device...");

        if let Err(e) = port.reenumerate(0x0E8D, 0x0003) {
            error!("Device did not come back online in time. Probably unsupported");
            bail!(e);
        }

        let mut pl = PlProtocol::new(port);

        pl.handshake()?;

        Ok(())
    }
}
