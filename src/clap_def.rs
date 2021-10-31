//! This module contains code that define the CLI API.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use clap::{crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg};

use crate::io::app_state::AppState;

/// Build a clap app. Only call once.
pub fn build_app() -> App<'static, 'static> {
    // common default values
    let default_config_dir = {
        let mut dir = PathBuf::from(env::var("HOME").expect("$HOME not set"));
        dir.push(".config/shadowsocks-gtk-rs");
        dir
    };

    // app
    let mut app = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .settings(&[
            AppSettings::AllowNegativeNumbers,
            AppSettings::ArgRequiredElseHelp,
            AppSettings::DisableHelpSubcommand,
        ]);

    // args
    let arg_profiles_dir = {
        let default_val: &'static str = {
            let mut dir = default_config_dir.clone();
            dir.push("config-profiles");
            Box::leak(dir.to_str().expect("default profiles-dir not UTF-8").into())
        };
        Arg::with_name("profiles-dir")
            .short("p")
            .long("profiles-dir")
            .takes_value(true)
            .default_value(default_val)
            .validator(move |arg| {
                if &arg == default_val {
                    // if default, then mkdir if absent
                    fs::create_dir_all(arg)
                } else {
                    // otherwise, make sure we can read dir
                    fs::read_dir(arg).map(|_| ())
                }
                .map_err(|err| err.to_string())
            })
            .help("The directory from which to load config profiles")
    };

    let arg_app_state_path = {
        let default_val: &'static str = {
            let mut path = default_config_dir.clone();
            path.push("app-state.yaml");
            Box::leak(path.to_str().expect("default app-state-path not UTF-8").into())
        };
        Arg::with_name("app-state-path")
            .long("app-state")
            .takes_value(true)
            .default_value(default_val)
            .validator(move |arg| {
                if &arg == default_val {
                    // if default, then mkdir and overwrite with default if parse error
                    fs::create_dir_all(&default_config_dir).map_err(|err| err.to_string())?;
                    match AppState::from_file(&arg) {
                        Ok(_state) => Ok(()),
                        Err(err) => {
                            println!(
                                "[Pre-init] error while parsing default app-state file ({}): {}, \
                                resetting with default",
                                &arg, err
                            );
                            AppState::default().write_to_file(&arg)
                        }
                    }
                } else if !Path::new(&arg).exists() {
                    // if the specified file doesn't exist
                    println!(
                        "[Pre-init] the specified app-state file ({}) doesn't exist, \
                        trying to create new with default",
                        &arg
                    );
                    AppState::default().write_to_file(&arg)
                } else {
                    // otherwise, try to read and parse file
                    AppState::from_file(&arg).map(|_| ())
                }
                .map_err(|err| err.to_string())
            })
            .help(
                "Load and store app state from&to a custom file path. \
                Useful if you want to run multiple instances",
            )
    };

    let arg_tray_icon_filename = Arg::with_name("tray-icon-filename")
        .long("icon-name")
        .takes_value(true)
        .default_value("shadowsocks-gtk-rs")
        .help("Search for a custom image to use for the tray icon");

    // note to packager: you probably want to copy `res/logo/shadowsocks-gtk-rs.png`
    // to `/usr/share/pixmap/` and not use this parameter
    let arg_icon_theme_dir = Arg::with_name("icon-theme-dir")
        .long("icon-theme-dir")
        .takes_value(true)
        .help(
            "Set a custom directory to search for the tray icon. Default is to \
            use system setting\nSee https://askubuntu.com/a/43951/1020143",
        );

    let arg_verbose = Arg::with_name("verbose")
        .short("v")
        .long("verbose")
        .multiple(true)
        .help("Increases the verbosity level of output. This is a repeated flag");

    let arg_quiet = Arg::with_name("quiet")
        .short("q")
        .long("quiet")
        .multiple(true)
        .help("Decreases the verbosity level of output. This is a repeated flag");

    app = app.args(&[
        arg_profiles_dir,
        arg_app_state_path,
        arg_tray_icon_filename,
        arg_icon_theme_dir,
        arg_verbose,
        arg_quiet,
    ]);

    // args behind feature flags
    #[cfg(feature = "runtime_api")]
    let arg_runtime_api_socket_path = {
        let default_val: &'static str = {
            let mut path = PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".into()));
            path.push("shadowsocks-gtk-rs.sock");
            Box::leak(path.to_str().expect("default runtime-api-socket-path not UTF-8").into())
        };
        Arg::with_name("runtime-api-socket-path")
            .long("api-socket")
            .takes_value(true)
            .default_value(default_val)
            .help(
                "Bind the runtime API listener to a custom socket. \
                Useful if you want to control multiple instances.",
            )
    };

    app = app.args(&[
        #[cfg(feature = "runtime_api")]
        arg_runtime_api_socket_path,
    ]);

    app
}
