/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use crate::error::{PenumbraError, Result};

#[derive(Debug, Clone)]
pub struct DiffChunk {
    pub offset: usize,
    #[allow(dead_code)]
    pub old: Vec<u8>,
    pub new: Vec<u8>,
}

pub fn find_pattern(data: &[u8], pattern: &[u8], offset: usize) -> Option<usize> {
    if pattern.is_empty() || offset > data.len().saturating_sub(pattern.len()) {
        return None;
    }

    data[offset..]
        .windows(pattern.len())
        .position(|window| window == pattern)
        .map(|pos| offset + pos)
}

/// Applies a byte patch to the data at the specified offset.
pub fn patch(data: &mut [u8], offset: usize, patch_bytes: &[u8]) -> Result<()> {
    if offset + patch_bytes.len() > data.len() {
        return Err(PenumbraError::PatchExceedsBounds.into());
    }

    data[offset..offset + patch_bytes.len()].copy_from_slice(patch_bytes);
    Ok(())
}

/// Finds a pattern and applies a patch at the found location.
pub fn patch_pattern_bytes(data: &mut [u8], pattern: &[u8], patch_bytes: &[u8]) -> Result<usize> {
    let pos = find_pattern(data, pattern, 0).ok_or(PenumbraError::PatternNotFound)?;
    patch(data, pos, patch_bytes)?;
    Ok(pos)
}

/// Finds a u32 value in the data and replaces it with another u32 value.
pub fn patch_u32(data: &mut [u8], from: u32, to: u32) -> Result<usize> {
    let from_bytes = from.to_le_bytes();
    let to_bytes = to.to_le_bytes();
    patch_pattern_bytes(data, &from_bytes, &to_bytes)
}

#[allow(dead_code)]
/// Diffs two bytes slices and returns a vector of diffs
pub fn get_diff(old: &[u8], new: &[u8]) -> Vec<DiffChunk> {
    let mut modified_chunks = Vec::new();
    let len = old.len();
    let mut i = 0;

    while i < len {
        if old[i] == new[i] {
            i += 1;
        } else {
            let start = i;
            while i < len && old[i] != new[i] {
                i += 1;
            }
            modified_chunks.push(DiffChunk {
                offset: start,
                old: old[start..i].to_vec(),
                new: new[start..i].to_vec(),
            });
        }
    }

    modified_chunks
}

pub fn get_diff_align(old: &[u8], new: &[u8]) -> Vec<DiffChunk> {
    let mut modified_chunks = Vec::new();
    let len = old.len();
    let mut i = 0;

    while i < len {
        if old[i] == new[i] {
            i += 1;
        } else {
            let start = i;
            while i < len && old[i] != new[i] {
                i += 1;
            }
            let end = i;

            let aligned_start = start & !3;
            let aligned_end = (end + 3) & !3;

            let safe_start = aligned_start;
            let safe_end = std::cmp::min(aligned_end, len);

            modified_chunks.push(DiffChunk {
                offset: safe_start,
                old: old[safe_start..safe_end].to_vec(),
                new: new[safe_start..safe_end].to_vec(),
            });
        }
    }

    modified_chunks
}
