//! This module contains code that defines the entire GUI application,
//! and holds all the GUI components.

use std::{
    fmt, io,
    path::PathBuf,
    process,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use crossbeam_channel::{unbounded as unbounded_channel, Receiver, Sender};
use gtk::prelude::*;
use log::{debug, error, info, trace, warn};

#[cfg(feature = "runtime-api")]
use shadowsocks_gtk_rs::runtime_api_msg::APICommand;
use shadowsocks_gtk_rs::{
    notify_method::NotifyMethod,
    util::{self, mutex_lock},
};

#[cfg(feature = "runtime-api")]
use crate::io::runtime_api::APIListener;
use crate::{
    clap_def::CliArgs,
    event::AppEvent,
    io::{
        app_state::AppState,
        profile_loader::{Profile, ProfileFolder, ProfileLoadError},
    },
    profile_manager::ProfileManager,
};

use super::{
    log_viewer::LogViewerWindow,
    notification::{notify, Level},
    tray::TrayItem,
};

#[derive(Debug)]
pub enum AppStartError {
    ProfileLoadError(ProfileLoadError),
    CtrlCError(ctrlc::Error),
    GLibBoolError(glib::BoolError),
    GLibError(glib::Error),
    IOError(io::Error),
}

impl fmt::Display for AppStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AppStartError::*;
        let prefix = "AppStartError";
        match self {
            ProfileLoadError(e) => write!(f, "{}-ProfileLoadError: {}", prefix, e),
            CtrlCError(e) => write!(f, "{}-CtrlCError: {}", prefix, e),
            GLibBoolError(e) => write!(f, "{}-GLibBoolError: {}", prefix, e),
            GLibError(e) => write!(f, "{}-GLibError: {}", prefix, e),
            IOError(e) => write!(f, "{}-IOError: {}", prefix, e),
        }
    }
}

impl From<ProfileLoadError> for AppStartError {
    fn from(err: ProfileLoadError) -> Self {
        Self::ProfileLoadError(err)
    }
}
impl From<ctrlc::Error> for AppStartError {
    fn from(err: ctrlc::Error) -> Self {
        Self::CtrlCError(err)
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
impl From<io::Error> for AppStartError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}

#[derive(Debug)]
struct GTKApp {
    // core
    app_state_path: PathBuf,
    profile_folder: ProfileFolder,
    profile_manager: Arc<RwLock<ProfileManager>>,
    events_tx: Sender<AppEvent>,
    events_rx: Receiver<AppEvent>,

    // runtime API
    #[cfg(feature = "runtime-api")]
    #[allow(dead_code)]
    api_listener: APIListener, // this needs to be stored to be kept alive
    #[cfg(feature = "runtime-api")]
    api_cmds_rx: Receiver<APICommand>,

    // GUI components
    tray: TrayItem,
    log_viewer_window: Option<LogViewerWindow>,

    // misc
    notify_method: NotifyMethod,
}

impl GTKApp {
    /// Construct the application.
    fn new(args: &CliArgs) -> Result<Self, AppStartError> {
        let CliArgs {
            profiles_dir,
            app_state_path,
            tray_icon_filename,
            icon_theme_dir,
            verbose: _,
            quiet: _,
            #[cfg(feature = "runtime-api")]
            runtime_api_socket_path,
        } = args;

        // init GTK
        gtk::init()?;

        // load profiles
        let profile_folder = ProfileFolder::from_path_recurse(profiles_dir)?;
        debug!(
            "Successfully loaded {} profiles in total",
            profile_folder.profile_count()
        );

        // load app state
        let previous_state = {
            let state_res = AppState::from_file(app_state_path);
            if let Err(ref err) = state_res {
                warn!("Failed to load saved app state: {}", err);
            }
            state_res.unwrap_or_default()
        };

        // resume core
        let (events_tx, events_rx) = unbounded_channel();
        let pm_arc = {
            let pm = ProfileManager::resume_from(&previous_state, &profile_folder, events_tx.clone());
            Arc::new(RwLock::new(pm))
        };

        // start runtime API
        #[cfg(feature = "runtime-api")]
        let (api_listener, api_cmds_rx) = {
            let (tx, rx) = unbounded_channel();
            let listener = APIListener::start(runtime_api_socket_path, tx)?;
            (listener, rx)
        };

        // build permanent GUI components
        let tray = {
            let mut tray = TrayItem::build_and_show(
                &tray_icon_filename,
                icon_theme_dir.as_deref(),
                events_tx.clone(),
                &profile_folder,
                previous_state.notify_method,
            );
            // set tray state to match profile manager state
            match util::rwlock_read(&pm_arc).current_profile() {
                Some(p) => tray.notify_profile_switch(p.metadata.display_name),
                None => tray.notify_sslocal_stop(),
            }
            tray
        };

        Ok(Self {
            app_state_path: app_state_path.clone(),
            profile_folder,
            profile_manager: pm_arc,
            events_tx,
            events_rx,

            #[cfg(feature = "runtime-api")]
            api_listener,
            #[cfg(feature = "runtime-api")]
            api_cmds_rx,

            tray,
            log_viewer_window: None,

            notify_method: previous_state.notify_method,
        })
    }

    /// Export the current application state.
    pub fn snapshot(&self) -> AppState {
        let pm = util::rwlock_read(&self.profile_manager);
        let most_recent_profile = pm.current_profile().map_or("".into(), |p| p.metadata.display_name);
        AppState {
            most_recent_profile,
            restart_limit: pm.restart_limit,
            notify_method: self.notify_method,
        }
    }

    /// Show the log viewer window, if not already shown.
    fn show_log_viewer(&mut self) {
        match self.log_viewer_window.as_ref() {
            Some(w) => {
                debug!("Log viewer window already showing; bringing to foreground");
                w.show();
            }
            None => {
                let pm_inner = util::rwlock_read(&self.profile_manager);
                let events_tx = self.events_tx.clone();
                let backlog = mutex_lock(&pm_inner.backlog).clone();
                let log_listener = pm_inner.new_listener();

                debug!("Opening log viewer window.");
                let window = LogViewerWindow::new(events_tx, backlog, log_listener);
                window.show();

                self.log_viewer_window = Some(window);
            }
        }
    }
    /// Drop the log viewer window without emitting an extra close event.
    ///
    /// Useful when the window has already been closed by an external source
    /// and we only need to drop the object.
    fn drop_log_viewer(&mut self) {
        match self.log_viewer_window.take() {
            None => debug!("Log viewer window is None; nothing to drop"),
            some => {
                debug!("Dropping log viewer window");
                drop(some);
            }
        }
    }
    /// Close the log viewer window if currently showing.
    fn close_log_viewer(&mut self) {
        match self.log_viewer_window.take() {
            None => debug!("Log viewer window is None; nothing to close"),
            Some(w) => {
                debug!("Closing log viewer window");
                w.close();
                drop(w);
            }
        }
    }
    /// Set the notification method.
    fn set_notify_method(&mut self, method: NotifyMethod) {
        info!("Setting notify method to {}", method);
        self.notify_method = method;
    }
    /// Restart the `sslocal` instance with the current profile.
    fn restart(&mut self) {
        match util::rwlock_read(&self.profile_manager).current_profile() {
            Some(p) => {
                let name = p.metadata.display_name.clone();
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
    fn switch_profile(&mut self, profile: Profile) {
        let name = profile.metadata.display_name.clone();
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
        // save app state
        match self.snapshot().write_to_file(&self.app_state_path) {
            Ok(_) => info!("App state saved to {:?}", self.app_state_path),
            Err(err) => error!("Failed to save app state: {}", err),
        };
        // stop any running `sslocal` process
        let _ = util::rwlock_write(&self.profile_manager).try_stop();

        // drop all optional windows
        debug!("Closing all optional windows");
        drop(self.log_viewer_window.take());

        gtk::main_quit();
    }

    /// Handles the queued incoming app events.
    fn handle_app_events(&mut self) {
        use AppEvent::*;
        // using `while let` rather than `for` due to borrow checker issue
        while let Some(event) = self.events_rx.try_iter().next() {
            trace!("Received an AppEvent: {:?}", event);
            match event {
                LogViewerShow => self.show_log_viewer(),
                LogViewerHide => self.drop_log_viewer(),
                SwitchProfile(p) => self.switch_profile(p),
                ManualStop => self.stop(),
                SetNotify(method) => self.set_notify_method(method),
                Quit => self.quit(),

                OkStop { instance_name } => {
                    // this event could be received because an old instance is stopped
                    // and a new one is started, therefore we first check for active instance
                    if !util::rwlock_read(&self.profile_manager).is_active() {
                        self.tray.notify_sslocal_stop();
                        let text_2 = format!("An instance has stopped: {}", instance_name.unwrap_or("None".into()));
                        notify(self.notify_method, Level::Warn, "Auto-restart Stopped", text_2);
                    }
                }
                ErrorStop { instance_name, err } => {
                    self.tray.notify_sslocal_stop();
                    let text_2 = format!(
                        "An instance has errored: {}\n{}",
                        instance_name.unwrap_or("None".into()),
                        err
                    );
                    notify(self.notify_method, Level::Error, "Auto-restart Stopped", text_2);
                }
            }
        }
    }

    /// Handles the queued incoming runtime API commands.
    #[cfg(feature = "runtime-api")]
    fn handle_api_commands(&mut self) {
        use APICommand::*;
        // using `while let` rather than `for` due to borrow checker issue
        while let Some(cmd) = self.api_cmds_rx.try_iter().next() {
            match cmd {
                LogViewerShow => self.show_log_viewer(),
                LogViewerHide => self.close_log_viewer(),
                SetNotify(method) => {
                    self.set_notify_method(method);
                    self.tray.notify_notify_method_change(method);
                }

                Restart => self.restart(),
                SwitchProfile(name) => match self.profile_folder.lookup(&name).cloned() {
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

/// Initialize all components and start the GTK main loop.
pub fn run(args: &CliArgs) -> Result<(), AppStartError> {
    // init app
    let mut app = GTKApp::new(args)?;

    // catch signals for soft shutdown
    let shutdown_trigger_count = Arc::new(Mutex::new(0usize));
    let events_tx = app.events_tx.clone();
    ctrlc::set_handler(move || {
        let mut count = util::mutex_lock(&shutdown_trigger_count);
        match *count {
            0 => {
                info!("Signal received, sending Quit event");
                if let Err(_) = events_tx.send(AppEvent::Quit) {
                    error!("Trying to send Quit event for soft shutdown, but all receivers have hung up");
                    error!("Performing hard shutdown; the app state may be unsaved");
                    process::exit(0);
                }
            }
            1 => warn!("Send one more signal for hard shutdown"),
            _ => {
                warn!("Performing hard shutdown; the app state may be unsaved");
                process::exit(0);
            }
        }
        *count += 1;
    })?;

    // starts looping event listeners
    let loop_action_id = glib::timeout_add_local(
        Duration::from_millis(10), // 100fps
        move || {
            app.handle_app_events();

            #[cfg(feature = "runtime-api")]
            app.handle_api_commands();

            Continue(true)
        },
    );

    // start GTK main loop
    info!("Application started");
    gtk::main(); // blocks until `gtk::main_quit` is called

    // cleanup
    // this is necessary because `app` was moved into the closure
    // and it needs to be dropped for its members to be dropped (hence cleaned up)
    loop_action_id.remove();

    Ok(())
}
