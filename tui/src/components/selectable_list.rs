/*
 * SPDX-License-Identifier: AGPL-3.0-or-later
 * SPDX-FileCopyrightText: 2025-2026 DiabloSat
 * SPDX-FileCopyrightText: 2025-2026 Shomy
 */

use derive_builder::Builder;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, StatefulWidgetRef};

use crate::app::AppCtx;
use crate::components::Component;
use crate::themes::Theme;

#[derive(PartialEq, Eq, Builder, Clone, Default)]
pub struct ListItemEntry {
    pub label: String,
    #[builder(default, setter(strip_option))]
    pub value: Option<String>,
    #[builder(default, setter(strip_option))]
    pub icon: Option<char>,
    #[builder(default, setter(strip_option))]
    pub style: Option<Style>,
    #[builder(private, default)]
    toggle: bool,
}

impl ListItemEntry {
    pub fn new(label: impl Into<String>, value: Option<String>, icon: Option<char>) -> Self {
        Self { label: label.into(), value, icon, style: None, toggle: false }
    }

    pub const fn is_toggled(&self) -> bool {
        self.toggle
    }
}

#[derive(Builder, Clone, Default)]
pub struct SelectableList {
    #[builder(default)]
    pub items: Vec<ListItemEntry>,
    #[builder(default = "{
        let mut s = ListState::default();
        s.select(Some(0));
        s
    }")]
    pub state: ListState,
    #[builder(setter(custom))]
    pub highlight_symbol: String,
    #[builder(default)]
    pub toggled: bool,
    #[builder(default)]
    pub borders: Borders,
    #[builder(default)]
    pub block_title: String,
    #[builder(default = true)]
    pub is_focused: bool,
    #[builder(default = true)]
    pub highlight_on_onfocus: bool,
}

impl SelectableList {
    pub fn next(&mut self) {
        if !self.items.is_empty() {
            let i = self.state.selected().unwrap_or(0);
            let next = if i >= self.items.len() - 1 { 0 } else { i + 1 };
            self.state.select(Some(next));
        }
    }

    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            let i = self.state.selected().unwrap_or(0);
            let prev = if i == 0 { self.items.len() - 1 } else { i - 1 };
            self.state.select(Some(prev));
        }
    }

    pub fn next_by(&mut self, step: usize) {
        if !self.items.is_empty() {
            let i = self.state.selected().unwrap_or(0);
            let next = (i + step).min(self.items.len() - 1);
            self.state.select(Some(next));
        }
    }

    pub fn previous_by(&mut self, step: usize) {
        if !self.items.is_empty() {
            let i = self.state.selected().unwrap_or(0);
            let prev = i.saturating_sub(step);
            self.state.select(Some(prev));
        }
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn selected_item(&self) -> Option<&ListItemEntry> {
        self.selected_index().and_then(|i| self.items.get(i))
    }

    pub fn toggle_selected(&mut self) {
        if self.toggled
            && let Some(i) = self.selected_index()
            && let Some(item) = self.items.get_mut(i)
        {
            item.toggle = !item.toggle;
        }
    }

    pub fn checked_items(&self) -> Vec<&ListItemEntry> {
        self.items.iter().filter(|item| item.toggle).collect()
    }

    pub fn clear_toggles(&mut self) {
        self.items.iter_mut().for_each(|item| item.toggle = false);
    }

    pub const fn set_focus(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    pub const fn is_focused(&self) -> bool {
        self.is_focused
    }
}

impl SelectableListBuilder {
    pub fn highlight_symbol(&mut self, s: impl Into<String>) -> &mut Self {
        self.highlight_symbol = Some(s.into());
        self
    }
}

impl Component for SelectableList {
    fn handle_key(&mut self, key: KeyEvent, _ctx: &mut AppCtx) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
                true
            }
            KeyCode::Char(' ') => {
                self.toggle_selected();
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if Some(i) == self.selected_index()
                    && (self.is_focused() || self.highlight_on_onfocus)
                {
                    theme.style_highlight()
                } else {
                    item.style.unwrap_or_else(|| theme.style_text())
                };

                let label = {
                    let mut parts = Vec::new();
                    if self.toggled {
                        parts.push(if item.toggle { "[x]" } else { "[ ]" }.to_string());
                    }
                    if let Some(icon) = &item.icon {
                        parts.push(icon.to_string());
                    }
                    parts.push(item.label.clone());
                    parts.join(" ")
                };

                ListItem::new(label).style(style)
            })
            .collect();

        let mut block =
            Block::default().borders(self.borders).border_style(theme.style_border(false));

        if !self.block_title.is_empty() {
            block = block.title(self.block_title.as_str());
        }

        let mut list = List::new(list_items).block(block);

        let highlight = if self.is_focused() {
            &self.highlight_symbol
        } else {
            let len = self.highlight_symbol.len();
            &" ".repeat(len)
        };

        list = list.highlight_symbol(highlight);
        StatefulWidgetRef::render_ref(&list, area, buf, &mut self.state);
    }
}
