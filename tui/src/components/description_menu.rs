/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::components::Component;
use crate::themes::Theme;

#[derive(Clone, Debug)]
pub struct DescriptionMenuItem<A> {
    pub icon: char,
    pub label: String,
    pub description: String,
    pub action: A,
}

impl<A> DescriptionMenuItem<A> {
    pub fn new(
        icon: char,
        label: impl Into<String>,
        description: impl Into<String>,
        action: A,
    ) -> Self {
        Self { icon, label: label.into(), description: description.into(), action }
    }
}

pub struct DescriptionMenu<A> {
    pub items: Vec<DescriptionMenuItem<A>>,
    pub selected: usize,
    scroll_offset: usize,
    max_visible: usize,
}

impl<A> DescriptionMenu<A> {
    const MAX_VISIBLE: usize = 8;

    pub const fn new(items: Vec<DescriptionMenuItem<A>>) -> Self {
        Self { items, selected: 0, scroll_offset: 0, max_visible: Self::MAX_VISIBLE }
    }

    pub fn selected_action(&self) -> Option<&A> {
        self.items.get(self.selected).map(|item| &item.action)
    }

    pub fn selected_item(&self) -> Option<&DescriptionMenuItem<A>> {
        self.items.get(self.selected)
    }

    pub const fn selected_index(&self) -> usize {
        self.selected
    }

    pub const fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
        self.adjust_scroll();
    }

    pub const fn previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.items.len() - 1;
        } else {
            self.selected -= 1;
        }
        self.adjust_scroll();
    }

    const fn adjust_scroll(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.max_visible {
            self.scroll_offset = self.selected + 1 - self.max_visible;
        }
    }

    pub fn set_max_visible(&mut self, max: usize) {
        self.max_visible = max.max(1).min(self.items.len().max(1));
        self.adjust_scroll();
    }

    fn wrap_text(s: &str, max_width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();

        for word in s.split_whitespace() {
            if !current.is_empty() && current.len() + word.len() + 1 > max_width {
                lines.push(current.clone());
                current.clear();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }

        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }
}

impl<A> Component for DescriptionMenu<A> {
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if self.items.is_empty() {
            return;
        }

        let y_spacing = 1u16;
        let avail = (area.height.saturating_sub(2)) as usize;
        self.set_max_visible(avail);

        let menu_width = 24u16;
        let desc_pad = 4u16;
        let desc_width = (area.width.saturating_sub(menu_width + desc_pad)).clamp(18, 36);
        let n_shown = self.items.len().min(self.max_visible);

        for i in 0..n_shown {
            let idx = self.scroll_offset + i;
            if idx >= self.items.len() {
                break;
            }

            let item = &self.items[idx];
            let is_selected = idx == self.selected;
            let y = area.y + (i as u16) * (1 + y_spacing);

            let prefix = if is_selected { "> " } else { "  " };
            let style = if is_selected { theme.style_highlight() } else { theme.style_text() };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{} ", item.icon), style),
                Span::styled(&item.label, style),
            ]);

            buf.set_line(area.x, y, &line, menu_width);
        }

        if let Some(item) = self.selected_item() {
            let desc_x = area.x + menu_width + desc_pad;
            let lines = Self::wrap_text(&item.description, desc_width.saturating_sub(4) as usize);

            let box_height = (lines.len() as u16 + 2).min(area.height);
            let max_box_height = area.height.min(box_height);

            let mut box_y = area.y
                + (self.selected.saturating_sub(self.scroll_offset) as u16) * (1 + y_spacing);
            if box_y + box_height > area.y + area.height {
                box_y = (area.y + area.height).saturating_sub(box_height);
            }

            let accent_top = format!("╭{}╮", "─".repeat(desc_width.saturating_sub(2) as usize));
            buf.set_string(desc_x, box_y, &accent_top, theme.style_border(false));

            for (j, line) in lines.iter().enumerate() {
                let y = box_y + 1 + j as u16;
                if (j as u16) + 1 >= max_box_height.saturating_sub(1) || y >= area.y + area.height {
                    continue;
                }

                let line_width = desc_width.saturating_sub(4) as usize;
                let padded_text = format!("{:<width$}", line, width = line_width);

                let line_spans = Line::from(vec![
                    Span::styled("│ ", theme.style_border(false)),
                    Span::styled(padded_text, theme.style_info()),
                    Span::styled(" │", theme.style_border(false)),
                ]);

                buf.set_line(desc_x, y, &line_spans, desc_width);
            }

            let accent_bottom = format!("╰{}╯", "─".repeat(desc_width.saturating_sub(2) as usize));
            buf.set_string(
                desc_x,
                box_y + box_height.saturating_sub(1),
                &accent_bottom,
                theme.style_border(false),
            );
        }
    }
}
