#!/usr/bin/sh
set -e # exit on error

cd $(dirname "$0") # enter project root dir
mkdir -p "./local-run" # create temp directory

# optionally run in release mode
if [[ "$1" == "release" ]]; then
  MODE_FLAG="--release"
  shift 1
else
  MODE_FLAG=""
fi
cargo run ${MODE_FLAG} -- \
  -v \
  --profiles-dir "./example-profiles" \
  --app-state "./local-run/app-state.yaml" \
  --api-socket "./local-run/shadowsocks-gtk-rs.sock" \
  --icon-theme-dir "./res/logo" \
  $1 # allows easy adjustment of verbosity level
