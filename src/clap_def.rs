//! This module contains code that define the CLI API.

use std::{env, fs, path::PathBuf};

use clap::{crate_version, App, AppSettings, Arg};

/// 0: `Error`, 1: `Warn`, 2: `Info`, 3: `Debug`, 4: `Trace`
pub const DEFAULT_LOG_VERBOSITY: i32 = 2;

/// Build a clap app. Only call once.
pub fn build_app() -> App<'static, 'static> {
    App::new("shadowsocks-gtk-client")
        .version(crate_version!())
        .author("spyophobia <76800505+spyophobia@users.noreply.github.com>")
        .about("A desktop GUI frontend for shadowsocks-rust client implemented with gtk-rs.")
        .settings(&[
            AppSettings::AllowNegativeNumbers,
            AppSettings::ArgRequiredElseHelp,
            AppSettings::DisableHelpSubcommand,
        ])
        .arg({
            let mut default_dir = PathBuf::from(env::var("HOME").expect("$HOME not set"));
            default_dir.push(".config/shadowsocks-gtk-client/config-profiles");
            // see https://stackoverflow.com/a/30527289/5637701
            let default_val: &'static str =
                Box::leak(default_dir.to_str().expect("default profiles-dir not UTF-8").into());
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
        })
        .arg(
            Arg::with_name("tray-icon-filename")
                .long("icon-name")
                .takes_value(true)
                .default_value("shadowsocks-gtk-client")
                .help("Search for a custom image to use for the tray icon"),
        )
        .arg(
            // note to packager: you probably want to copy `res/logo/shadowsocks-gtk-client.png`
            // to `/usr/share/pixmap/` and not use this parameter
            Arg::with_name("icon-theme-dir")
                .long("icon-theme-dir")
                .takes_value(true)
                .help(
                    "Set a custom directory to search for the tray icon. Default is to \
                    use system setting\nSee https://askubuntu.com/a/43951/1020143",
                ),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Increases the verbosity level of output. This is a repeated flag"),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .multiple(true)
                .help("Decreases the verbosity level of output. This is a repeated flag"),
        )
}
