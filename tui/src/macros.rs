/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

#[macro_export]
macro_rules! description_menu {
    ($( ($icon:expr, $label:expr, $desc:expr => $action:expr) ),* $(,)?) => {{
        let items = vec![
            $(
                $crate::components::DescriptionMenuItem::new($icon, $label, $desc, $action)
            ),*
        ];
        $crate::components::DescriptionMenu::new(items)
    }};
}

#[macro_export]
macro_rules! dropdown_options {
    ($( $label:expr => $val:expr ),* $(,)?) => {
        vec![
            $(
                $crate::components::DropdownOption {
                    label: $label.into(),
                    value: $val.into(),
                }
            ),*
        ]
    };
}

#[macro_export]
macro_rules! stars {
    () => {
        $crate::components::Stars::default()
    };
    (sparse) => {
        $crate::components::Stars::sparse(false)
    };
    (dense) => {
        $crate::components::Stars::dense(false)
    };
    ($density:expr) => {
        $crate::components::Stars::new($density, false)
    };
}

#[macro_export]
macro_rules! dropdown {
    ($label:expr, [ $( ($opt_label:expr, $opt_val:expr) ),* $(,)? ], $selected:expr) => {{
        let options = vec![
            $( $crate::components::DropdownOption::new($opt_label, $opt_val) ),*
        ];
        $crate::components::Dropdown::new($label, options, $selected)
    }};
}

#[macro_export]
macro_rules! selectable_list {
    ($title:expr, [ $( ($label:expr, $val:expr) ),* $(,)? ]) => {{
        let items = vec![
            $( $crate::components::ListItemEntry::new($label, Some($val.into()), None) ),*
        ];
        $crate::components::SelectableListBuilder::default()
            .block_title($title)
            .highlight_symbol("> ")
            .items(items)
            .build()
            .unwrap()
    }};
}

#[macro_export]
macro_rules! show_dialog {
    ($ctx:expr, $builder_fn:ident, $msg:expr) => {
        $ctx.dialog = Some(
            $crate::components::DialogBuilder::$builder_fn($msg, &$ctx.theme)
                .button($crate::components::DialogButton::new("OK", || {}))
                .build()
                .unwrap(),
        );
    };
    ($ctx:expr, $builder_fn:ident, $msg:expr, $($button:expr),+ $(,)?) => {
        $ctx.dialog = Some({
            let mut builder = $crate::components::DialogBuilder::$builder_fn($msg, &$ctx.theme);
            $( builder.button($button); )+
            builder.build().unwrap()
        });
    };
}

/// Information dialog
#[macro_export]
macro_rules! info_dialog {
    ($ctx:expr, $msg:expr $(, $button:expr)*) => {
        $crate::show_dialog!($ctx, info, $msg $(, $button)*);
    };
}

/// Error dialog
#[macro_export]
macro_rules! error_dialog {
    ($ctx:expr, $msg:expr $(, $button:expr)*) => {
        $crate::show_dialog!($ctx, error, $msg $(, $button)*);
    };
}

/// Confirmation dialog (Yes/No)
#[macro_export]
macro_rules! confirm_dialog {
    ($ctx:expr, $msg:expr, $on_confirm:expr) => {
        $crate::confirm_dialog!($ctx, $msg, $on_confirm, || {});
    };
    ($ctx:expr, $msg:expr, $on_confirm:expr, $on_cancel:expr) => {
        $ctx.dialog = Some(
            $crate::components::DialogBuilder::other($msg, &$ctx.theme)
                .button($crate::components::DialogButton::new("Yes", $on_confirm))
                .button($crate::components::DialogButton::new("No", $on_cancel))
                .build()
                .unwrap(),
        );
    };
}

#[macro_export]
macro_rules! dialog {
    ($type_fn:ident, $msg:expr, $theme:expr, [ $( $title:expr => $action:expr ),* $(,)? ]) => {{
        let colors = match stringify!($type_fn) {
            "error" => $crate::components::DialogColors::new($theme.error, $theme.background),
            "info" => $crate::components::DialogColors::new($theme.info, $theme.background),
            _ => $crate::components::DialogColors::new($theme.accent, $theme.background),
        };
        $crate::components::DialogBuilder::default()
            .dialog_type(match stringify!($type_fn) {
                "error" => $crate::components::DialogType::Error,
                "info" => $crate::components::DialogType::Info,
                _ => $crate::components::DialogType::Other,
            })
            .message($msg)
            .colors(colors)
            .buttons(vec![
                $( $crate::components::DialogButton::new($title, $action) ),*
            ])
            .build()
            .unwrap()
    }};
}

macro_rules! form_item {
    ($id:expr, $label:expr, $desc:expr, $field:expr) => {
        $crate::components::FormItem::new($id, $label, $desc, $field)
    };
}

#[macro_export]
macro_rules! form_section {
    ($title:expr, [ $( $item:expr ),* $(,)? ]) => {
       $crate::components::FormSection::new($title, vec![$($item),*])
    };
}

#[macro_export]
macro_rules! dropdown_field {
    ($label:expr, $options:expr, $sel:expr) => {
        OptionField::Dropdown($crate::components::Dropdown::new($label, $options, $sel))
    };
}

#[macro_export]
macro_rules! toggle_field {
    ($val:expr) => {
        OptionField::Toggle($crate::components::Toggle::new($val))
    };
}

#[macro_export]
macro_rules! text_field {
    () => {
        OptionField::TextInput($crate::components::TextInput::new())
    };
    (masked) => {{
        let mut _t = $crate::components::TextInput::new();
        _t.set_masked(true);
        OptionField::TextInput(_t)
    }};
}

macro_rules! declare_options {
    (
        sections: [ $(
            ($section_title:expr, [ $( ($sec:ident . $field:ident, $label:expr, $desc:expr, $kind:ident $(, $key:ident : $val:expr )* $(,)?) ),* $(,)? ])
        ),* $(,)? ]
    ) => {
        fn build_form() -> FormPage<OptionField> {
            let mut sections_vec: Vec<FormSection<OptionField>> = Vec::new();
            $(
                let mut items_vec: Vec<FormItem<OptionField>> = Vec::new();
                $(
                    let field = declare_options!(@make_field $kind, $label $(, $key : $val)*);
                    items_vec.push($crate::components::FormItem::new(
                        stringify!($sec.$field),
                        $label,
                        $desc,
                        field
                    ));
                )*
                sections_vec.push($crate::components::FormSection::new($section_title, items_vec));
            )*

            FormPage::new(sections_vec).with_on_change(|ctx, id, value| {
                let _ = (&mut *ctx, value);
                $(
                    $(
                        if id == stringify!($sec.$field) {
                            declare_options!(@on_change_expr ctx, id, value, $sec . $field $(, $key : $val)*);
                        }
                    )*
                )*
            })
        }


        impl OptionsPage {


            fn sync_from_config(&mut self, ctx: &mut AppCtx) {
                $(
                    $(
                        {
                            let val = declare_options!(@read_expr ctx, $sec . $field $(, $key : $val)*);
                            for section in self.form.sections_mut() {
                                for item in &mut section.items {
                                    if item.id == stringify!($sec.$field) {
                                        item.field.set_value(&val);
                                    }
                                }
                            }
                        }
                    )*
                )*

                let theme_id = ctx.config.tui.theme.clone();
                Self::apply_theme(ctx, &theme_id);
            }

            fn save_to_disk(&self, ctx: &mut AppCtx) {
                let mut config = ctx.config.as_ref().clone();

                $(
                    $(
                        {
                            for section in self.form.sections() {
                                for item in &section.items {
                                    if item.id == stringify!($sec.$field) {
                                        let value = item.field.value();
                                        declare_options!(@write_expr config, value, $sec . $field $(, $key : $val)*);
                                    }
                                }
                            }
                        }
                    )*
                )*

                let _ = config.save();
                ctx.replace_config(config);
            }
        }
    };

    (@make_field dropdown, $label:expr, options: $opts:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        OptionField::Dropdown($crate::components::Dropdown::new("", $opts, 0))
    };
    (@make_field dropdown, $label:expr, $other_key:ident : $other_val:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        declare_options!(@make_field dropdown, $label $(, $rest_key : $rest_val)*)
    };
    (@make_field dropdown, $label:expr) => {
        OptionField::Dropdown($crate::components::Dropdown::new("", Vec::new(), 0))
    };
    (@make_field toggle, $label:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        OptionField::Toggle($crate::components::Toggle::new(false))
    };
    (@make_field text, $label:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        OptionField::TextInput($crate::components::TextInput::new())
    };
    (@make_field masked, $label:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        {
            let mut _t = $crate::components::TextInput::new();
            _t.set_masked(true);
            OptionField::TextInput(_t)
        }
    };

    // Read value
    (@read_expr $ctx:ident, $sec:ident . $field:ident, read: $r:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        $r
    };
    (@read_expr $ctx:ident, $sec:ident . $field:ident, $other_key:ident : $other_val:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        declare_options!(@read_expr $ctx, $sec . $field $(, $rest_key : $rest_val)*)
    };
    (@read_expr $ctx:ident, $sec:ident . $field:ident) => {
        $ctx.config.$sec.$field.to_form_string()
    };

    // Write value
    (@write_expr $config:ident, $value:ident, $sec:ident . $field:ident, write: $w:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        $w
    };
    (@write_expr $config:ident, $value:ident, $sec:ident . $field:ident, $other_key:ident : $other_val:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        declare_options!(@write_expr $config, $value, $sec . $field $(, $rest_key : $rest_val)*)
    };
    (@write_expr $config:ident, $value:ident, $sec:ident . $field:ident) => {
        $config.$sec.$field.set_from_form_string(&$value)
    };

    // What shall we do after a value has changed? :D
    (@on_change_expr $ctx:ident, $id:ident, $value:ident, $sec:ident . $field:ident, on_change: $oc:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        ($oc)($ctx, $value);
    };
    (@on_change_expr $ctx:ident, $id:ident, $value:ident, $sec:ident . $field:ident, $other_key:ident : $other_val:expr $(, $rest_key:ident : $rest_val:expr)*) => {
        declare_options!(@on_change_expr $ctx, $id, $value, $sec . $field $(, $rest_key : $rest_val)*)
    };
    (@on_change_expr $ctx:ident, $id:ident, $value:ident, $sec:ident . $field:ident) => {};
}
