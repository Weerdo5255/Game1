#!/bin/sh
# Builds the release wasm and puts it beside index.html. Serve this directory over http to play.
set -eu
cd "$(dirname "$0")/.."
cargo build -p game --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/quietly_everything.wasm web/quietly_everything.wasm
ls -l web/quietly_everything.wasm
