/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::app::AppCtx;
use crate::components::FormField;
use crate::components::layout::{MainLayout, get_scrollable_rect};
use crate::themes::Theme;

type OnChangeCallback = Box<dyn Fn(&mut AppCtx, &str, &str) + Send + Sync>;

pub struct FormItem<F: FormField> {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub field: F,
}

impl<F: FormField> FormItem<F> {
    pub const fn new(
        id: &'static str,
        label: &'static str,
        description: &'static str,
        field: F,
    ) -> Self {
        Self { id, label, description, field }
    }
}

pub struct FormSection<F: FormField> {
    pub title: &'static str,
    pub items: Vec<FormItem<F>>,
}

impl<F: FormField> FormSection<F> {
    pub const fn new(title: &'static str, items: Vec<FormItem<F>>) -> Self {
        Self { title, items }
    }
}

pub struct FormPage<F: FormField> {
    sections: Vec<FormSection<F>>,
    selected_idx: usize,
    scroll_y: u16,
    on_change: Option<OnChangeCallback>,
}

impl<F: FormField> FormPage<F> {
    pub fn new(sections: Vec<FormSection<F>>) -> Self {
        Self { sections, selected_idx: 0, scroll_y: 0, on_change: None }
    }

    pub fn with_on_change<G>(mut self, callback: G) -> Self
    where
        G: Fn(&mut AppCtx, &str, &str) + Send + Sync + 'static,
    {
        self.on_change = Some(Box::new(callback));
        self
    }

    pub fn total_items(&self) -> usize {
        self.sections.iter().map(|s| s.items.len()).sum()
    }

    fn get_item_mut(&mut self, index: usize) -> Option<&mut FormItem<F>> {
        let mut current = 0;
        for section in &mut self.sections {
            if index < current + section.items.len() {
                return Some(&mut section.items[index - current]);
            }
            current += section.items.len();
        }
        None
    }

    fn get_item(&self, index: usize) -> Option<&FormItem<F>> {
        let mut current = 0;
        for section in &self.sections {
            if index < current + section.items.len() {
                return Some(&section.items[index - current]);
            }
            current += section.items.len();
        }
        None
    }

    pub fn next_field(&mut self) {
        let total = self.total_items();
        if self.selected_idx < total {
            self.selected_idx += 1;
        }
    }

    pub const fn previous_field(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    pub fn selected_item_mut(&mut self) -> Option<&mut FormItem<F>> {
        self.get_item_mut(self.selected_idx)
    }

    pub fn selected_item(&self) -> Option<&FormItem<F>> {
        self.get_item(self.selected_idx)
    }

    pub const fn selected_index(&self) -> usize {
        self.selected_idx
    }

    pub const fn set_selected_index(&mut self, idx: usize) {
        self.selected_idx = idx;
    }

    pub fn sections(&self) -> &[FormSection<F>] {
        &self.sections
    }

    pub fn sections_mut(&mut self) -> &mut [FormSection<F>] {
        &mut self.sections
    }

    pub fn render_form(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        title: &str,
        help_text: &str,
    ) {
        // TODO: Make macros because, please.
        MainLayout::new().with_footer(help_text).render(area, buf, theme, |content_area, buf| {
            let inner_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Title
                    Constraint::Min(1),    // Form
                ])
                .margin(2)
                .split(content_area);

            Paragraph::new(title)
                .alignment(Alignment::Center)
                .style(theme.style_title())
                .render(inner_layout[0], buf);

            let list_center = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Fill(1), Constraint::Length(80), Constraint::Fill(1)])
                .split(inner_layout[1])[1];

            let total_items = self.total_items();
            let mut target_top = 0u16;
            let mut target_bottom = 0u16;
            let mut global_idx = 0;
            let mut v_y = 0u16;

            for section in &self.sections {
                let section_height = (section.items.len() as u16 * 4).saturating_add(2);
                let inner_v_y = v_y.saturating_add(1);

                for (local_idx, _) in section.items.iter().enumerate() {
                    let item_v_y = inner_v_y + (local_idx as u16 * 4);
                    if global_idx == self.selected_index() {
                        target_top = item_v_y;
                        target_bottom = item_v_y.saturating_add(4);
                    }
                    global_idx += 1;
                }
                v_y += section_height.saturating_add(1);
            }

            let back_v_y = v_y;
            let back_height = 3u16;
            if self.selected_idx == total_items {
                target_top = back_v_y;
                target_bottom = back_v_y + back_height;
            }

            v_y += back_height;
            let total_v_height = v_y;

            let visible_height = list_center.height;
            if target_bottom > self.scroll_y + visible_height {
                self.scroll_y = target_bottom.saturating_sub(visible_height);
            }
            if target_top < self.scroll_y {
                self.scroll_y = target_top;
            }
            let max_scroll = total_v_height.saturating_sub(visible_height);
            self.scroll_y = self.scroll_y.min(max_scroll);

            let scroll_y = self.scroll_y;

            global_idx = 0;
            let mut current_v_y = 0u16;

            for section in &mut self.sections {
                let section_height = (section.items.len() as u16 * 4).saturating_add(2);
                let section_area = get_scrollable_rect(
                    current_v_y,
                    section_height,
                    list_center.x,
                    list_center.width,
                    scroll_y,
                    list_center,
                );

                if section_area.height > 0 && section_area.width > 0 {
                    Block::default()
                        .title(format!(" {} ", section.title))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.muted))
                        .render(section_area, buf);
                }

                let inner_v_y = current_v_y.saturating_add(1);
                let inner_x = list_center.x.saturating_add(1);
                let inner_width = list_center.width.saturating_sub(4);

                for (local_idx, item) in section.items.iter_mut().enumerate() {
                    let active = global_idx == self.selected_idx;
                    let item_v_y = inner_v_y + (local_idx as u16 * 4);
                    let item_area = get_scrollable_rect(
                        item_v_y,
                        4,
                        inner_x,
                        inner_width,
                        scroll_y,
                        list_center,
                    );

                    if item_area.height > 0 && item_area.width > 0 {
                        let chunks = Layout::default()
                            .constraints([Constraint::Length(1), Constraint::Length(3)])
                            .split(item_area);

                        if chunks[0].height > 0 {
                            let label_style = theme.style_label(active);
                            let highlight_symbol = if active { "> " } else { "  " };

                            let line = Line::from(vec![
                                Span::styled(highlight_symbol, label_style),
                                Span::styled(item.label, label_style),
                                Span::styled(" - ", theme.style_description()),
                                Span::styled(item.description, theme.style_description()),
                            ]);
                            Paragraph::new(vec![line]).render(chunks[0], buf);
                        }

                        if chunks[1].height > 0 {
                            let mut field_area = chunks[1];
                            field_area.x = field_area.x.saturating_add(2);
                            field_area.width = field_area.width.saturating_sub(2);

                            item.field.render(field_area, buf, theme);
                        }
                    }
                    global_idx += 1;
                }
                current_v_y += section_height.saturating_add(1);
            }

            let back_area = get_scrollable_rect(
                back_v_y,
                back_height,
                list_center.x,
                list_center.width,
                scroll_y,
                list_center,
            );
            if back_area.height > 0 && back_area.width > 0 {
                let back_btn_style = if self.selected_idx == total_items {
                    theme.style_accent()
                } else {
                    theme.style_muted_bold()
                };

                Paragraph::new(" [ Back ] ")
                    .alignment(Alignment::Center)
                    .style(back_btn_style)
                    .render(back_area, buf);
            }

            current_v_y = 0;
            for section in &mut self.sections {
                let inner_v_y = current_v_y.saturating_add(1);
                for (local_idx, item) in section.items.iter_mut().enumerate() {
                    let item_v_y = inner_v_y + (local_idx as u16 * 4);
                    let item_area = get_scrollable_rect(
                        item_v_y,
                        4,
                        list_center.x.saturating_add(1),
                        list_center.width.saturating_sub(4),
                        scroll_y,
                        list_center,
                    );

                    if item_area.height > 0 && item_area.width > 0 {
                        let chunks = Layout::default()
                            .constraints([Constraint::Length(1), Constraint::Length(3)])
                            .split(item_area);

                        let mut overlay_area = chunks[1];
                        overlay_area.x = overlay_area.x.saturating_add(2);
                        overlay_area.width = overlay_area.width.saturating_sub(2);

                        item.field.render_overlay(overlay_area, buf, theme);
                    }
                }
                let section_height = (section.items.len() as u16 * 4).saturating_add(2);
                current_v_y += section_height.saturating_add(1);
            }
        });
    }

    pub fn handle_form_input(&mut self, ctx: &mut AppCtx, key: KeyEvent) -> bool {
        let total = self.total_items();

        if let Some(item) = self.get_item_mut(self.selected_idx)
            && item.field.is_focused()
        {
            let was_focused = item.field.is_focused();
            let handled = item.field.handle_key(key, ctx);
            if handled && was_focused && !item.field.is_focused() {
                let id = item.id;
                let value = item.field.value();
                let on_change = self.on_change.as_ref();
                if let Some(cb) = on_change {
                    cb(ctx, id, &value);
                }
            }
            return true;
        }

        match key.code {
            KeyCode::Up if self.selected_idx > 0 => {
                self.selected_idx -= 1;
                true
            }
            KeyCode::Down if self.selected_idx < total => {
                self.selected_idx += 1;
                true
            }
            KeyCode::Tab => {
                self.next_field();
                true
            }
            KeyCode::BackTab => {
                self.previous_field();
                true
            }
            _ => {
                if let Some(item) = self.get_item_mut(self.selected_idx)
                    && item.field.handle_key(key, ctx)
                {
                    if !item.field.is_focused() {
                        let id = item.id;
                        let value = item.field.value();
                        let on_change = self.on_change.as_ref();
                        if let Some(cb) = on_change {
                            cb(ctx, id, &value);
                        }
                    }
                    return true;
                }
                false
            }
        }
    }
}
