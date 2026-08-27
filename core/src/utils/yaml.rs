/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use std::str::FromStr;

use rust_yaml::Value;

pub trait YamlValueExt {
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn get_num<T: FromStr>(&self, key: &str) -> Option<T>;
}

impl YamlValueExt for Value {
    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_str(key)?.as_bool()
    }

    fn get_num<T: FromStr>(&self, key: &str) -> Option<T> {
        let val = self.get_str(key)?;

        match val {
            Self::Int(i) => i.to_string().parse::<T>().ok(),
            Self::Float(f) => f.to_string().parse::<T>().ok(),
            Self::String(s) => s.parse::<T>().ok(),
            _ => None,
        }
    }
}
