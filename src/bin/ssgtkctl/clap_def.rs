//! This module contains code that define the CLI API.

use std::{env, path::PathBuf};

use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use shadowsocks_gtk_rs::{notify_method::NotifyMethod, runtime_api_msg::APICommand};

lazy_static! {
    static ref RUNTIME_API_SOCKET_PATH_DEFAULT: PathBuf =
        PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".into())).join("shadowsocks-gtk-rs.sock");
    static ref RUNTIME_API_SOCKET_PATH_DEFAULT_STR: String = RUNTIME_API_SOCKET_PATH_DEFAULT
        .to_str()
        .expect("default runtime-api-socket-path not UTF-8")
        .into();
}

#[derive(Debug, Clone, Parser)]
#[clap(
    name = "ssgtkctl",
    author,
    version,
    about = "A delegate binary for ssgtk that sends commands to the runtime API for your convenience.",
    disable_help_subcommand = true,
    infer_subcommands = true,
    subcommand_required = true
)]
pub struct CliArgs {
    /// Send command to the runtime API listener at a custom socket path.
    ///
    /// Useful if you want to control multiple instances.
    #[clap(short = 'a', long = "api-socket", value_name = "PATH", default_value = &RUNTIME_API_SOCKET_PATH_DEFAULT_STR)]
    pub runtime_api_socket_path: PathBuf,

    #[clap(subcommand)]
    pub sub_cmd: SubCmd,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SubCmd {
    /// Show the backlog window or bring it to foreground.
    #[clap(name = "backlog-show")]
    BacklogShow,

    /// Hide the backlog window if opened.
    #[clap(name = "backlog-hide")]
    BacklogHide,

    /// Use a particular method for all future notifications.
    #[clap(name = "set-notify")]
    SetNotify {
        /// The notification method to use.
        #[clap(index = 1, value_name = "METHOD", value_enum)]
        notify_method: NotifyMethod,
    },

    /// Restart the currently running sslocal instance.
    #[clap(name = "restart")]
    Restart,

    /// Switch to a new profile by starting a new sslocal instance.
    #[clap(name = "switch-profile")]
    SwitchProfile {
        /// The display name of the profile to switch to (CASE SENSITIVE)
        #[clap(index = 1, value_name = "NAME")]
        profile_name: String,
    },

    /// Stop the currently running sslocal instance.
    #[clap(name = "stop")]
    Stop,

    /// Quit the application.
    #[clap(name = "quit")]
    Quit,
}

impl From<SubCmd> for APICommand {
    fn from(cmd: SubCmd) -> Self {
        match cmd {
            SubCmd::BacklogShow => APICommand::BacklogShow,
            SubCmd::BacklogHide => APICommand::BacklogHide,
            SubCmd::SetNotify { notify_method } => APICommand::SetNotify(notify_method),
            SubCmd::Restart => APICommand::Restart,
            SubCmd::SwitchProfile { profile_name } => APICommand::SwitchProfile(profile_name),
            SubCmd::Stop => APICommand::Stop,
            SubCmd::Quit => APICommand::Quit,
        }
    }
}
