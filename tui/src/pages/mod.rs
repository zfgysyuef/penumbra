/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
pub mod device;
pub mod options;
pub mod welcome;
pub use device::DevicePage;
pub use options::OptionsPage;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
pub use welcome::WelcomePage;

use crate::app::AppCtx;

pub const LOGO: &str = include_str!("../logo.txt");
pub const LOGO_ASCII: &str = include_str!("../logo_ascii.txt");

pub trait Page {
    fn render(&mut self, frame: &mut Frame<'_>, ctx: &mut AppCtx);
    fn handle_input(&mut self, ctx: &mut AppCtx, key: KeyEvent);
    fn on_enter(&mut self, _ctx: &mut AppCtx) {}
    fn on_exit(&mut self, _ctx: &mut AppCtx) {}
    fn update(&mut self, _ctx: &mut AppCtx) {}
}
