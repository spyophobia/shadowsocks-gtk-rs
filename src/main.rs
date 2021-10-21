use std::{
    path::Path,
    sync::{Arc, RwLock},
    thread,
};

use clap::ArgMatches;
use gui::tray;
use log::{error, warn};
use profile_manager::ProfileManager;

use crate::io::{app_state::AppState, config_loader::ConfigFolder};

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

    // load profiles
    let config_folder = {
        let dir = clap_matches.value_of("profiles-dir").unwrap(); // clap sets default
        ConfigFolder::from_path_recurse(dir).map_err(|err| err.to_string())?
    };

    // load app state and resume
    let app_state_path = clap_matches.value_of("app-state-path").unwrap(); // clap sets default
    let pm = {
        let previous_state = AppState::from_file(app_state_path).unwrap(); // Ok guaranteed by clap validator
        ProfileManager::resume_from(&previous_state, &config_folder.get_profiles())
    };

    // TEMP: pipe output
    let stdout = pm.stdout_rx.clone();
    let stderr = pm.stderr_rx.clone();
    thread::spawn(move || stdout.iter().for_each(|s| println!("stdout: {}", s)));
    thread::spawn(move || stderr.iter().for_each(|s| println!("stderr: {}", s)));

    // wrap in smart pointer
    let pm_arc = Arc::new(RwLock::new(pm));

    // start GUI loop
    gui_run(&clap_matches, &config_folder, Arc::clone(&pm_arc));

    // cleanup
    cleanup(pm_arc, app_state_path);

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

fn gui_run(clap_matches: &ArgMatches, config_folder: &ConfigFolder, profile_manager: Arc<RwLock<ProfileManager>>) {
    gtk::init().unwrap();
    let icon_name = clap_matches.value_of("tray-icon-filename").unwrap(); // clap sets default
    let icon_theme_dir_abs = clap_matches
        .value_of("icon-theme-dir")
        .and_then(|p| match Path::new(p).canonicalize() {
            Ok(p) => p.to_str().map(|s| s.to_string()),
            Err(err) => {
                warn!("Cannot resolve the specified icon theme directory: {}", err);
                warn!("Reverting back to system setting - you may get a blank icon");
                None
            }
        });
    let _tray_item = tray::build_and_show(
        config_folder,
        &icon_name,
        icon_theme_dir_abs.as_deref(),
        profile_manager,
    );
    gtk::main();
}

fn cleanup<P>(profile_manager: Arc<RwLock<ProfileManager>>, save_path: P)
where
    P: AsRef<Path>,
{
    let mut pm = profile_manager.write().unwrap_or_else(|err| {
        warn!("Write lock on profile manager poisoned, recovering");
        err.into_inner()
    });
    // save app state
    if let Err(err) = pm.snapshot().write_to_file(save_path) {
        error!("Failed to save app state: {}", err);
    };
    // stop any running `sslocal` process
    let _ = pm.stop();
}

#[cfg(test)]
mod test {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    use log::debug;

    use crate::{
        io::config_loader::ConfigFolder,
        profile_manager::{OnFailure, ProfileManager},
        util::leaky_bucket::NaiveLeakyBucketConfig,
    };

    /// This test will always pass. You need to examine the outputs manually.
    ///
    /// `cargo test example_profiles_test_run -- --nocapture`
    #[test]
    fn example_profiles_test_run() {
        simple_logger::init().unwrap();

        // parse example configs
        let eg_configs = ConfigFolder::from_path_recurse("example-config-profiles").unwrap();
        let profile_list = eg_configs.get_profiles();
        debug!("Loaded {} profiles.", profile_list.len());

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
