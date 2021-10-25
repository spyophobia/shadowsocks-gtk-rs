//! This module contains code that defines the entire GUI application,
//! and holds all the GUI components.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use clap::ArgMatches;
use log::{error, warn};

use crate::{
    gui::tray,
    io::{app_state::AppState, config_loader::ConfigFolder},
    profile_manager::ProfileManager,
    util,
};

use super::{backlog::BacklogWindow, tray::TrayItem};

#[derive(Debug)]
pub struct GTKApp {
    app_state_path: PathBuf,
    profile_manager: Arc<RwLock<ProfileManager>>,

    // permanent GUI components
    tray: TrayItem,

    // optionally opened windows
    backlog: Option<BacklogWindow>,
}

impl GTKApp {
    /// Construct the application.
    pub fn new(clap_matches: &ArgMatches, config_folder: &ConfigFolder) -> Self {
        let app_state_path = clap_matches.value_of("app-state-path").unwrap().into(); // clap sets default
        let pm_arc = {
            let previous_state = AppState::from_file(&app_state_path).unwrap(); // Ok guaranteed by clap validator
            let pm = ProfileManager::resume_from(&previous_state, &config_folder.get_profiles());
            Arc::new(RwLock::new(pm))
        };

        gtk::init().expect("Failed to init GTK");

        let tray = {
            let icon_name = clap_matches.value_of("tray-icon-filename").unwrap(); // clap sets default
            let theme_dir = clap_matches.value_of("icon-theme-dir").and_then(
                |p| match Path::new(p).canonicalize() // AppIndicator requires an absolute path for this
                    {
                        Ok(p) => p.to_str().map(|s| s.to_string()),
                        Err(err) => {
                            warn!("Cannot resolve the specified icon theme directory: {}", err);
                            warn!("Reverting back to system setting - you may get a blank icon");
                            None
                        }
                    },
            );
            tray::build_and_show(config_folder, &icon_name, theme_dir.as_deref(), Arc::clone(&pm_arc))
        };

        Self {
            app_state_path,
            profile_manager: pm_arc,
            tray,
            backlog: None,
        }
    }

    /// Starts the GTK main loop and performs cleanup on exit.
    pub fn run_and_cleanup(&self) {
        gtk::main(); // GTK main loop blocks here

        // cleanup
        let pm_arc = Arc::clone(&self.profile_manager);
        let mut pm = util::rwlock_write(&pm_arc);
        // save app state
        if let Err(err) = pm.snapshot().write_to_file(&self.app_state_path) {
            error!("Failed to save app state: {}", err);
        };
        // stop any running `sslocal` process
        let _ = pm.try_stop();
    }
}
