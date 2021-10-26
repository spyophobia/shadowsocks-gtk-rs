use clap::ArgMatches;
use log::debug;

use crate::{gui::app, io::config_loader::ConfigFolder};

mod clap_def;
mod gui;
mod io;
mod profile_manager;
mod util;

fn main() -> Result<(), String> {
    // init clap app
    let clap_matches = clap_def::build_app().get_matches();

    // init logger
    logger_init(&clap_matches);

    // TODO: catch signals

    // load profiles
    let config_folder = {
        let dir = clap_matches.value_of("profiles-dir").unwrap(); // clap sets default
        ConfigFolder::from_path_recurse(dir).map_err(|err| err.to_string())?
    };
    debug!(
        "Successfully loaded {} profiles in total",
        config_folder.profile_count()
    );

    // start app
    app::run(&clap_matches, &config_folder);

    Ok(())
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
