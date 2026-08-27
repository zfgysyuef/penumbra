/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;
use log::{info, warn};
use penumbra::hacc::{Da, DaVersion, TryRead, TryWrite};

use crate::cli::CliCommand;
use crate::cli::common::CommandMetadata;
use crate::cli::state::PersistedDeviceState;

#[derive(Args, Debug)]
pub struct PatchDaArgs {
    /// The input DA file to patch
    pub input: PathBuf,
    /// The output DA file to write the patched DA
    pub output: PathBuf,
}

impl CommandMetadata for PatchDaArgs {
    fn visible_aliases() -> &'static [&'static str] {
        &["patchda"]
    }

    fn about() -> &'static str {
        "Patch a DA file"
    }

    fn long_about() -> &'static str {
        "Patch a DA file using the same patches used during exploitation."
    }
}

impl CliCommand for PatchDaArgs {
    fn run(&self, _state: &mut PersistedDeviceState) -> Result<()> {
        let buffer = std::fs::read(&self.input)?;

        info!("Reading DA file: {:?}", self.input);

        let mut new_data = buffer.clone();

        let Ok(mut da) = Da::try_read(&buffer) else {
            bail!("Fail to parse DA file (Not a DA file?)")
        };

        info!("DA info:");
        info!(" DA count: {:?}", da.header().da_count());
        info!(" DA header version: {:?}", da.header().version());
        info!("==================================================");

        for mut entry in da.entries() {
            info!(
                "Patching 0x{:X?} (0x{:X?} - {:?})",
                entry.hw_code(),
                entry.hw_sub_code(),
                entry.version()
            );
            match entry.version() {
                DaVersion::V5 => penumbra::da::xflash::patch_da(&mut entry).unwrap(),
                DaVersion::V6 => penumbra::da::xml::patch_da(&mut entry).unwrap(),
                _ => warn!(
                    "Unsupported DA version: {:?} - ({:X?})",
                    entry.version(),
                    entry.hw_code()
                ),
            }

            let start = entry.da1().offset();
            let end = entry.da1().end_offset();
            new_data[start..end].copy_from_slice(entry.da1_code());

            let start = entry.da2().offset();
            let end = entry.da2().end_offset();
            new_data[start..end].copy_from_slice(entry.da2_code());

            info!("--------------------------------------------------");
        }

        info!("==================================================");

        let header = da.header_mut();
        let suffix = b"_antumbra\0";
        let desc_bytes = header.desc().as_bytes();
        let copy_len = desc_bytes.len().min(64 - suffix.len());

        let mut new_desc = [0u8; 64];
        new_desc[..copy_len].copy_from_slice(&desc_bytes[..copy_len]);
        new_desc[copy_len..copy_len + suffix.len()].copy_from_slice(suffix);

        header.set_desc(&new_desc);
        header.try_write(&mut new_data)?;

        let mut output_file = File::create(&self.output)?;
        output_file.write_all(&new_data)?;

        info!("Patched DA file written to: {:?}", self.output);

        Ok(())
    }
}
