/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::io::{Read, Write};

use enum_dispatch::enum_dispatch;
use wincode::{Deserialize, SchemaRead, SchemaWrite};

use crate::DeviceLog;
use crate::connection::Connection;
use crate::connection::port::ConnectionType;
use crate::core::chip::ChipInfo;
use crate::core::devinfo::DeviceInfo;
use crate::core::seccfg::LockFlag;
use crate::core::storage::{Partition, PartitionKind, RpmbRegion, StorageKind, StorageType};
use crate::core::{FromBytes, ToBytes};
use crate::da::{DA, DAEntryRegion, XFlash, Xml};
use crate::error::Result;

/// MAGIC value for V5/V6 packets.
pub const MAGIC: u32 = 0xFEEEEEEF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Normal,
    HomeScreen,
    Fastboot,
    Test,
    Meta,
}

impl BootMode {
    pub const fn to_text(&self) -> Option<&'static str> {
        match self {
            Self::Fastboot => Some("FASTBOOT"),
            Self::Meta => Some("META"),
            Self::Test => Some("ANDROID-TEST-MODE"),
            Self::Normal | Self::HomeScreen => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, SchemaRead, SchemaWrite)]
#[repr(u32)]
pub enum DataType {
    #[wincode(tag = 1)]
    Flow = 0x1,
    #[wincode(tag = 2)]
    Message = 0x2,
}

impl DataType {
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0x1 => Some(Self::Flow),
            0x2 => Some(Self::Message),
            _ => None,
        }
    }
}

/// 12 byte packet header shared by all packet types.
///
/// Format:
/// ```text
/// [0..4]   magic      (must be 0xFEEEEEEF)
/// [4..8]   data_type  (1 = Flow, 2 = Message)
/// [8..12]  length     (byte count of the payload that follows)
/// ```
///
/// For `Message` packets, the payload starts with a 4 byte priority
/// field followed by the actual message body.
/// The length of a `Message` packet also includes the 4 byte from priority.
#[derive(Debug, Clone, Copy, SchemaRead, SchemaWrite, ToBytes)]
pub struct PacketHeader {
    pub magic: u32,
    pub data_type: DataType,
    pub length: u32,
}

impl PacketHeader {
    pub const SIZE: usize = size_of::<Self>();

    pub const fn new(length: u32) -> Self {
        Self { magic: MAGIC, data_type: DataType::Flow, length }
    }

    pub fn from_bytes(raw: &[u8]) -> Option<Self> {
        if raw.len() < Self::SIZE {
            return None;
        }

        let hdr = Self::deserialize(raw).ok()?;
        if hdr.magic != MAGIC {
            return None;
        }

        Some(hdr)
    }
}

impl FromBytes for PacketHeader {
    const SIZE: usize = size_of::<Self>();

    fn from_bytes(raw: &[u8]) -> Option<Self> {
        let hdr = Self::from_bytes(raw)?;

        if hdr.magic != MAGIC {
            return None;
        }

        Some(hdr)
    }
}

pub struct DAProtocolParams {
    pub da: DA,
    pub devinfo: DeviceInfo,
    pub device_log: DeviceLog,
    pub verbose: bool,
    pub usb_log_channel: bool,
    pub force_heapb8: bool,
    pub preloader: Option<Vec<u8>>, // TODO: Switch to Preloader type
}

#[enum_dispatch(DownloadProtocol)]
pub enum DAProtocol {
    V5(XFlash),
    V6(Xml),
}

#[enum_dispatch]
pub trait DownloadProtocol {
    // Main helpers
    fn upload_da(&mut self) -> Result<bool>;
    fn boot_to(&mut self, addr: u32, data: &[u8]) -> Result<bool>;
    fn send(&mut self, data: &[u8]) -> Result<bool>;
    fn send_data(&mut self, data: &[&[u8]]) -> Result<bool>;
    fn get_status(&mut self) -> Result<u32>;
    fn shutdown(&mut self) -> Result<()>;
    fn reboot(&mut self, bootmode: BootMode) -> Result<()>;
    // FLASH operations
    // fn read_partition(&mut self, name: &str) -> Result<Vec<u8>, Error>;
    fn read_flash<W, F>(
        &mut self,
        addr: u64,
        size: usize,
        section: PartitionKind,
        writer: W,
        progress: F,
    ) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send;

    fn write_flash<R, F>(
        &mut self,
        addr: u64,
        size: usize,
        section: PartitionKind,
        reader: R,
        progress: F,
    ) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send;

    fn erase_flash<F>(
        &mut self,
        addr: u64,
        size: usize,
        section: PartitionKind,
        progress: F,
    ) -> Result<()>
    where
        F: FnMut(usize, usize) + Send;

    fn download<R, F>(
        &mut self,
        part_name: &str,
        size: usize,
        reader: R,
        progress: F,
    ) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send;

    fn upload<W, F>(&mut self, part_name: &str, writer: W, progress: F) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send;

    fn format<F>(&mut self, part_name: &str, progress: F) -> Result<()>
    where
        F: FnMut(usize, usize) + Send;

    // Memory
    fn read32(&mut self, addr: u32) -> Result<u32>;
    fn write32(&mut self, addr: u32, value: u32) -> Result<()>;

    fn get_usb_speed(&mut self) -> Result<u32>;
    // fn set_usb_speed(&mut self, speed: u32) -> Result<(), Error>;

    // Connection
    fn get_connection(&mut self) -> &mut Connection;
    fn set_connection_type(&mut self, conn_type: ConnectionType) -> Result<()>;

    fn get_storage(&mut self) -> Option<StorageKind>;
    fn get_storage_type(&mut self) -> StorageType;
    fn get_partitions(&mut self) -> Vec<Partition>;

    // DevInfo helpers
    fn get_devinfo(&self) -> DeviceInfo;
    fn get_da(&self) -> &DA;

    fn chip(&self) -> &'static ChipInfo {
        self.get_devinfo().chip()
    }

    /* EXTENSIONS / EXPLOITS
     * These functions won't be included if the "no_exploits" feature is enabled
     */

    // Sec
    #[cfg(not(feature = "no_exploits"))]
    fn set_seccfg_lock_state(&mut self, locked: LockFlag) -> Option<[u8; 512]>;

    #[cfg(not(feature = "no_exploits"))]
    fn peek<W, F>(&mut self, addr: u32, length: usize, writer: W, progress: F) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send;

    #[cfg(not(feature = "no_exploits"))]
    fn poke<R, F>(&mut self, addr: u32, length: usize, reader: R, progress: F) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send;

    #[cfg(not(feature = "no_exploits"))]
    fn read_rpmb<W, F>(
        &mut self,
        region: RpmbRegion,
        start_sector: u32,
        sectors_count: u32,
        writer: W,
        progress: F,
    ) -> Result<()>
    where
        W: Write + Send,
        F: FnMut(usize, usize) + Send;

    #[cfg(not(feature = "no_exploits"))]
    fn write_rpmb<R, F>(
        &mut self,
        region: RpmbRegion,
        start_sector: u32,
        sectors_count: u32,
        reader: R,
        progress: F,
    ) -> Result<()>
    where
        R: Read + Send,
        F: FnMut(usize, usize) + Send;

    #[cfg(not(feature = "no_exploits"))]
    fn auth_rpmb(&mut self, region: RpmbRegion, key: &[u8]) -> Result<()>;

    /// Derives the device RPMB key and verifies it against the authenticated
    /// write-counter response without writing RPMB data.
    #[cfg(not(feature = "no_exploits"))]
    fn verify_derived_rpmb_key(&mut self, region: RpmbRegion) -> Result<()>;

    /// Returns the authenticated 256-byte sector count for an RPMB region.
    #[cfg(not(feature = "no_exploits"))]
    fn get_rpmb_sector_count(&mut self, region: RpmbRegion) -> Result<u32>;

    /// Returns whether an RPMB region is enabled and its configured sector count.
    #[cfg(not(feature = "no_exploits"))]
    fn get_rpmb_region_info(&mut self, region: RpmbRegion) -> Result<(bool, u32)>;

    // DA Patching utils. These *must* be protocol specific, as different protocols
    // have different DA implementations
    #[cfg(not(feature = "no_exploits"))]
    fn patch_da(&mut self) -> Option<DA>;
    #[cfg(not(feature = "no_exploits"))]
    fn patch_da1(&mut self) -> Option<DAEntryRegion>;
    #[cfg(not(feature = "no_exploits"))]
    fn patch_da2(&mut self) -> Option<DAEntryRegion>;
}
