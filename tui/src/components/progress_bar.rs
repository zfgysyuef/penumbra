/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::time::{Duration, Instant};

use human_bytes::human_bytes;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Buffer, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, WidgetRef};

use crate::app::AppCtx;
use crate::components::Component;
use crate::themes::Theme;

#[derive(Debug, Clone)]
pub enum ProgressMode {
    Idle,
    Active,
    Finished { since: Instant },
}

pub struct ProgressBar {
    mode: ProgressMode,
    total_bytes: u64,
    written_bytes: u64,
    message: String,
    start_time: Option<Instant>,
}

impl ProgressBar {
    const FINISH_MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);
    const IDLE_TEXT: &'static str = "No active operation";

    pub fn new() -> Self {
        Self {
            mode: ProgressMode::Idle,
            total_bytes: 0,
            written_bytes: 0,
            message: String::from(Self::IDLE_TEXT),
            start_time: None,
        }
    }

    pub fn start(&mut self, total_bytes: u64, message: impl Into<String>) {
        self.mode = ProgressMode::Active;
        self.total_bytes = total_bytes;
        self.written_bytes = 0;
        self.message = message.into();
        self.start_time = Some(Instant::now());
    }

    pub fn set_written(&mut self, bytes: u64) {
        if matches!(self.mode, ProgressMode::Active) {
            self.written_bytes = bytes.min(self.total_bytes);
        }
    }

    pub const fn set_total(&mut self, total_bytes: u64) {
        if matches!(self.mode, ProgressMode::Active) {
            self.total_bytes = total_bytes;
        }
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        if matches!(self.mode, ProgressMode::Active) {
            self.message = message.into();
        }
    }

    pub const fn is_active(&self) -> bool {
        matches!(self.mode, ProgressMode::Active)
    }

    pub fn finish(&mut self, message: impl Into<String>) {
        self.mode = ProgressMode::Finished { since: Instant::now() };
        self.total_bytes = 0;
        self.written_bytes = 0;
        self.message = message.into();
        self.start_time = None;
    }

    pub fn reset(&mut self) {
        self.mode = ProgressMode::Idle;
        self.total_bytes = 0;
        self.written_bytes = 0;
        self.message = String::from(Self::IDLE_TEXT);
        self.start_time = None;
    }

    fn ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.written_bytes as f64 / self.total_bytes as f64
        }
    }

    fn speed(&self) -> f64 {
        self.start_time.map_or(0.0, |start| {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 { self.written_bytes as f64 / elapsed } else { 0.0 }
        })
    }
}

impl Component for ProgressBar {
    fn tick(&mut self, _ctx: &mut AppCtx) {
        if let ProgressMode::Finished { since } = self.mode
            && since.elapsed() >= Self::FINISH_MESSAGE_TIMEOUT
        {
            self.reset();
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height < 2 {
            return;
        }

        let style = match self.mode {
            ProgressMode::Idle => theme.style_muted().add_modifier(Modifier::ITALIC),
            ProgressMode::Finished { .. } => Style::default().fg(theme.success),
            ProgressMode::Active => theme.style_accent(),
        };

        let [_, inner_area, _] =
            Layout::horizontal([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
                .areas(area);

        match self.mode {
            ProgressMode::Idle | ProgressMode::Finished { .. } => {
                let lines = vec![
                    Line::from(Span::styled(self.message.as_str(), style)),
                    Line::from(Span::raw("")),
                ];

                Paragraph::new(lines).render_ref(inner_area, buf);
            }

            ProgressMode::Active => {
                let [bar_area, text_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
                        .areas(inner_area);

                let bar_width = bar_area.width.saturating_sub(6) as usize;
                let filled = (self.ratio() * bar_width as f64).round() as usize;
                let empty = bar_width.saturating_sub(filled);
                let percent = (self.ratio() * 100.0).round() as u8;

                let bar = format!("{}{} {:>3}%", "█".repeat(filled), "░".repeat(empty), percent);

                let written = human_bytes(self.written_bytes as f64);
                let total = human_bytes(self.total_bytes as f64);
                let speed = human_bytes(self.speed());

                let stats = format!("{written} / {total}  •  {speed}/s");
                let stats_width = stats.chars().count() as u16;

                let [msg_area, stats_area] =
                    Layout::horizontal([Constraint::Min(0), Constraint::Length(stats_width)])
                        .areas(text_area);

                Paragraph::new(Line::from(Span::styled(bar, style))).render_ref(bar_area, buf);
                Paragraph::new(Line::from(Span::styled(&self.message, style)))
                    .render_ref(msg_area, buf);
                Paragraph::new(Line::from(Span::styled(stats, theme.style_text())))
                    .render_ref(stats_area, buf);
            }
        }
    }
}
