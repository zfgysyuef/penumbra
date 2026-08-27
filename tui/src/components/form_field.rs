/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use super::Component;

/// Components that can be used in forms
pub trait FormField: Component {
    fn value(&self) -> String;

    fn set_value(&mut self, value: &str);

    /// dropdown open, text input active, etc..
    fn is_focused(&self) -> bool {
        false
    }
}
