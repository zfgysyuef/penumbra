/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TuiConfig {
    pub theme: String,
    #[serde(default)]
    pub compatibility_mode: bool,
    #[serde(default)]
    pub show_stars: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self { theme: "system".to_string(), compatibility_mode: false, show_stars: true }
    }
}

#[derive(Debug, Default, Deserialize, Clone, Serialize)]
pub struct AuthConfig {
    pub online_auth: bool,
    pub endpoint: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone, Serialize)]
pub struct AntumbraConfig {
    pub tui: TuiConfig,
    pub auth: AuthConfig,
}

impl AntumbraConfig {
    pub fn load() -> Result<Arc<Self>> {
        let mut builder = Config::builder();
        let defaults = Self::default();

        builder = builder.set_default("tui.theme", defaults.tui.theme)?;
        builder = builder.set_default("tui.compatibility_mode", defaults.tui.compatibility_mode)?;
        builder = builder.set_default("tui.show_stars", defaults.tui.show_stars)?;
        builder = builder.set_default("auth.online_auth", defaults.auth.online_auth)?;
        builder = builder.set_default("auth.endpoint", defaults.auth.endpoint)?;
        builder = builder.set_default("auth.username", defaults.auth.username)?;
        builder = builder.set_default("auth.password", defaults.auth.password)?;

        if let Some(path) = Self::get_path() {
            builder = builder.add_source(File::from(path).required(false));
        }

        builder = builder.add_source(Environment::with_prefix("ANTUMBRA"));
        let (cfg, parsed) = match builder.build().and_then(|c| c.try_deserialize::<Self>()) {
            Ok(cfg) => (cfg, true),
            Err(e) => {
                log::warn!("Could not read config ({e})");
                (Self::default(), false)
            }
        };

        if parsed {
            cfg.save()?;
        }

        Ok(Arc::new(cfg))
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::get_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            let toml_string = toml::to_string_pretty(self)?;
            fs::write(path, toml_string)?;
        }
        Ok(())
    }

    fn get_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("antumbra/config.toml"))
    }
}
