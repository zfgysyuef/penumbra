/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

// I'd rather avoid warnings here, since most
// of these are unused for the time being.
#![allow(dead_code)]

pub mod aarch64;
pub mod arm;
pub mod thumb;

pub use aarch64::Aarch64Analyzer;
pub use arm::ArmAnalyzer;
use enum_dispatch::enum_dispatch;
pub use thumb::Thumb2Analyzer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Arm,
    Aarch64,
    Thumb2,
}

impl Arch {
    pub const fn is_arm64(self) -> bool {
        matches!(self, Self::Aarch64)
    }
}

/// Architecture-agnostic binary analysis trait.
#[enum_dispatch]
pub trait ArchAnalyzer {
    /// Returns the underlying binary data.
    fn data(&self) -> &[u8];

    fn arch(&self) -> Arch;

    /// Returns the length of the binary data.
    fn len(&self) -> usize {
        self.data().len()
    }

    /// Returns true if the binary data is empty.
    fn is_empty(&self) -> bool {
        self.data().is_empty()
    }

    /// Reads a little-endian u32 at the given offset.
    fn read_u32(&self, offset: usize) -> Option<u32> {
        let bytes = self.data().get(offset..offset + 4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Converts a virtual address to a file offset.
    fn va_to_off(&self, va: u64) -> Option<usize>;

    /// Converts a file offset to a virtual address.
    fn off_to_va(&self, offset: usize) -> Option<u64>;

    /// Finds the file offset of the start of a function that references a specific string.
    fn fn_from_str(&self, s: &str) -> Option<usize>;

    /// Finds a function pointer passed as an argument to a call that follows a string reference.
    fn find_call_arg_from_string(&self, s: &str, arg_idx: u8) -> Option<u64>;

    /// Returns the target address (VA) of a BL instruction at the given offset.
    fn bl_target(&self, offset: usize) -> Option<u64>;

    /// Returns the target address (VA) of a B instruction at the given offset.
    fn b_target(&self, offset: usize) -> Option<u64>;

    /// Finds the next BL instruction from the given file offset.
    fn next_bl_from_off(&self, offset: usize) -> Option<usize>;

    /// Finds the next B instruction from the given file offset.
    fn next_b_from_off(&self, offset: usize) -> Option<usize>;

    /// Finds the first reference to the given string, returning the file offset.
    fn str_xref(&self, target_str: &str) -> Option<usize>;

    /// Finds the start of a function containing the given offset.
    fn fn_from_off(&self, offset: usize) -> Option<usize>;

    /// Finds the value of a register at a given offset, looking back a certain number of
    /// instructions.
    fn reg_value(&self, offset: usize, reg: u8, lookback: usize) -> Option<u64>;

    /// Returns the file offset target of a BL instruction.
    fn bl_target_off(&self, offset: usize) -> Option<usize> {
        let va = self.bl_target(offset)?;
        self.va_to_off(va)
    }
}

#[enum_dispatch(ArchAnalyzer)]
pub enum Analyzer {
    Aarch64(Aarch64Analyzer),
    Arm(ArmAnalyzer),
    Thumb2(Thumb2Analyzer),
}
