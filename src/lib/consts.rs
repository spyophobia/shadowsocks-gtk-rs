//! This module contains predefined shared constants.

/// Default buffer size for a `bus::Bus`.
pub const BUS_BUFFER_SIZE: usize = 1000;

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
