//! This module contains code that handles profile switching and automatic restarting.

use std::{
    fmt::Display,
    io::{self, BufRead, BufReader, Read},
    process::{Child, ExitStatus, Stdio},
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel as cbc;
use log::{debug, error, info, warn};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use serde::{Deserialize, Serialize};

use crate::{
    io::{app_state_manager::AppState, config_loader::ConfigProfile},
    util::{NaiveLeakyBucket, NaiveLeakyBucketConfig},
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

impl Display for ActiveSSInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(PID: {}, Profile: {})",
            self.sslocal_pid,
            self.profile.display_name.as_ref().unwrap() // `display_name` is always set
        )
    }
}

impl Drop for ActiveSSInstance {
    /// Kill the `sslocal` child process when going out of scope.
    ///
    /// Also cleans up all daemon threads.
    fn drop(&mut self) {
        let self_name = self.to_string();

        debug!("Instance {} is getting dropped", self_name);

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
    fn pipe_any_impl<R>(&mut self, source: R, source_type: &'static str, tx: cbc::Sender<String>) -> io::Result<()>
    where
        R: Read + Send + 'static,
    {
        let source = BufReader::new(source);
        let self_name = self.to_string();
        let handle = thread::Builder::new()
            .name(format!("{} piper daemon for instance {}", source_type, self_name))
            .spawn(move || {
                for line_res in source.lines() {
                    let line = line_res.unwrap_or_else(|err| format!("Error reading {}: {:?}", &source_type, err));
                    if let Err(err) = tx.send(line) {
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
    fn pipe_stdout(&mut self, tx: cbc::Sender<String>) -> io::Result<()> {
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
    fn pipe_stderr(&mut self, tx: cbc::Sender<String>) -> io::Result<()> {
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
    fn alert_on_exit(&mut self) -> io::Result<cbc::Receiver<ExitStatus>> {
        let self_name = self.to_string();
        let mut proc = self
            .sslocal_process
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "sslocal process handle already consumed"))?;
        let (exit_tx, exit_rx) = cbc::unbounded();
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

/// What to do when a `sslocal` instance fails with a non-0 exit code.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OnFailure {
    /// Set `ProfileManager` to inactive state on failure.
    Halt { prompt: bool },
    /// Attempt to restart `sslocal`, up to the limit specified by `limit`.
    ///
    /// Scenarios in which a restart will not be attempted:
    /// - Limit reached
    /// - `sslocal` instance terminated by a signal
    /// - Various errors which make it impossible for monitoring to continue
    Restart { limit: NaiveLeakyBucketConfig },
}

/// A daemon that manages profile-switching and restarts.
#[derive(Debug)]
pub struct ProfileManager {
    on_fail: OnFailure,
    /// Inner value of `None` means `Self` is inactive.
    active_instance: Arc<RwLock<Option<ActiveSSInstance>>>,

    // pipes for the output of `sslocal`
    /// Receives `sslocal`'s `stdout`.
    stdout_tx: cbc::Sender<String>,
    /// Receives `sslocal`'s `stderr`.
    stderr_tx: cbc::Sender<String>,
    /// Clone me to handle `sslocal`'s `stdout`.
    pub stdout_rx: cbc::Receiver<String>,
    /// Clone me to handle `sslocal`'s `stderr`.
    pub stderr_rx: cbc::Receiver<String>,

    /// The daemon threads that need to be cleanup up when deactivating.
    daemon_handles: Vec<JoinHandle<()>>,
}

impl Drop for ProfileManager {
    /// Halts any active `sslocal` instance when going out of scope.
    ///
    /// Also cleans up all daemon threads.
    fn drop(&mut self) {
        debug!("ProfileManager is getting dropped");

        // deactivate `sslocal` instance
        let _ = self.stop();

        // make sure all daemon threads finish
        for handle in self.daemon_handles.drain(..) {
            if let Err(err) = handle.join() {
                warn!("A daemon of ProfileManager panicked unexpectedly: {:?}", err);
            };
        }
    }
}

impl ProfileManager {
    pub fn new(on_fail: OnFailure) -> Self {
        let (stdout_tx, stdout_rx) = crossbeam_channel::unbounded();
        let (stderr_tx, stderr_rx) = crossbeam_channel::unbounded();
        Self {
            on_fail,
            active_instance: Arc::new(RwLock::new(None)),
            stdout_tx,
            stderr_tx,
            stdout_rx,
            stderr_rx,
            daemon_handles: vec![],
        }
    }

    pub fn resume_from(state: &AppState, profiles: &[&ConfigProfile]) -> Self {
        let mut pm = Self::new(state.on_fail);
        match state.most_recent_profile.as_str() {
            "" => info!("Most recent profile is none; will not attempt to resume"),
            name => {
                let name_hit = profiles.iter().find(|&&p| p.display_name.as_ref().unwrap() == name);
                match name_hit {
                    Some(&p) => {
                        if let Err(err) = pm.switch_to(p.clone()) {
                            error!("Cannot resume - switch to profile \"{}\" failed: {}", name, err);
                        }
                    }
                    None => warn!("Cannot resume - profile \"{}\" not found", name),
                }
            }
        };
        pm
    }

    /// Indicate whether a `sslocal` instance is currently running.
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active_instance
            .read()
            .unwrap_or_else(|err| {
                warn!("Read lock on active instance poisoned, recovering");
                err.into_inner()
            })
            .is_some()
    }

    /// Get the profile of the currently active instance.
    pub fn current_profile(&self) -> Option<ConfigProfile> {
        self.active_instance
            .read()
            .unwrap_or_else(|err| {
                warn!("Read lock on active instance poisoned, recovering");
                err.into_inner()
            })
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
        let _ = self.stop();

        // activate the new instance
        let mut new_instance = ActiveSSInstance::new(new_profile)?;

        // pipe `sslocal`'s `stdout` & `stderr`
        new_instance.pipe_stdout(self.stdout_tx.clone())?;
        new_instance.pipe_stderr(self.stderr_tx.clone())?;

        // monitor for failure
        let exit_alert = new_instance.alert_on_exit()?;

        // set
        *self.active_instance.write().unwrap_or_else(|err| {
            warn!("Write lock on active instance poisoned, recovering");
            err.into_inner()
        }) = Some(new_instance);

        // monitor
        self.set_on_fail(exit_alert, self.on_fail)?;

        Ok(())
    }

    /// Starts a monitoring thread that waits for the underlying `sslocal` instance
    /// to fail, when it will attempt to perform the action specified by `on_fail`.
    pub fn set_on_fail(&mut self, listener: cbc::Receiver<ExitStatus>, on_fail: OnFailure) -> io::Result<()> {
        // variables that need to be moved into thread
        let instance = Arc::clone(&self.active_instance);
        let profile = self
            .current_profile()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Not active"))?;
        let stdout_tx = self.stdout_tx.clone();
        let stderr_tx = self.stderr_tx.clone();

        // create thread
        let handle = thread::Builder::new()
            .name("ProfileManager failure monitor daemon".into())
            .spawn(move || match on_fail {
                OnFailure::Halt { prompt } => {
                    if prompt {
                        unimplemented!("Prompt user")
                    }

                    // leave ProfileManager inactive
                    drop(
                        instance
                            .write()
                            .unwrap_or_else(|err| {
                                warn!("Write lock on active instance poisoned, recovering");
                                err.into_inner()
                            })
                            .take(),
                    );
                }
                OnFailure::Restart { limit } => {
                    // profile stays the same across restarts, therefore outside of loop
                    let profile_name = profile.display_name.as_ref().unwrap(); // display_name is always set
                    let mut exit_listener = listener; // is set to new listener in every iteration
                    let mut restart_counter: NaiveLeakyBucket = limit.into();
                    // restart loop can exit for a variety of reasons; see code
                    loop {
                        let instance_name = match &*instance.read().unwrap_or_else(|err| {
                            warn!("Read lock on active instance poisoned, recovering");
                            err.into_inner()
                        }) {
                            Some(instance) => instance.to_string(),
                            None => {
                                info!("ProfileManager has been set to inactive; auto-restart stopped");
                                break;
                            }
                        };

                        // wait for `sslocal` instance exit signal
                        match exit_listener.recv() {
                            Ok(status) if status.success() => {
                                // most likely because `ActiveInstance` gets dropped
                                // causing `sslocal` to exit gracefully,
                                // or if the user calls `sslocal --version` or something
                                info!(
                                    "Instance {} has exited successfully; auto-restart stopped",
                                    instance_name
                                );
                                break;
                            }
                            Err(err) => {
                                // we no longer know the status of `sslocal`
                                warn!(
                                    "The exit alert daemon for instance {} has hung up: {}; auto-restart stopped",
                                    instance_name, err
                                );
                                break;
                            }
                            Ok(bad_status) => {
                                // do restart
                                warn!("Instance {} has failed; restarting", instance_name);
                                warn!("Exit status: {}", bad_status);
                            }
                        };

                        // Check if restart counter has overflowed
                        if let Err(err) = restart_counter.push() {
                            error!(
                                "sslocal exits excessively with profile \"{}\"; auto-restart stopped",
                                profile_name
                            );
                            error!("{}", err);
                            break;
                        }

                        // Restart
                        /// Temporary helper builder function to simplify error handling.
                        fn start_pipe_alert(
                            profile: ConfigProfile,
                            stdout_tx: cbc::Sender<String>,
                            stderr_tx: cbc::Sender<String>,
                            exit_listener: &mut cbc::Receiver<ExitStatus>,
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
                                    "Failed to restart with profile \"{}\": {:?}. Failure monitor daemon stopping",
                                    profile_name, err
                                );
                                break;
                            }
                        };

                        // Set new active instance
                        *instance.write().unwrap_or_else(|err| {
                            warn!("Write lock on active instance poisoned, recovering");
                            err.into_inner()
                        }) = Some(new_instance);
                    }
                    // loop exit means we should leave ProfileManager inactive
                    drop(
                        instance
                            .write()
                            .unwrap_or_else(|err| {
                                warn!("Write lock on active instance poisoned, recovering");
                                err.into_inner()
                            })
                            .take(),
                    );
                }
            })?;
        self.daemon_handles.push(handle);

        Ok(())
    }

    /// Stop the `sslocal` instance if active.
    ///
    /// Returns `Err(())` if already inactive.
    pub fn stop(&mut self) -> Result<(), ()> {
        let instance = self
            .active_instance
            .write()
            .unwrap_or_else(|err| {
                warn!("Write lock on active instance poisoned, recovering");
                err.into_inner()
            })
            .take();
        instance.map(|_| ()).ok_or(())
        // `sslocal` instance dropped implicitly
    }
}
