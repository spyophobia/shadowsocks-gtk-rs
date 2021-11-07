//! This module defines a way to interact with the application via CLI in runtime,
//! enabled behind the "runtime_api" feature.
//!
//! This is useful if you want to, say for example,
//! bind a system shortcut to a particular action.

use std::{
    fmt, fs,
    io::{self, BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::Sender;
use log::{debug, error, trace, warn};
use serde::{Deserialize, Serialize};

use crate::{notify_method::NotifyMethod, util};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum APICommand {
    // GUI
    BacklogShow,
    BacklogHide,
    SetNotify(NotifyMethod),

    // core
    // IDEA: some kind of query command?
    Restart,
    SwitchProfile(String),
    Stop,
    Quit,
}

impl fmt::Display for APICommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use APICommand::*;
        let msg = match self {
            BacklogShow => "Show Backlog".into(),
            BacklogHide => "Hide Backlog".into(),
            SetNotify(method) => format!("Set notification method to {}", method),

            Restart => "Restart current profile".into(),
            SwitchProfile(name) => format!("Switch Profile to {}", name),
            Stop => "Stop current profile".into(),
            Quit => "Quit application".into(),
        };
        write!(f, "{}", msg)
    }
}

#[derive(Debug)]
enum CmdError {
    IOError(io::Error),
    ParseError(json5::Error),
    SendError,
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmdError::IOError(e) => write!(f, "CmdError-IOError: {}", e),
            CmdError::ParseError(e) => write!(f, "CmdError-ParseError: {}", e),
            CmdError::SendError => write!(f, "CmdError-SendError: Command receiver has hung up"),
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
    socket_path: PathBuf,
    /// Default: false. Set to true to halt the listener on next poll.
    halt_flag: Arc<RwLock<bool>>,
    /// Wrapped in `Option` so that it can be joined on drop.
    listener_handle: Option<JoinHandle<()>>,
}

impl Drop for APIListener {
    fn drop(&mut self) {
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
    }
}

impl APIListener {
    pub fn start<P>(bind_addr: P, cmds_tx: Sender<APICommand>) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let socket_path = bind_addr.as_ref().into();
        let listener = {
            // IDEA: use lock file
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

pub fn send_cmd<P>(destination: P, cmd: APICommand) -> io::Result<()>
where
    P: AsRef<Path>,
{
    let mut socket = UnixStream::connect(destination)?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    socket.write_all(
        json5::to_string(&cmd)
            .expect("serialising APICommand to json5 is infallible")
            .as_bytes(),
    )?;
    socket.flush()?;
    socket.shutdown(Shutdown::Both)
}

#[cfg(test)]
mod test {
    use super::APICommand;

    /// This test is intended to show a list of example commands
    /// and will always pass.
    ///
    /// `cargo test print_cmd_egs -- --nocapture`
    #[test]
    fn print_cmd_egs() {
        use APICommand::*;
        let egs = vec![
            BacklogShow,
            BacklogHide,
            Restart,
            SwitchProfile("Example Profile".into()),
            Stop,
            Quit,
        ];
        println!("{}", "-".repeat(50));
        println!("Those are some of the commands you can issue (CASE SENSITIVE):");
        for cmd in egs.into_iter() {
            let cmd_str = json5::to_string(&cmd)
                .expect("Manually created, shouldn't error")
                .replace("\"", "\\\""); // escape quotes for shell
            println!("\techo {} | nc -U /path/to/shadowsocks-gtk-rs.sock", cmd_str);
        }
        println!(
            "Note 0: you likely need the BSD variant of netcat to be able to connect \
            to Unix sockets (see https://unix.stackexchange.com/a/26781/375550)\n\
            Note 1: due to technical limitations and my laziness (mainly the latter) \
            the JSON5 command string must be a single line"
        );
        println!(
            "For the default socket path and how to manually set a different one, see\n\
            \tcargo run --release -- --help"
        );
        println!("{}", "-".repeat(50));
    }
}
