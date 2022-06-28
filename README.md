# shadowsocks-gtk-rs

![Auto compile test](https://github.com/spyophobia/shadowsocks-gtk-rs/actions/workflows/auto-compile.yml/badge.svg?branch=master)

A desktop GUI frontend for shadowsocks-rust client implemented with gtk-rs.

This application is currently **Linux only**. Compatibility with other OSes isn't planned,
because there already exists plenty of alternative solutions for Windows and MacOS.

This package contains two binaries:

| Binary     | Functionality                                                                            |
|------------|------------------------------------------------------------------------------------------|
| `ssgtk`    | The main executable; launches the GUI application.                                       |
| `ssgtkctl` | The runtime API controller; see [Q&A](/res/QnA.md#can-i-bind-a-shortcut-to-some-action). |

## Table of Contents

- [shadowsocks-gtk-rs](#shadowsocks-gtk-rs)
  - [Table of Contents](#table-of-contents)
    - [Work in Progress](#work-in-progress)
  - [Install](#install)
    - [Read the Documentation!](#read-the-documentation)
    - [Arch Linux and Derivatives](#arch-linux-and-derivatives)
    - [Any Linux](#any-linux)
      - [Limitations of Using `cargo-install`](#limitations-of-using-cargo-install)
  - [Build](#build)
    - [Dependencies](#dependencies)
    - [Clone Source and Run](#clone-source-and-run)
  - [Useful Reading](#useful-reading)

### Work in Progress

Be advised that this application may be incomplete and/or buggy. But do rest assured
that it won't destroy your OS or something.

Your input is very welcomed! If you have any suggestions or have found any issue
(no matter how small or unimportant) with the code or the documentation, please
feel free to raise an issue. Or better yet, submit a PR if you can!

## Install

### Read the Documentation!

**If you are using this application for the first time, you should first read the [configuration guide](/res/config-guide.md).**

### Arch Linux and Derivatives

You can install the [AUR package](https://aur.archlinux.org/packages/shadowsocks-gtk-rs) that I maintain.

```sh
# install with paru
paru shadowsocks-gtk-rs
```

### Any Linux

You can always install directly from [crates.io](https://crates.io/crates/shadowsocks-gtk-rs).

```sh
cargo install shadowsocks-gtk-rs
```

#### Limitations of Using `cargo-install`
 - you will need to [manually install dependencies](#dependencies) first.
 - support files (e.g. desktop entry, icon) cannot be automatically installed.

## Build

### Dependencies

 - A working installation of `rust`, see [here](https://www.rust-lang.org/tools/install).
 - The `sslocal` binary from [`shadowsocks-rust`](https://github.com/shadowsocks/shadowsocks-rust) as the backend.
   - Strictly speaking, this is only required at runtime.
 - `GTK3` and `libappindicator`, using your distro's package manager.

The latest versions are **highly recommended**.

| Distro        | GTK3           | libappindicator        |
|---------------|----------------|------------------------|
| Arch `pacman` | `gtk3`         | `libappindicator-gtk3` |
| Debian `apt`  | `libgtk-3-dev` | `libappindicator3-dev` |
| Fedora `dnf`  | `gtk3-devel`   | `libappindicator-gtk3` |

If you are using any recent version of Gnome as your desktop environment, you also need
[gnome-shell-extension-appindicator](https://extensions.gnome.org/extension/615) for the tray icon to show up.

### Clone Source and Run

```sh
git clone https://github.com/spyophobia/shadowsocks-gtk-rs.git
cd shadowsocks-gtk-rs
# this script runs locally in the project directory
./run-local.sh
```

## Useful Reading

 - [Q&A](/res/QnA.md)
 - [Stay Safe](/res/stay-safe.md)
