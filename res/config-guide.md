
# Configuration guide

- [Configuration guide](#configuration-guide)
  - [Defining a profile](#defining-a-profile)
    - [The config file: `profile.yaml`](#the-config-file-profileyaml)
  - [Organizing your profiles](#organizing-your-profiles)
    - [Single profile](#single-profile)
    - [Grouping multiple profiles](#grouping-multiple-profiles)
  - [Other miscellaneous details](#other-miscellaneous-details)

## Defining a profile

```
# An example
My-profile
├── profile.yaml
└── ...<other files>
```

This is the basic directory structure of a profile directory. You can name this directory anything you want.
By default the displayed name of this profile is the directory name (`My-profile` in this case),
but this can be customized in `profile.yaml`.

The only required file in this directory is `profile.yaml`. See below.
You may put other profile-specific files and directories under this directory too
if you need to reference them from `profile.yaml`, such as a `ss.json5`.

### The config file: `profile.yaml`

`profile.yaml` defines how the underlying `sslocal` backend binary should be run.

There are currently 3 modes available (more coming soon™️):
 - `config-file`: if you want to pass a [JSON5](https://json5.org/) config file to `sslocal`.
   - This is the most flexible mode. You can basically do anything.
 - `proxy`: if you want to run `sslocal` as a proxy server.
 - `tun`: if you want to run `sslocal` as a `tun` device.

See [/example-profiles/Group-of-good-profiles](/example-profiles/Group-of-good-profiles) for examples.

## Organizing your profiles

By default, `ssgtk` loads your profiles from `$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles`,
or `~/.config/shadowsocks-gtk-rs/profiles` if `$XDG_CONFIG_HOME` is unset.
But you can override this with the `--profiles-dir` option.

Within this document though, we will assume you are using the default value.

### Single profile

If you only have one profile, simply place it under `$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles`:

```
$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles
└── My-profile
    ├── profile.yaml
    └── ...<other files>
```

Actually this will work too:
```
$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles
├── profile.yaml
└── ...<other files>
```

### Grouping multiple profiles

If you have multiple profiles, you can organize them into groups.

```
$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles
├── My-profile-A
│   ├── profile.yaml
│   └── ...<other files>
└── My-nested-group
    ├── My-profile-B
    │   ├── profile.yaml
    │   └── ...<other files>
    ├── My-profile-C
    │   └── profile.yaml
    └── ...<more nesting>
```

You can create nested groups to as many layers as you want, but I doubt its practicality beyond layer 2 or 3.

Note:
 - A group directory **should not** have regular files as its **direct descendants**.
     So in this example, you cannot have a `$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles/foo.txt`
     or `$XDG_CONFIG_HOME/shadowsocks-gtk-rs/profiles/My-nested-group/bar.conf`.
 - The one exception to this is the `.ss_ignore` file. See [Other miscellaneous details](#other-miscellaneous-details).
 - Symlinks are not currently supported. I recognize their potential usefulness,
     but I am concerned about circular symlinking causing unnecessary trouble.

## Other miscellaneous details

 - You can create a file named `.ss_ignore` in any profile or group's directory
     to disable it and all its children.
