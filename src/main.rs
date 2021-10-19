use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use clap::ArgMatches;
use config_loader::ConfigFolder;

use crate::{
    gui::*,
    profile_manager::{OnFailure, ProfileManager},
    util::NaiveLeakyBucketConfig,
};

mod clap_def;
mod config_loader;
mod gui;
mod profile_manager;
mod util;

fn main() -> Result<(), String> {
    // init clap app
    let clap_matches = clap_def::build_app().get_matches();

    // init logger
    logger_init(&clap_matches);

    // load profiles
    let profiles_dir = clap_matches.value_of("profiles-dir").unwrap(); // clap sets default
    let cf = ConfigFolder::from_path_recurse(profiles_dir).map_err(|err| format!("{:?}", err))?;

    // start ProfileManager
    let on_fail = OnFailure::Restart {
        limit: NaiveLeakyBucketConfig::new(3, Duration::from_secs(10)),
    };
    let mgr = ProfileManager::new(on_fail);
    // TEMP: pipe output
    let stdout = mgr.stdout_rx.clone();
    let stderr = mgr.stderr_rx.clone();
    thread::spawn(move || stdout.iter().for_each(|s| println!("stdout: {}", s)));
    thread::spawn(move || stderr.iter().for_each(|s| println!("stderr: {}", s)));
    // wrap in smart pointer
    let profile_manager = Arc::new(RwLock::new(mgr));

    // start GUI
    gtk::init().unwrap();
    let _tray_item = tray::build_and_show(profile_manager, &cf);
    gtk::main();

    Ok(())
}

fn logger_init(matches: &ArgMatches) {
    use log::Level::*;

    let mut verbosity = clap_def::DEFAULT_LOG_VERBOSITY;
    verbosity += matches.occurrences_of("verbose") as i32;
    verbosity -= matches.occurrences_of("quiet") as i32;
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

#[cfg(test)]
mod test {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    use log::debug;

    use crate::{
        config_loader::ConfigFolder,
        profile_manager::{OnFailure, ProfileManager},
        util::NaiveLeakyBucketConfig,
    };

    /// This test will always pass. You need to examine the outputs manually.
    ///
    /// `cargo test example_profiles_test_run -- --nocapture`
    #[test]
    fn example_profiles_test_run() {
        simple_logger::init().unwrap();

        // parse example configs
        let eg_configs = ConfigFolder::from_path_recurse("example_config_profiles").unwrap();
        let eg_configs = Box::leak(Box::new(eg_configs));
        let profile_list = eg_configs.get_profiles();
        debug!("Loaded {:?} profiles.", profile_list.len());

        // setup ProfileManager
        let on_fail = OnFailure::Restart {
            limit: NaiveLeakyBucketConfig::new(3, Duration::from_secs(10)),
        };
        let mut mgr = ProfileManager::new(on_fail);

        // pipe output
        let stdout = mgr.stdout_rx.clone();
        let stderr = mgr.stderr_rx.clone();
        thread::spawn(move || stdout.iter().for_each(|s| println!("stdout: {}", s)));
        thread::spawn(move || stderr.iter().for_each(|s| println!("stderr: {}", s)));

        // run through all example profiles
        for p in profile_list {
            println!();
            mgr.switch_to(p.clone()).unwrap();
            sleep(Duration::from_millis(2500));
        }
        let _ = mgr.stop();
    }
}
