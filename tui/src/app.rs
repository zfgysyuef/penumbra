/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::fs::read;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use penumbra::da::DAFile;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::{DefaultTerminal, Frame};

use crate::cli::CliArgs;
use crate::components::ThemedWidgetRef;
use crate::components::dialog::{Dialog, DialogBuilder};
use crate::config::AntumbraConfig;
use crate::pages::{DevicePage, OptionsPage, Page, WelcomePage};
use crate::themes::{Theme, load_themes};

#[derive(PartialEq, Clone, Copy, Default)]
pub enum AppPage {
    #[default]
    Welcome,
    DevicePage,
    Options,
}

pub struct AppCtx {
    loader: Option<Loader>,
    preloader: Option<Preloader>,
    exit: bool,
    current_page_id: AppPage,
    next_page_id: Option<AppPage>,
    config: AntumbraConfig,
    pub theme: Theme,
    pub dialog: Option<Dialog>,
    force_heapb8: bool,
}

pub struct App {
    current_page: Box<dyn Page + Send>,
    pub context: AppCtx,
}

pub struct Loader {
    path: PathBuf,
    file: DAFile,
}

impl Loader {
    pub fn new(path: PathBuf, file: DAFile) -> Self {
        Self { path, file }
    }

    pub fn file(&self) -> &DAFile {
        &self.file
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn loader_name(&self) -> Option<String> {
        self.path().file_name().and_then(|name| name.to_str()).map(|s| s.to_string())
    }
}

pub struct Preloader {
    path: PathBuf,
    data: Vec<u8>,
}

impl Preloader {
    pub fn new(path: PathBuf, data: Vec<u8>) -> Self {
        Self { path, data }
    }

    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn file_name(&self) -> Option<String> {
        self.path().file_name().and_then(|name| name.to_str()).map(|s| s.to_string())
    }
}

impl AppCtx {
    pub fn loader(&self) -> Option<&Loader> {
        self.loader.as_ref()
    }

    pub fn set_loader(&mut self, loader_path: PathBuf, loader_file: DAFile) {
        if let Some(loader) = self.loader.as_mut() {
            loader.path = loader_path;
            loader.file = loader_file;
        } else {
            self.loader = Some(Loader::new(loader_path, loader_file));
        }
    }

    pub fn loader_name(&self) -> String {
        self.loader.as_ref().and_then(|l| l.loader_name()).unwrap_or("Unknown DA".to_string())
    }

    pub fn preloader(&self) -> Option<&Preloader> {
        self.preloader.as_ref()
    }

    pub fn preloader_name(&self) -> String {
        self.preloader.as_ref().and_then(|p| p.file_name()).unwrap_or("No Preloader".to_string())
    }

    pub fn set_preloader(&mut self, preloader_path: PathBuf, preloader_data: Vec<u8>) {
        if let Some(preloader) = self.preloader.as_mut() {
            preloader.path = preloader_path;
            preloader.data = preloader_data;
        } else {
            self.preloader = Some(Preloader::new(preloader_path, preloader_data));
        }
    }

    pub fn set_dialog(&mut self, dialog: &mut DialogBuilder) {
        self.dialog = Some(dialog.build().expect("Failed to build dialog"));
    }

    pub fn change_page(&mut self, page: AppPage) {
        self.next_page_id = Some(page);
    }

    pub fn quit(&mut self) {
        self.exit = true;
    }

    pub fn set_theme(&mut self, theme_id: &str) {
        let themes = load_themes();
        if let Some(theme) = themes.get(theme_id) {
            self.theme = theme();
            self.config.theme = self.theme.id.to_string();
            self.config.save().ok();
        }
    }

    pub fn config(&mut self) -> &mut AntumbraConfig {
        &mut self.config
    }

    pub const fn force_heapb8(&self) -> bool {
        self.force_heapb8
    }
}

impl Default for AppCtx {
    fn default() -> Self {
        let config = AntumbraConfig::load();
        let theme_map = load_themes();

        let theme = theme_map
            .get(config.theme.as_str())
            .map(|constructor| constructor())
            .unwrap_or_default();

        Self {
            loader: None,
            preloader: None,
            exit: false,
            current_page_id: AppPage::default(),
            next_page_id: None,
            config,
            theme,
            dialog: None,
            force_heapb8: false,
        }
    }
}

impl App {
    pub fn new(args: &CliArgs) -> App {
        let mut ctx = AppCtx::default();
        ctx.force_heapb8 = args.force_heapb8;

        if let Some(da_path) = &args.da_file
            && let Ok(raw_data) = read(da_path)
            && let Ok(file) = DAFile::parse_da(&raw_data)
        {
            ctx.set_loader(da_path.clone(), file)
        }

        App { current_page: Box::new(WelcomePage::new()), context: ctx }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.current_page.on_enter(&mut self.context).await;

        while !self.context.exit {
            if let Some(next_page) = self.context.next_page_id.take() {
                self.switch_to(next_page).await;
            }

            self.current_page.update(&mut self.context).await;
            terminal.draw(|f: &mut Frame<'_>| self.draw(f))?;

            self.handle_events().await?;
        }
        Ok(())
    }

    async fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            // Force exit: [Ctrl + Delete]
            if key.code == KeyCode::Delete && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.context.quit();
            }

            if let Some(dialog) = &mut self.context.dialog {
                match key.code {
                    KeyCode::Left => dialog.move_left(),
                    KeyCode::Right => dialog.move_right(),
                    KeyCode::Enter => {
                        dialog.press_selected();
                        self.context.dialog = None;
                    }
                    KeyCode::Esc => {
                        self.context.dialog = None;
                    }
                    _ => {}
                }
                return Ok(());
            }

            self.current_page.handle_input(&mut self.context, key).await;
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let size = frame.area();

        let style = Style::default().bg(self.context.theme.background);
        let background = Block::default().style(style);
        frame.render_widget(background, size);

        self.current_page.render(frame, &mut self.context);

        if let Some(dialog) = &self.context.dialog {
            dialog.render_ref(size, frame.buffer_mut(), &self.context.theme);
        }
    }

    pub async fn switch_to(&mut self, page: AppPage) {
        self.current_page.on_exit(&mut self.context).await;

        self.context.current_page_id = page;

        let new_page: Box<dyn Page + Send> = match page {
            AppPage::Welcome => Box::new(WelcomePage::new()),
            AppPage::DevicePage => Box::new(DevicePage::new()),
            AppPage::Options => Box::new(OptionsPage::new()),
        };

        self.current_page = new_page;
        self.current_page.on_enter(&mut self.context).await;
    }
}
