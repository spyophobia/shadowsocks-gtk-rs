# shadowsocks-gtk-client

A desktop GUI frontend for shadowsocks-rust client implemented with gtk-rs.

## Work in Progress

Might be incomplete and/or buggy. Use with caution.

## OS Support

For the moment: **LINUX ONLY**. But it should work on pretty much all distros.

Compatibility with other OSes isn't priority, because there already exists plenty of alternative solutions for Windows and MacOS.

# To Compile

### First install dependencies

You will need to first install: `rust`, `shadowsocks-rust`, `GTK3`, and `libappindicator`.
 - For `rust`, see [here](https://www.rust-lang.org/tools/install).
 - For `shadowsocks-rust`, see [here](https://www.rust-lang.org/tools/install). Strictly speaking, this is only required at runtime.
 - For `GTK3` and `libappindicator`, use your distro's package manager.

| Distro        | GTK3           | libappindicator        |
| ------------- | -------------- | ---------------------- |
| Arch `pacman` | `gtk3`         | `libappindicator-gtk3` |
| Debian `apt`  | `libgtk-3-dev` | // TODO                |
| Redhat `dnf`  | `gtk3-devel`   | `libappindicator-gtk3` |

If you are using any recent version of Gnome as your desktop environment, you also need [gnome-shell-extension-appindicator](https://extensions.gnome.org/extension/615) for the very useful tray icon.

### Then clone source and run

```sh
cd /my/code/directory
git clone https://github.com/spyophobia/shadowsocks-gtk-client.git
cd shadowsocks-gtk-client
```

```sh
cargo run --release -- --help
# or test locally in the project directory
./run-local.sh
```

If you are using this application for the first time, you probably want to read the [configuration guide](res/QnA.md#how-to-customise-configuration) first.

# Useful Reading

#### [Q&A](res/QnA.md)

#### [Stay Safe](res/stay-safe.md)
