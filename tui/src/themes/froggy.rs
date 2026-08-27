/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use ratatui::style::Color;

use crate::themes::Theme;

pub const fn froggy_dark() -> Theme {
    Theme {
        name: "Froggy",
        id: "froggy",
        is_dark: true,
        // #131820
        background: Color::Rgb(19, 24, 32),
        // #1a2027
        foreground: Color::Rgb(26, 32, 39),
        // #233a2d
        highlight: Color::Rgb(35, 58, 45),
        // #e8eaed
        text: Color::Rgb(232, 234, 237),
        // #81c784
        accent: Color::Rgb(129, 199, 132),
        // #fc85a5
        error: Color::Rgb(252, 133, 165),
        // #ffcf72
        warning: Color::Rgb(255, 207, 114),
        // #a78bfa
        info: Color::Rgb(167, 139, 250),
        // #4caf50
        success: Color::Rgb(76, 175, 80),
        // #aebed0
        muted: Color::Rgb(174, 190, 208),
    }
}
