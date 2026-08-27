/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::fmt::Debug;
use std::sync::{Arc, Mutex};

pub type OnPush = Box<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct DeviceLog {
    inner: Arc<Inner>,
}

struct Inner {
    entries: Mutex<Vec<String>>,
    on_push: Option<OnPush>,
}

impl Debug for DeviceLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceLog")
            .field("entries", &self.inner.entries)
            .field("on_push", &self.inner.on_push.as_ref().map(|_| ".."))
            .finish()
    }
}

impl Default for DeviceLog {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceLog {
    pub fn new() -> Self {
        Self { inner: Arc::new(Inner { entries: Mutex::new(Vec::new()), on_push: None }) }
    }

    pub fn with_on_push(on_push: OnPush) -> Self {
        Self { inner: Arc::new(Inner { entries: Mutex::new(Vec::new()), on_push: Some(on_push) }) }
    }

    pub fn push(&self, message: String) {
        if let Some(ref cb) = self.inner.on_push {
            cb(&message);
        }

        if let Ok(mut entries) = self.inner.entries.lock() {
            entries.push(message);
        }
    }

    pub fn entries(&self) -> Vec<String> {
        self.inner.entries.lock().map(|e| e.clone()).unwrap_or_default()
    }

    pub fn drain(&self) -> Vec<String> {
        self.inner.entries.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.inner.entries.lock().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.inner.entries.lock() {
            entries.clear();
        }
    }
}
