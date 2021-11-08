//! This module contains code that handles profile switching and automatic restarting.

use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    process::{Child, ExitStatus, Stdio},
    sync::{Arc, Mutex, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::{unbounded as unbounded_channel, Receiver, Sender};
use log::{debug, error, info, trace, warn};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use shadowsocks_gtk_rs::util::{
    self,
    leaky_bucket::{NaiveLeakyBucket, NaiveLeakyBucketConfig},
};

use crate::{
    event::AppEvent,
    io::{
        app_state::AppState,
        config_loader::{ConfigFolder, ConfigProfile},
    },
};

/// Represents a currently running `sslocal` instance, storing the relevant information
/// for its subprocess(es).
///
/// Automatically kills `sslocal` when dropped.
#[derive(Debug)]
struct ActiveSSInstance {
    /// Ownership instead of reference due to need for restart.
    profile: ConfigProfile,
    /// We store PID separately because we need it even if the handle is consumed.
    sslocal_pid: Pid,
    /// The handle of the subprocess; wrapped in `Option` because it could be
    /// consumed by an exit monitor daemon.
    sslocal_process: Option<Child>,
    /// The daemon threads that need to be cleanup up when deactivating.
    daemon_handles: Vec<JoinHandle<()>>,
}

impl fmt::Display for ActiveSSInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(PID: {}, Profile: {})", self.sslocal_pid, self.profile.display_name)
    }
}

impl Drop for ActiveSSInstance {
    /// Kill the `sslocal` child process when going out of scope.
    ///
    /// Also cleans up all daemon threads.
    fn drop(&mut self) {
        let self_name = self.to_string();

        trace!("Instance {} is getting dropped", self_name);

        // send stop signal to `sslocal` process
        // could return Err if `sslocal` already exited
        let _ = signal::kill(self.sslocal_pid, Signal::SIGINT);

        // sleep for a short time to allow `sslocal` to exit fully
        thread::sleep(Duration::from_millis(100));

        // make sure all daemon threads finish
        for handle in self.daemon_handles.drain(..) {
            if let Err(err) = handle.join() {
                warn!(
                    "A daemon of sslocal instance {} panicked unexpectedly: {:?}",
                    self_name, err
                );
            };
        }
    }
}

impl ActiveSSInstance {
    fn new(new_profile: ConfigProfile) -> io::Result<Self> {
        // start `sslocal` subprocess
        let proc = new_profile.run_sslocal(Some(Stdio::piped()), Some(Stdio::piped()))?;
        let sslocal_pid = Pid::from_raw(proc.id() as i32);

        Ok(Self {
            profile: new_profile,
            sslocal_pid,
            sslocal_process: Some(proc),
            daemon_handles: vec![],
        })
    }

    /// The common implementation for `Self::pipe_stdout` & `Self::pipe_stderr`.
    ///
    /// Do not use directly.
    fn pipe_any_impl<R>(&mut self, source: R, source_type: &'static str, tx: Sender<String>) -> io::Result<()>
    where
        R: Read + Send + 'static,
    {
        let source = BufReader::new(source);
        let self_name = self.to_string();
        let handle = thread::Builder::new()
            .name(format!("{} piper daemon for instance {}", source_type, self_name))
            .spawn(move || {
                for line_res in source.lines() {
                    let line = {
                        let raw = line_res.unwrap_or_else(|err| format!("Error reading {}: {}", &source_type, err));
                        format!("[{}] {}\n", source_type, raw)
                    };
                    // try to send through channel
                    if let Err(err) = tx.send(line.clone()) {
                        warn!(
                            "Instance {} wrote to {}, but all receivers have hung up. Piper daemon stopping.",
                            self_name, source_type
                        );
                        warn!("Last line written was \"{}\".", err.0);
                        break;
                    }
                }
                // thread exits when the source is closed
            })?;
        self.daemon_handles.push(handle);
        Ok(())
    }
    /// Pipe all lines of `sslocal`'s `stdout` to a channel.
    fn pipe_stdout(&mut self, tx: Sender<String>) -> io::Result<()> {
        let proc = self.sslocal_process.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Cannot pipe stdout; process handle already consumed",
            )
        })?;
        let stdout = proc.stdout.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Cannot pipe stdout; ChildStdout stream already consumed",
            )
        })?;
        self.pipe_any_impl(stdout, "stdout", tx)
    }
    /// Pipe all lines of `sslocal`'s `stderr` to a channel.
    fn pipe_stderr(&mut self, tx: Sender<String>) -> io::Result<()> {
        let proc = self.sslocal_process.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Cannot pipe stderr; process handle already consumed",
            )
        })?;
        let stderr = proc.stderr.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Cannot pipe stderr; ChildStderr stream already consumed",
            )
        })?;
        self.pipe_any_impl(stderr, "stderr", tx)
    }

    /// Starts a monitoring thread that waits for the underlying `sslocal`
    /// to terminate, when it will emit its `ExitStatus` via the returned channel.
    ///
    /// This will consume `sslocal`'s process handle, so make sure to
    /// set up `stdout` & `stderr` piping first.
    fn alert_on_exit(&mut self) -> io::Result<Receiver<ExitStatus>> {
        let self_name = self.to_string();
        let mut proc = self
            .sslocal_process
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "sslocal process handle already consumed"))?;
        let (exit_tx, exit_rx) = unbounded_channel();
        let handle = thread::Builder::new()
            .name(format!("exit alert daemon for instance {}", self_name))
            .spawn(move || {
                let status = proc.wait().unwrap(); // process already running for sure
                if let Err(err) = exit_tx.send(status) {
                    warn!(
                        "Instance {} exit detected: {}, but the receiver has hung up.",
                        self_name, err.0
                    );
                }
            })?;
        self.daemon_handles.push(handle);
        Ok(exit_rx)
    }
}

/// A daemon that manages profile-switching and restarts.
#[derive(Debug)]
pub struct ProfileManager {
    /// Attempt to restart `sslocal` up to this limit before
    /// setting `ProfileManager` to inactive state.
    /// What to do when a `sslocal` instance fails with a non-0 exit code.
    ///
    /// Scenarios in which a restart will not be attempted:
    /// - Limit reached
    /// - `sslocal` instance terminated by a signal
    /// - Various errors which make it impossible for monitoring to continue
    pub restart_limit: NaiveLeakyBucketConfig,
    events_tx: Sender<AppEvent>,
    /// Inner value of `None` means `Self` is inactive.
    active_instance: Arc<RwLock<Option<ActiveSSInstance>>>,

    // pipes for the output of `sslocal`
    /// Receives `sslocal`'s `stdout`.
    stdout_tx: Sender<String>,
    /// Receives `sslocal`'s `stderr`.
    stderr_tx: Sender<String>,
    /// Clone me to handle `sslocal`'s `stdout`.
    pub stdout_rx: Receiver<String>,
    /// Clone me to handle `sslocal`'s `stderr`.
    pub stderr_rx: Receiver<String>,

    /// A string holding the combined backlog history of `stdout` & `stderr`.
    pub backlog: Arc<Mutex<String>>,

    /// The daemon threads that need to be cleanup up when deactivating.
    daemon_handles: Vec<JoinHandle<()>>,
}

impl Drop for ProfileManager {
    /// Halts any active `sslocal` instance when going out of scope.
    ///
    /// Also cleans up all daemon threads.
    fn drop(&mut self) {
        trace!("ProfileManager is getting dropped");

        // deactivate `sslocal` instance
        let _ = self.try_stop();

        // make sure all daemon threads finish
        for handle in self.daemon_handles.drain(..) {
            if let Err(err) = handle.join() {
                warn!("A daemon of ProfileManager panicked unexpectedly: {:?}", err);
            };
        }
    }
}

impl ProfileManager {
    pub fn new(restart_limit: NaiveLeakyBucketConfig, events_tx: Sender<AppEvent>) -> Self {
        let (stdout_tx, stdout_rx) = unbounded_channel();
        let (stderr_tx, stderr_rx) = unbounded_channel();
        Self {
            restart_limit,
            events_tx,
            active_instance: RwLock::new(None).into(),
            stdout_tx,
            stderr_tx,
            stdout_rx,
            stderr_rx,
            backlog: Mutex::new(String::new()).into(),
            daemon_handles: vec![],
        }
    }

    /// Resume from a previously saved state.
    pub fn resume_from(state: &AppState, profiles: &ConfigFolder, events_tx: Sender<AppEvent>) -> Self {
        let mut pm = Self::new(state.restart_limit, events_tx);
        match state.most_recent_profile.as_str() {
            "" => debug!("Most recent profile is none; will not attempt to resume"),
            name => match profiles.lookup(name) {
                Some(p) => match pm.switch_to(p.clone()) {
                    Ok(_) => info!("Successfully resumed with profile \"{}\"", name),
                    Err(err) => error!("Cannot resume - switch to profile \"{}\" failed: {}", name, err),
                },
                None => warn!("Cannot resume - profile \"{}\" not found", name),
            },
        };
        pm
    }

    /// Indicate whether a `sslocal` instance is currently running.
    pub fn is_active(&self) -> bool {
        util::rwlock_read(&self.active_instance).is_some()
    }

    /// Get the profile of the currently active instance.
    pub fn current_profile(&self) -> Option<ConfigProfile> {
        util::rwlock_read(&self.active_instance)
            .as_ref()
            .map(|instance| instance.profile.clone())
    }

    /// Start a `sslocal` instance with a new profile, replacing the old one if necessary.
    ///
    /// Returns `Ok(())` if and only if the new instance starts successfully and the old one is cleaned up.
    ///
    /// If the new instance fails to start, this `ProfileManager` will be left in deactivated state.
    pub fn switch_to(&mut self, new_profile: ConfigProfile) -> io::Result<()> {
        // deactivate the old instance
        let _ = self.try_stop();

        // activate the new instance
        let mut new_instance = ActiveSSInstance::new(new_profile)?;

        // pipe `sslocal`'s `stdout` & `stderr`
        new_instance.pipe_stdout(self.stdout_tx.clone())?;
        new_instance.pipe_stderr(self.stderr_tx.clone())?;

        // monitor for failure
        let exit_alert_rx = new_instance.alert_on_exit()?;

        // set
        *util::rwlock_write(&self.active_instance) = Some(new_instance);

        // monitor
        self.set_on_fail(exit_alert_rx)?;

        Ok(())
    }

    /// Starts a monitoring thread that waits for the underlying `sslocal` instance
    /// to fail, when it will attempt to perform the action specified by `Self::on_fail`.
    pub fn set_on_fail(&mut self, listener: Receiver<ExitStatus>) -> io::Result<()> {
        // variables that need to be moved into thread
        let restart_limit = self.restart_limit;
        let events_tx = self.events_tx.clone();
        let instance = Arc::clone(&self.active_instance);
        let profile = self
            .current_profile()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Not active"))?;
        let stdout_tx = self.stdout_tx.clone();
        let stderr_tx = self.stderr_tx.clone();

        // create thread
        let handle = thread::Builder::new()
            .name("ProfileManager failure monitor daemon".into())
            .spawn(move || {
                // profile stays the same across restarts, therefore outside of loop
                let profile_name = profile.display_name.clone();
                let mut exit_listener = listener; // is set to new listener in every iteration
                let mut restart_counter: NaiveLeakyBucket = restart_limit.into();
                // restart loop can exit for a variety of reasons; see code
                loop {
                    let instance_name = match &*util::rwlock_read(&instance) {
                        Some(inst) => inst.to_string(),
                        None => {
                            debug!("ProfileManager has been set to inactive; auto-restart stopped");
                            if let Err(_) = events_tx.send(AppEvent::OkStop { instance_name: None }) {
                                error!("Trying to send OkStop event, but all receivers have hung up.");
                            }
                            break;
                        }
                    };

                    // wait for `sslocal` instance exit signal
                    match exit_listener.recv() {
                        Ok(status) if status.success() => {
                            // most likely because `ActiveInstance` gets dropped
                            // causing `sslocal` to exit gracefully,
                            // or if the user calls `sslocal --version` or something
                            debug!(
                                "Instance {} has exited successfully; auto-restart stopped",
                                instance_name
                            );
                            if let Err(_) = events_tx.send(AppEvent::OkStop {
                                instance_name: Some(instance_name),
                            }) {
                                error!("Trying to send OkStop event, but all receivers have hung up.");
                            }
                            break;
                        }
                        Err(err) => {
                            // we no longer know the status of `sslocal`, so fail fast
                            error!(
                                "The exit alert daemon for instance {} has hung up: {}; auto-restart stopped",
                                instance_name, err
                            );
                            if let Err(_) = events_tx.send(AppEvent::ErrorStop {
                                instance_name: Some(instance_name),
                                err: err.to_string(),
                            }) {
                                error!("Trying to send ErrorStop event, but all receivers have hung up.");
                            }
                            break;
                        }
                        Ok(bad_status) => {
                            // do restart
                            warn!("Instance {} has failed; restarting", instance_name);
                            warn!("Exit status: {}", bad_status);
                        }
                    }

                    // Check if restart counter has overflowed
                    if let Err(err) = restart_counter.push() {
                        error!(
                            "sslocal exits excessively with profile \"{}\"; auto-restart stopped",
                            profile_name
                        );
                        error!("{}", err);
                        if let Err(_) = events_tx.send(AppEvent::ErrorStop {
                            instance_name: Some(instance_name),
                            err: err.to_string(),
                        }) {
                            error!("Trying to send ErrorStop event, but all receivers have hung up.");
                        }
                        break;
                    }

                    // Restart
                    /// Temporary helper builder function to simplify error handling.
                    fn start_pipe_alert(
                        profile: ConfigProfile,
                        stdout_tx: Sender<String>,
                        stderr_tx: Sender<String>,
                        exit_listener: &mut Receiver<ExitStatus>,
                    ) -> io::Result<ActiveSSInstance> {
                        let mut instance = ActiveSSInstance::new(profile)?;
                        instance.pipe_stdout(stdout_tx)?;
                        instance.pipe_stderr(stderr_tx)?;
                        *exit_listener = instance.alert_on_exit()?;
                        Ok(instance)
                    }

                    let new_instance = match start_pipe_alert(
                        profile.clone(),
                        stdout_tx.clone(),
                        stderr_tx.clone(),
                        &mut exit_listener,
                    ) {
                        Ok(p) => p,
                        Err(err) => {
                            error!(
                                "Failed to restart with profile \"{}\": {}. Failure monitor daemon stopping",
                                profile_name, err
                            );
                            if let Err(_) = events_tx.send(AppEvent::ErrorStop {
                                instance_name: Some(instance_name),
                                err: err.to_string(),
                            }) {
                                error!("Trying to send ErrorStop event, but all receivers have hung up.");
                            }
                            break;
                        }
                    };

                    // Set new active instance
                    *util::rwlock_write(&instance) = Some(new_instance);
                }
                // loop exit means we should leave ProfileManager inactive
                drop(util::rwlock_write(&instance).take());
            })?;
        self.daemon_handles.push(handle);

        Ok(())
    }

    /// Stop the `sslocal` instance if active.
    ///
    /// Returns `Err(())` if already inactive.
    pub fn try_stop(&mut self) -> Result<(), ()> {
        let instance = util::rwlock_write(&self.active_instance).take();
        instance.map(|_| ()).ok_or(())
        // `sslocal` instance dropped implicitly
    }
}

#[cfg(test)]
mod test {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    use crossbeam_channel::unbounded as unbounded_channel;
    use log::debug;
    use shadowsocks_gtk_rs::util::leaky_bucket::NaiveLeakyBucketConfig;

    use super::*;
    use crate::io::config_loader::ConfigFolder;

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
        let restart_limit = NaiveLeakyBucketConfig::new(3, Duration::from_secs(10));
        let (events_tx, _) = unbounded_channel();
        let mut mgr = ProfileManager::new(restart_limit, events_tx);

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
        let _ = mgr.try_stop();
    }
}