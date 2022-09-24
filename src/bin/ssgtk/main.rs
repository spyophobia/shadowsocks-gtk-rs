use gui::app::{self, AppStartError};
use log::{error, SetLoggerError};
use notify_rust::Urgency;
use shadowsocks_gtk_rs::consts::*;

use crate::gui::notification::notify_toast;

mod clap_def;
mod core;
mod event;
mod gui;
mod io;

fn main() -> Result<(), AppStartError> {
    // init clap app
    let args = clap_def::parse_and_validate();

    // init logger
    logger_init(args.verbose as i32 - args.quiet as i32).unwrap(); // never produces error on first call of init

    // start app
    let start_res = app::run(&args);
    if let Err(ref err) = start_res {
        error!("ssgtk failed to load, sending notification");
        let text_2 = format!("Error: {}", err);
        // if this fails, too bad
        let _ = notify_toast(Urgency::Critical, "Failed to start", &text_2);
    }
    start_res
}

fn logger_init(relative_verbosity: i32) -> Result<(), SetLoggerError> {
    use log::LevelFilter::*;
    use simplelog::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};

    let level_filter = match DEFAULT_LOG_LEVEL + relative_verbosity {
        0 => Error,
        1 => Warn,
        2 => Info,
        3 => Debug,
        4.. => Trace,
        _ => Off, // negative == disable logging
    };

    let logger_config = ConfigBuilder::new()
        .add_filter_allow_str("shadowsocks-gtk-rs") // crate lib
        .add_filter_allow_str("ssgtk") // crate bin
        .build();
    TermLogger::init(level_filter, logger_config, TerminalMode::Stdout, ColorChoice::Auto)
}
