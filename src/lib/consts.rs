//! This module contains predefined shared constants.

use std::path::PathBuf;

use lazy_static::lazy_static;

// Static strings
// ========================================

/// The application's name.
///
/// For now, the display name and the OS name (no-space-name used for directories)
/// are identical.
pub const APP_NAME: &str = "shadowsocks-gtk-rs";

/// The default name of the directory under the XDG config directory
/// which contains all profiles.
pub const PROFILES_DIR_NAME_DEFAULT: &str = "profiles";

/// The default name of the state file under the XDG state directory.
pub const STATE_FILE_NAME_DEFAULT: &str = "app-state.yaml";

/// The default name of the socket file under the XDG runtime directory
/// used for the runtime API.
#[cfg(feature = "runtime-api")]
pub const RUNTIME_API_SOCKET_NAME_DEFAULT: &str = "shadowsocks-gtk-rs.sock";

/// The existence of this file in a directory indicates that
/// this directory is a launch profile.
pub const PROFILE_CONFIG_FILE_NAME: &str = "profile.yaml";

/// The existence of this file in a directory marks the directory
/// as ignored during the loading process.
pub const PROFILE_IGNORE_FILE_NAME: &str = ".ss_ignore";

/// The default binary to lookup in $PATH, if not overridden by profile.
pub const SSLOCAL_LOOKUP_NAME_DEFAULT: &str = "sslocal";

// Hard-coded constants
// ========================================

/// Default logging level for the CLI logger.
///
/// 0: `Error`, 1: `Warn`, 2: `Info`, 3: `Debug`, 4: `Trace`
pub const DEFAULT_LOG_LEVEL: i32 = 2;

/// Default buffer size for a `bus::Bus`.
pub const BUS_BUFFER_SIZE: usize = 20;

// Static runtime paths
// ========================================

lazy_static! {
    pub static ref XDG_DIRS: xdg::BaseDirectories = xdg::BaseDirectories::with_prefix(APP_NAME).expect("XDG error");
    pub static ref PROFILES_DIR_PATH_DEFAULT: PathBuf = XDG_DIRS.get_config_file(PROFILES_DIR_NAME_DEFAULT);
    pub static ref STATE_FILE_PATH_DEFAULT: PathBuf = XDG_DIRS.get_state_file(STATE_FILE_NAME_DEFAULT);
}

#[cfg(feature = "runtime-api")]
lazy_static! {
    pub static ref RUNTIME_API_SOCKET_PATH_DEFAULT: PathBuf = XDG_DIRS
        .get_runtime_file(RUNTIME_API_SOCKET_NAME_DEFAULT)
        .expect("Error accessing XDG runtime directory");
}
