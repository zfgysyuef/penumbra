/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use ratatui::style::Color;

use crate::themes::Theme;

pub const fn penumbra() -> Theme {
    Theme {
        name: "Penumbra",
        id: "penumbra",
        is_dark: true,
        // #020517
        background: Color::Rgb(2, 5, 23),
        // #16192e
        foreground: Color::Rgb(22, 25, 46),
        // #b7a8d9
        highlight: Color::Rgb(183, 168, 217),
        // #e0def4
        text: Color::Rgb(224, 222, 244),
        // #ffe1f6
        accent: Color::Rgb(255, 225, 246),
        // #eb6f92
        error: Color::Rgb(235, 111, 146),
        // #fbf8b3
        warning: Color::Rgb(251, 248, 179),
        // #9ccfd8
        info: Color::Rgb(156, 207, 216),
        // #c4a7e7
        success: Color::Rgb(196, 167, 231),
        // #d1c7e8
        muted: Color::Rgb(209, 199, 232),
    }
}
