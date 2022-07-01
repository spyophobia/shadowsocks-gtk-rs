# Common Questions and Answers

- [Common Questions and Answers](#common-questions-and-answers)
  - [My tray icon is blank.](#my-tray-icon-is-blank)
  - [Can I bind a shortcut to \<some action\>?](#can-i-bind-a-shortcut-to-some-action)
  - [Why did you pick GTK instead of QT?](#why-did-you-pick-gtk-instead-of-qt)
  - [Why aren't you using GTK4?](#why-arent-you-using-gtk4)
  - [Why target `sslocal` command line API?](#why-target-sslocal-command-line-api)
  - [Why do you only use English?](#why-do-you-only-use-english)

## My tray icon is blank.

This means your system is missing icon files. Usually this happens if you installed using `cargo-install`
(which at present only supports installing the binaries).

You can resolve this by copying [/res/logo/shadowsocks-gtk-rs.png](/res/logo/shadowsocks-gtk-rs.png)
to `/usr/share/icons/hicolor/512x512/apps/` or `~/.local/share/icons/hicolor/512x512/apps/`.
After a reboot the icon should be picked up.

Alternatively `ssgtk` has launch parameters `--icon-theme-dir` and `--icon-name` if you want to use a custom icon.


## Can I bind a shortcut to \<some action\>?

- Yes! `runtime-api` is a default feature of this crate, which provides a `ssgtkctl` binary.
  You can use it to make the application do various things. All you need to do is to bind a system shortcut to it.
  To see what it can do, simply run:
```sh
ssgtkctl --help
```
- Underneath the hood, `ssgtk` built with the `runtime-api` feature starts a listener on a Unix socket,
  to which you can send commands in [JSON5](https://json5.org/).
  The `ssgtkctl` binary is merely a delegate to simplify the sending of said command.
- If you wish to interface with the Unix socket directly, you can take a look at some example commands by running:
```sh
ssgtkctl --print-socket-examples
```

## Why did you pick GTK instead of QT?

GTK's rust binding has significantly better support than that of QT.
I'm too lazy to support both so the choice is obvious.

## Why aren't you using GTK4?

This project depends on `libappindicator` for tray icon support, which only supports GTK3.

In fact `libappindicator` development has moved to `libayatana-appindicator`, which also doesn't support GTK4 just yet
but at least has an open issue for it. See [here](https://github.com/AyatanaIndicators/libayatana-appindicator/issues/22).

And finally, GTK4 is simply a bit too new and shiny for my liking.
I much more prefer the stability and reputation offered by and associated with GTK3.

## Why target `sslocal` command line API?

*Because I'm a lazy arse.*

More seriously though, because it's a stable API and it works. What more can you ask for?

Also making UI is painful. There are so many different flags and arguments you can set,
and it will take me forever to create a UI element for each of them. Much easier instead,
to create UI elements for the most commonly used items, while also giving more advanced users the option
to specify more obscure settings using CLI arguments directly.

As an unintentional by-product, this also means you can just as well specify any other executable with arbitrary arguments,
and this app will happily run it for you. It will just make all the fancy GUI settings useless.

## Why do you only use English?

It's complicated. I choose not to answer this question for the sake of my privacy and security.
