//! This module contains code that define the CLI API.

use std::{fs, path::PathBuf};

use clap::{ArgAction, IntoApp, Parser};
use shadowsocks_gtk_rs::consts::*;

#[derive(Debug, Clone, Parser)]
#[clap(name = "ssgtk", author, version, about, disable_help_subcommand = true)]
pub struct CliArgs {
    /// The directory from which to load config profiles.
    #[clap(short = 'p', long = "profiles-dir", value_name = "DIR", default_value_os = PROFILES_DIR_PATH_DEFAULT.as_os_str())]
    pub profiles_dir: PathBuf,

    /// Load and store app state from&to a custom file path.
    ///
    /// Useful if you want to run multiple instances".
    #[clap(long = "app-state", value_name = "PATH", default_value_os = STATE_FILE_PATH_DEFAULT.as_os_str())]
    pub app_state_path: PathBuf,

    /// Search for a custom image to use for the tray icon.
    #[clap(long = "icon-name", value_name = "NAME", default_value = "shadowsocks-gtk-rs")]
    pub tray_icon_filename: String,

    /// Set a custom directory to search for the tray icon.
    ///
    /// Useful for testing (when the icon is not installed in standard
    /// system directories; see https://askubuntu.com/a/43951/1020143).
    #[clap(long = "icon-theme-dir", value_name = "DIR")]
    pub icon_theme_dir: Option<PathBuf>,

    /// Increase the verbosity level of output.
    /// This is a repeatable flag.
    #[clap(short = 'v', long = "verbose", action = ArgAction::Count)]
    pub verbose: u8,

    /// Decrease the verbosity level of output.
    /// This is a repeatable flag.
    #[clap(short = 'q', long = "quiet", action = ArgAction::Count)]
    pub quiet: u8,

    /// Bind the runtime API listener to a custom socket.
    ///
    /// Useful if you want to control multiple instances.
    #[cfg(feature = "runtime-api")]
    #[clap(long = "api-socket", value_name = "PATH", default_value_os = RUNTIME_API_SOCKET_PATH_DEFAULT.as_os_str())]
    pub runtime_api_socket_path: PathBuf,
}

/// Build a clap app and return matches. Only call once.
pub fn parse_and_validate() -> CliArgs {
    match validate_impl(CliArgs::parse()) {
        Ok(args) => args,
        Err(err) => err.exit(),
    }
}

fn validate_impl(mut args: CliArgs) -> Result<CliArgs, clap::Error> {
    // validate profiles_dir
    let profiles_dir = &args.profiles_dir;
    if PROFILES_DIR_PATH_DEFAULT.eq(profiles_dir) {
        // if default, then mkdir if absent
        fs::create_dir_all(profiles_dir)?;
    }

    // validate app_state_path
    let app_state_path = &args.app_state_path;
    if STATE_FILE_PATH_DEFAULT.eq(app_state_path) {
        // if default, then mkdir if absent
        let _ = XDG_DIRS.place_state_file(STATE_FILE_NAME_DEFAULT)?;
    }

    // validate and canonicalize icon_theme_dir
    if let Some(theme_dir) = &args.icon_theme_dir {
        // AppIndicator requires an absolute path
        let abs_dir = theme_dir.canonicalize()?;
        if abs_dir.to_str().is_none() {
            Err(CliArgs::command().error(
                clap::ErrorKind::InvalidUtf8,
                format!("Canonicalized icon_theme_dir ({:?})", abs_dir),
            ))?;
        }
        args.icon_theme_dir = Some(abs_dir);
    }

    Ok(args)
}
