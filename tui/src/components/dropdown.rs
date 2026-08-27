/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::app::AppCtx;
use crate::components::{Component, FormField};
use crate::themes::Theme;

#[derive(Clone, Debug)]
pub struct DropdownOption {
    pub label: String,
    pub value: String,
}

impl DropdownOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into() }
    }
}

pub struct Dropdown {
    label: String,
    options: Vec<DropdownOption>,
    selected: usize,
    open: bool,
}

impl Dropdown {
    pub fn new(label: impl Into<String>, options: Vec<DropdownOption>, selected: usize) -> Self {
        Self { label: label.into(), options, selected, open: false }
    }

    pub fn value(&self) -> &String {
        &self.options[self.selected].value
    }

    pub fn selected_label(&self) -> &String {
        &self.options[self.selected].label
    }

    pub const fn is_open(&self) -> bool {
        self.open
    }

    pub const fn set_selected(&mut self, idx: usize) {
        if idx < self.options.len() {
            self.selected = idx;
        }
    }

    pub fn set_by_value(&mut self, value: &str) {
        if let Some(index) = self.options.iter().position(|opt| opt.value == value) {
            self.selected = index;
        }
    }
}

impl Component for Dropdown {
    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppCtx) -> bool {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.open = !self.open;
                true
            }
            KeyCode::Up | KeyCode::Char('k') if self.open => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            KeyCode::Down | KeyCode::Char('j') if self.open => {
                if self.selected + 1 < self.options.len() {
                    self.selected += 1;
                }
                true
            }
            KeyCode::Esc if self.open => {
                self.open = false;
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let [label, box_block] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(10)])
            .areas(area);

        let label_style = Style::default().fg(theme.text);
        Paragraph::new(self.label.as_str()).style(label_style).render(label, buf);

        let border_style = theme.style_border(self.open);

        let arrow = if self.open { " ▲" } else { " ▼" };
        let display_text = format!(" {}{}", self.selected_label(), arrow);

        Paragraph::new(display_text)
            .style(Style::default().fg(theme.text).bg(theme.background))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .bg(theme.background),
            )
            .render(box_block, buf);
    }

    fn render_overlay(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if !self.open {
            return;
        }

        let box_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(10)])
            .split(area)[1];

        let list_height = (self.options.len() as u16).min(10);
        let list_area = Rect {
            x: box_area.x,
            y: box_area.y + box_area.height,
            width: box_area.width,
            height: list_height.saturating_add(2),
        };

        Clear.render(list_area, buf);

        let lines: Vec<Line> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                let style = if i == self.selected {
                    theme.style_highlight()
                } else {
                    Style::default().fg(theme.text).bg(theme.background)
                };
                Line::from(Span::styled(format!("  {}", opt.label), style))
            })
            .collect();

        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent))
                    .bg(theme.background),
            )
            .render(list_area, buf);
    }
}

impl FormField for Dropdown {
    fn value(&self) -> String {
        self.value().clone()
    }

    fn set_value(&mut self, value: &str) {
        self.set_by_value(value);
    }

    fn is_focused(&self) -> bool {
        self.open
    }
}
