/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

#[macro_export]
macro_rules! cli_commands {
    (
        device {
            $( $dev_variant:ident ($dev_ty:ty) ),* $(,)?
        }
        cli {
            $( $loc_variant:ident ($loc_ty:ty) ),* $(,)?
        }
    ) => {
        #[derive(clap::Subcommand, Debug)]
        pub enum Commands {
            $(
                #[command(
                    subcommand_help_heading = "Device Commands",
                    aliases = <$dev_ty as $crate::cli::common::CommandMetadata>::aliases(),
                    visible_aliases = <$dev_ty as $crate::cli::common::CommandMetadata>::visible_aliases(),
                    about = <$dev_ty as $crate::cli::common::CommandMetadata>::about(),
                    long_about = <$dev_ty as $crate::cli::common::CommandMetadata>::long_about(),
                    hide = <$dev_ty as $crate::cli::common::CommandMetadata>::hide(),
                )]
                $dev_variant($dev_ty),
            )*
            $(
                #[command(
                    subcommand_help_heading = "CLI Commands",
                    aliases = <$loc_ty as $crate::cli::common::CommandMetadata>::aliases(),
                    visible_aliases = <$loc_ty as $crate::cli::common::CommandMetadata>::visible_aliases(),
                    about = <$loc_ty as $crate::cli::common::CommandMetadata>::about(),
                    long_about = <$loc_ty as $crate::cli::common::CommandMetadata>::long_about(),
                    hide = <$loc_ty as $crate::cli::common::CommandMetadata>::hide(),
                )]
                $loc_variant($loc_ty),
            )*
        }

        impl Commands {
            pub fn execute(
                &self,
                args: &$crate::cli::CliArgs,
                state: &mut $crate::cli::state::PersistedDeviceState,
            ) -> anyhow::Result<()> {
                match self {
                    $(
                        Commands::$dev_variant(inner) => {
                            let mut da_buf = None;
                            if let Some(da_path) = &args.da_file {
                                da_buf = Some(std::fs::read(da_path)?);
                            } else if let Some(da_path_str) = &state.da_file_path {
                                da_buf = Some(std::fs::read(std::path::Path::new(da_path_str))?);
                            }

                            let pl_buf = if let Some(pl_path) = &args.preloader_file { Some(std::fs::read(pl_path)?) } else { None };

                            let auth_buf = if let Some(auth_path) = &args.auth_file { Some(std::fs::read(auth_path)?) } else { None };

                            let mut dev = $crate::cli::helpers::setup_device(
                                args,
                                state,
                                da_buf.as_deref(),
                                pl_buf.as_deref(),
                                auth_buf.as_deref()
                            )?;

                            $crate::cli::DeviceCommand::run(inner, &mut dev, state)?;
                            state.target_config = dev.devinfo().target_config();
                            state.usb_log = args.usb_log;
                            if let Some(da_path) = &args.da_file {
                                state.da_file_path = Some(da_path.to_string_lossy().to_string());
                            }

                            Ok(())
                        }

                    )*
                    $(
                        Commands::$loc_variant(inner) => {
                            $crate::cli::CliCommand::run(inner, state)?;
                            Ok(())
                        }
                    )*
                }
            }
        }
    };
}

pub(crate) use cli_commands;
