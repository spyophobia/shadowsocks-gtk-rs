use std::{
    io::{self, Write},
    net,
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use clap::{IntoApp, Parser};
use clap_def::CliArgs;
use shadowsocks_gtk_rs::{notify_method::NotifyMethod, runtime_api_msg::APICommand};

mod clap_def;

fn main() -> io::Result<()> {
    // init clap app
    let CliArgs {
        runtime_api_socket_path,
        sub_cmd,
        print_socket_examples,
    } = CliArgs::parse();

    // print examples
    if print_socket_examples {
        print_socket_egs();
        return Ok(());
    }

    // subcommand required past this point
    let sub_cmd = match sub_cmd {
        Some(cmd) => cmd,
        None => CliArgs::command()
            .error(clap::ErrorKind::MissingSubcommand, "a subcommand is required")
            .exit(),
    };

    // send
    let send_res = send_cmd(runtime_api_socket_path, sub_cmd.into());
    match &send_res {
        Ok(_) => println!("Command sent successfully"),
        Err(_) => println!("Failed to send command"),
    }
    send_res
}

fn print_socket_egs() {
    use APICommand::*;
    let egs = vec![
        LogViewerShow,
        LogViewerHide,
        SetNotify(NotifyMethod::Toast),
        Restart,
        SwitchProfile("Example Profile".into()),
        Stop,
        Quit,
    ];
    println!("{}", "-".repeat(50));
    println!("Here are some of the commands you can issue (CASE SENSITIVE):");
    for cmd in egs.into_iter() {
        let cmd_str = json5::to_string(&cmd).expect("Manually created, shouldn't error");
        println!("\t`echo \'{}\' | nc -U /path/to/shadowsocks-gtk-rs.sock`", cmd_str);
    }
    println!(
        "Note 0: you likely need the BSD variant of netcat to be able to connect \
        to Unix sockets (see https://unix.stackexchange.com/a/26781/375550)\n\
        Note 1: due to technical limitations and my laziness (mainly the latter) \
        the JSON5 command string must be a single line"
    );
    println!(
        "For the default socket path and how to manually set a different one, see\n\
        \t`ssgtk --help` and `ssgtkctl --help`"
    );
    println!("{}", "-".repeat(50));
}

fn send_cmd(destination: impl AsRef<Path>, cmd: APICommand) -> io::Result<()> {
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
