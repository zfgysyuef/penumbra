/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

pub struct Card<'a> {
    pub label: &'a str,
    pub value: &'a str,
    pub width: u16,
    pub border_style: Style,
}

impl<'a> Card<'a> {
    pub const fn new(label: &'a str, value: &'a str, width: u16, border_style: Style) -> Self {
        Self { label, value, width, border_style }
    }

    pub fn render(&self, buf: &mut Buffer, x: u16, y: u16) {
        // Like, FR?
        if self.width < 4 {
            return;
        }

        // -2 accounting for borders
        let inner_width = (self.width - 2) as usize;
        let label_width = UnicodeWidthStr::width(self.label);
        let available_width = inner_width.saturating_sub(label_width + 1);

        let mut truncated_value = String::new();
        let mut current_width = 0;
        for c in self.value.chars() {
            let w = UnicodeWidthStr::width(c.encode_utf8(&mut [0; 4]));
            if current_width + w > available_width - 1 {
                truncated_value.push('…');
                break;
            }
            truncated_value.push(c);
            current_width += w;
        }

        let mut content = format!("{} {}", self.label, truncated_value);
        let content_width = UnicodeWidthStr::width(content.as_str());

        if content_width < inner_width {
            // Padding is needed to align the right |
            let padding = " ".repeat(inner_width - content_width);
            content.push_str(&padding);
        } else if content_width > inner_width {
            content = content.chars().take(inner_width).collect();
        }

        buf.set_string(x, y, format!("╭{}╮", "─".repeat(inner_width)), self.border_style);
        buf.set_string(x, y + 1, format!("│{}│", content), self.border_style);
        buf.set_string(x, y + 2, format!("╰{}╯", "─".repeat(inner_width)), self.border_style);
    }
}

pub struct CardRow<'a> {
    pub cards: Vec<Card<'a>>,
    pub pad: u16,
}

impl<'a> CardRow<'a> {
    pub const fn new(cards: Vec<Card<'a>>, pad: u16) -> Self {
        Self { cards, pad }
    }

    pub fn render(&self, buf: &mut Buffer, area_x: u16, area_width: u16, y: u16) {
        let total_width: u16 = self.cards.iter().map(|c| c.width).sum::<u16>()
            + self.pad.saturating_mul(self.cards.len().saturating_sub(1) as u16);

        let mut x = area_x + area_width.saturating_sub(total_width) / 2;

        for card in &self.cards {
            card.render(buf, x, y);
            x += card.width + self.pad;
        }
    }
}
