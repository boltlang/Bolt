#!/bin/sh
# Build the language server for the browser, and run a session with it in
# Node. Needs Node and wasm-bindgen-cli, at the version of wasm-bindgen in
# Cargo.lock: `cargo install wasm-bindgen-cli --version <version>`.
set -eu
crate=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$crate/../.." && pwd)
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
cargo build --quiet --release --target wasm32-unknown-unknown -p bolt-language-server-wasm --manifest-path "$root/Cargo.toml"
wasm-bindgen --target nodejs --out-dir "$out" "$root/target/wasm32-unknown-unknown/release/bolt_language_server_wasm.wasm"
node "$crate/tests/session.cjs" "$out/bolt_language_server_wasm.js"
