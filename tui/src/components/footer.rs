/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::themes::Theme;

#[derive(Debug, Clone)]
pub struct Footer {
    text: String,
    alignment: Alignment,
}

impl Default for Footer {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl Footer {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), alignment: Alignment::Center }
    }

    pub const fn aligned(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height == 0 || self.text.is_empty() {
            return;
        }

        let mut spans = Vec::new();
        let mut current_text = String::new();
        let mut in_bracket = false;

        for c in self.text.chars() {
            if c == '[' {
                if !current_text.is_empty() {
                    spans
                        .push(Span::styled(current_text.clone(), Style::default().fg(theme.muted)));
                    current_text.clear();
                }
                in_bracket = true;
                current_text.push(c);
            } else if c == ']' && in_bracket {
                current_text.push(c);
                spans.push(Span::styled(current_text.clone(), Style::default().fg(theme.accent)));
                current_text.clear();
                in_bracket = false;
            } else {
                current_text.push(c);
            }
        }

        if !current_text.is_empty() {
            let style = if in_bracket {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.muted)
            };
            spans.push(Span::styled(current_text, style));
        }

        ratatui::widgets::Paragraph::new(Line::from(spans))
            .alignment(self.alignment)
            .render(area, buf);
    }
}
