# Changelog

- [Changelog](#changelog)
  - [Unreleased](#unreleased)
    - [Breaking changes](#breaking-changes)
    - [New features](#new-features)
    - [Fixes & maintenance](#fixes--maintenance)
  - [0.3.1](#031)

## Unreleased

### Breaking changes

 - Default profile directory is now `~/config/shadowsocks-gtk-rs/profiles` instead of `~/config/shadowsocks-gtk-rs/config-profiles`.
   - This is mainly to make the vocabulary used in the codebase more consistent.
 - Having multiple profiles with the same name is no longer allowed.
   - This is so that `ssgtkctl switch-profile <NAME>` becomes deterministic.
 - Profile config file (`profile.yaml`) has been reworked to be more structured and robust.
   - Notably, a new `mode` field is now mandatory.
   - You have to update your profiles manually. Sorry about that.
 - The command `BackLog{Show,Hide}` has been renamed to `LogViewer{Show,Hide}`.
   - You should only notice this change if you use `ssgtkctl`.

### New features

 - You can now easily specify a profile to run in `tun` mode, which allows you to use `sslocal` as a system-wide VPN.

### Fixes & maintenance

 - Use `simplelog` crate instead of `simple_logger` crate, which allows for a bit more configuration.
 - Revamped the way `ssgtk` manages and pipes `sslocal` logs. Should improve overall stability.

## 0.3.1

Changes were not documented prior to this version.
