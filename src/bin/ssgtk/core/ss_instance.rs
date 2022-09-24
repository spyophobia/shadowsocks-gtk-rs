use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    os::unix::net::UnixStream,
    process::ExitStatus,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use bus::{Bus, BusReader};
use crossbeam_channel::{unbounded as unbounded_channel, Receiver};
use derivative::Derivative;
use duct::{unix::HandleExt, Handle};
use itertools::Itertools;
use log::{trace, warn};
use nix::sys::signal::Signal;
use shadowsocks_gtk_rs::{
    consts::*,
    util::{mutex_lock, OutputKind},
};

use crate::io::profile_loader::Profile;

/// Represents a currently running `sslocal` instance, storing the relevant information
/// for its subprocess(es).
///
/// Automatically kills `sslocal` when dropped.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct SSInstance {
    /// Ownership instead of reference due to need for restart.
    pub profile: Profile,
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

impl fmt::Display for SSInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pids_repr = self.sslocal_process.pids().iter().map(u32::to_string).join(", ");
        write!(
            f,
            "SSInstance(Profile: {}, PIDs: [{}])",
            self.profile.metadata.display_name, pids_repr
        )
    }
}

impl Drop for SSInstance {
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

impl SSInstance {
    /// Start a new instance of `sslocal`.
    pub fn new(profile: Profile) -> io::Result<Self> {
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
    pub fn new_listener(&self, output_kind: OutputKind) -> BusReader<String> {
        let brd = match output_kind {
            OutputKind::Stdout => &self.stdout_brd,
            OutputKind::Stderr => &self.stderr_brd,
        };
        mutex_lock(brd).add_rx()
    }

    /// Starts a monitoring thread that waits for the underlying `sslocal`
    /// to terminate, when it will emit its `ExitStatus` via the returned channel.
    pub fn alert_on_exit(&mut self) -> io::Result<Receiver<ExitStatus>> {
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
