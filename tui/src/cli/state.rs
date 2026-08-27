/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::fs::{metadata, read, remove_file, write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct PersistedDeviceState {
    pub da_file_path: Option<String>,
    pub soc_id: [u8; 32],
    pub meid: [u8; 16],
    pub hw_code: u16,
    pub hw_subcode: u16,
    pub target_config: u32,
    pub connection_type: u8,
    pub flash_mode: u8,
    pub usb_log: bool,
}

impl PersistedDeviceState {
    const STATE_FILE: &'static str = ".antumbra_state";

    /// Loads the state from the `.antumbra_state` file.
    /// Returns default state if file doesn't exist or parsing fails.
    pub fn load() -> Self {
        read(Self::STATE_FILE).map_or_else(
            |_| Self::default(),
            |json| serde_json::from_slice(&json).unwrap_or_default(),
        )
    }

    /// Saves the current state to the `.antumbra_state` file.
    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        write(Self::STATE_FILE, json)?;
        Ok(())
    }

    /// Resets the current state and deletes the persisted file if it exists.
    pub fn reset(&mut self) -> Result<()> {
        if metadata(Self::STATE_FILE).is_ok() {
            remove_file(Self::STATE_FILE)?;
        }
        *self = Self::default();
        Ok(())
    }
}
