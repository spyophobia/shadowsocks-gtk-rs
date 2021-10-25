//! This module contains code that defines the entire GUI application,
//! and holds all the GUI components.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use clap::ArgMatches;
use crossbeam_channel::{unbounded as unbounded_channel, Receiver, Sender};
use gtk::prelude::*;
use log::{error, info, warn};

use crate::{
    gui::tray,
    io::{app_state::AppState, config_loader::ConfigFolder},
    profile_manager::ProfileManager,
    util,
};

use super::{backlog::BacklogWindow, event::AppEvent, tray::TrayItem};

#[derive(Debug)]
struct GTKApp {
    // core
    app_state_path: PathBuf,
    profile_manager: Arc<RwLock<ProfileManager>>,
    events_tx: Sender<AppEvent>,
    events_rx: Receiver<AppEvent>,

    // permanent GUI components
    tray: TrayItem,

    // optionally opened windows
    backlog_window: Option<BacklogWindow>,
}

impl GTKApp {
    /// Construct the application.
    fn new(clap_matches: &ArgMatches, config_folder: &ConfigFolder) -> Self {
        // init GTK
        gtk::init().expect("Failed to init GTK");

        // resume core
        let app_state_path = clap_matches.value_of("app-state-path").unwrap().into(); // clap sets default
        let pm_arc = {
            let previous_state = AppState::from_file(&app_state_path).unwrap(); // Ok guaranteed by clap validator
            let pm = ProfileManager::resume_from(&previous_state, &config_folder.get_profiles());
            Arc::new(RwLock::new(pm))
        };
        let (events_tx, events_rx) = unbounded_channel();

        // build permanent GUI components
        let tray = {
            let icon_name = clap_matches.value_of("tray-icon-filename").unwrap(); // clap sets default
            let theme_dir = clap_matches.value_of("icon-theme-dir").and_then(
                |p| match Path::new(p).canonicalize() // AppIndicator requires an absolute path for this
                            {
                                Ok(abs) => abs.to_str().map(|s| s.to_string()),
                                Err(err) => {
                                    warn!("Cannot resolve the specified icon theme directory: {}", err);
                                    warn!("Reverting back to system setting - you may get a blank icon");
                                    None
                                }
                            },
            );
            tray::build_and_show(config_folder, &icon_name, theme_dir.as_deref(), events_tx.clone())
        };

        Self {
            app_state_path,
            profile_manager: pm_arc,
            events_tx,
            events_rx,
            tray,
            backlog_window: None,
        }
    }
}

/// Initialise all components, start the GTK main loop, and perform cleanup on exit.
pub fn run(clap_matches: &ArgMatches, config_folder: &ConfigFolder) {
    // init app
    let mut app = GTKApp::new(clap_matches, config_folder);

    // references used during cleanup
    let profile_manager = Arc::clone(&app.profile_manager);

    // starts event listener
    glib::source::timeout_add_local(
        Duration::from_micros(16_666), // 60fps
        move || {
            use AppEvent::*;
            for ev in app.events_rx.try_iter() {
                match ev {
                    BacklogShow => match app.backlog_window {
                        Some(_) => info!("Backlog window already showing"),
                        None => {
                            let pm_inner = util::rwlock_read(&app.profile_manager);
                            let backlog = util::mutex_lock(&pm_inner.backlog);

                            info!("Opening backlog window");
                            let mut window = BacklogWindow::with_backlog(&backlog, app.events_tx.clone());
                            window.pipe(pm_inner.stdout_rx.clone());
                            window.pipe(pm_inner.stderr_rx.clone());
                            window.show();

                            app.backlog_window = Some(window);
                        }
                    },
                    BacklogHide => {
                        info!("Closing backlog window");
                        drop(app.backlog_window.take());
                    }
                    SwitchProfile(p) => {
                        let name = p.display_name.clone();
                        info!("Switching profile to \"{}\"", name);
                        let switch_res = util::rwlock_write(&app.profile_manager).switch_to(p);
                        if let Err(err) = switch_res {
                            error!("Cannot switch to profile \"{}\": {}", name, err);
                        }
                    }
                    Stop => {
                        let mut pm_inner = util::rwlock_write(&app.profile_manager);
                        if pm_inner.is_active() {
                            info!("Sending stop signal to sslocal");
                            let _ = pm_inner.try_stop();
                        } else {
                            info!("sslocal is not running; nothing to stop");
                        }
                    }
                    Quit => {
                        info!("Quit");
                        gtk::main_quit();
                    }
                }
            }
            Continue(true)
        },
    );

    // start GTK main loop
    gtk::main(); // blocks until `gtk::main_quit` is called

    // cleanup
    let mut pm = util::rwlock_write(&profile_manager);
    // save app state
    if let Err(err) = pm.snapshot().write_to_file(&app.app_state_path) {
        error!("Failed to save app state: {}", err);
    };
    // stop any running `sslocal` process
    let _ = pm.try_stop();
}
