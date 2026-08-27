/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui_textarea::{Input, Key, TextArea};

use crate::app::AppCtx;
use crate::components::{Component, FormField};
use crate::themes::Theme;

pub struct TextInput {
    textarea: TextArea<'static>,
    focused: bool,
    masked: bool,
}

impl TextInput {
    pub fn new() -> Self {
        Self { textarea: TextArea::default(), focused: false, masked: false }
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        let mut textarea = TextArea::default();
        textarea.insert_str(text.into());
        Self { textarea, focused: false, masked: false }
    }

    pub const fn focus(&mut self) {
        self.focused = true;
    }

    pub const fn unfocus(&mut self) {
        self.focused = false;
    }

    pub const fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TextInput {
    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppCtx) -> bool {
        if !self.focused {
            if key.code == KeyCode::Enter {
                self.focus();
                return true;
            }

            return false;
        }

        if key.code == KeyCode::Esc {
            self.unfocus();
            return true;
        }

        self.textarea.input(Input {
            key: map_key(key.code),
            ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
            alt: key.modifiers.contains(KeyModifiers::ALT),
            shift: key.modifiers.contains(KeyModifiers::SHIFT),
        });

        true
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let border_style = theme.style_border(self.focused);
        let content_style = theme.style_text();
        let content = if self.textarea.lines().is_empty() {
            String::new()
        } else if self.masked {
            "*".repeat(self.textarea.lines().join("\n").chars().count())
        } else {
            self.textarea.lines().join("\n")
        };

        Paragraph::new(content)
            .style(content_style.add_modifier(if self.focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }))
            .block(Block::default().borders(Borders::ALL).border_style(border_style))
            .render(area, buf);
    }
}

impl FormField for TextInput {
    fn value(&self) -> String {
        self.textarea.lines().join("\n")
    }

    fn set_value(&mut self, value: &str) {
        let _ = self.textarea.clear();
        self.textarea.insert_str(value);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }
}

const fn map_key(code: KeyCode) -> Key {
    match code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::F(n) => Key::F(n),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        _ => Key::Null,
    }
}
