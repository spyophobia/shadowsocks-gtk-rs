//! This module contains code that defines the entire GUI application,
//! and holds all the GUI components.

use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use clap::ArgMatches;
use crossbeam_channel::{unbounded as unbounded_channel, Receiver, Sender};
use gtk::prelude::*;
use log::{debug, error, info, warn};

#[cfg(feature = "runtime_api")]
use crate::io::runtime_api::{APICommand, APIListener};
use crate::{
    io::{
        app_state::AppState,
        config_loader::{ConfigFolder, ConfigProfile},
    },
    profile_manager::ProfileManager,
    util,
};

use super::{backlog::BacklogWindow, tray::TrayItem, AppEvent};

#[derive(Debug)]
pub enum AppStartError {
    GLibBoolError(glib::BoolError),
    GLibError(glib::Error),
    IOError(std::io::Error),
}

impl Display for AppStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use AppStartError::*;
        match self {
            GLibBoolError(e) => write!(f, "AppStartError-GLibBoolError: {}", e),
            GLibError(e) => write!(f, "AppStartError-GLibError: {}", e),
            IOError(e) => write!(f, "AppStartError-IOError: {}", e),
        }
    }
}

impl From<glib::BoolError> for AppStartError {
    fn from(err: glib::BoolError) -> Self {
        Self::GLibBoolError(err)
    }
}
impl From<glib::Error> for AppStartError {
    fn from(err: glib::Error) -> Self {
        Self::GLibError(err)
    }
}
impl From<std::io::Error> for AppStartError {
    fn from(err: std::io::Error) -> Self {
        Self::IOError(err)
    }
}

#[derive(Debug)]
struct GTKApp {
    // core
    app_state_path: PathBuf,
    config_folder: ConfigFolder,
    profile_manager: Arc<RwLock<ProfileManager>>,
    events_tx: Sender<AppEvent>,
    events_rx: Receiver<AppEvent>,

    // runtime API
    #[cfg(feature = "runtime_api")]
    api_listener: APIListener,
    #[cfg(feature = "runtime_api")]
    api_cmds_tx: Sender<APICommand>,
    #[cfg(feature = "runtime_api")]
    api_cmds_rx: Receiver<APICommand>,

    // permanent GUI components
    tray: TrayItem,

    // optionally opened windows
    backlog_window: Option<BacklogWindow>,
}

impl GTKApp {
    /// Construct the application.
    fn new(clap_matches: &ArgMatches, config_folder: ConfigFolder) -> Result<Self, AppStartError> {
        // init GTK
        gtk::init()?;

        // resume core
        let app_state_path = clap_matches.value_of("app-state-path").unwrap().into(); // clap sets default
        let (events_tx, events_rx) = unbounded_channel();
        let pm_arc = {
            let previous_state = AppState::from_file(&app_state_path).unwrap(); // Ok guaranteed by clap validator
            let pm = ProfileManager::resume_from(&previous_state, &config_folder, events_tx.clone());
            Arc::new(RwLock::new(pm))
        };

        // start runtime API
        #[cfg(feature = "runtime_api")]
        let (api_listener, api_cmds_tx, api_cmds_rx) = {
            let socket_path = clap_matches.value_of("runtime-api-socket-path").unwrap(); // clap sets default
            let (tx, rx) = unbounded_channel();
            let listener = APIListener::start(socket_path, tx.clone())?;
            (listener, tx, rx)
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
            let mut tray =
                TrayItem::build_and_show(&config_folder, &icon_name, theme_dir.as_deref(), events_tx.clone());
            // set tray state to match profile manager state
            match util::rwlock_read(&pm_arc).current_profile() {
                Some(p) => tray.notify_profile_switch(p.display_name),
                None => tray.notify_sslocal_stop(),
            }
            tray
        };

        Ok(Self {
            app_state_path,
            config_folder,
            profile_manager: pm_arc,
            events_tx,
            events_rx,

            #[cfg(feature = "runtime_api")]
            api_listener,
            #[cfg(feature = "runtime_api")]
            api_cmds_tx,
            #[cfg(feature = "runtime_api")]
            api_cmds_rx,

            tray,

            backlog_window: None,
        })
    }

    /// Show the backlog window, if not already shown.
    fn show_backlog(&mut self) {
        match self.backlog_window.as_ref() {
            Some(w) => {
                debug!("Backlog window already showing; bringing to foreground");
                w.show();
            }
            None => {
                let pm_inner = util::rwlock_read(&self.profile_manager);

                debug!("Opening backlog window");
                let mut window = BacklogWindow::with_backlog(Arc::clone(&pm_inner.backlog), self.events_tx.clone());
                window.pipe(pm_inner.stdout_rx.clone());
                window.pipe(pm_inner.stderr_rx.clone());
                window.show();

                self.backlog_window = Some(window);
            }
        }
    }
    /// Hide the backlog window, if currently showing.
    fn hide_backlog(&mut self) {
        debug!("Closing backlog window");
        drop(self.backlog_window.take());
    }
    /// Restart the `sslocal` instance with the current profile.
    fn restart(&mut self) {
        let current_profile = util::rwlock_read(&self.profile_manager).current_profile();
        match current_profile {
            Some(p) => {
                let name = p.display_name.clone();
                info!("Restarting profile \"{}\"", name);
                let switch_res = util::rwlock_write(&self.profile_manager).switch_to(p);
                if let Err(err) = switch_res {
                    error!("Failed to restart profile \"{}\": {}", name, err);
                }
            }
            None => warn!("Cannot restart because no sslocal instance is running"),
        }
    }
    /// Switch to the specified profile.
    fn switch_profile(&mut self, profile: ConfigProfile) {
        let name = profile.display_name.clone();
        info!("Switching profile to \"{}\"", name);
        let switch_res = util::rwlock_write(&self.profile_manager).switch_to(profile);
        if let Err(err) = switch_res {
            error!("Cannot switch to profile \"{}\": {}", name, err);
        }
    }
    /// Stop the current `sslocal` instance.
    fn stop(&mut self) {
        let mut pm_inner = util::rwlock_write(&self.profile_manager);
        if pm_inner.is_active() {
            info!("Sending stop signal to sslocal");
            let _ = pm_inner.try_stop();
        } else {
            info!("sslocal is not running; nothing to stop");
        }
    }
    /// Quit the application.
    fn quit(&mut self) {
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
        debug!("Closing all optional windows");
        drop(self.backlog_window.take());

        gtk::main_quit();
    }

    /// Handles the queued incoming GUI events.
    fn handle_gui_events(&mut self) {
        use AppEvent::*;
        // using `while let` rather than `for` due to borrow checker issue
        while let Some(event) = self.events_rx.try_iter().next() {
            match event {
                BacklogShow => self.show_backlog(),
                BacklogHide => self.hide_backlog(),
                SwitchProfile(p) => self.switch_profile(p),
                ManualStop => self.stop(),
                Quit => self.quit(),

                OkStop => {
                    // this event could be received because an old instance is stopped
                    // and a new one is started, therefore we first check for active instance
                    if !util::rwlock_read(&self.profile_manager).is_active() {
                        self.tray.notify_sslocal_stop();
                    }
                }
                ErrorStop => {
                    self.tray.notify_sslocal_stop();
                }
            }
        }
    }

    /// Handles the queued incoming runtime API commands.
    #[cfg(feature = "runtime_api")]
    fn handle_api_commands(&mut self) {
        use APICommand::*;
        // using `while let` rather than `for` due to borrow checker issue
        while let Some(cmd) = self.api_cmds_rx.try_iter().next() {
            match cmd {
                BacklogShow => self.show_backlog(),
                BacklogHide => self.hide_backlog(),
                Restart => self.restart(),
                SwitchProfile(name) => match self.config_folder.lookup(&name).cloned() {
                    Some(p) => {
                        self.switch_profile(p);
                        self.tray.notify_profile_switch(&name);
                    }
                    None => error!("Cannot find a profile named \"{}\"; did nothing", name),
                },
                Stop => {
                    self.stop();
                    self.tray.notify_sslocal_stop();
                }
                Quit => self.quit(),
            }
        }
    }
}

/// Initialise all components and start the GTK main loop.
pub fn run(clap_matches: &ArgMatches, config_folder: ConfigFolder) -> Result<(), AppStartError> {
    // init app
    let mut app = GTKApp::new(clap_matches, config_folder)?;

    // starts event listeners
    let loop_action_id = glib::timeout_add_local(
        Duration::from_millis(10), // 100fps
        move || {
            app.handle_gui_events();

            #[cfg(feature = "runtime_api")]
            app.handle_api_commands();

            Continue(true)
        },
    );

    // start GTK main loop
    gtk::main(); // blocks until `gtk::main_quit` is called

    // cleanup
    // this is necessary because `app` was moved into the closure
    // and it needs to be dropped for its members to be dropped (hence cleaned up)
    glib::source_remove(loop_action_id);

    Ok(())
}
