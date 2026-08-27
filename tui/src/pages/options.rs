/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AppCtx, AppPage};
use crate::components::{
    Component,
    Dropdown,
    DropdownOption,
    FormField,
    FormItem,
    FormPage,
    FormSection,
    Stars,
    TextInput,
    Toggle,
};
use crate::pages::Page;
use crate::themes::load_themes;

pub trait ConfigValue {
    fn to_form_string(&self) -> String;
    fn set_from_form_string(&mut self, value: &str);
}

impl ConfigValue for String {
    fn to_form_string(&self) -> String {
        self.clone()
    }

    fn set_from_form_string(&mut self, value: &str) {
        *self = value.to_string();
    }
}

impl ConfigValue for bool {
    fn to_form_string(&self) -> String {
        if *self { "true".to_string() } else { "false".to_string() }
    }

    fn set_from_form_string(&mut self, value: &str) {
        *self = value == "true" || value == "1";
    }
}

impl ConfigValue for Option<String> {
    fn to_form_string(&self) -> String {
        self.clone().unwrap_or_default()
    }

    fn set_from_form_string(&mut self, value: &str) {
        *self = (!value.is_empty()).then_some(value.to_string());
    }
}

enum OptionField {
    Dropdown(Dropdown),
    Toggle(Toggle),
    TextInput(TextInput),
}

impl Component for OptionField {
    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppCtx) -> bool {
        match self {
            Self::Dropdown(field) => field.handle_key(key, ctx),
            Self::Toggle(field) => field.handle_key(key, ctx),
            Self::TextInput(field) => field.handle_key(key, ctx),
        }
    }

    fn render(
        &mut self,
        area: ratatui::layout::Rect,
        buf: &mut Buffer,
        theme: &crate::themes::Theme,
    ) {
        match self {
            Self::Dropdown(field) => field.render(area, buf, theme),
            Self::Toggle(field) => field.render(area, buf, theme),
            Self::TextInput(field) => field.render(area, buf, theme),
        }
    }

    fn render_overlay(
        &mut self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &crate::themes::Theme,
    ) {
        if let Self::Dropdown(field) = self {
            field.render_overlay(area, buf, theme);
        }
    }
}

impl FormField for OptionField {
    fn value(&self) -> String {
        match self {
            Self::Dropdown(field) => field.value().clone(),
            Self::Toggle(field) => field.value(),
            Self::TextInput(field) => field.value(),
        }
    }

    fn set_value(&mut self, value: &str) {
        match self {
            Self::Dropdown(field) => field.set_value(value),
            Self::Toggle(field) => field.set_value(value),
            Self::TextInput(field) => field.set_value(value),
        }
    }

    fn is_focused(&self) -> bool {
        match self {
            Self::Dropdown(field) => field.is_focused(),
            Self::Toggle(field) => field.is_focused(),
            Self::TextInput(field) => field.is_focused(),
        }
    }
}

pub struct OptionsPage {
    form: FormPage<OptionField>,
    stars: Stars,
}

impl OptionsPage {
    pub fn new() -> Self {
        let form = build_form();
        Self { form, stars: Stars::dense(false) }
    }

    fn build_theme_options() -> Vec<DropdownOption> {
        let mut theme_options: Vec<DropdownOption> = load_themes()
            .into_iter()
            .map(|(id, constructor)| {
                let theme_data = constructor();
                let variant = if theme_data.is_dark { "dark" } else { "light" };
                DropdownOption {
                    label: format!("{} ({})", theme_data.name, variant),
                    value: id.to_string(),
                }
            })
            .collect();
        theme_options.sort_by(|a, b| a.label.cmp(&b.label));
        theme_options
    }

    fn apply_theme(ctx: &mut AppCtx, theme_id: &str) {
        let themes = load_themes();
        if let Some(constructor) = themes.get(theme_id) {
            ctx.theme = constructor();
        }
    }

    fn apply_compatibility_mode(ctx: &mut AppCtx, value: &str) {
        let is_compact = value == "true" || value == "1";
        let mut config = ctx.config.as_ref().clone();
        config.tui.compatibility_mode = is_compact;
        ctx.replace_config(config);
    }

    fn apply_show_stars(ctx: &mut AppCtx, value: &str) {
        let show = value == "true" || value == "1";
        let mut config = ctx.config.as_ref().clone();
        config.tui.show_stars = show;
        ctx.replace_config(config);
    }
}

declare_options! {
    sections: [
        ("INTERFACE", [
            (
                tui.theme,
                "Antumbra Theme",
                "Visual style for Antumbra",
                dropdown,
                options: OptionsPage::build_theme_options(),
                on_change: OptionsPage::apply_theme,
            ),
            (
                tui.compatibility_mode,
                "Compatibility Mode",
                "Use standard ASCII characters for stars (*, +, .)",
                toggle,
                on_change: OptionsPage::apply_compatibility_mode,
            ),
            (
                tui.show_stars,
                "Show stars",
                "Display animated stars in the background",
                toggle,
                on_change: OptionsPage::apply_show_stars,
            ),

        ]),
        ("AUTH", [
            (auth.online_auth, "Online Auth", "Enable remote auth for SLA", toggle),
            (auth.endpoint, "Endpoint", "Auth endpoint URL", text),
            (auth.username, "Username", "Account username", text),
            (auth.password, "Password", "Account password", masked),
        ]),
    ]
}

impl Page for OptionsPage {
    fn render(&mut self, f: &mut Frame, ctx: &mut AppCtx) {
        let area = f.area();
        self.stars.tick(ctx);

        if ctx.config.tui.show_stars {
            self.stars.compact = ctx.config.tui.compatibility_mode;

            self.stars.render(area, f.buffer_mut(), &ctx.theme);
        }

        self.form.render_form(
            area,
            f.buffer_mut(),
            &ctx.theme,
            "SETTINGS",
            "[↑/↓] Navigate   •   [Tab] Next   •   [Esc] Back",
        );
    }

    fn handle_input(&mut self, ctx: &mut AppCtx, key: KeyEvent) {
        if self.form.handle_form_input(ctx, key) {
            return;
        }

        match key.code {
            KeyCode::Esc => ctx.change_page(AppPage::Welcome),
            KeyCode::Enter if self.form.selected_index() == self.form.total_items() => {
                ctx.change_page(AppPage::Welcome)
            }
            _ => {}
        }
    }

    fn on_enter(&mut self, ctx: &mut AppCtx) {
        self.sync_from_config(ctx);
    }

    fn on_exit(&mut self, ctx: &mut AppCtx) {
        self.save_to_disk(ctx);
    }

    fn update(&mut self, _: &mut AppCtx) {}
}
