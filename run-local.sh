#!/usr/bin/sh

cd $(dirname "$0") # enter project root dir
mkdir "./local-run"
cargo run --release -- \
  -v \
  --profiles-dir "./example-config-profiles" \
  --app-settings "./local-run/app-settings.yaml" \
  --icon-theme-dir "./res/logo"
