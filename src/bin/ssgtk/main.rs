use gui::app;
use log::SetLoggerError;
use shadowsocks_gtk_rs::consts::*;

mod clap_def;
mod event;
mod gui;
mod io;
mod profile_manager;

fn main() -> Result<(), String> {
    // init clap app
    let args = clap_def::parse_and_validate();

    // init logger
    logger_init(args.verbose as i32 - args.quiet as i32).unwrap(); // never produces error on first call of init

    // start app
    app::run(&args).map_err(|err| err.to_string())
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
