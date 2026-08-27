/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget};

use crate::app::AppCtx;
use crate::components::{Component, FormField};
use crate::themes::Theme;

pub struct Toggle {
    enabled: bool,
}

impl Toggle {
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub const fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

impl Component for Toggle {
    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppCtx) -> bool {
        match key.code {
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.toggle();
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let display = if self.enabled { "[✓]" } else { "[ ]" };
        let style = if self.enabled { theme.style_accent_bold() } else { theme.style_muted() };

        Paragraph::new(Span::styled(display, style)).render(area, buf);
    }
}

impl FormField for Toggle {
    fn value(&self) -> String {
        if self.enabled { "true" } else { "false" }.to_string()
    }

    fn set_value(&mut self, value: &str) {
        self.enabled = matches!(value, "true" | "yes" | "1" | "on");
    }
}
