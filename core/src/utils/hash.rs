/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use md5;
use sha1::{Digest, Sha1};
use sha2::Sha256;

#[derive(Debug, Clone, Copy)]
pub enum HashType {
    Md5,
    Sha1,
    Sha256,
    Unknown,
}

pub fn hash(hash_type: HashType, data: &[u8]) -> Vec<u8> {
    match hash_type {
        HashType::Md5 => md5::compute(data).0.to_vec(),
        HashType::Sha1 => {
            let mut hasher = Sha1::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        }
        HashType::Sha256 => Sha256::digest(data).to_vec(),
        HashType::Unknown => vec![],
    }
}
