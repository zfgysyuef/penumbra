/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use ratatui::style::Color;

use crate::themes::Theme;

pub const fn catppuccin_mocha() -> Theme {
    Theme {
        name: "Catppuccin Mocha",
        id: "catppuccin_mocha",
        is_dark: true,
        // #1e1e2e (Base)
        background: Color::Rgb(30, 30, 46),
        // #313244 (Surface 0)
        foreground: Color::Rgb(49, 50, 68),
        // #585b70 (Surface 2)
        highlight: Color::Rgb(88, 91, 112),
        // #cdd6f4 (Text)
        text: Color::Rgb(205, 214, 244),
        // #cba6f7 (Mauve)
        accent: Color::Rgb(203, 166, 247),
        // #f38ba8 (Red)
        error: Color::Rgb(243, 139, 168),
        // #f9e2af (Yellow)
        warning: Color::Rgb(249, 226, 175),
        // #89b4fa (Blue)
        info: Color::Rgb(137, 180, 250),
        // #a6e3a1 (Green)
        success: Color::Rgb(166, 227, 161),
        // #a6adc8 (Subtext 0)
        muted: Color::Rgb(166, 173, 200),
    }
}
