/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use ratatui::style::Color;

use crate::themes::Theme;

pub const fn rose_pine_moon() -> Theme {
    Theme {
        name: "Rosé Pine Moon",
        id: "rose_pine_moon",
        is_dark: true,
        // #232136
        background: Color::Rgb(35, 33, 54),
        // #393552
        foreground: Color::Rgb(57, 53, 82),
        // ##2a283e
        highlight: Color::Rgb(42, 40, 62),
        // #e0def4
        text: Color::Rgb(224, 222, 244),
        // #ea9a97
        accent: Color::Rgb(234, 154, 151),
        // #eb6f92
        error: Color::Rgb(235, 111, 146),
        // #f6c177
        warning: Color::Rgb(246, 193, 119),
        // #9ccfd8
        info: Color::Rgb(156, 207, 216),
        // #c4a7e7
        success: Color::Rgb(196, 167, 231),
        // #6e6a86
        muted: Color::Rgb(110, 106, 134),
    }
}
