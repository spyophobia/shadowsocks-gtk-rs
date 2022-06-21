use std::{
    io::{self, Write},
    net,
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use clap::Parser;
use clap_def::CliArgs;
use shadowsocks_gtk_rs::runtime_api_msg::APICommand;

mod clap_def;

fn main() -> io::Result<()> {
    // init clap app
    let CliArgs {
        runtime_api_socket_path,
        sub_cmd,
    } = CliArgs::parse();

    // send
    let send_res = send_cmd(runtime_api_socket_path, sub_cmd.into());
    match &send_res {
        Ok(_) => println!("Command sent successfully"),
        Err(_) => println!("Failed to send command"),
    }
    send_res
}

fn send_cmd<P>(destination: P, cmd: APICommand) -> io::Result<()>
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
    socket.shutdown(net::Shutdown::Both)
}
