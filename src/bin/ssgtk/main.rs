use gui::app;

mod clap_def;
mod event;
mod gui;
mod io;
mod profile_manager;

fn main() -> Result<(), String> {
    // init clap app
    let args = clap_def::parse_and_validate();

    // init logger
    logger_init(args.verbose as i32 - args.quiet as i32);

    // start app
    app::run(&args).map_err(|err| err.to_string())
}

fn logger_init(relative_verbosity: i32) {
    use log::Level::*;

    /// 0: `Error`, 1: `Warn`, 2: `Info`, 3: `Debug`, 4: `Trace`
    pub const DEFAULT_LOG_VERBOSITY: i32 = 2;

    let level = match DEFAULT_LOG_VERBOSITY + relative_verbosity {
        0 => Some(Error),
        1 => Some(Warn),
        2 => Some(Info),
        3 => Some(Debug),
        4.. => Some(Trace),
        _ => None, // negative == disable logging
    };
    if let Some(l) = level {
        simple_logger::init_with_level(l).unwrap(); // never produces error on first call of init
    }
}
