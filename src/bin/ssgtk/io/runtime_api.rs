//! This module defines a way to interact with the application via CLI in runtime,
//! enabled behind the "runtime_api" feature.
//!
//! This is useful if you want to, say for example,
//! bind a system shortcut to a particular action.

use std::{
    fmt,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::Sender;
use fs2::FileExt;
use log::{debug, error, trace, warn};
use shadowsocks_gtk_rs::{runtime_api_msg::APICommand, util};

#[derive(Debug)]
enum CmdError {
    IOError(io::Error),
    ParseError(json5::Error),
    SendError,
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CmdError::*;
        match self {
            IOError(e) => write!(f, "CmdError-IOError: {}", e),
            ParseError(e) => write!(f, "CmdError-ParseError: {}", e),
            SendError => write!(f, "CmdError-SendError: Command receiver has hung up"),
        }
    }
}

impl From<io::Error> for CmdError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}
impl From<json5::Error> for CmdError {
    fn from(err: json5::Error) -> Self {
        Self::ParseError(err)
    }
}

/// An active listener on a unix socket that handles
/// incoming connections and commands.
///
/// Terminates the underlying listener thread when dropped.
#[derive(Debug)]
pub struct APIListener {
    /// Saved so that we can remove it on drop.
    lock_file_path: PathBuf,
    /// Saved so that we can unlock on drop.
    lock_file: File,
    /// Saved so that we can remove it on drop.
    socket_path: PathBuf,
    /// Default: false. Set to true to halt the listener on next poll.
    halt_flag: Arc<RwLock<bool>>,
    /// Wrapped in `Option` so that it can be joined on drop.
    listener_handle: Option<JoinHandle<()>>,
}

impl Drop for APIListener {
    fn drop(&mut self) {
        trace!("Runtime API listener is getting dropped");

        // notify listener halt
        *util::rwlock_write(&self.halt_flag) = true;

        // wait for daemon threads to finish
        if let Some(handle) = self.listener_handle.take() {
            if let Err(err) = handle.join() {
                warn!(
                    "Runtime API's listener daemon thread has panicked unexpectedly: {:?}",
                    err
                );
            };
        }

        // remove socket file
        match fs::remove_file(&self.socket_path) {
            Ok(_) => debug!("Removed socket file at {:?}", &self.socket_path),
            Err(err) => error!(
                "Failed to cleanup runtime API's socket file at {:?}: {}",
                &self.socket_path, err
            ),
        }

        // unlock and remove lock file
        match self.lock_file.unlock() {
            Ok(_) => trace!("Unlocked lock file at {:?}", &self.lock_file_path),
            Err(err) => error!("Failed to unlock lock file at {:?}: {}", &self.lock_file_path, err),
        }
        match fs::remove_file(&self.lock_file_path) {
            Ok(_) => trace!("Removed lock file at {:?}", &self.lock_file_path),
            Err(err) => warn!("Failed to remove lock file at {:?}: {}", &self.lock_file_path, err),
        }
    }
}

impl APIListener {
    pub fn start(bind_addr: impl AsRef<Path>, cmds_tx: Sender<APICommand>) -> io::Result<Self> {
        // try to lock lock file
        let lock_file_path = {
            let mut path = bind_addr.as_ref().to_path_buf();
            let mut name = path
                .file_name()
                .ok_or(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Listener socket path cannot be \"/\" or end with \"..\"",
                ))?
                .to_str()
                .ok_or(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Listener socket path is not UTF-8",
                ))?
                .to_string();
            name.push_str(".lock");
            path.set_file_name(name);
            path
        };
        trace!("Creating and locking lock file at {:?}", lock_file_path);
        let lock_file = File::create(&lock_file_path)?;
        if let Err(err) = lock_file.try_lock_exclusive() {
            error!("Failed to obtain lock on lock file {:?}: {}", lock_file_path, err);
            return Err(err);
        }

        let socket_path = bind_addr.as_ref().to_path_buf();
        let listener = {
            if bind_addr.as_ref().exists() {
                // since lock was successful, this means the application
                // has panicked last time and hasn't performed cleanup
                // therefore it's safe to remove the listener
                fs::remove_file(&bind_addr)?;
            }
            debug!("Binding runtime API listener to {:?}", bind_addr.as_ref());
            let bind_res = UnixListener::bind(&bind_addr);
            if let Err(err) = &bind_res {
                error!("Runtime API cannot bind to {:?}: {}", bind_addr.as_ref(), err);
            }
            let listener = bind_res?;
            listener.set_nonblocking(true)?;
            listener
        };
        let halt_flag = RwLock::new(false).into();
        let halt_flag_clone = Arc::clone(&halt_flag);

        let listener_handle = thread::Builder::new()
            .name("Runtime API Listener".into())
            .spawn(move || loop {
                thread::sleep(Duration::from_millis(10)); // 100fps

                // check for halt
                if *util::rwlock_read(&halt_flag_clone) {
                    trace!("Runtime API halt flag has been set; daemon exiting");
                    break;
                }

                // handle connection errors
                let (stream, peer_addr) = match listener.accept() {
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => continue, // no connections, skip
                    Err(err) => {
                        warn!("Runtime API connection error: {}", err);
                        continue;
                    }
                    Ok(client) => client,
                };

                // handle client
                trace!("Accepted an incoming connection from {:?}", peer_addr);
                if let Err(err) = handle_client(stream, &cmds_tx) {
                    warn!("Runtime API command error: {}", err);
                }
            })?
            .into();

        let ret = Self {
            lock_file_path,
            lock_file,
            socket_path,
            halt_flag,
            listener_handle,
        };
        Ok(ret)
    }
}

/// Handles a single client connect request.
fn handle_client(stream: UnixStream, cmds_tx: &Sender<APICommand>) -> Result<(), CmdError> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    let cmd = {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        json5::from_str::<APICommand>(&line)?
    };
    debug!("Runtime API received a command: {}", cmd);
    cmds_tx.send(cmd).map_err(|_| CmdError::SendError)
}
