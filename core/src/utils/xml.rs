/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::str::FromStr;

use simple_xml;

use crate::error::{Error, Result};

pub fn get_tag<T>(xml: &str, path: &str) -> Result<T>
where
    T: FromStr,
{
    let root =
        simple_xml::from_string(xml).map_err(|_| Error::ParseError("XML parsing error".into()))?;

    let mut node = &root;
    for subnode in path.split('/') {
        let sub_nodes = node.get_nodes(subnode);

        let sub_nodes = sub_nodes
            .ok_or_else(|| Error::ParseError(format!("XML tag `{}` not found", subnode)))?;

        if sub_nodes.is_empty() {
            return Err(Error::ParseError(format!("XML tag `{}` empty", subnode)));
        }

        node = &sub_nodes[0];
    }

    node.content
        .trim()
        .parse::<T>()
        .map_err(|_| Error::ParseError(format!("Failed to parse XML tag `{}`", path)))
}

pub fn get_tag_usize(xml: &str, path: &str) -> Result<usize> {
    let raw_value: String = get_tag(xml, path)?;

    let trimmed = raw_value.trim_start_matches("0x");

    usize::from_str_radix(trimmed, 16)
        .map_err(|_| Error::ParseError(format!("Failed to parse hex XML tag `{}`", path)))
}
