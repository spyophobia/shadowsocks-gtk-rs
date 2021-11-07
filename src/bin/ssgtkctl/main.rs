use std::{io, path::PathBuf};

use shadowsocks_gtk_rs::runtime_api::{self, APICommand};

mod clap_def;

fn main() -> io::Result<()> {
    // init clap app
    let clap_matches = clap_def::build_app().get_matches();

    // get destination
    let socket_path: PathBuf = clap_matches
        .value_of("runtime-api-socket-path")
        .unwrap() // clap sets default
        .into();

    // get command
    let cmd = {
        use APICommand::*;
        match clap_matches.subcommand() {
            ("backlog-show", _) => BacklogShow,
            ("backlog-hide", _) => BacklogHide,
            ("restart", _) => Restart,
            ("switch-profile", Some(m)) => {
                let name = m.value_of("profile-name").unwrap(); // required by clap
                SwitchProfile(name.into())
            }
            ("stop", _) => Stop,
            ("quit", _) => Quit,
            _ => unreachable!("all possible subcommands covered"),
        }
    };

    // send
    runtime_api::send_cmd(socket_path, cmd)
}
