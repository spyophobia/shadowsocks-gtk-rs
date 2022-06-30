//! This module contains code that define the CLI API.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use shadowsocks_gtk_rs::{consts::*, notify_method::NotifyMethod, runtime_api_msg::APICommand};

#[derive(Debug, Clone, Parser)]
#[clap(
    name = "ssgtkctl",
    author,
    version,
    about = "A delegate binary for ssgtk that sends commands to the runtime API for your convenience.",
    disable_help_subcommand = true,
    infer_subcommands = true
)]
pub struct CliArgs {
    /// Send command to the runtime API listener at a custom socket path.
    ///
    /// Useful if you want to control multiple instances.
    #[clap(short = 'a', long = "api-socket", value_name = "PATH", default_value = &RUNTIME_API_SOCKET_PATH_DEFAULT_STR)]
    pub runtime_api_socket_path: PathBuf,

    /// Print examples of how to interface with the Unix socket directly.
    #[clap(long = "print-socket-examples")]
    pub print_socket_examples: bool,

    #[clap(subcommand)]
    pub sub_cmd: Option<SubCmd>,
}

#[derive(Debug, Clone, Subcommand)]
#[clap(rename_all = "kebab-case")]
pub enum SubCmd {
    /// Show the log viewer window or bring it to foreground.
    LogViewerShow,

    /// Hide the log viewer window if opened.
    LogViewerHide,

    /// Use a particular method for all future notifications.
    SetNotify {
        /// The notification method to use.
        #[clap(index = 1, value_name = "METHOD", value_enum)]
        notify_method: NotifyMethod,
    },

    /// Restart the currently running sslocal instance.
    Restart,

    /// Switch to a new profile by starting a new sslocal instance.
    SwitchProfile {
        /// The display name of the profile to switch to (CASE SENSITIVE)
        #[clap(index = 1, value_name = "NAME")]
        profile_name: String,
    },

    /// Stop the currently running sslocal instance.
    Stop,

    /// Quit the application.
    Quit,
}

impl From<SubCmd> for APICommand {
    fn from(cmd: SubCmd) -> Self {
        match cmd {
            SubCmd::LogViewerShow => APICommand::LogViewerShow,
            SubCmd::LogViewerHide => APICommand::LogViewerHide,
            SubCmd::SetNotify { notify_method } => APICommand::SetNotify(notify_method),
            SubCmd::Restart => APICommand::Restart,
            SubCmd::SwitchProfile { profile_name } => APICommand::SwitchProfile(profile_name),
            SubCmd::Stop => APICommand::Stop,
            SubCmd::Quit => APICommand::Quit,
        }
    }
}
