/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use penumbra::hacc::gfh::{GfhFile, GfhType};
use penumbra::hacc::{Da, Preloader, TryRead};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

use crate::app::{AppCtx, AppPage};
use crate::components::layout::MainLayout;
use crate::components::{Component, DescriptionMenu, ExplorerResult, FileExplorer, RectExt, Stars};
use crate::pages::{LOGO, LOGO_ASCII, Page};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    EnterDaMode,
    SelectDa,
    SelectPreloader,
    SelectAuth,
    Options,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileTarget {
    Da,
    Preloader,
    Auth,
}

#[derive(Default)]
enum WelcomeState {
    #[default]
    Idle,
    Browsing {
        explorer: FileExplorer,
        target: FileTarget,
    },
}

pub struct WelcomePage {
    state: WelcomeState,
    menu: DescriptionMenu<MenuAction>,
    stars: Stars,
}

impl WelcomePage {
    pub fn new() -> Self {
        let menu = description_menu![
            ('*', "Enter DA Mode", "Connect to the device and enter Download Agent mode." => MenuAction::EnterDaMode),
            ('*', "Select DA", "Select a custom Download Agent file (.bin)." => MenuAction::SelectDa),
            ('*', "Select Preloader", "Select a custom Preloader file (.bin)." => MenuAction::SelectPreloader),
            ('*', "Select Auth file", "Select an Auth file for BROM (secure devices only)" => MenuAction::SelectAuth),
            ('*', "Options", "Configure Antumbra settings." => MenuAction::Options),
            ('*', "Quit", "Exit Antumbra." => MenuAction::Quit),
        ];

        Self { state: WelcomeState::Idle, menu, stars: Stars::default() }
    }

    fn open_explorer(&mut self, title: &str, target: FileTarget) {
        if let Ok(explorer) = FileExplorer::new(title) {
            let explorer = explorer.extensions(&["bin", "img", "auth"]);
            self.state = WelcomeState::Browsing { explorer, target };
        }
    }

    fn handle_menu_selection(&mut self, ctx: &mut AppCtx) {
        match self.menu.selected_action() {
            Some(MenuAction::SelectDa) => self.open_explorer("Select DA File", FileTarget::Da),
            Some(MenuAction::SelectPreloader) => {
                self.open_explorer("Select Preloader File", FileTarget::Preloader)
            }
            Some(MenuAction::SelectAuth) => {
                self.open_explorer("Select Auth File", FileTarget::Auth)
            }
            Some(MenuAction::EnterDaMode) => {
                if ctx.da_path.is_none() {
                    error_dialog!(ctx, "No DA file selected! Please select a DA file first.");
                    return;
                }

                ctx.change_page(AppPage::DevicePage);
            }
            Some(MenuAction::Options) => {
                ctx.change_page(AppPage::Options);
            }
            Some(MenuAction::Quit) => ctx.quit(),
            None => {}
        }
    }
}

impl Page for WelcomePage {
    fn update(&mut self, ctx: &mut AppCtx) {
        self.stars.tick(ctx);
    }

    fn render(&mut self, frame: &mut Frame<'_>, ctx: &mut AppCtx) {
        let area = frame.area();
        let buf = frame.buffer_mut();

        if ctx.config.tui.show_stars {
            self.stars.compact = ctx.config.tui.compatibility_mode;
            self.stars.render(area, buf, &ctx.theme);
        }

        let banner = if ctx.config.tui.compatibility_mode { LOGO_ASCII } else { LOGO };

        MainLayout::new().with_footer("[↑/↓] Navigate   •   [Enter] Select").render(
            area,
            buf,
            &ctx.theme,
            |content_area, buf| {
                // Group Logo, Menu, and Files together with fixed gaps,
                // using equal top/bottom Fill(1) to center the entire group.
                let [_, logo_block, _, menu_block, _, files_block, _] = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(8), // Logo
                    Constraint::Length(1),
                    Constraint::Length(13), // Menu
                    Constraint::Length(1),
                    Constraint::Length(3), // Files
                    Constraint::Fill(1),
                ])
                .areas(content_area);

                let logo = Paragraph::new(banner)
                    .alignment(Alignment::Center)
                    .style(ctx.theme.style_accent());
                Widget::render(logo, logo_block, buf);

                let menu_area = menu_block.centered_fixed(80, 13);
                self.menu.render(menu_area, buf, &ctx.theme);

                let da_span = match &ctx.da_path {
                    Some(p) => Span::styled(
                        p.file_name().unwrap_or_default().to_string_lossy(),
                        Style::default().fg(ctx.theme.success),
                    ),
                    None => Span::styled("None", ctx.theme.style_muted_bold()),
                };

                let pl_span = match &ctx.preloader_path {
                    Some(p) => Span::styled(
                        p.file_name().unwrap_or_default().to_string_lossy(),
                        Style::default().fg(ctx.theme.success),
                    ),
                    None => Span::styled("None", ctx.theme.style_muted_bold()),
                };

                let auth_span = match &ctx.auth_file_path {
                    Some(p) => Span::styled(
                        p.file_name().unwrap_or_default().to_string_lossy(),
                        Style::default().fg(ctx.theme.success),
                    ),
                    None => Span::styled("None", ctx.theme.style_muted_bold()),
                };

                let selected_files = vec![
                    Line::from(vec![Span::styled("DA: ", ctx.theme.style_muted()), da_span]),
                    Line::from(vec![Span::styled("Preloader: ", ctx.theme.style_muted()), pl_span]),
                    Line::from(vec![Span::styled("Auth: ", ctx.theme.style_muted()), auth_span]),
                ];

                let files_indicator = Paragraph::new(selected_files).alignment(Alignment::Center);
                Widget::render(files_indicator, files_block, buf);
            },
        );

        if let WelcomeState::Browsing { explorer, .. } = &mut self.state {
            let popup_area = area.centered_pct(80, 80);
            Clear.render(popup_area, buf);
            explorer.render(popup_area, buf, &ctx.theme);
        }
    }

    fn handle_input(&mut self, ctx: &mut AppCtx, key: KeyEvent) {
        if let WelcomeState::Browsing { explorer, target } = &mut self.state {
            match explorer.handle_key_event(key) {
                ExplorerResult::Selected(path) => {
                    match target {
                        FileTarget::Da => {
                            let data = std::fs::read(&path).unwrap_or_default();
                            let da = Da::try_read(&data);
                            if da.is_err() {
                                error_dialog!(ctx, "Not a valid DA file!");
                                return;
                            }

                            ctx.da_path = Some(path)
                        }

                        FileTarget::Preloader => {
                            let data = std::fs::read(&path).unwrap_or_default();
                            let preloader = Preloader::try_read(&data);
                            if preloader.is_err() {
                                error_dialog!(ctx, "Not a valid Preloader file!");
                                return;
                            }

                            ctx.preloader_path = Some(path)
                        }
                        FileTarget::Auth => {
                            let data = std::fs::read(&path).unwrap_or_default();
                            let auth = GfhFile::try_read(&data);
                            if auth.is_err() {
                                error_dialog!(ctx, "Not a valid Auth file!");
                                return;
                            }

                            let Some(_) = auth.unwrap().get_gfh(GfhType::ToolAuth) else {
                                error_dialog!(ctx, "Not an Auth file!, missing ToolAuth GFH!");
                                return;
                            };

                            ctx.auth_file_path = Some(path)
                        }
                    }
                    self.state = WelcomeState::Idle;
                }
                ExplorerResult::Cancelled => {
                    self.state = WelcomeState::Idle;
                }
                ExplorerResult::Pending => {}
            }
            return;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.menu.previous(),
            KeyCode::Down | KeyCode::Char('j') => self.menu.next(),
            KeyCode::Enter => self.handle_menu_selection(ctx),
            _ => {}
        }
    }
}
