//! This module contains code that handles profile switching and automatic restarting.

use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    os::unix::net::UnixStream,
    process::ExitStatus,
    sync::{Arc, Mutex, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use bus::{Bus, BusReader};
use crossbeam_channel::{unbounded as unbounded_channel, Receiver, Sender};
use derivative::Derivative;
use duct::{unix::HandleExt, Handle};
use itertools::Itertools;
use log::{debug, error, info, trace, warn};
use nix::sys::signal::Signal;
use shadowsocks_gtk_rs::{
    consts::*,
    util::{
        self,
        leaky_bucket::{NaiveLeakyBucket, NaiveLeakyBucketConfig},
        mutex_lock, rwlock_read, OutputKind,
    },
};

use crate::{
    event::AppEvent,
    io::{
        app_state::AppState,
        profile_loader::{Profile, ProfileFolder},
    },
};

/// Represents a currently running `sslocal` instance, storing the relevant information
/// for its subprocess(es).
///
/// Automatically kills `sslocal` when dropped.
#[derive(Derivative)]
#[derivative(Debug)]
struct ActiveSSInstance {
    /// Ownership instead of reference due to need for restart.
    profile: Profile,
    /// The handle of the subprocess.
    sslocal_process: Arc<Handle>,
    /// Subscribe to me to handle `sslocal`'s `stdout`.
    #[derivative(Debug(format_with = "shadowsocks_gtk_rs::util::hacks::omit_bus"))]
    stdout_brd: Arc<Mutex<Bus<String>>>,
    /// Subscribe to me to handle `sslocal`'s `stderr`.
    #[derivative(Debug(format_with = "shadowsocks_gtk_rs::util::hacks::omit_bus"))]
    stderr_brd: Arc<Mutex<Bus<String>>>,
    /// The daemon threads that need to be cleanup up when deactivating.
    daemon_handles: Vec<JoinHandle<()>>,
}

impl fmt::Display for ActiveSSInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pids_repr = self.sslocal_process.pids().iter().map(u32::to_string).join(", ");
        write!(
            f,
            "ActiveSSInstance(Profile: {}, PIDs: [{}])",
            self.profile.metadata.display_name, pids_repr
        )
    }
}

impl Drop for ActiveSSInstance {
    /// Kill the `sslocal` child process when going out of scope.
    ///
    /// Also cleans up all daemon threads.
    fn drop(&mut self) {
        let self_name = self.to_string();

        trace!("{} is getting dropped", self_name);

        // send stop signal to `sslocal` process
        if let Err(err) = self.sslocal_process.send_signal(Signal::SIGINT as i32) {
            trace!("{}'s underlying process has already exited: {}", self_name, err);
        }

        // sleep for a short time to allow `sslocal` to exit fully
        thread::sleep(Duration::from_millis(100));

        // make sure all daemon threads finish
        for handle in self.daemon_handles.drain(..) {
            if let Err(err) = handle.join() {
                warn!("A daemon of {} panicked unexpectedly: {:?}", self_name, err);
            };
        }
    }
}

impl ActiveSSInstance {
    /// Start a new instance of `sslocal`.
    fn new(profile: Profile) -> io::Result<Self> {
        let (stdout_stream_tx, stdout_stream_rx) = UnixStream::pair()?;
        let (stderr_stream_tx, stderr_stream_rx) = UnixStream::pair()?;

        // start instance
        let proc = profile.run_sslocal(Some(stdout_stream_tx), Some(stderr_stream_tx))?;
        let mut instance = Self {
            profile,
            sslocal_process: proc.into(),
            stdout_brd: Mutex::new(Bus::new(BUS_BUFFER_SIZE)).into(),
            stderr_brd: Mutex::new(Bus::new(BUS_BUFFER_SIZE)).into(),
            daemon_handles: vec![],
        };

        // pipe output
        instance.pipe_to_broadcast(stdout_stream_rx, OutputKind::Stdout)?;
        instance.pipe_to_broadcast(stderr_stream_rx, OutputKind::Stderr)?;

        Ok(instance)
    }

    /// Start a daemon to pipe output from a readable source to a broadcasting channel.
    fn pipe_to_broadcast<R>(&mut self, source: R, output_kind: OutputKind) -> io::Result<()>
    where
        R: Read + Send + 'static,
    {
        let self_name = self.to_string();
        let source = BufReader::new(source);
        let brd = match output_kind {
            OutputKind::Stdout => Arc::clone(&self.stdout_brd),
            OutputKind::Stderr => Arc::clone(&self.stderr_brd),
        };
        let handle = thread::Builder::new()
            .name(format!("{} piper daemon for {}", output_kind, self_name))
            .spawn(move || {
                trace!("{} piper daemon for {} started", output_kind, self_name);
                for line_res in source.lines() {
                    let line = {
                        let raw = line_res.unwrap_or_else(|err| format!("Error reading {}: {}", &output_kind, err));
                        format!("[{}] {}\n", output_kind, raw)
                    };
                    trace!("Broadcasting: {}", line);
                    // try to send through channel
                    if let Err(_) = mutex_lock(&brd).try_broadcast(line) {
                        warn!(
                            "{} wrote to {}, but the broadcasting channel is full.",
                            self_name, output_kind
                        );
                    }
                }
                // thread exits when the source is closed
            })?;
        self.daemon_handles.push(handle);
        Ok(())
    }

    /// Convenience function to create a new broadcast listener.
    fn new_listener(&self, output_kind: OutputKind) -> BusReader<String> {
        let brd = match output_kind {
            OutputKind::Stdout => &self.stdout_brd,
            OutputKind::Stderr => &self.stderr_brd,
        };
        mutex_lock(brd).add_rx()
    }

    /// Starts a monitoring thread that waits for the underlying `sslocal`
    /// to terminate, when it will emit its `ExitStatus` via the returned channel.
    fn alert_on_exit(&mut self) -> io::Result<Receiver<ExitStatus>> {
        let self_name = self.to_string();
        let proc = Arc::clone(&self.sslocal_process);
        let (exit_tx, exit_rx) = unbounded_channel();
        let handle = thread::Builder::new()
            .name(format!("exit alert daemon for instance {}", self_name))
            .spawn(move || {
                let status = proc
                    .wait()
                    .unwrap() // process already running for sure
                    .status;
                if let Err(err) = exit_tx.send(status) {
                    warn!("{} exit detected: {}, but the receiver has hung up.", self_name, err.0);
                }
            })?;
        self.daemon_handles.push(handle);
        Ok(exit_rx)
    }
}

/// A daemon that manages profile-switching and restarts.
#[derive(Derivative)]
#[derivative(Debug)]
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

    /// A string holding the combined backlog history of `stdout` & `stderr`.
    pub backlog: Arc<Mutex<String>>,
    /// A channel that broadcasts the combined logs of `stdout` & `stderr`.
    #[derivative(Debug(format_with = "shadowsocks_gtk_rs::util::hacks::omit_bus"))]
    pub logs_brd: Arc<Mutex<Bus<String>>>,

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
        Self {
            restart_limit,
            events_tx,
            active_instance: RwLock::new(None).into(),
            backlog: Mutex::new(String::new()).into(),
            logs_brd: Mutex::new(Bus::new(BUS_BUFFER_SIZE)).into(),
            daemon_handles: vec![],
        }
    }

    /// Resume from a previously saved state.
    pub fn resume_from(state: &AppState, profiles: &ProfileFolder, events_tx: Sender<AppEvent>) -> Self {
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
    pub fn current_profile(&self) -> Option<Profile> {
        util::rwlock_read(&self.active_instance)
            .as_ref()
            .map(|instance| instance.profile.clone())
    }

    /// Start a `sslocal` instance with a new profile, replacing the old one if necessary.
    ///
    /// Returns `Ok(())` if and only if the new instance starts successfully and the old one is cleaned up.
    ///
    /// If the new instance fails to start, this `ProfileManager` will be left in deactivated state.
    pub fn switch_to(&mut self, profile: Profile) -> io::Result<()> {
        // deactivate the old instance
        let _ = self.try_stop();

        // activate the new instance
        let mut new_instance = ActiveSSInstance::new(profile)?;

        // monitor for failure
        let exit_alert_rx = new_instance.alert_on_exit()?;

        // set
        *util::rwlock_write(&self.active_instance) = Some(new_instance);

        // pipe output
        self.log_piping_setup(OutputKind::Stdout)?;
        self.log_piping_setup(OutputKind::Stderr)?;

        // monitor
        self.handle_fail(exit_alert_rx)?;

        Ok(())
    }

    /// Convenience function to create a new broadcast listener.
    pub fn new_listener(&self) -> BusReader<String> {
        mutex_lock(&self.logs_brd).add_rx()
    }

    /// Stop the `sslocal` instance if active.
    ///
    /// Returns `Err(())` if already inactive.
    pub fn try_stop(&mut self) -> Result<(), ()> {
        let instance = util::rwlock_write(&self.active_instance).take();
        instance.map(|_| ()).ok_or(())
        // `sslocal` instance dropped implicitly
    }

    /// Start a daemon that subscribes to an output broadcast of
    /// the underlying `sslocal` instance, then re-broadcasts the logs
    /// and appends them to the backlog.
    fn log_piping_setup(&mut self, output_kind: OutputKind) -> io::Result<()> {
        let instance_opt = rwlock_read(&self.active_instance);
        let instance = instance_opt
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Not active"))?;
        let re_brd = Arc::clone(&self.logs_brd);
        let backlog = Arc::clone(&self.backlog);

        // create thread
        let handle = log_piping_setup_impl(&instance, output_kind, re_brd, backlog)?;
        self.daemon_handles.push(handle);

        Ok(())
    }

    /// Starts a monitoring thread that waits for the underlying `sslocal` instance
    /// to fail, when it will attempt to perform a restart as specified by
    /// `Self::restart_limit`.
    fn handle_fail(&mut self, listener: Receiver<ExitStatus>) -> io::Result<()> {
        // variables that need to be moved into thread
        let restart_limit = self.restart_limit;
        let events_tx = self.events_tx.clone();
        let instance = Arc::clone(&self.active_instance);
        let profile = self
            .current_profile()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Not active"))?;
        let logs_brd = Arc::clone(&self.logs_brd);
        let backlog = Arc::clone(&self.backlog);

        // create thread
        let handle = thread::Builder::new()
            .name("ProfileManager failure monitor daemon".into())
            .spawn(move || {
                // profile stays the same across restarts, therefore outside of loop
                let profile_name = profile.metadata.display_name.clone();
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
                            debug!("{} has exited successfully; auto-restart stopped", instance_name);
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
                                "The exit alert daemon for {} has hung up: {}; auto-restart stopped",
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
                            warn!("{} has failed; restarting", instance_name);
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
                        profile: Profile,
                        re_brd: Arc<Mutex<Bus<String>>>,
                        backlog: Arc<Mutex<String>>,
                        exit_listener: &mut Receiver<ExitStatus>,
                    ) -> io::Result<ActiveSSInstance> {
                        let mut instance = ActiveSSInstance::new(profile)?;
                        log_piping_setup_impl(
                            &instance,
                            OutputKind::Stdout,
                            Arc::clone(&re_brd),
                            Arc::clone(&backlog),
                        )?;
                        log_piping_setup_impl(&instance, OutputKind::Stderr, re_brd, backlog)?;
                        *exit_listener = instance.alert_on_exit()?;
                        Ok(instance)
                    }

                    let new_instance = {
                        let start_res = start_pipe_alert(
                            profile.clone(),
                            Arc::clone(&logs_brd),
                            Arc::clone(&backlog),
                            &mut exit_listener,
                        );
                        match start_res {
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
}

/// This is not an associated function because it has to be called by
/// threads created by `ProfileManager::handle_fail`.
fn log_piping_setup_impl(
    instance: &ActiveSSInstance,
    output_kind: OutputKind,
    re_brd: Arc<Mutex<Bus<String>>>,
    backlog: Arc<Mutex<String>>,
) -> io::Result<JoinHandle<()>> {
    // variables that need to be moved into thread
    let instance_name = instance.to_string();
    let mut listener = instance.new_listener(output_kind);
    // create thread
    thread::Builder::new()
        .name(format!("{} log porter daemon for {}", output_kind, instance_name))
        .spawn(move || {
            trace!("{} log porter daemon for {} started", output_kind, instance_name);
            for line in listener.iter() {
                // doing those two in reverse to eliminate `line.clone()` call
                // append to backlog
                mutex_lock(&backlog).push_str(&line);
                // rebroadcast
                mutex_lock(&re_brd).broadcast(line);
            }
            // thread exits when broadcast stops
        })
}

#[cfg(test)]
mod test {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    use crossbeam_channel::unbounded as unbounded_channel;
    use log::{debug, LevelFilter};
    use shadowsocks_gtk_rs::util::{leaky_bucket::NaiveLeakyBucketConfig, rwlock_read, OutputKind};
    use simplelog::{Config, SimpleLogger};

    use crate::{io::profile_loader::ProfileFolder, profile_manager::ProfileManager};

    /// This test will always pass. You need to examine the outputs manually.
    ///
    /// `cargo test example_profiles_test_run -- --nocapture`
    #[test]
    fn example_profiles_test_run() {
        SimpleLogger::init(LevelFilter::Trace, Config::default()).unwrap();

        // parse example configs
        let eg_configs = ProfileFolder::from_path_recurse("example-profiles").unwrap();
        let profile_list = eg_configs.get_profiles();
        debug!("Loaded {} profiles.", profile_list.len());

        // setup ProfileManager
        let restart_limit = NaiveLeakyBucketConfig::new(3, Duration::from_secs(10));
        let (events_tx, _) = unbounded_channel();
        let mut mgr = ProfileManager::new(restart_limit, events_tx);

        // run through all example profiles
        for p in profile_list {
            println!();
            mgr.switch_to(p.clone()).unwrap();
            let (mut stdout_listener, mut stderr_listener) = {
                let instance_opt = rwlock_read(&mgr.active_instance);
                let instance = instance_opt.as_ref().unwrap();
                (
                    instance.new_listener(OutputKind::Stdout),
                    instance.new_listener(OutputKind::Stderr),
                )
            };
            thread::spawn(move || stdout_listener.iter().for_each(|s| println!("stdout: {}", s)));
            thread::spawn(move || stderr_listener.iter().for_each(|s| println!("stderr: {}", s)));
            sleep(Duration::from_millis(3000));
        }
        let _ = mgr.try_stop();
    }
}
