/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
mod common;
mod emmc;
pub mod gpt;
mod ufs;

pub use common::{
    Partition,
    PartitionKind,
    Partitions,
    RPMB_FRAME_DATA_SZ,
    RpmbRegion,
    Storage,
    StorageKind,
    StorageType,
    is_pl_part,
    is_sparse,
};
pub use emmc::{EmmcInfo, EmmcPartition, EmmcStorage};
pub use gpt::{Gpt, GptEntry, GptHeader, GptType};
pub use ufs::{UfsInfo, UfsInfoV1, UfsInfoV2, UfsPartition, UfsStorage};
