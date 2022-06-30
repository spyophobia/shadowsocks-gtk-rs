//! This module contains code that handles profile loading.

use std::{
    collections::HashSet,
    ffi::OsString,
    fmt,
    fs::read_to_string,
    io,
    net::{IpAddr, Ipv6Addr},
    os::unix::prelude::IntoRawFd,
    path::{Path, PathBuf},
};

use derivative::Derivative;
use duct::{cmd, Handle};
use ipnet::IpNet;
use itertools::Itertools;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use shadowsocks_gtk_rs::consts::*;
use which::which;

/// Optional fields which allow a config to override its profile's default metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataOverride {
    display_name: Option<String>,
    pwd: Option<PathBuf>,
    bin_path: Option<PathBuf>,
}

trait ToLaunchArgs {
    fn to_launch_args(&self) -> Vec<OsString>;
}

/// Fields for a "Config file"-type ProfileConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileOptions {
    config_path: PathBuf,
}
impl ToLaunchArgs for ConfigFileOptions {
    fn to_launch_args(&self) -> Vec<OsString> {
        vec!["--config".into(), (&self.config_path).into()]
    }
}

/// Common fields for ProfileConfig types that do not use a config file.
#[derive(Derivative, Clone, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct ConnectOptions {
    local_addr: (IpAddr, u16),
    server_addr: (String, u16),
    #[derivative(Debug(format_with = "password_omit"))]
    password: String,
    encrypt_method: String,
}
impl ToLaunchArgs for ConnectOptions {
    fn to_launch_args(&self) -> Vec<OsString> {
        let mut args = vec![];
        // local address
        let local_addr = {
            let (a, p) = self.local_addr;
            match a {
                IpAddr::V4(v4) => format!("{}:{}", v4, p),
                IpAddr::V6(v6) => format!("[{}]:{}", v6, p),
            }
        };
        args.extend_from_slice(&["--local-addr".into(), local_addr.into()]);
        // server address
        let server_addr = {
            let (a, p) = &self.server_addr;
            match a.parse::<Ipv6Addr>() {
                Ok(_) => format!("[{}]:{}", a, p), // IPv6
                Err(_) => format!("{}:{}", a, p),  // Domain or IPv4
            }
        };
        args.extend_from_slice(&["--server-addr".into(), server_addr.into()]);
        // password
        args.extend_from_slice(&["--password".into(), (&self.password).into()]);
        // encrypt_method
        args.extend_from_slice(&["--encrypt-method".into(), (&self.encrypt_method).into()]);
        args
    }
}

/// Helper function for `derivative(Debug)`.
fn password_omit(_: &str, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    write!(fmt, "*hidden*")
}

/// Fields for a "Proxy"-type ProfileConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyOptions {
    // TODO: Add protocol selection
}
impl ToLaunchArgs for ProxyOptions {
    fn to_launch_args(&self) -> Vec<OsString> {
        vec![]
    }
}

/// Fields for a "Tun"-type ProfileConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunOptions {
    if_name: Option<String>,
    if_addr: Option<IpNet>,
}
impl ToLaunchArgs for TunOptions {
    fn to_launch_args(&self) -> Vec<OsString> {
        // protocol
        let mut args = vec!["--protocol".into(), "tun".into()];
        // interface name
        if let Some(if_name) = &self.if_name {
            args.extend_from_slice(&["--tun-interface-name".into(), if_name.into()]);
        }
        // interface address
        if let Some(if_addr) = &self.if_addr {
            args.extend_from_slice(&["--tun-interface-address".into(), if_addr.to_string().into()]);
        }
        args
    }
}

/// Extra configs for advanced users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedOptions {
    // IMPRV: more to come
    extra_args: Option<Vec<String>>,
}
impl ToLaunchArgs for AdvancedOptions {
    fn to_launch_args(&self) -> Vec<OsString> {
        let mut args = vec![];
        // extra args
        if let Some(extra) = &self.extra_args {
            args.append(&mut extra.iter().map_into().collect())
        }
        args
    }
}

/// The static configuration for a profile. Represents the file on disk faithfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")] // See https://serde.rs/enum-representations.html#internally-tagged
pub enum ProfileConfig {
    /// Profile launches `sslocal` with arbitrary config file using `sslocal --config <CONFIG>`.
    ConfigFile {
        #[serde(flatten)]
        metadata: MetadataOverride,
        #[serde(flatten)]
        opts: ConfigFileOptions,
        #[serde(flatten)]
        adv_opts: AdvancedOptions,
    },
    /// Profile launches `sslocal` in proxy mode.
    Proxy {
        #[serde(flatten)]
        metadata: MetadataOverride,
        #[serde(flatten)]
        conn_opts: ConnectOptions,
        #[serde(flatten)]
        opts: ProxyOptions,
        #[serde(flatten)]
        adv_opts: AdvancedOptions,
    },
    /// Profile launches `sslocal` in tun mode.
    Tun {
        #[serde(flatten)]
        metadata: MetadataOverride,
        #[serde(flatten)]
        conn_opts: ConnectOptions,
        #[serde(flatten)]
        opts: TunOptions,
        #[serde(flatten)]
        adv_opts: AdvancedOptions,
    },
}

impl ProfileConfig {
    fn get_metadata_override(&self) -> &MetadataOverride {
        use ProfileConfig::*;
        match self {
            ConfigFile { metadata, .. } => metadata,
            Proxy { metadata, .. } => metadata,
            Tun { metadata, .. } => metadata,
        }
    }
    fn to_launch_args(&self) -> Vec<OsString> {
        use ProfileConfig::*;
        match self {
            ConfigFile { opts, adv_opts, .. } => {
                let mut args = opts.to_launch_args();
                args.append(&mut adv_opts.to_launch_args());
                args
            }
            Proxy {
                conn_opts,
                opts,
                adv_opts,
                ..
            } => {
                let mut args = conn_opts.to_launch_args();
                args.append(&mut opts.to_launch_args());
                args.append(&mut adv_opts.to_launch_args());
                args
            }
            Tun {
                conn_opts,
                opts,
                adv_opts,
                ..
            } => {
                let mut args = conn_opts.to_launch_args();
                args.append(&mut opts.to_launch_args());
                args.append(&mut adv_opts.to_launch_args());
                args
            }
        }
    }
}

/// Dynamically generated and patched metadata for a profile.
#[derive(Debug, Clone)]
pub struct ProfileMetadata {
    pub display_name: String,
    pwd: PathBuf,
    bin_path: PathBuf,
}

/// A complete `sslocal` launch profile.
#[derive(Debug, Clone)]
pub struct Profile {
    pub metadata: ProfileMetadata,
    config: ProfileConfig,
}

impl Profile {
    /// Run `sslocal` using the settings specified by this profile.
    ///
    /// If `stdout` or `stderr` is `None`, the corresponding output
    /// is redirected to`/dev/null` (discarded) by default.
    pub fn run_sslocal(&self, stdout: Option<impl IntoRawFd>, stderr: Option<impl IntoRawFd>) -> io::Result<Handle> {
        let ProfileMetadata { pwd, bin_path, .. } = &self.metadata;
        let mut expr = cmd(bin_path, self.config.to_launch_args()).dir(pwd).stdin_null();
        expr = match stdout {
            Some(fd) => expr.stdout_file(fd),
            None => expr.stdout_null(),
        };
        expr = match stderr {
            Some(fd) => expr.stderr_file(fd),
            None => expr.stderr_null(),
        };
        expr.unchecked() // check for abnormal termination elsewhere
            .start()
    }
}

/// A group containing multiple profiles and/or subgroups.
#[derive(Debug, Clone)]
pub struct ProfileGroup {
    pub display_name: String,
    pub content: Vec<ProfileFolder>,
}

#[derive(Debug)]
pub enum ProfileLoadError {
    /// Each profile should be its own directory, which can be placed under other directories to form groups.
    NotDirectory(String),
    /// The profile's config file cannot be parsed.
    ConfigParseError(serde_yaml::Error),
    /// Cannot resolve a binary for this profile.
    BadBinary(which::Error),
    /// At least two profiles share the same name.
    NameConflict(String),
    /// The directory contains files (which means it's considered a profile folder),
    /// but there's no config file.
    NoConfigFile(String),
    /// The directory contains neither files nor other valid profiles.
    EmptyGroup(String),
    /// The filesystem encountered an IOError.
    IOError(io::Error),
}

impl fmt::Display for ProfileLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ProfileLoadError::*;

        let prefix = "ProfileLoadError";
        match self {
            NotDirectory(s) => write!(f, "{}-NotDirectory: {}", prefix, s),
            ConfigParseError(e) => write!(f, "{}-ConfigParseError: {}", prefix, e),
            BadBinary(e) => write!(f, "{}-BadBinary: {}", prefix, e),
            NameConflict(s) => write!(f, "{}-NameConflict: {}", prefix, s),
            NoConfigFile(s) => write!(f, "{}-NoConfigFile: {}", prefix, s),
            EmptyGroup(s) => write!(f, "{}-EmptyGroup: {}", prefix, s),
            IOError(e) => write!(f, "{}-IOError: {}", prefix, e),
        }
    }
}

impl From<serde_yaml::Error> for ProfileLoadError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::ConfigParseError(err)
    }
}
impl From<which::Error> for ProfileLoadError {
    fn from(err: which::Error) -> Self {
        Self::BadBinary(err)
    }
}
impl From<io::Error> for ProfileLoadError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub enum ProfileFolder {
    #[derivative(Debug = "transparent")]
    Profile(Profile),
    #[derivative(Debug = "transparent")]
    Group(ProfileGroup),
}

impl ProfileFolder {
    /// Recursively loads all nested profiles within the specified directory.
    ///
    /// **Symlinking is not currently supported.**
    ///
    /// If a call to this function with the user-specified base path fails,
    /// then run the program as if there are no existing configs.
    pub fn from_path_recurse(path: impl AsRef<Path>) -> Result<Self, ProfileLoadError> {
        let mut seen_names = HashSet::new();
        Self::from_path_recurse_impl(path.as_ref(), &mut seen_names)?
            .ok_or(ProfileLoadError::EmptyGroup(path.as_ref().to_string_lossy().into()))
    }

    /// Returns Ok(None) when this directory is ignored.
    fn from_path_recurse_impl(
        path: impl AsRef<Path>,
        seen_names: &mut HashSet<String>,
    ) -> Result<Option<Self>, ProfileLoadError> {
        let path = path.as_ref().canonicalize()?;
        let full_path_str = path.to_string_lossy();

        // make sure path is a directory
        if !path.is_dir() {
            return Err(ProfileLoadError::NotDirectory(full_path_str.into()));
        }
        // make sure directory doesn't contain the ignore file
        if path.join(LOAD_IGNORE_FILE_NAME).is_file() {
            return Ok(None);
        }

        // use directory name as folder's display name
        let default_display_name = path
            .file_name()
            .unwrap() // path has already been canonicalized
            .to_str()
            .unwrap() // UTF-8 has already been verified
            .to_string();

        // if directory contains the config file, then consider it a profile
        let config_path = path.join(CONFIG_FILE_NAME);
        if config_path.is_file() {
            // config
            let content = read_to_string(config_path)?;
            let config: ProfileConfig = serde_yaml::from_str(&content)?;

            // metadata
            let metadata = {
                let mo = config.get_metadata_override().clone();

                let display_name = mo.display_name.unwrap_or(default_display_name);
                if let Some(_) = seen_names.replace(display_name.clone()) {
                    return Err(ProfileLoadError::NameConflict(display_name));
                }
                let pwd = mo.pwd.unwrap_or(path.clone());
                let bin_path = mo
                    .bin_path
                    .map(|p| which(p)) // try to resolve
                    .unwrap_or(SSLOCAL_DEFAULT_RESOLVED.clone())?;

                ProfileMetadata {
                    display_name,
                    pwd,
                    bin_path,
                }
            };

            return Ok(Some(Self::Profile(Profile { metadata, config })));
        }

        // otherwise, check if it contains files at all
        // if so consider it a profile that's missing the config file.
        let has_files = path.read_dir()?.any(|ent_res| match ent_res {
            Ok(ent) => ent.path().is_file(),
            Err(err) => {
                warn!("Cannot open a file or directory: {}", err);
                false
            }
        });
        if has_files {
            return Err(ProfileLoadError::NoConfigFile(full_path_str.into()));
        }

        // otherwise, consider it a group
        let mut subdirs = vec![];
        for ent_res in path.read_dir()? {
            // recursively load all subdirectories
            let subdir_path = ent_res?.path();
            match Self::from_path_recurse_impl(&subdir_path, seen_names) {
                Ok(Some(cf)) => subdirs.push(cf),
                Ok(None) => info!("Ignored a directory and its children: {:?}", subdir_path),
                Err(err) => {
                    error!("Cannot load a subdirectory: {}", err);
                    return Err(err);
                }
            };
        }
        if subdirs.is_empty() {
            error!(
                "The specified profile directory is empty; \
                please read Q&A for a guide on creating a configuration"
            );
            error!("See https://github.com/spyophobia/shadowsocks-gtk-rs/blob/master/res/QnA.md");
            Err(ProfileLoadError::EmptyGroup(full_path_str.into()))
        } else {
            Ok(Some(ProfileFolder::Group(ProfileGroup {
                display_name: default_display_name,
                content: subdirs,
            })))
        }
    }

    /// Recursively count the number of nested profiles within this `ConfigFolder`.
    pub fn profile_count(&self) -> usize {
        use ProfileFolder::*;
        match self {
            Profile(_) => 1,
            Group(g) => g.content.iter().map(|pf| pf.profile_count()).sum(),
        }
    }

    /// Recursively get all the nested profiles within this `ProfileFolder`,
    /// flattened and returned by reference.
    #[allow(dead_code)]
    pub fn get_profiles(&self) -> Vec<&Profile> {
        use ProfileFolder::*;
        match self {
            Profile(p) => vec![p],
            Group(g) => g.content.iter().flat_map(|pf| pf.get_profiles()).collect(),
        }
    }

    /// Recursively searches all the nested profiles within this `ProfileFolder`
    /// for a `Profile` with a matching name.
    pub fn lookup(&self, name: impl AsRef<str>) -> Option<&Profile> {
        use ProfileFolder::*;
        match self {
            Profile(p) if p.metadata.display_name == name.as_ref() => Some(p),
            Profile(_) => None,
            Group(g) => g.content.iter().find_map(|pf| pf.lookup(name.as_ref())),
        }
    }
}
