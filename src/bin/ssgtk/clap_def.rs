//! This module contains code that define the CLI API.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use clap::{ArgAction, ErrorKind, IntoApp, Parser};
use lazy_static::lazy_static;

use crate::io::app_state::{AppState, AppStateError};

lazy_static! {
    static ref DEFAULT_CONFIG_DIR: PathBuf =
        PathBuf::from(env::var("HOME").expect("$HOME not set")).join(".config/shadowsocks-gtk-rs");
    static ref PROFILES_DIR_DEFAULT: PathBuf = DEFAULT_CONFIG_DIR.join("config-profiles");
    static ref PROFILES_DIR_DEFAULT_STR: String = PROFILES_DIR_DEFAULT
        .to_str()
        .expect("default profiles-dir not UTF-8")
        .into();
    static ref APP_STATE_PATH_DEFAULT: PathBuf = DEFAULT_CONFIG_DIR.join("app-state.yaml");
    static ref APP_STATE_PATH_DEFAULT_STR: String = APP_STATE_PATH_DEFAULT
        .to_str()
        .expect("default app-state-path not UTF-8")
        .into();
    static ref RUNTIME_API_SOCKET_PATH_DEFAULT: PathBuf =
        PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".into())).join("shadowsocks-gtk-rs.sock");
    static ref RUNTIME_API_SOCKET_PATH_DEFAULT_STR: String = RUNTIME_API_SOCKET_PATH_DEFAULT
        .to_str()
        .expect("default runtime-api-socket-path not UTF-8")
        .into();
}

#[derive(Debug, Clone, Parser)]
#[clap(name = "ssgtk", author, version, about, disable_help_subcommand = true)]
pub struct CliArgs {
    /// The directory from which to load config profiles.
    #[clap(short = 'p', long = "profiles-dir", value_name = "DIR", default_value = &PROFILES_DIR_DEFAULT_STR)]
    pub profiles_dir: PathBuf,

    /// Load and store app state from&to a custom file path.
    ///
    /// Useful if you want to run multiple instances".
    #[clap(long = "app-state", value_name = "PATH", default_value = &APP_STATE_PATH_DEFAULT_STR)]
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

    #[cfg(feature = "runtime-api")]
    /// Bind the runtime API listener to a custom socket.
    ///
    /// Useful if you want to control multiple instances.
    #[clap(long = "api-socket", value_name = "PATH", default_value = &RUNTIME_API_SOCKET_PATH_DEFAULT_STR)]
    pub runtime_api_socket_path: PathBuf,
}

/// Build a clap app and return matches. Only call once.
pub fn parse_and_validate() -> CliArgs {
    let mut args = CliArgs::parse();

    // validate profiles_dir
    let profiles_dir = &args.profiles_dir;
    let res = if profiles_dir == PROFILES_DIR_DEFAULT.as_path() {
        // if default, then mkdir if absent
        fs::create_dir_all(profiles_dir)
    } else {
        // otherwise, make sure we can read dir
        fs::read_dir(profiles_dir).map(|_| ())
    };
    if let Err(io_err) = res {
        CliArgs::command().error(ErrorKind::Io, io_err).exit();
    }

    // validate app_state_path
    if let Err(io_err) = validate_app_state_path(&args.app_state_path) {
        CliArgs::command().error(ErrorKind::Io, io_err).exit();
    }

    // validate and canonicalize icon_theme_dir
    if let Some(theme_dir) = &args.icon_theme_dir {
        // AppIndicator requires an absolute path
        match theme_dir.canonicalize() {
            Ok(dir) if dir.to_str().is_some() => args.icon_theme_dir = Some(dir),
            Ok(dir) => CliArgs::command()
                .error(
                    ErrorKind::Io,
                    format!("Canonicalized icon_theme_dir ({:?}) not valid UTF-8", dir),
                )
                .exit(),
            Err(io_err) => CliArgs::command().error(ErrorKind::Io, io_err).exit(),
        }
    }

    args
}

fn validate_app_state_path(app_state_path: impl AsRef<Path>) -> Result<(), AppStateError> {
    let app_state_path = app_state_path.as_ref();
    if app_state_path == APP_STATE_PATH_DEFAULT.as_path() {
        // if default, then mkdir and overwrite with default if parse error
        // this could happen if the state file is corrupted,
        // or if an updated version modified the state file format
        fs::create_dir_all(DEFAULT_CONFIG_DIR.as_path())?;
        match AppState::from_file(app_state_path) {
            Ok(_state) => Ok(()),
            Err(err) => {
                println!(
                    "[Pre-init] error while parsing default app-state file ({:?}): {}, \
                    resetting with default",
                    app_state_path, err
                );
                AppState::default().write_to_file(app_state_path)
            }
        }
    } else if !Path::new(app_state_path).exists() {
        // if the specified file doesn't exist
        println!(
            "[Pre-init] the specified app-state file ({:?}) doesn't exist, \
            trying to create new with default",
            app_state_path
        );
        AppState::default().write_to_file(app_state_path)
    } else {
        // otherwise, try to read and parse file
        AppState::from_file(app_state_path).map(|_| ())
    }
}
