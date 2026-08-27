/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::{DefaultTerminal, Frame};

use crate::cli::CliArgs;
use crate::components::dialog::{Dialog, DialogBuilder};
use crate::components::{Component, RectExt};
use crate::config::AntumbraConfig;
use crate::pages::{DevicePage, OptionsPage, Page, WelcomePage};
use crate::themes::{Theme, load_themes};

pub enum AppEvent {
    Input(KeyEvent),
    Resize(u16, u16),
    Tick,
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug)]
pub enum AppPage {
    #[default]
    Welcome,
    DevicePage,
    Options,
}

pub struct AppCtx {
    pub da_path: Option<PathBuf>,
    pub preloader_path: Option<PathBuf>,
    pub auth_file_path: Option<PathBuf>,

    pub exit: bool,
    pub current_page_id: AppPage,
    pub next_page_id: Option<AppPage>,
    pub config: Arc<AntumbraConfig>,
    pub theme: Theme,
    pub dialog: Option<Dialog>,
    pub event_tx: Sender<AppEvent>,
}

impl AppCtx {
    pub fn new(config: Arc<AntumbraConfig>, event_tx: Sender<AppEvent>) -> Self {
        let theme_map = load_themes();
        let theme = theme_map
            .get(config.tui.theme.as_str())
            .map(|constructor| constructor())
            .unwrap_or_default();

        Self {
            da_path: None,
            preloader_path: None,
            auth_file_path: None,
            exit: false,
            current_page_id: AppPage::default(),
            next_page_id: None,
            config,
            theme,
            dialog: None,
            event_tx,
        }
    }

    pub fn set_dialog(&mut self, dialog: &DialogBuilder) {
        self.dialog = Some(dialog.build().expect("Failed to build dialog"));
    }

    pub const fn change_page(&mut self, page: AppPage) {
        self.next_page_id = Some(page);
    }

    pub fn replace_config(&mut self, config: AntumbraConfig) {
        self.config = Arc::new(config);
    }

    pub const fn quit(&mut self) {
        self.exit = true;
    }
}

pub struct App {
    current_page: Box<dyn Page + Send>,
    pub context: AppCtx,
    event_rx: Receiver<AppEvent>,
}

impl App {
    const TICK_RATE: Duration = Duration::from_millis(30);

    pub fn new(args: &CliArgs, config: Arc<AntumbraConfig>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut ctx = AppCtx::new(config, tx.clone());

        if let Some(da_path) = &args.da_file {
            ctx.da_path = Some(da_path.clone());
        }

        if let Some(pl_path) = &args.preloader_file {
            ctx.preloader_path = Some(pl_path.clone());
        }

        if let Some(auth_path) = &args.auth_file {
            ctx.auth_file_path = Some(auth_path.clone());
        }

        Self::spawn_input_thread(tx);

        Self { current_page: Box::new(WelcomePage::new()), context: ctx, event_rx: rx }
    }

    fn spawn_input_thread(tx: Sender<AppEvent>) {
        thread::spawn(move || {
            let mut last_tick = Instant::now();

            loop {
                let timeout = Self::TICK_RATE.saturating_sub(last_tick.elapsed());

                if matches!(event::poll(timeout), Ok(true)) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) if tx.send(AppEvent::Input(key)).is_err() => {
                            break;
                        }
                        Ok(CrosstermEvent::Resize(w, h))
                            if tx.send(AppEvent::Resize(w, h)).is_err() =>
                        {
                            break;
                        }
                        _ => {}
                    }
                }

                if last_tick.elapsed() >= Self::TICK_RATE {
                    if tx.send(AppEvent::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        self.current_page.on_enter(&mut self.context);

        while !self.context.exit {
            if let Some(next_page) = self.context.next_page_id.take() {
                self.switch_to(next_page);
            }

            terminal.draw(|f: &mut Frame<'_>| self.draw(f))?;

            if let Ok(event) = self.event_rx.recv() {
                self.handle_event(event);
            }

            while let Ok(event) = self.event_rx.try_recv() {
                self.handle_event(event);
            }
        }

        self.current_page.on_exit(&mut self.context);

        Ok(())
    }

    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Input(key) => {
                // Windows sends double key events, causing
                // double input.
                #[cfg(target_os = "windows")]
                if key.kind != ratatui::crossterm::event::KeyEventKind::Press {
                    return;
                }

                if key.code == KeyCode::Delete && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.context.quit();
                    return;
                }

                if let Some(mut dialog) = self.context.dialog.take() {
                    let activated = dialog.handle_key(key, &mut self.context)
                        && matches!(key.code, KeyCode::Enter | KeyCode::Char(' '));

                    let dismissed = activated || key.code == KeyCode::Esc;

                    if !dismissed && self.context.dialog.is_none() {
                        self.context.dialog = Some(dialog);
                    }

                    return;
                }

                self.current_page.handle_input(&mut self.context, key);
            }
            AppEvent::Tick => {
                self.current_page.update(&mut self.context);
            }
            AppEvent::Resize(..) => {}
        }
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let size = frame.area();

        let style = Style::default().bg(self.context.theme.background);
        let background = Block::default().style(style);
        frame.render_widget(background, size);

        self.current_page.render(frame, &mut self.context);

        if let Some(dialog) = &mut self.context.dialog {
            let size = size.centered_pct(50, 50);
            dialog.render(size, frame.buffer_mut(), &self.context.theme);
        }
    }

    pub fn switch_to(&mut self, page: AppPage) {
        self.current_page.on_exit(&mut self.context);

        self.context.current_page_id = page;

        let new_page: Box<dyn Page + Send> = match page {
            AppPage::Welcome => Box::new(WelcomePage::new()),
            AppPage::DevicePage => Box::new(DevicePage::new()),
            AppPage::Options => Box::new(OptionsPage::new()),
        };

        self.current_page = new_page;
        self.current_page.on_enter(&mut self.context);
    }
}
