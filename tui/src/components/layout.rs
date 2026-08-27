/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::components::footer::Footer;
use crate::themes::Theme;

pub trait RectExt {
    fn centered_pct(&self, percent_x: u16, percent_y: u16) -> Rect;
    fn centered_fixed(&self, width: u16, height: u16) -> Rect;
}

impl RectExt for Rect {
    fn centered_pct(&self, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(*self);

        Layout::horizontal([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
    }

    fn centered_fixed(&self, width: u16, height: u16) -> Rect {
        let x = self.x + self.width.saturating_sub(width) / 2;
        let y = self.y + self.height.saturating_sub(height) / 2;
        Self::new(x, y, width.min(self.width), height.min(self.height))
    }
}

pub struct MainLayout<'a> {
    footer_text: Option<&'a str>,
}

impl<'a> MainLayout<'a> {
    pub const fn new() -> Self {
        Self { footer_text: None }
    }

    pub const fn with_footer(mut self, text: &'a str) -> Self {
        self.footer_text = Some(text);
        self
    }

    pub fn render<F>(self, area: Rect, buf: &mut Buffer, theme: &Theme, render_content: F)
    where
        F: FnOnce(Rect, &mut Buffer),
    {
        let [content, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

        render_content(content, buf);

        if let Some(text) = self.footer_text {
            Footer::new(text).render(footer, buf, theme);
        }
    }
}

pub fn get_scrollable_rect(
    v_y: u16,
    height: u16,
    x: u16,
    width: u16,
    scroll_y: u16,
    container: Rect,
) -> Rect {
    let raw_y = (container.y as i32) + (v_y as i32) - (scroll_y as i32);
    let list_top = container.y as i32;
    let list_bottom = (container.y + container.height) as i32;

    if raw_y + (height as i32) <= list_top || raw_y >= list_bottom {
        return Rect::default();
    }

    let (effective_y, effective_height) = if raw_y < list_top {
        let top_cut = list_top - raw_y;
        (container.y, (height as i32 - top_cut).max(0) as u16)
    } else {
        (raw_y as u16, height)
    };

    Rect::new(x, effective_y, width, effective_height).intersection(container)
}
