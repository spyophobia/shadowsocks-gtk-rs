# Common Questions and Answers

## My tray icon is blank.

By default, GTK searches [a preset list of system directories](https://askubuntu.com/a/43951/1020143) for the application icon. If the icon is blank it means the image file `shadowsocks-gtk-rs[.ext]` is not found in those directories. You can resolve this by getting [/res/logo/shadowsocks-gtk-rs.png](/res/logo/shadowsocks-gtk-rs.png) and placing it under `/usr/share/pixmaps/`. After a reboot the icon should be picked up.

Alternatively `shadowsocks-gtk-rs` has launch parameters `--icon-theme-dir` and `--icon-name` if you want to use a custom icon. For usage, run:
```sh
cargo run --release -- --help
# or
shadowsocks-gtk-rs --help
```

## How to customise configuration?

### Basic, single-profile config
```
# An example
My-profile
├── profile.yaml
└── ...<other files>
```
We will call this a "profile directory". You can name this directory anything you want; `My-profile` is only an example. By default the displayed name of this profile is the directory name (`My-profile` in this case), but this can be customised in `profile.yaml`.

`profile.yaml` is the definition file for a profile: a directory is considered a "profile directory" only if it contains a `profile.yaml`. You may put other profile-specific files and directories under this directory too if you need to reference them from `profile.yaml`, such as a `ss-config.json`.

For examples of how to manually write a `profile.yaml`, see [/example-config-profiles/Group-of-good-profiles](/example-config-profiles/Group-of-good-profiles).

### Grouped multi-profile config
```
# An example
Root-group
├── My-profile-A
│   ├── profile.yaml
│   └── ...<other files>
└── My-nested-group
    ├── My-profile-B
    │   ├── profile.yaml
    │   └── ...<other files>
    └── My-profile-C
        └── profile.yaml
```
You can put multiple "profile directories" under a single "group directory" to make a group. The group's display name is the name of this "group directory".
You can nest "group directories" to as many layers as you want, but I doubt its practicality beyond layer 2 or 3.

Note that a "group directory" can only have "profile directories" and other "group directories" as its **direct descendants**, not regular files.

Also note that symlinks are not currently supported in "group directories". I recognise their potential usefulness, but I am concerned about circular symlinking causing unnecessary trouble.

### Other miscellaneous details
 - You can create an empty file named `.ss_ignore` in any "profile directory" or "group directory" to disable it and all its children.

## Can I bind a shortcut to \<some action>?
 - Yes! `runtime_api` is a default feature of this crate, which provides a `ssgtkctl` binary. You can use it to make the application do various things. All you need to do is to bind a system shortcut to it. To see what it can do, simply run:
```sh
cargo run --release --bin ssgtkctl -- --help
# or
ssgtkctl --help
```
 - Underneath the hood, the `runtime_api` feature starts a listener on a Unix socket, to which you can send commands in [JSON5](https://json5.org/). The `ssgtkctl` binary is merely a delegate to simplify the sending of said command.
 - If you wish to interface with the Unix socket directly, you can take a look at some example commands by running:
```sh
cargo test print_cmd_egs -- --nocapture
```

## Why did you pick GTK instead of QT?
GTK's rust binding has significantly better support than that of QT. I'm too lazy to support both so the choice is obvious.

## Why aren't you using GTK4?
This project depends on `libappindicator` for tray icon support, which does not yet support GTK4.

See [here](https://github.com/AyatanaIndicators/libayatana-appindicator/issues/22).

## Why target `sslocal` command line API?
*Because I'm a lazy arse.*

More seriously though, because it's a stable API and it works. What more can you ask for?

Also making UI is painful. There are so many different flags and arguments you can set, and it will take me forever to create a UI element for each of them. Much easier instead, to create UI elements for the most commonly used items, while also giving more advanced users the option to specify more obscure settings using CLI arguments directly.

As an unintentional by-product, this also means you can just as well specify any other executable with arbitrary arguments, and this app will happily run it for you. It will just make all the fancy GUI settings useless.

## Why do you only use English?
It's complicated. I choose not to answer this question for the sake of my privacy and security.
