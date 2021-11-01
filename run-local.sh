#!/usr/bin/sh
set -e # exit on error

cd $(dirname "$0") # enter project root dir
mkdir -p "./local-run" # create temp directory

cargo run --release -- \
  -v \
  --profiles-dir "./example-config-profiles" \
  --app-state "./local-run/app-state.yaml" \
  --api-socket "./local-run/shadowsocks-gtk-rs.sock" \
  --icon-theme-dir "./res/logo" \
  $1 # allows easy adjustment of verbosity level
