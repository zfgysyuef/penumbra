/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc;
use std::thread;

use log::warn;
use penumbra::DeviceLog;

pub fn setup_file_logger(path: &str) -> Option<DeviceLog> {
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => {
            let (tx, rx) = mpsc::sync_channel::<String>(100);

            thread::spawn(move || {
                while let Ok(msg) = rx.recv() {
                    if file.write_all(msg.as_bytes()).is_err() {
                        break;
                    }
                }
            });

            let device_log = DeviceLog::with_on_push(Box::new(move |msg| {
                let _ = tx.try_send(msg.into());
            }));

            Some(device_log)
        }
        Err(e) => {
            warn!("Failed to open {} for writing device logs: {}", path, e);
            None
        }
    }
}
