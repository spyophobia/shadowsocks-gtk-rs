use clap::ArgMatches;
use gui::app;

mod clap_def;
mod event;
mod gui;
mod io;
mod profile_manager;

fn main() -> Result<(), String> {
    // init clap app
    let clap_matches = clap_def::build_app().get_matches();

    // init logger
    logger_init(&clap_matches);

    // start app
    app::run(&clap_matches).map_err(|err| err.to_string())
}

fn logger_init(matches: &ArgMatches) {
    use log::Level::*;

    /// 0: `Error`, 1: `Warn`, 2: `Info`, 3: `Debug`, 4: `Trace`
    pub const DEFAULT_LOG_VERBOSITY: i32 = 2;

    let verbosity =
        DEFAULT_LOG_VERBOSITY + matches.occurrences_of("verbose") as i32 - matches.occurrences_of("quiet") as i32;
    let level = match verbosity {
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
