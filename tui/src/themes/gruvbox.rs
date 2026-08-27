/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use ratatui::style::Color;

use crate::themes::Theme;

pub const fn gruvbox_light() -> Theme {
    Theme {
        name: "Gruvbox",
        id: "gruvbox_light",
        is_dark: false,
        // #fbf1c7
        background: Color::Rgb(251, 241, 199),
        // #3c3836
        foreground: Color::Rgb(60, 56, 54),
        // #ebdbb2
        highlight: Color::Rgb(235, 219, 178),
        // #3c3836
        text: Color::Rgb(60, 56, 54),
        // #af3a03 faded_orange
        accent: Color::Rgb(175, 58, 3),
        // #9d0006 faded_red
        error: Color::Rgb(157, 0, 6),
        // #b57614 faded_yellow
        warning: Color::Rgb(181, 118, 20),
        // #076678 faded_blue
        info: Color::Rgb(7, 102, 120),
        // #79740e faded_green
        success: Color::Rgb(121, 116, 14),
        // #7c6f64 dark4
        muted: Color::Rgb(124, 111, 100),
    }
}

pub const fn gruvbox_dark() -> Theme {
    Theme {
        name: "Gruvbox Dark",
        id: "gruvbox_dark",
        is_dark: true,
        // #282828
        background: Color::Rgb(40, 40, 40),
        // #ebdbb2
        foreground: Color::Rgb(235, 219, 178),
        // #3c3836
        highlight: Color::Rgb(60, 56, 54),
        // #ebdbb2
        text: Color::Rgb(235, 219, 178),
        // #fe8019 bright_orange
        accent: Color::Rgb(254, 128, 25),
        // #fb4934 bright_red
        error: Color::Rgb(251, 73, 52),
        // #fabd2f bright_yellow
        warning: Color::Rgb(250, 189, 47),
        // #83a598 bright_blue
        info: Color::Rgb(131, 165, 152),
        // #b8bb26 bright_green
        success: Color::Rgb(184, 187, 38),
        // #a89984 light4
        muted: Color::Rgb(168, 153, 132),
    }
}
