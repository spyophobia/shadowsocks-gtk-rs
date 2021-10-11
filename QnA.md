# Common Questions and Answers

## Why did I pick GTK instead of QT?
GTK's rust binding has significantly better support than that of QT. I'm too lazy to support both so the choice is obvious.

## Why am I not using GTK4?
This project depends on `libappindicator` for tray icon support, which does not yet support GTK4.

See [here](https://github.com/AyatanaIndicators/libayatana-appindicator/issues/22).

## Why target `sslocal` command line API?
*Because I'm a lazy arse.*

More seriously though, because it's a stable API and it works. What more can you ask for?
