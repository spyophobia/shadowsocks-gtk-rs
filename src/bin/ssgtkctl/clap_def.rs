//! This module contains code that define the CLI API.

use std::{env, path::PathBuf};

use clap::{crate_authors, crate_version, App, AppSettings, Arg, SubCommand};
use shadowsocks_gtk_rs::notify_method::NotifyMethod;
use strum::VariantNames;

/// Build a clap app. Only call once.
pub fn build_app() -> App<'static, 'static> {
    // app
    let mut app = App::new("ssgtkctl")
        .version(crate_version!())
        .author(crate_authors!())
        .about("A delegate binary that sends commands to the runtime API for your convenience.")
        .settings(&[
            AppSettings::AllowNegativeNumbers,
            AppSettings::DisableHelpSubcommand,
            AppSettings::InferSubcommands,
            AppSettings::SubcommandRequiredElseHelp,
        ]);

    // args
    let arg_runtime_api_socket_path = {
        let default_val: &'static str = {
            let mut path = PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".into()));
            path.push("shadowsocks-gtk-rs.sock");
            Box::leak(path.to_str().expect("default runtime-api-socket-path not UTF-8").into())
        };
        Arg::with_name("runtime-api-socket-path")
            .short("a")
            .long("api-socket")
            .takes_value(true)
            .default_value(default_val)
            .help(
                "Send command to the runtime API listener at a custom socket path. \
                Useful if you want to control multiple instances.",
            )
    };

    app = app.arg(arg_runtime_api_socket_path);

    // subcommands
    let cmd_backlog_show =
        SubCommand::with_name("backlog-show").about("Show the backlog window or bring it to foreground");
    let cmd_backlog_hide = SubCommand::with_name("backlog-hide").about("Hide the backlog window if opened");
    let cmd_set_notify = SubCommand::with_name("set-notify")
        .arg({
            Arg::with_name("notify-method")
                .required(true)
                .index(1)
                .takes_value(true)
                .possible_values(NotifyMethod::VARIANTS)
                .help("The notification method to use")
        })
        .about("Use a particular method for all future notifications");

    let cmd_restart = SubCommand::with_name("restart").about("Restart the currently running sslocal instance");
    let cmd_switch_profile = SubCommand::with_name("switch-profile")
        .arg(
            Arg::with_name("profile-name")
                .required(true)
                .index(1)
                .takes_value(true)
                .help("The display name of the profile to switch to (CASE SENSITIVE)"),
        )
        .about("Switch to a new profile by starting a new sslocal instance");
    let cmd_stop = SubCommand::with_name("stop").about("Stop the currently running sslocal instance");
    let cmd_quit = SubCommand::with_name("quit").about("Quit the application");

    app = app.subcommands(vec![
        cmd_backlog_show,
        cmd_backlog_hide,
        cmd_set_notify,
        cmd_restart,
        cmd_switch_profile,
        cmd_stop,
        cmd_quit,
    ]);

    app
}
