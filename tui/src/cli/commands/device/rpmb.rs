/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use log::info;
use penumbra::Device;
use penumbra::core::storage::{RpmbRegion, Storage, StorageType};

use crate::cli::DeviceCommand;
use crate::cli::common::{CONN_DA, CommandMetadata};
use crate::cli::helpers::AntumbraProgress;
use crate::cli::state::PersistedDeviceState;

#[derive(Debug, Args)]
pub struct RpmbReadArgs {
    /// RPMB region to use.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub region: u8,
    /// Starting sector to read from.
    #[arg(long, default_value_t = 0)]
    pub start_sector: u32,
    /// Number of sectors to read.
    #[arg(short, long)]
    pub num_sectors: Option<u32>,
    /// File to write the read data to.
    pub file: PathBuf,
}

#[derive(Debug, Args)]
pub struct RpmbWriteArgs {
    /// RPMB region to use.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub region: u8,
    /// Starting sector to write to.
    #[arg(long, default_value_t = 0)]
    pub start_sector: u32,
    /// Number of sectors to write.
    #[arg(short, long)]
    pub num_sectors: Option<u32>,
    /// RPMB authentication key in hex. If omitted on UFS, Antumbra will try device-side
    /// derivation.
    #[arg(long)]
    pub key: Option<String>,
    /// File to read the data from.
    pub file: PathBuf,
}

#[derive(Debug, Args)]
pub struct RpmbAuthArgs {
    /// RPMB region to use.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub region: u8,
    /// The authentication key in hex
    pub key: String,
}

#[derive(Debug, Args)]
pub struct RpmbVerifyDerivedArgs {
    /// RPMB region to verify.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub region: u8,
}

#[derive(Debug, Args)]
pub struct RpmbEraseArgs {
    /// Erase one complete RPMB region.
    #[arg(
        long,
        value_parser = clap::value_parser!(u8).range(0..=3),
        conflicts_with = "all_regions",
        required_unless_present = "all_regions"
    )]
    pub region: Option<u8>,
    /// Erase every enabled RPMB region after all regions pass authentication.
    #[arg(long, conflicts_with = "region", required_unless_present = "region")]
    pub all_regions: bool,
}

#[derive(Debug, Args)]
pub struct RpmbInfoArgs {
    /// Show only one RPMB region; omit to display every region.
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub region: Option<u8>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct RpmbArgs {
    #[command(subcommand)]
    pub command: RpmbCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum RpmbCommand {
    /// Read from RPMB.
    Read(RpmbReadArgs),
    /// Write to RPMB.
    Write(RpmbWriteArgs),
    /// Authenticate with RPMB.
    Auth(RpmbAuthArgs),
    /// Derive and authenticate the device RPMB key without writing data.
    VerifyDerived(RpmbVerifyDerivedArgs),
    /// Irreversibly overwrite an entire RPMB region with zeroes, without readback verification.
    Erase(RpmbEraseArgs),
    /// Display device-reported RPMB region capacities.
    Info(RpmbInfoArgs),
}

impl CommandMetadata for RpmbArgs {
    fn about() -> &'static str {
        "Perform RPMB operations."
    }

    fn long_about() -> &'static str {
        "Perform RPMB operations. DA Extensions must be loaded for this command to work."
    }
}

fn perform_rpmb_io(
    dev: &mut Device,
    region: RpmbRegion,
    start_sector: u32,
    num_sectors: Option<u32>,
    file_path: &PathBuf,
    is_read: bool,
) -> Result<()> {
    let storage =
        dev.dev_info.storage().ok_or_else(|| anyhow!("Failed to retrieve storage information"))?;

    let rpmb_size = storage.get_rpmb_size();
    let max_sectors = if rpmb_size == 0 { None } else { Some((rpmb_size / 256) as u32) };

    let num_sectors = match (num_sectors, max_sectors) {
        (Some(num_sectors), _) => num_sectors,
        (None, Some(max_sectors)) => max_sectors.saturating_sub(start_sector),
        (None, None) => {
            return Err(anyhow!(
                "Device did not report RPMB size; pass --num-sectors/-n to specify how many sectors to {}",
                if is_read { "read" } else { "write" }
            ));
        }
    };

    if num_sectors == 0 {
        return Err(anyhow!("RPMB sector count must be greater than 0"));
    }

    if let Some(max_sectors) = max_sectors {
        if start_sector.saturating_add(num_sectors) > max_sectors {
            return Err(anyhow!(
                "RPMB {} out of bounds! Maximum sectors available: {}",
                if is_read { "read" } else { "write" },
                max_sectors
            ));
        }
    } else {
        info!(
            "Device did not report RPMB size; using requested sector count without host-side bounds check"
        );
    }

    info!(
        "{} {} sectors from RPMB starting at sector {} {} {}",
        if is_read { "Reading" } else { "Writing" },
        num_sectors,
        start_sector,
        if is_read { "into" } else { "from" },
        file_path.display()
    );

    let pb = AntumbraProgress::new(num_sectors as u64 * 256);
    let mut progress_callback = pb.get_callback(
        if is_read { "Reading RPMB..." } else { "Writing RPMB..." },
        if is_read { "RPMB Read Complete!" } else { "RPMB Write Complete!" },
    );

    if is_read {
        let file = File::create(file_path)?;
        let mut writer = BufWriter::new(file);
        dev.read_rpmb(region, start_sector, num_sectors, &mut writer, &mut progress_callback)?;
        writer.flush()?;
    } else {
        let file = File::open(file_path)?;
        let mut reader = BufReader::new(file);
        dev.write_rpmb(region, start_sector, num_sectors, &mut reader, &mut progress_callback)?;
    }

    Ok(())
}

fn decode_rpmb_key(key: &str) -> Result<Vec<u8>> {
    let key = hex::decode(key.trim())?;
    if key.len() != 32 {
        return Err(anyhow!("RPMB key must be exactly 32 bytes / 64 hex characters"));
    }

    Ok(key)
}

fn erase_rpmb(dev: &mut Device, args: &RpmbEraseArgs) -> Result<()> {
    let requested_regions: Vec<RpmbRegion> = if args.all_regions {
        vec![RpmbRegion::R1, RpmbRegion::R2, RpmbRegion::R3, RpmbRegion::R4]
    } else {
        vec![RpmbRegion::try_from(args.region.unwrap())
            .map_err(|_| anyhow!("Invalid RPMB region"))?]
    };

    let mut regions = Vec::new();
    for region in requested_regions {
        let sectors = dev.get_rpmb_sector_count(region)?;
        if sectors == 0 {
            if args.all_regions && region != RpmbRegion::R1 {
                info!("RPMB region {} is disabled; skipping it", region as u32);
                continue;
            }
            return Err(anyhow!(
                "RPMB region {} capacity is unavailable; refusing whole-region erase",
                region as u32
            ));
        }
        regions.push((region, sectors));
    }

    if regions.is_empty() {
        return Err(anyhow!("No enabled RPMB regions were reported by the device"));
    }

    // Preflight every region before changing any data. Advanced UFS RPMB
    // regions can have independent keys and write counters.
    for (region, sectors) in &regions {
        info!(
            "Preflight authenticating RPMB region {} ({} sectors / {} bytes)",
            *region as u32,
            sectors,
            *sectors as u64 * 256
        );
        dev.verify_derived_rpmb_key(*region)
            .with_context(|| format!("RPMB region {} authentication failed", *region as u32))?;
    }

    for (region, sectors) in regions {
        let byte_count = sectors as u64 * 256;
        info!(
            "ERASING RPMB region {}: authenticated zero-fill of {} sectors ({} bytes)",
            region as u32, sectors, byte_count
        );

        let pb = AntumbraProgress::new(byte_count);
        let mut progress = pb.get_callback("Erasing RPMB...", "RPMB Erase Complete!");
        let mut zeroes = std::io::repeat(0).take(byte_count);
        dev.write_rpmb(region, 0, sectors, &mut zeroes, &mut progress).with_context(|| {
            format!(
                "RPMB region {} erase failed; the region may be partially zero-filled",
                region as u32
            )
        })?;

    }

    info!(
        "RPMB erase write completed without readback verification. Authentication keys and write counters were not reset."
    );
    Ok(())
}

fn show_rpmb_info(dev: &mut Device, args: &RpmbInfoArgs) -> Result<()> {
    let regions: Vec<RpmbRegion> = if let Some(region) = args.region {
        vec![RpmbRegion::try_from(region).map_err(|_| anyhow!("Invalid RPMB region"))?]
    } else {
        vec![RpmbRegion::R1, RpmbRegion::R2, RpmbRegion::R3, RpmbRegion::R4]
    };

    for region in regions {
        let sectors = dev.get_rpmb_sector_count(region)?;
        if sectors == 0 {
            info!("RPMB region {}: disabled or capacity unavailable", region as u32);
        } else {
            info!(
                "RPMB region {}: {} sectors, {} bytes ({:.2} MiB)",
                region as u32,
                sectors,
                sectors as u64 * 256,
                sectors as f64 * 256.0 / (1024.0 * 1024.0)
            );
        }
    }
    Ok(())
}

impl DeviceCommand for RpmbArgs {
    fn run(&self, dev: &mut Device, state: &mut PersistedDeviceState) -> Result<()> {
        let region_number = match &self.command {
            RpmbCommand::Read(args) => args.region,
            RpmbCommand::Write(args) => args.region,
            RpmbCommand::Auth(args) => args.region,
            RpmbCommand::VerifyDerived(args) => args.region,
            RpmbCommand::Erase(args) => args.region.unwrap_or(0),
            RpmbCommand::Info(args) => args.region.unwrap_or(0),
        };
        let region = RpmbRegion::try_from(region_number)
            .map_err(|_| anyhow!("Invalid RPMB region {region_number}; expected 0 through 3"))?;

        dev.enter_da_mode()?;

        state.connection_type = CONN_DA;
        state.flash_mode = 1;

        match &self.command {
            RpmbCommand::Read(args) => {
                perform_rpmb_io(
                    dev,
                    region,
                    args.start_sector,
                    args.num_sectors,
                    &args.file,
                    true,
                )?;
            }
            RpmbCommand::Write(args) => {
                let storage = dev
                    .dev_info
                    .storage()
                    .ok_or_else(|| anyhow!("Failed to retrieve storage information"))?;

                if let Some(key) = &args.key {
                    info!("Authenticating RPMB using provided key before write...");
                    let key = decode_rpmb_key(key)?;
                    dev.auth_rpmb(region, &key)?;
                    info!("RPMB authentication was successful!");
                } else if storage.kind() == StorageType::Ufs {
                    info!(
                        "No RPMB key provided; trying device-side UFS RPMB key derivation before write"
                    );
                }

                perform_rpmb_io(
                    dev,
                    region,
                    args.start_sector,
                    args.num_sectors,
                    &args.file,
                    false,
                )?;
            }
            RpmbCommand::Auth(args) => {
                info!("Authenticating RPMB using provided key...");
                let key = decode_rpmb_key(&args.key)?;
                dev.auth_rpmb(region, &key)?;
                info!("Authentication was successful!");
            }
            RpmbCommand::VerifyDerived(_) => {
                info!("Deriving and verifying the device RPMB key without writing RPMB data...");
                dev.verify_derived_rpmb_key(region)?;
                info!("Device-derived RPMB key verification was successful!");
            }
            RpmbCommand::Erase(args) => erase_rpmb(dev, args)?,
            RpmbCommand::Info(args) => show_rpmb_info(dev, args)?,
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: RpmbCommand,
    }

    #[test]
    fn parses_read_sector_arguments_with_declared_types() {
        let cli = TestCli::try_parse_from([
            "test",
            "read",
            "--region",
            "3",
            "--start-sector",
            "256",
            "--num-sectors",
            "200",
            "rpmb.bin",
        ])
        .unwrap();

        let RpmbCommand::Read(args) = cli.command else {
            panic!("expected RPMB read command");
        };
        assert_eq!(args.region, 3);
        assert_eq!(args.start_sector, 256);
        assert_eq!(args.num_sectors, Some(200));
    }

    #[test]
    fn parses_write_sector_arguments_with_declared_types() {
        let cli = TestCli::try_parse_from([
            "test",
            "write",
            "--start-sector",
            "65536",
            "--num-sectors",
            "1",
            "rpmb.bin",
        ])
        .unwrap();

        let RpmbCommand::Write(args) = cli.command else {
            panic!("expected RPMB write command");
        };
        assert_eq!(args.region, 0);
        assert_eq!(args.start_sector, 65_536);
        assert_eq!(args.num_sectors, Some(1));
    }

    #[test]
    fn rejects_out_of_range_region() {
        let error = TestCli::try_parse_from(["test", "read", "--region", "4", "rpmb.bin"])
            .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn parses_verify_derived_command() {
        let cli = TestCli::try_parse_from(["test", "verify-derived", "--region", "2"]).unwrap();
        let RpmbCommand::VerifyDerived(args) = cli.command else {
            panic!("expected RPMB verify-derived command");
        };
        assert_eq!(args.region, 2);
    }

    #[test]
    fn parses_single_region_erase() {
        let cli = TestCli::try_parse_from(["test", "erase", "--region", "0"]).unwrap();
        let RpmbCommand::Erase(args) = cli.command else {
            panic!("expected RPMB erase command");
        };
        assert_eq!(args.region, Some(0));
        assert!(!args.all_regions);
    }

    #[test]
    fn parses_all_region_erase() {
        let cli = TestCli::try_parse_from(["test", "erase", "--all-regions"]).unwrap();
        let RpmbCommand::Erase(args) = cli.command else {
            panic!("expected RPMB erase command");
        };
        assert!(args.all_regions);
        assert_eq!(args.region, None);
    }

    #[test]
    fn erase_requires_exactly_one_scope() {
        assert!(TestCli::try_parse_from(["test", "erase"]).is_err());
        assert!(
            TestCli::try_parse_from(["test", "erase", "--region", "0", "--all-regions"])
                .is_err()
        );
    }

    #[test]
    fn parses_rpmb_info_scope() {
        let cli = TestCli::try_parse_from(["test", "info"]).unwrap();
        let RpmbCommand::Info(args) = cli.command else {
            panic!("expected RPMB info command");
        };
        assert_eq!(args.region, None);
    }

    #[test]
    fn parses_rpmb_info_single_region_filter() {
        let cli = TestCli::try_parse_from(["test", "info", "--region", "2"]).unwrap();
        let RpmbCommand::Info(args) = cli.command else {
            panic!("expected RPMB info command");
        };
        assert_eq!(args.region, Some(2));
    }
}
