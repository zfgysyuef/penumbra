/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::prelude::Buffer;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, WidgetRef};
use ratatui_explorer::{FileExplorer as Inner, Theme as ExplorerTheme};

use crate::components::Component;
use crate::themes::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerResult {
    Selected(PathBuf),
    Cancelled,
    Pending,
}

pub struct FileExplorer {
    inner: Inner,
    title: String,
    extensions: Option<Vec<String>>,
    directories_only: bool,
    search_buffer: String,
    last_input_time: Instant,
    new_folder_input: Option<String>,
    last_error: Option<String>,
}

impl FileExplorer {
    const ELAPSED_MAX: Duration = Duration::from_millis(500);

    pub fn new(title: impl Into<String>) -> Result<Self> {
        let theme = ExplorerTheme::default();

        let inner = Inner::with_theme(theme)?;
        Ok(Self {
            inner,
            title: title.into(),
            extensions: None,
            directories_only: false,
            search_buffer: String::new(),
            last_input_time: Instant::now(),
            new_folder_input: None,
            last_error: None,
        })
    }

    /// A list of allowed file extensions
    pub fn extensions(mut self, exts: &[&str]) -> Self {
        self.extensions = Some(exts.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Whether to allow only directory in the view/selection
    pub const fn directories_only(mut self) -> Self {
        self.directories_only = true;
        self
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> ExplorerResult {
        if let Some(buffer) = self.new_folder_input.as_mut() {
            match key.code {
                KeyCode::Esc => {
                    self.new_folder_input = None;
                }
                KeyCode::Enter => {
                    let name = buffer.clone();
                    self.new_folder_input = None;
                    self.create_folder(&name);
                }
                KeyCode::Backspace => {
                    buffer.pop();
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                }
                _ => {}
            }
            return ExplorerResult::Pending;
        }

        if key.code == KeyCode::Esc {
            return ExplorerResult::Cancelled;
        }

        if self.directories_only && key.code == KeyCode::Char(' ') {
            return ExplorerResult::Selected(self.inner.cwd().to_path_buf());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
            self.new_folder_input = Some(String::new());
            self.last_error = None;
            return ExplorerResult::Pending;
        }

        let is_enter = key.code == KeyCode::Enter;

        if matches!(self.inner.handle(&Event::Key(key)), Ok(())) {
            let current_item = self.inner.current();

            if is_enter && !current_item.is_dir() {
                let path = current_item.path();
                if self.is_valid_extension(path) {
                    return ExplorerResult::Selected(path.to_path_buf());
                }
            }
        }

        if self.last_input_time.elapsed() > Self::ELAPSED_MAX {
            self.search_buffer.clear();
        }

        if let KeyCode::Char(c) = key.code {
            self.search_buffer.push(c);
            self.last_input_time = Instant::now();
            self.jump_to_matching();
        }

        ExplorerResult::Pending
    }

    fn jump_to_matching(&mut self) {
        if self.search_buffer.is_empty() {
            return;
        }

        let search = self.search_buffer.to_lowercase();
        let files = self.inner.files();

        if let Some(index) = files.iter().position(|f| {
            f.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_lowercase().starts_with(&search))
                .unwrap_or(false)
        }) {
            let current = self.inner.selected_idx();
            if index > current {
                for _ in 0..(index - current) {
                    self.inner.handle(&Event::Key(KeyCode::Down.into())).ok();
                }
            } else if index < current {
                for _ in 0..(current - index) {
                    self.inner.handle(&Event::Key(KeyCode::Up.into())).ok();
                }
            }
        }
    }

    fn is_valid_extension(&self, path: &Path) -> bool {
        let Some(ref exts) = self.extensions else {
            return true;
        };
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };

        exts.iter().any(|e| e.eq_ignore_ascii_case(ext))
    }

    fn create_folder(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }

        if name == "." || name == ".." || name.chars().any(std::path::is_separator) {
            self.last_error = Some(format!("Invalid folder name: {name}"));
            return;
        }

        let new_dir = self.inner.cwd().join(name);

        match fs::create_dir(&new_dir) {
            Ok(()) => {
                self.last_error = None;
                let cwd = self.inner.cwd().clone();
                if let Err(e) = self.inner.set_cwd(cwd) {
                    self.last_error = Some(format!("Created, but couldn't refresh listing: {e}"));
                }
            }
            Err(e) => {
                self.last_error = Some(format!("Couldn't create '{name}': {e}"));
            }
        }
    }
}

impl Component for FileExplorer {
    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent))
            .style(Style::default().bg(theme.background))
            .title(format!(" {} ", self.title));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let [hdr, explorer, help_block] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
            .areas(inner_area);

        let header_text = if let Some(input) = &self.new_folder_input {
            format!(" 📁 New folder: {input}█ ")
        } else if let Some(err) = &self.last_error {
            format!(" ⚠ {err} ")
        } else {
            let path_str = self.inner.cwd().display().to_string();
            format!(" 📂 {path_str} ")
        };
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD));

        header.render(hdr, buf);
        self.inner.set_theme(
            ExplorerTheme::default()
                .with_dir_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
                .with_highlight_item_style(
                    Style::default()
                        .bg(theme.accent)
                        .fg(theme.background)
                        .add_modifier(Modifier::BOLD),
                )
                .with_item_style(Style::default().fg(theme.text))
                .with_highlight_dir_style(
                    Style::default()
                        .bg(theme.accent)
                        .fg(theme.background)
                        .add_modifier(Modifier::BOLD),
                )
                .with_style(Style::default().fg(theme.text)),
        );

        self.inner.widget().render_ref(explorer, buf);

        let help_text = if self.new_folder_input.is_some() {
            " [Enter] Create • [Esc] Cancel "
        } else if self.directories_only {
            " [↑/↓] Nav • [Space] Select Dir • [Ctrl+N] New Folder • [Esc] Cancel "
        } else {
            " [↑/↓] Nav • [Enter] Select • [Ctrl+N] New Folder • [Esc] Cancel "
        };

        let help = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted));

        help.render(help_block, buf);
    }
}
