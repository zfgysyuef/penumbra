/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use colored::Colorize;
use env_logger::fmt::Formatter;
use log::{Level, LevelFilter, Record};

pub const LOG_FILE_PATH: &str = "antumbra.log";
pub const LOGGER_PREIX: &str = "Antumbra";
pub const INFO_SYMBOL: &str = "✦";
pub const WARN_SYMBOL: &str = "✧";
pub const ERROR_SYMBOL: &str = "❂";

pub fn init_logger(tui_mode: bool, verbose: bool) {
    let mut builder = env_logger::Builder::new();

    let log_file: Option<Arc<Mutex<File>>> = if verbose {
        match File::create(LOG_FILE_PATH) {
            Ok(file) => Some(Arc::new(Mutex::new(file))),
            Err(e) => {
                eprintln!("Failed to create log file: {}", e);
                None
            }
        }
    } else {
        None
    };

    builder.format(move |buf: &mut Formatter, record: &Record| {
        if tui_mode {
            if verbose && let Some(ref log_file) = log_file {
                let mut file = log_file.lock().unwrap();
                let level = record.level();
                writeln!(file, "[{}] {}", level, record.args())?;
                drop(file);
            }

            Ok(())
        } else if record.level() == Level::Debug {
            if verbose && let Some(ref log_file) = log_file {
                return {
                    let mut file = log_file.lock().unwrap();
                    writeln!(file, "[DEBUG] {}", record.args())
                };
            }
            Ok(())
        } else {
            let prefix = LOGGER_PREIX.bold().purple();
            let message = match record.level() {
                Level::Info => format!("{}  {}", INFO_SYMBOL.purple(), record.args()).white(),
                Level::Warn => format!("{}  {}", WARN_SYMBOL.yellow(), record.args()).yellow(),
                Level::Error => format!("{}  {}", ERROR_SYMBOL.red(), record.args()).red().bold(),
                _ => return Ok(()),
            };

            writeln!(buf, "{} {}", prefix, message)
        }
    });

    builder.filter_level(if verbose { LevelFilter::Debug } else { LevelFilter::Info });
    builder.filter_module("nusb", LevelFilter::Off); // Annoying logs :D

    builder.target(env_logger::Target::Stdout);
    builder.init();
}
