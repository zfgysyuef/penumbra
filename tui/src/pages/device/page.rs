/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use human_bytes::human_bytes;
use penumbra::activity::Activity;
use penumbra::port::ConnectionType;
use penumbra::{DevInfoData, Partition};
use ratatui::Frame;
#[cfg(target_os = "windows")]
use ratatui::crossterm::event::KeyEventKind;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Row, Table, Widget};
use unicode_width::UnicodeWidthStr;

use super::actions::actions;
use super::worker::{ConnectParams, DeviceCommand, DeviceEvent, DeviceStatus};
use crate::app::{AppCtx, AppPage};
use crate::components::footer::Footer;
use crate::components::selectable_list::{
    ListItemEntry,
    ListItemEntryBuilder,
    SelectableList,
    SelectableListBuilder,
};
use crate::components::{
    ActivityExt,
    ActivityIndicator,
    Badge,
    Component,
    ExplorerResult,
    FileExplorer,
    ProgressBar,
    RectExt,
    Stars,
};
use crate::helpers::SoCExt;
use crate::pages::Page;
use crate::pages::device::worker;

/// Which panel currently receives key input.
enum FocusedPanel {
    Menu,
    PartitionMenu,
}

pub struct DevicePage {
    cmd_tx: Option<Sender<DeviceCommand>>,
    event_rx: Option<Receiver<DeviceEvent>>,

    status: DeviceStatus,
    activity: Activity,
    header_status: Option<String>,
    devinfo: Option<DevInfoData>,

    stars: Stars,
    progress_bar: ProgressBar,
    menu: SelectableList,
    partition_list: SelectableList,
    explorer: Option<FileExplorer>,
    current_time: String,

    focused: FocusedPanel,

    /// Cleared when a respawn cannot possibly succeed, so `update` stops retrying.
    reconnect: bool,
    busy: bool,
    explorer_dirs_only: bool,
}

impl DevicePage {
    const HIGHLIGHT_SYMBOL: &'static str = ">> ";
    const TIME_FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";

    fn now() -> String {
        chrono::Local::now().format(Self::TIME_FORMAT).to_string()
    }

    pub fn new() -> Self {
        let action_labels = actions();
        let menu_items = action_labels
            .iter()
            .map(|a| ListItemEntryBuilder::default().label(a.label().into()).build().unwrap())
            .chain(std::iter::once(
                ListItemEntryBuilder::default().label("Back to Menu".into()).build().unwrap(),
            ))
            .collect();

        let menu = SelectableListBuilder::default()
            .items(menu_items)
            .highlight_symbol(Self::HIGHLIGHT_SYMBOL)
            .build()
            .unwrap();

        let partition_list = SelectableListBuilder::default()
            .items(Vec::new())
            .highlight_symbol(Self::HIGHLIGHT_SYMBOL)
            .highlight_on_onfocus(false)
            .build()
            .unwrap();

        Self {
            cmd_tx: None,
            event_rx: None,
            status: DeviceStatus::Disconnected,
            activity: Activity::Idle,
            header_status: None,
            devinfo: None,
            stars: Stars::default(),
            progress_bar: ProgressBar::new(),
            current_time: Self::now(),
            menu,
            partition_list,
            explorer: None,
            focused: FocusedPanel::Menu,
            reconnect: true,
            busy: false,
            explorer_dirs_only: false,
        }
    }

    fn connect(&mut self, ctx: &AppCtx) {
        if self.cmd_tx.is_some() {
            return;
        }

        let read = |p: &Option<PathBuf>| p.as_ref().and_then(|p| std::fs::read(p).ok());

        // TODO: Allow to specify VID/PID from config or CLI args
        let params = ConnectParams {
            vid: None,
            pid: None,
            da_data: read(&ctx.da_path),
            preloader_data: read(&ctx.preloader_path),
            auth_data: read(&ctx.auth_file_path),
        };

        let (cmd_tx, event_rx) = worker::spawn(params);
        self.cmd_tx = Some(cmd_tx);
        self.event_rx = Some(event_rx);
    }

    fn partition_items(partitions: &[Partition]) -> Vec<ListItemEntry> {
        partitions
            .iter()
            .map(|p| {
                ListItemEntry::new(
                    format!("{} ({})", p.name, human_bytes(p.size as f64)),
                    Some(p.name.clone()),
                    None,
                )
            })
            .collect()
    }

    fn send(&self, cmd: DeviceCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    fn reset(&mut self) {
        self.cmd_tx = None;
        self.event_rx = None;
        self.status = DeviceStatus::Disconnected;
        self.activity = Activity::Idle;
        self.progress_bar.reset();
        self.explorer = None;
        self.focused = FocusedPanel::Menu;
        self.busy = false;
        self.header_status = None;
    }

    fn process_events(&mut self, ctx: &mut AppCtx) {
        let Some(rx) = &self.event_rx else { return };

        while let Ok(event) = rx.try_recv() {
            match event {
                DeviceEvent::StatusChanged(status) => {
                    if matches!(status, DeviceStatus::Disconnected) {
                        self.progress_bar.reset();
                        self.busy = false;
                    }
                    self.status = status;
                }

                DeviceEvent::Connected { devinfo, partitions } => {
                    self.devinfo = Some(devinfo);
                    self.partition_list.items = Self::partition_items(&partitions);
                }

                DeviceEvent::PartitionsChanged(partitions) => {
                    self.partition_list.items = Self::partition_items(&partitions);
                    let steps = self.partition_list.selected_index().unwrap_or(0).abs_diff(0);
                    self.partition_list.previous_by(steps);
                }

                DeviceEvent::NeedPartitions => {
                    self.busy = false;
                    self.focused = FocusedPanel::PartitionMenu;
                    self.partition_list.toggled = true;
                    self.partition_list.clear_toggles();
                }

                DeviceEvent::NeedFile { title, directories_only, extensions } => {
                    self.explorer_dirs_only = directories_only;
                    let explorer = FileExplorer::new(title).map(|explorer| {
                        if directories_only { explorer.directories_only() } else { explorer }
                    });

                    let explorer = explorer.map(|explorer| {
                        if let Some(extensions) = extensions {
                            explorer.extensions(&extensions)
                        } else {
                            explorer
                        }
                    });

                    match explorer {
                        Ok(explorer) => self.explorer = Some(explorer),
                        Err(e) => {
                            error_dialog!(ctx, format!("Failed to open file browser: {e}"));
                            self.send(DeviceCommand::Cancel);
                        }
                    }
                }

                DeviceEvent::ProgressStart { total_bytes, message } => {
                    self.busy = true;
                    self.progress_bar.start(total_bytes, message);
                }
                DeviceEvent::ProgressUpdate { written, total, message } => {
                    // Some operations are dynamically sized (scatter) because
                    // of how protocol works in this regards.
                    if let Some(total) = total {
                        self.progress_bar.set_total(total);
                    }

                    self.progress_bar.set_written(written);
                    if let Some(msg) = message {
                        self.progress_bar.set_message(msg);
                    }
                }
                DeviceEvent::ProgressFinish { message } => {
                    self.progress_bar.finish(message);
                }

                DeviceEvent::HeaderStatus(msg) => self.header_status = Some(msg),

                DeviceEvent::ActivityChanged(activity) => {
                    if let Some(detail) = activity.detail() {
                        self.progress_bar.set_message(detail);
                    }
                    self.activity = activity;
                }

                DeviceEvent::ActionFinished => {
                    if self.progress_bar.is_active() {
                        self.progress_bar.reset();
                    }

                    self.activity = Activity::Idle;
                    self.busy = false;
                    self.focused = FocusedPanel::Menu;
                    self.partition_list.toggled = false;
                }

                DeviceEvent::Fatal(msg) => {
                    // Hi, who did this to you :(
                    self.reconnect = false;
                    self.progress_bar.reset();
                    self.activity = Activity::Idle;
                    self.busy = false;

                    error_dialog!(ctx, msg);
                    ctx.change_page(AppPage::Welcome);
                }

                DeviceEvent::Error(msg) => {
                    self.progress_bar.reset();
                    self.activity = Activity::Idle;
                    self.busy = false;
                    error_dialog!(ctx, msg);
                }
            }
        }

        if matches!(rx.try_recv(), Err(TryRecvError::Disconnected)) {
            self.reset();
        }
    }

    fn handle_menu_input(&mut self, ctx: &mut AppCtx, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::PageUp => self.menu.previous(),
            KeyCode::Down | KeyCode::PageDown => self.menu.next(),
            KeyCode::Enter => {
                let Some(idx) = self.menu.selected_index() else { return };
                let total_actions = actions().len();

                if idx == total_actions {
                    ctx.change_page(AppPage::Welcome);
                    return;
                }

                if !matches!(self.status, DeviceStatus::Connected(_)) {
                    error_dialog!(ctx, "Device not connected");
                    return;
                }

                self.busy = true;
                self.send(DeviceCommand::RunAction(idx));
            }
            _ => {}
        }
    }

    fn handle_partition_input(&mut self, ctx: &mut AppCtx, key: KeyEvent) {
        if self.partition_list.handle_key(key, ctx) {
            return;
        }

        match key.code {
            KeyCode::PageUp => self.partition_list.previous_by(5),
            KeyCode::PageDown => self.partition_list.next_by(5),
            KeyCode::Char('x') => self.partition_list.toggle_selected(),
            KeyCode::Esc => {
                self.partition_list.toggled = false;
                self.partition_list.clear_toggles();
                self.focused = FocusedPanel::Menu;
                self.send(DeviceCommand::Cancel);
            }
            KeyCode::Enter => {
                // The worker holds the real partitions data,
                // we only need to tell it which names were selected.
                let names: Vec<String> = self
                    .partition_list
                    .checked_items()
                    .into_iter()
                    .filter_map(|item| item.value.clone())
                    .collect();

                if names.is_empty() {
                    return;
                }

                self.send(DeviceCommand::PartitionsChosen(names));
            }
            _ => {}
        }
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect, ctx: &AppCtx) {
        let (status_str, status_style) = match self.status {
            DeviceStatus::Disconnected => (" Disconnected ", Style::default().fg(ctx.theme.muted)),
            DeviceStatus::Connecting => (" Connecting ", Style::default().fg(ctx.theme.warning)),
            DeviceStatus::Connected(conn_type) => {
                let status_str = match conn_type {
                    ConnectionType::Brom => " Connected (BROM) ",
                    ConnectionType::Preloader => " Connected (Preloader) ",
                    ConnectionType::Da => " Connected (DA) ",
                };
                (status_str, Style::default().fg(ctx.theme.success))
            }
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(ctx.theme.accent));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let left_len = (" Antumbra  | ".len() + status_str.len()) as u16;

        let [left_area, msg_area, _] = Layout::horizontal([
            Constraint::Length(left_len),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .areas(inner_area);

        let left_header = Paragraph::new(Line::from(vec![
            Span::styled(" Antumbra ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(status_str, status_style),
        ]));

        frame.render_widget(left_header, left_area);

        if let Some(msg) = &self.header_status {
            let available_width = msg_area.width as usize;
            if available_width == 0 {
                return;
            }

            let style = Style::default().fg(ctx.theme.info);

            let msg_header = match msg.char_indices().nth(available_width) {
                Some(_) => {
                    if available_width > 1 {
                        let cutoff = msg
                            .char_indices()
                            .nth(available_width - 1)
                            .map_or(msg.len(), |(idx, _)| idx);

                        Paragraph::new(Line::from(vec![
                            Span::styled(&msg[..cutoff], style),
                            Span::styled("…", style),
                        ]))
                    } else {
                        let cutoff = msg
                            .char_indices()
                            .nth(available_width)
                            .map_or(msg.len(), |(idx, _)| idx);

                        Paragraph::new(Line::from(Span::styled(&msg[..cutoff], style)))
                    }
                }
                None => Paragraph::new(Line::from(Span::styled(msg.as_str(), style))),
            };

            frame.render_widget(msg_header.alignment(Alignment::Right), msg_area);
        }
    }

    fn render_content(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppCtx) {
        let [menu_area, _, info_area] = Layout::horizontal([
            Constraint::Percentage(30),
            Constraint::Length(2),
            Constraint::Percentage(70),
        ])
        .areas(area);

        match self.focused {
            FocusedPanel::Menu => {
                self.menu.set_focus(true);
                self.partition_list.set_focus(false);
            }
            FocusedPanel::PartitionMenu => {
                self.menu.set_focus(false);
                self.partition_list.set_focus(true);
            }
        }

        let menu_block = Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(ctx.theme.style_border(self.menu.is_focused()));
        frame.render_widget(menu_block.clone(), menu_area);
        self.menu.render(menu_block.inner(menu_area), frame.buffer_mut(), &ctx.theme);

        let info_block = Block::default()
            .title(" Device Info ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(ctx.theme.style_border(self.partition_list.is_focused()));
        frame.render_widget(info_block.clone(), info_area);
        let inner = info_block.inner(info_area);

        if !matches!(self.status, DeviceStatus::Connected(_)) {
            let message = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    " Waiting for device connection…",
                    Style::default().fg(ctx.theme.warning).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    " (Plug device in BOOTROM or Preloader mode)",
                    Style::default().fg(ctx.theme.muted),
                )),
            ])
            .alignment(Alignment::Center);
            frame.render_widget(message, inner);
            return;
        }

        let [table_area, _, list_area] =
            Layout::vertical([Constraint::Length(8), Constraint::Length(1), Constraint::Min(0)])
                .areas(inner);

        if let Some(devinfo) = &self.devinfo {
            let hw_code = format!("0x{:X}", devinfo.hw_code);
            let sbc = if devinfo.target_config & 0x1 != 0 { "Yes" } else { "No" };
            let sla = if devinfo.target_config & 0x2 != 0 { "Yes" } else { "No" };
            let daa = if devinfo.target_config & 0x4 != 0 { "Yes" } else { "No" };

            let chip_name =
                devinfo.chip.map_or_else(|| "Unknown".into(), |c| c.marketing_seg_name());
            let socid = hex::encode(devinfo.soc_id);
            let meid = hex::encode(devinfo.meid);
            let rows = vec![
                Row::new(vec!["HW Code", hw_code.as_str()]),
                Row::new(vec!["Chip Name", chip_name.as_str()]),
                Row::new(vec!["SOC ID", socid.as_str()]),
                Row::new(vec!["MEID", meid.as_str()]),
                Row::new(vec!["Secure Boot (SBC)", sbc]),
                Row::new(vec!["Serial Link Auth (SLA)", sla]),
                Row::new(vec!["Download Agent Auth (DAA)", daa]),
            ];
            let table = Table::new(rows, [Constraint::Percentage(45), Constraint::Percentage(55)])
                .block(
                    Block::default()
                        .borders(Borders::BOTTOM)
                        .border_style(ctx.theme.style_border(self.partition_list.is_focused()))
                        .padding(Padding::horizontal(Self::HIGHLIGHT_SYMBOL.len() as u16)),
                )
                .column_spacing(1)
                .style(Style::default().fg(ctx.theme.text));

            frame.render_widget(table, table_area);
        }

        self.partition_list.render(list_area, frame.buffer_mut(), &ctx.theme);
    }

    fn render_progress(&mut self, frame: &mut Frame<'_>, area: Rect, ctx: &AppCtx) {
        let block = Block::default().padding(Padding::top(1));
        frame.render_widget(block.clone(), area);
        self.progress_bar.render(block.inner(area), frame.buffer_mut(), &ctx.theme);
    }

    const fn hints(&self) -> &'static str {
        if self.explorer.is_some() {
            "[↑/↓] Navigate • [Enter] Select • [Esc] Cancel"
        } else if matches!(self.focused, FocusedPanel::PartitionMenu) {
            "[↑/↓] Navigate • [Space] Toggle • [Enter] Confirm • [Esc] Cancel"
        } else if self.busy {
            "Operation in progress..."
        } else {
            "[↑/↓] Navigate • [Enter] Select"
        }
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect, ctx: &AppCtx) {
        let area = Block::default().padding(Padding::new(1, 1, 1, 0)).inner(area);

        let badge = match self.status {
            DeviceStatus::Disconnected => Badge::Disconnected,
            DeviceStatus::Connecting => Badge::Connecting,
            DeviceStatus::Connected(_) => {
                Badge::from_activity(&self.activity, self.progress_bar.is_active())
            }
        };

        let indicator = ActivityIndicator::new(badge);

        // The indicator can change size based on the name, so we need to keep this in mind.
        let side = indicator.width().max(UnicodeWidthStr::width(self.current_time.as_str()) as u16);

        let [badge_area, hints_area, datetime_area] = Layout::horizontal([
            Constraint::Length(side),
            Constraint::Min(0),
            Constraint::Length(side),
        ])
        .areas(area);

        indicator.render(badge_area, frame.buffer_mut(), &ctx.theme);

        let footer = Footer::new(self.hints());
        let datetime_footer = Footer::new(&self.current_time).aligned(Alignment::Right);
        footer.render(hints_area, frame.buffer_mut(), &ctx.theme);
        datetime_footer.render(datetime_area, frame.buffer_mut(), &ctx.theme);
    }
}

impl Page for DevicePage {
    fn render(&mut self, frame: &mut Frame<'_>, ctx: &mut AppCtx) {
        let area = frame.area();
        let buf = frame.buffer_mut();

        if ctx.config.tui.show_stars {
            self.stars.compact = ctx.config.tui.compatibility_mode;

            self.stars.render(area, buf, &ctx.theme);
        }

        let [header, content, progress, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .margin(1)
        .areas(area);

        self.render_header(frame, header, ctx);
        self.render_content(frame, content, ctx);
        self.render_progress(frame, progress, ctx);
        self.render_footer(frame, footer, ctx);

        if let Some(explorer) = &mut self.explorer {
            let popup_area = area.centered_pct(80, 80);
            let buf = frame.buffer_mut();
            Clear.render(popup_area, buf);
            explorer.render(popup_area, buf, &ctx.theme);
        }
    }

    fn handle_input(&mut self, ctx: &mut AppCtx, key: KeyEvent) {
        #[cfg(target_os = "windows")]
        if key.kind != KeyEventKind::Press {
            return;
        }

        if let Some(explorer) = &mut self.explorer {
            match explorer.handle_key_event(key) {
                ExplorerResult::Selected(path) => {
                    self.explorer = None;
                    self.send(DeviceCommand::FileChosen(path));
                }
                ExplorerResult::Cancelled => {
                    self.explorer = None;
                    self.send(DeviceCommand::Cancel);
                }
                ExplorerResult::Pending => {}
            }
            return;
        }

        if self.busy {
            return;
        }

        match self.focused {
            FocusedPanel::Menu => self.handle_menu_input(ctx, key),
            FocusedPanel::PartitionMenu => self.handle_partition_input(ctx, key),
        }
    }

    fn on_enter(&mut self, ctx: &mut AppCtx) {
        self.reconnect = true;
        self.status = DeviceStatus::Disconnected;
        self.connect(ctx);
    }

    fn on_exit(&mut self, _ctx: &mut AppCtx) {
        self.send(DeviceCommand::Shutdown);
        self.cmd_tx = None;
        self.event_rx = None;
    }

    fn update(&mut self, ctx: &mut AppCtx) {
        self.process_events(ctx);

        if self.reconnect {
            self.connect(ctx);
        }

        self.stars.tick(ctx);
        self.progress_bar.tick(ctx);
        self.current_time = Self::now();
    }
}
