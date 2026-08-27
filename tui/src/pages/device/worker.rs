/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use penumbra::activity::{Activity, DeviceActivity};
use penumbra::hacc::{Da, Preloader, TryRead};
use penumbra::port::{ConnectionType, PortBackend, PortType};
use penumbra::{DevInfoData, Device, DeviceBuilder, Partition};

use super::actions::{DeviceIo, actions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Disconnected,
    Connecting,
    Connected(ConnectionType),
}

pub struct ConnectParams {
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub da_data: Option<Vec<u8>>,
    pub preloader_data: Option<Vec<u8>>,
    pub auth_data: Option<Vec<u8>>,
}

impl ConnectParams {
    const PROBE_TIME: Duration = Duration::from_secs(1);

    fn device_present(&self) -> bool {
        let found =
            |vid, pid| matches!(PortType::find_device(vid, pid, PortBackend::Auto), Ok(Some(_)));

        found(self.vid, self.pid)
            || ((self.vid.is_some() || self.pid.is_some()) && found(None, None))
    }
}

pub enum DeviceCommand {
    RunAction(usize),
    PartitionsChosen(Vec<String>),
    FileChosen(PathBuf),
    Cancel,
    Shutdown,
}

pub enum DeviceEvent {
    StatusChanged(DeviceStatus),
    Connected { devinfo: DevInfoData, partitions: Vec<Partition> },
    PartitionsChanged(Vec<Partition>),

    NeedPartitions,
    NeedFile { title: String, directories_only: bool, extensions: Option<Vec<&'static str>> },

    ProgressStart { total_bytes: u64, message: String },
    ProgressUpdate { written: u64, total: Option<u64>, message: Option<String> },
    ProgressFinish { message: String },

    HeaderStatus(String),
    ActivityChanged(Activity),

    ActionFinished,
    Error(String),
    Fatal(String),
}

pub fn spawn(params: ConnectParams) -> (Sender<DeviceCommand>, Receiver<DeviceEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<DeviceCommand>();
    let (event_tx, event_rx) = mpsc::channel::<DeviceEvent>();

    thread::spawn(move || run_worker(params, cmd_rx, event_tx));

    (cmd_tx, event_rx)
}

fn run_worker(
    params: ConnectParams,
    cmd_rx: Receiver<DeviceCommand>,
    event_tx: Sender<DeviceEvent>,
) {
    /* Validate da and preloader */
    match params.da_data.as_deref() {
        Some(bytes) => match Da::try_read(bytes) {
            Ok(da) => Some(da),
            Err(e) => {
                let _ = event_tx.send(DeviceEvent::Fatal(format!(
                    "Invalid DA file: {e}.\nPick a valid DA from the main menu."
                )));
                return;
            }
        },
        None => None,
    };

    match params.preloader_data.as_deref() {
        Some(bytes) => match Preloader::try_read(bytes) {
            Ok(pl) => Some(pl),
            Err(e) => {
                let _ = event_tx.send(DeviceEvent::Fatal(format!(
                    "Invalid Preloader file: {e}.\nPick a valid preloader from the main menu."
                )));
                return;
            }
        },
        None => None,
    };

    /* Find the port */

    let mut last_error = None;

    let port = loop {
        match PortType::find_and_open(params.vid, params.pid, PortBackend::Auto) {
            Ok(Some(port)) => break port,
            Ok(None) => {}
            Err(e) => {
                let message = format!("Waiting for device: {e}");

                if last_error.as_ref() != Some(&message) {
                    let _ = event_tx.send(DeviceEvent::HeaderStatus(message.clone()));
                    last_error = Some(message);
                }
            }
        }

        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(DeviceCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            _ => {}
        }
    };

    let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Connecting));

    let mut builder = DeviceBuilder::new(port);
    if let Some(da) = params.da_data.as_deref() {
        builder = builder.with_da_data(da);
    }
    if let Some(pl) = params.preloader_data.as_deref() {
        builder = builder.with_preloader(pl);
    }
    if let Some(auth) = params.auth_data.as_deref() {
        builder = builder.with_auth(auth);
    }

    let activity_tx = event_tx.clone();
    let activity = DeviceActivity::with_on_change(Box::new(move |activity| {
        let _ = activity_tx.send(DeviceEvent::ActivityChanged(activity.clone()));
    }));

    builder = builder.with_activity(activity.clone());

    let mut dev: Device<'_, PortType> = match builder.build() {
        Ok(dev) => dev,
        Err(e) => {
            let _ = event_tx.send(DeviceEvent::Error(format!("Failed to build device: {e}")));
            let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Disconnected));
            return;
        }
    };

    if let Err(e) = dev.init() {
        let _ = event_tx.send(DeviceEvent::Error(format!("Init failed: {e}")));
        let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Disconnected));
        return;
    }

    if let Err(e) = dev.enter_da_mode() {
        let _ = event_tx.send(DeviceEvent::Error(format!("Entering DA mode failed: {e}")));
        let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Disconnected));
        return;
    }

    let mut partitions = dev.partitions();
    let devinfo = dev.devinfo().data();

    let conn_type = dev.get_connection_type();
    let _ = event_tx.send(DeviceEvent::Connected { devinfo, partitions: partitions.clone() });
    let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Connected(conn_type)));

    let registry = actions();

    loop {
        let cmd = match cmd_rx.recv_timeout(ConnectParams::PROBE_TIME) {
            Ok(cmd) => cmd,

            Err(RecvTimeoutError::Timeout) => {
                if params.device_present() {
                    continue;
                }

                let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Disconnected));
                break;
            }

            Err(RecvTimeoutError::Disconnected) => break,
        };

        match cmd {
            DeviceCommand::RunAction(idx) => {
                let Some(action) = registry.get(idx) else {
                    let _ = event_tx.send(DeviceEvent::Error("Unknown action".into()));
                    continue;
                };

                let result = {
                    let io = DeviceIo::new(&event_tx, &cmd_rx, &partitions, &activity);
                    action.run(&mut dev, &io)
                };

                match result {
                    Err(e) => {
                        let lost = !params.device_present();
                        let _ = event_tx.send(DeviceEvent::Error(e.to_string()));

                        if lost {
                            let _ = event_tx
                                .send(DeviceEvent::StatusChanged(DeviceStatus::Disconnected));
                            break;
                        }
                    }
                    Ok(true) if action.changes_layout() => {
                        // If the action changed the layout, we need to rebuild the
                        // partition list to match the device, or we risk using
                        // stale data.
                        dev.devinfo().set_partitions(vec![]);
                        partitions = dev.partitions();

                        let _ = event_tx.send(DeviceEvent::PartitionsChanged(partitions.clone()));
                    }
                    Ok(_) => {}
                }

                let _ = event_tx.send(DeviceEvent::ActionFinished);
            }
            DeviceCommand::Shutdown => {
                let _ = event_tx.send(DeviceEvent::StatusChanged(DeviceStatus::Disconnected));

                dev.shutdown().unwrap_or_else(|e| {
                    let _ = event_tx.send(DeviceEvent::Error(format!("Shutdown failed: {e}")));
                });
                break;
            }
            _ => {}
        }
    }
}
