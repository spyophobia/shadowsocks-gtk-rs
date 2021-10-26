# Common Questions and Answers

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

For examples of how to manually write a `profile.yaml`, see [example-config-profiles/A-good-profile-using-cli-args](example-config-profiles/A-good-profile-using-cli-args) and [example-config-profiles/A-good-profile-using-config-file](example-config-profiles/A-good-profile-using-config-file).
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

Note that a "group directory" can only contain "profile directories" and other "group directories", not regular files.

Also note that symlinks are not currently supported in "group directories". I recognise their potential usefulness, but I am concerned about circular symlinking causing unnecessary trouble.

## Why did I pick GTK instead of QT?
GTK's rust binding has significantly better support than that of QT. I'm too lazy to support both so the choice is obvious.

## Why am I not using GTK4?
This project depends on `libappindicator` for tray icon support, which does not yet support GTK4.

See [here](https://github.com/AyatanaIndicators/libayatana-appindicator/issues/22).

## Why target `sslocal` command line API?
*Because I'm a lazy arse.*

More seriously though, because it's a stable API and it works. What more can you ask for?

Also making UI is painful. There are so many different flags and arguments you can set, and it will take me forever to create a UI element for each of them. Much easier instead, to create UI elements for the most commonly used items, while also giving more advanced users the option to specify more obscure settings using CLI arguments directly.

As an unintentional by-product, this also means you can just as well specify any other executable with arbitrary arguments, and this app will happily run it for you. It will just make all the fancy GUI settings useless.

## Why do I only use English?
It's complicated. I choose not to answer this question for the sake of my privacy and security.
