/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 DiabloSat
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use derive_builder::Builder;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use strum_macros::AsRefStr;

use crate::app::AppCtx;
use crate::components::Component;
use crate::themes::Theme;

#[derive(Clone, Copy, AsRefStr, Default)]
pub enum DialogType {
    #[strum(serialize = " [!] Error ")]
    Error,
    #[strum(serialize = " [i] Info ")]
    Info,
    #[strum(serialize = " [o] Dialog ")]
    #[default]
    Other,
}

pub struct DialogButton {
    pub title: String,
    pub action: Box<dyn FnMut() + Send>,
}

impl DialogButton {
    pub fn new<F>(title: &str, action: F) -> Self
    where
        F: FnMut() + Send + 'static,
    {
        Self { title: title.to_string(), action: Box::new(action) }
    }
}

impl Clone for DialogButton {
    fn clone(&self) -> Self {
        Self { title: self.title.clone(), action: Box::new(|| {}) }
    }
}

#[derive(Clone)]
pub struct DialogColors {
    pub title_color: Color,
    pub bg_color: Color,
}

impl DialogColors {
    pub const fn new(title_color: Color, bg_color: Color) -> Self {
        Self { title_color, bg_color }
    }
}

impl DialogBuilder {
    pub fn error(message: impl Into<String>, theme: &Theme) -> Self {
        let mut builder = Self::default();
        builder.dialog_type(DialogType::Error);
        builder.colors(DialogColors::new(theme.error, theme.background));
        builder.message(message);
        builder
    }

    pub fn info(message: impl Into<String>, theme: &Theme) -> Self {
        let mut builder = Self::default();
        builder.dialog_type(DialogType::Info);
        builder.colors(DialogColors::new(theme.info, theme.background));
        builder.message(message);
        builder
    }

    pub fn other(message: impl Into<String>, theme: &Theme) -> Self {
        let mut builder = Self::default();
        builder.dialog_type(DialogType::Other);
        builder.colors(DialogColors::new(theme.accent, theme.background));
        builder.message(message);
        builder
    }
}

#[derive(Builder)]
pub struct Dialog {
    #[builder(default)]
    pub dialog_type: DialogType,
    #[builder(setter(into))]
    pub message: String,
    #[builder(default, setter(each = "button"))]
    pub buttons: Vec<DialogButton>,
    #[builder(default)]
    pub selected: usize,
    pub colors: DialogColors,
}

impl Dialog {
    pub fn press_selected(&mut self) {
        if let Some(button) = self.buttons.get_mut(self.selected) {
            (button.action)();
        }
    }

    pub const fn move_left(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub const fn move_right(&mut self) {
        if self.selected + 1 < self.buttons.len() {
            self.selected += 1;
        }
    }

    fn init_buttons(&self, inner: &Rect, buffer: &mut Buffer) {
        let buttons_y = inner.y + inner.height.saturating_sub(2);
        let total_width: u16 = self.buttons.iter().map(|b| b.title.len() as u16 + 4).sum();
        let mut buttons_x = inner.x + (inner.width.saturating_sub(total_width)) / 2;

        for (i, button) in self.buttons.iter().enumerate() {
            let mut style = Style::default().fg(self.colors.title_color);

            if i == self.selected {
                style = style.add_modifier(Modifier::BOLD);
            };

            let label = format!("[ {} ]", button.title);
            buffer.set_string(buttons_x, buttons_y, &label, style);
            buttons_x += label.len() as u16 + 1;
        }
    }

    fn clean_area(&self, area: &Rect, buffer: &mut Buffer, bg_color: Color) {
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buffer.cell_mut((x, y)) {
                    cell.reset();
                    cell.set_bg(bg_color);
                }
            }
        }
    }
}

impl Component for Dialog {
    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppCtx) -> bool {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.move_left();
                true
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.move_right();
                true
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.press_selected();
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = Block::default()
            .title(self.dialog_type.as_ref())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.colors.title_color))
            .padding(Padding::horizontal(1))
            .bg(self.colors.bg_color);

        let inner = block.inner(area);
        self.clean_area(&area, buf, self.colors.bg_color);
        block.render(area, buf);

        Paragraph::new(self.message.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(theme.text).bg(self.colors.bg_color))
            .render(inner, buf);

        self.init_buttons(&inner, buf);
    }
}
