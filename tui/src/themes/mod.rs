/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};

mod catpuccin;
mod froggy;
mod gruvbox;
mod penumbra;
mod rose_pine;

pub type ThemeConstructor = fn() -> Theme;
pub type ThemeRegistry = HashMap<&'static str, ThemeConstructor>;

pub struct Theme {
    pub name: &'static str,
    pub id: &'static str,
    pub is_dark: bool,
    pub background: Color,
    pub foreground: Color,
    pub highlight: Color,
    pub text: Color,
    pub accent: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub success: Color,
    pub muted: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "System",
            id: "system",
            is_dark: true,
            background: Color::Reset,
            foreground: Color::Gray,
            highlight: Color::Black,
            text: Color::Reset,
            accent: Color::Cyan,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::LightBlue,
            success: Color::LightGreen,
            muted: Color::DarkGray,
        }
    }
}

impl Theme {
    pub fn style_accent(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn style_accent_bold(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn style_text(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn style_muted(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn style_muted_bold(&self) -> Style {
        Style::default().fg(self.muted).add_modifier(Modifier::BOLD)
    }

    pub fn style_title(&self) -> Style {
        self.style_accent_bold()
    }

    pub fn style_label(&self, active: bool) -> Style {
        if active { self.style_accent_bold() } else { self.style_text() }
    }

    pub fn style_description(&self) -> Style {
        self.style_muted()
    }

    pub fn style_border(&self, active: bool) -> Style {
        if active { self.style_accent() } else { self.style_muted() }
    }

    pub fn style_highlight(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn style_info(&self) -> Style {
        Style::default().fg(self.info).add_modifier(Modifier::ITALIC | Modifier::BOLD)
    }
}

pub fn load_themes() -> ThemeRegistry {
    let mut themes: ThemeRegistry = HashMap::new();

    themes.insert("system", Theme::default);
    themes.insert("rose_pine_moon", rose_pine::rose_pine_moon);
    themes.insert("gruvbox_light", gruvbox::gruvbox_light);
    themes.insert("gruvbox_dark", gruvbox::gruvbox_dark);
    themes.insert("catppuccin_mocha", catpuccin::catppuccin_mocha);
    themes.insert("penumbra", penumbra::penumbra);
    themes.insert("froggy", froggy::froggy_dark);

    themes
}
