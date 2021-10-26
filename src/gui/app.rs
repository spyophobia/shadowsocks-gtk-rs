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
use log::{debug, error, info, warn};

use crate::{
    io::{app_state::AppState, config_loader::ConfigFolder},
    profile_manager::ProfileManager,
    util,
};

use super::{backlog::BacklogWindow, tray::TrayItem, AppEvent};

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
        let (events_tx, events_rx) = unbounded_channel();
        let pm_arc = {
            let previous_state = AppState::from_file(&app_state_path).unwrap(); // Ok guaranteed by clap validator
            let pm = ProfileManager::resume_from(&previous_state, &config_folder.get_profiles(), events_tx.clone());
            Arc::new(RwLock::new(pm))
        };

        // build permanent GUI components
        let tray = {
            let icon_name = clap_matches.value_of("tray-icon-filename").unwrap(); // clap sets default
            let theme_dir = clap_matches.value_of("icon-theme-dir").and_then(
                // AppIndicator requires an absolute path for this
                |p| match Path::new(p).canonicalize() {
                    Ok(abs) => abs.to_str().map(|s| s.to_string()),
                    Err(err) => {
                        warn!("Cannot resolve the specified icon theme directory: {}", err);
                        warn!("Reverting back to system setting - you may get a blank icon");
                        None
                    }
                },
            );
            TrayItem::build_and_show(config_folder, &icon_name, theme_dir.as_deref(), events_tx.clone())
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

    /// Handles the queued incoming events.
    fn handle_events(&mut self) {
        use AppEvent::*;
        for event in self.events_rx.try_iter() {
            match event {
                BacklogShow => match self.backlog_window.as_ref() {
                    Some(w) => {
                        debug!("Backlog window already showing; bringing to foreground");
                        w.show();
                    }
                    None => {
                        let pm_inner = util::rwlock_read(&self.profile_manager);

                        debug!("Opening backlog window");
                        let mut window =
                            BacklogWindow::with_backlog(Arc::clone(&pm_inner.backlog), self.events_tx.clone());
                        window.pipe(pm_inner.stdout_rx.clone());
                        window.pipe(pm_inner.stderr_rx.clone());
                        window.show();

                        self.backlog_window = Some(window);
                    }
                },
                BacklogHide => {
                    debug!("Closing backlog window");
                    drop(self.backlog_window.take());
                }
                SwitchProfile(p) => {
                    let name = p.display_name.clone();
                    info!("Switching profile to \"{}\"", name);
                    let switch_res = util::rwlock_write(&self.profile_manager).switch_to(p);
                    if let Err(err) = switch_res {
                        error!("Cannot switch to profile \"{}\": {}", name, err);
                    }
                }
                ManualStop => {
                    let mut pm_inner = util::rwlock_write(&self.profile_manager);
                    if pm_inner.is_active() {
                        info!("Sending stop signal to sslocal");
                        let _ = pm_inner.try_stop();
                    } else {
                        info!("sslocal is not running; nothing to stop");
                    }
                }
                Quit => {
                    info!("Quit");

                    // cleanup
                    let mut pm = util::rwlock_write(&self.profile_manager);
                    // save app state
                    match pm.snapshot().write_to_file(&self.app_state_path) {
                        Ok(_) => info!("App state saved to {:?}", self.app_state_path),
                        Err(err) => error!("Failed to save app state: {}", err),
                    };
                    // stop any running `sslocal` process
                    let _ = pm.try_stop();

                    // drop all optional windows
                    drop(self.backlog_window.take());

                    gtk::main_quit();
                }
                OkStop => {
                    // this event could be received because an old instance is stopped
                    // and a new one is started, therefore we first check for active instance
                    if !util::rwlock_read(&self.profile_manager).is_active() {
                        debug!("Setting tray to stopped state");
                        self.tray.notify_sslocal_stop();
                    }
                }
                ErrorStop => {
                    debug!("Setting tray to stopped state");
                    self.tray.notify_sslocal_stop();
                }
            }
        }
    }
}

/// Initialise all components and start the GTK main loop.
pub fn run(clap_matches: &ArgMatches, config_folder: &ConfigFolder) {
    // init app
    let mut app = GTKApp::new(clap_matches, config_folder);

    // starts event listener
    glib::source::timeout_add_local(
        Duration::from_millis(10), // 100fps
        move || {
            app.handle_events();
            Continue(true)
        },
    );

    // start GTK main loop
    gtk::main(); // blocks until `gtk::main_quit` is called
}
