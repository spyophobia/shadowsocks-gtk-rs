# shadowsocks-gtk-rs

A desktop GUI frontend for shadowsocks-rust client implemented with gtk-rs.

## Work in Progress

Be advised that this application may be incomplete and/or buggy. But do rest assured that it won't destroy your OS or something.

Your input is very welcomed! If you have any suggestions or have found any issue (no matter how small or unimportant) with the code or the documentation, please feel free to raise an issue. Or better yet, submit a PR if you can!

## OS Support

For the moment: **LINUX ONLY**. But it should work on pretty much all distros.

Compatibility with other OSes isn't priority, because there already exists plenty of alternative solutions for Windows and MacOS.

# Dependencies

You will need to first install: `rust`, `shadowsocks-rust`, `GTK3`, and `libappindicator`.
 - For `rust`, see [here](https://www.rust-lang.org/tools/install).
 - For `shadowsocks-rust`, see [here](https://www.rust-lang.org/tools/install). Strictly speaking, this is only required at runtime.
 - For `GTK3` and `libappindicator`, use your distro's package manager.

The latest versions are **highly recommended**.

| Distro        | GTK3           | libappindicator              |
| ------------- | -------------- | ---------------------------- |
| Arch `pacman` | `gtk3`         | `libappindicator-gtk3`       |
| Debian `apt`  | `libgtk-3-dev` | `libayatana-appindicator3-1` |
| Redhat `dnf`  | `gtk3-devel`   | `libappindicator-gtk3`       |

If you are using any recent version of Gnome as your desktop environment, you also need [gnome-shell-extension-appindicator](https://extensions.gnome.org/extension/615) for the tray icon to show up.

# Read the documentation!

If you are using this application for the first time, you probably want to read the [configuration guide](res/QnA.md#how-to-customise-configuration) first.

# Then either...

### Clone source and run without installing

```sh
cd /my/code/directory
git clone https://github.com/spyophobia/shadowsocks-gtk-rs.git
cd shadowsocks-gtk-rs
```

```sh
cargo run --release -- --help
# or test locally in the project directory
./run-local.sh
```

### Alternatively, install from [crates.io](https://crates.io/crates/shadowsocks-gtk-rs)

```sh
cargo install shadowsocks-gtk-rs
shadowsocks-gtk-rs --help
```

# Useful Reading

#### [Q&A](res/QnA.md)

#### [Stay Safe](res/stay-safe.md)
