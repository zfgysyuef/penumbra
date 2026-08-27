/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use penumbra::activity::Activity;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::themes::Theme;

pub trait ActivityExt {
    fn status(&self) -> &'static str;
    fn action(&self) -> &'static str;
    fn detail(&self) -> Option<String>;
    fn color(&self, theme: &Theme) -> Color;
}

impl ActivityExt for Activity {
    // What's shown on the status badge
    fn status(&self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::UploadingDa => "Connecting",
            Self::Reading { .. } => "Reading",
            Self::Flashing { .. } => "Writing",
            Self::Erasing { .. } => "Erasing",
            _ => "Busy",
        }
    }

    // What gets used for progress messages
    fn action(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::UploadingDa => "Uploading DA",
            Self::Reading { .. } => "Reading",
            Self::Flashing { .. } => "Flashing",
            Self::Erasing { .. } => "Erasing",
            _ => "Working",
        }
    }

    fn detail(&self) -> Option<String> {
        let partition = match self {
            Self::Flashing { partition }
            | Self::Reading { partition }
            | Self::Erasing { partition } => partition,
            _ => return None,
        };

        Some(format!("{} '{}'", self.action(), partition))
    }

    fn color(&self, theme: &Theme) -> Color {
        match self {
            Self::Idle => theme.success,
            Self::Erasing { .. } => theme.error,
            Self::Flashing { .. } => theme.warning,
            _ => theme.info,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Badge {
    Disconnected,
    Connecting,
    Ready,
    Busy,
    Active(Activity),
}

impl Badge {
    pub fn from_activity(activity: &Activity, working: bool) -> Self {
        match activity {
            Activity::Idle if working => Self::Busy,
            Activity::Idle => Self::Ready,
            other => Self::Active(other.clone()),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Ready => "Ready",
            Self::Busy => "Busy",
            Self::Active(activity) => activity.status(),
        }
    }

    pub fn color(&self, theme: &Theme) -> Color {
        match self {
            Self::Disconnected => theme.muted,
            Self::Connecting | Self::Busy => theme.info,
            Self::Ready => theme.success,
            Self::Active(activity) => activity.color(theme),
        }
    }
}

pub struct ActivityIndicator {
    badge: Badge,
}

impl ActivityIndicator {
    const PADDING: u16 = 1;

    pub const fn new(badge: Badge) -> Self {
        Self { badge }
    }

    pub fn width(&self) -> u16 {
        UnicodeWidthStr::width(self.badge.label()) as u16 + Self::PADDING * 2
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let width = self.width().min(area.width) as usize;

        let style = Style::default()
            .bg(self.badge.color(theme))
            .fg(theme.background)
            .add_modifier(Modifier::BOLD);

        buf.set_string(area.x, area.y, Self::pad(self.badge.label(), width), style);
    }

    fn pad(label: &str, width: usize) -> String {
        if width == 0 {
            return String::new();
        }

        let label_width = UnicodeWidthStr::width(label);

        if label_width <= width {
            let padding = width - label_width;
            let left = padding / 2;

            return format!("{}{}{}", " ".repeat(left), label, " ".repeat(padding - left));
        }

        let mut truncated = String::new();
        let mut current = 0;

        for c in label.chars() {
            let w = UnicodeWidthStr::width(c.encode_utf8(&mut [0; 4]));
            if current + w > width.saturating_sub(1) {
                truncated.push('…');
                break;
            }
            truncated.push(c);
            current += w;
        }

        let truncated_width = UnicodeWidthStr::width(truncated.as_str());
        truncated.push_str(&" ".repeat(width.saturating_sub(truncated_width)));

        truncated
    }
}
