//! This module contains predefined shared constants.

use std::{env, path::PathBuf};

use lazy_static::lazy_static;
use which::which;

/// Default buffer size for a `bus::Bus`.
pub const BUS_BUFFER_SIZE: usize = 20;

/// The existence of this file in a directory indicates that
/// this directory is a launch profile.
pub const CONFIG_FILE_NAME: &str = "profile.yaml";

/// Default logging level for the CLI logger.
///
/// 0: `Error`, 1: `Warn`, 2: `Info`, 3: `Debug`, 4: `Trace`
pub const DEFAULT_LOG_LEVEL: i32 = 2;

/// The existence of this file in a directory marks the directory
/// as ignored during the loading process.
pub const LOAD_IGNORE_FILE_NAME: &str = ".ss_ignore";

/// The default binary to lookup in $PATH, if not overridden by profile.
pub const SSLOCAL_DEFAULT_LOOKUP_NAME: &str = "sslocal";

/// The name shown when mouseover on the tray icon.
pub const TRAY_TITLE: &str = "Shadowsocks GTK";

lazy_static! {
    pub static ref DEFAULT_CONFIG_DIR: PathBuf =
        PathBuf::from(env::var("HOME").expect("$HOME not set")).join(".config/shadowsocks-gtk-rs");
    pub static ref PROFILES_DIR_DEFAULT: PathBuf = DEFAULT_CONFIG_DIR.join("profiles");
    pub static ref PROFILES_DIR_DEFAULT_STR: String = PROFILES_DIR_DEFAULT
        .to_str()
        .expect("default profiles-dir not UTF-8")
        .into();
    pub static ref APP_STATE_PATH_DEFAULT: PathBuf = DEFAULT_CONFIG_DIR.join("app-state.yaml");
    pub static ref APP_STATE_PATH_DEFAULT_STR: String = APP_STATE_PATH_DEFAULT
        .to_str()
        .expect("default app-state-path not UTF-8")
        .into();
    pub static ref RUNTIME_API_SOCKET_PATH_DEFAULT: PathBuf =
        PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".into())).join("shadowsocks-gtk-rs.sock");
    pub static ref RUNTIME_API_SOCKET_PATH_DEFAULT_STR: String = RUNTIME_API_SOCKET_PATH_DEFAULT
        .to_str()
        .expect("default runtime-api-socket-path not UTF-8")
        .into();
    pub static ref SSLOCAL_DEFAULT_RESOLVED: Result<PathBuf, which::Error> = which(SSLOCAL_DEFAULT_LOOKUP_NAME);
}
