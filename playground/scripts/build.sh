#!/bin/sh
# Build the component and the demo page.
#
# 1. src/vendor/: what the component needs from outside this folder: the
#    grammar compiled to wasm, the language server compiled to wasm, and as
#    modules the highlighting queries and the files of ../packages/std.
# 2. dist/: the component, compiled from src/ with its types, the wasm
#    files and its style sheet. That's the package.
# 3. out/: the demo page, built by Vite from demo/ with the component of
#    dist/, as an app would. It's what the Pages workflow deploys.
#
# Run it with `npm run build`, which puts tsc and vite on the PATH, after
# `npm install`. It needs the tree-sitter CLI, which downloads the
# wasi-sdk the first time, and for the language server the wasm32 target
# of Rust (`rustup target add wasm32-unknown-unknown`) and wasm-bindgen-cli
# at the version of wasm-bindgen in Cargo.lock.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/.." && pwd)
grammar="$root/tree-sitter-bolt"
vendor="$here/src/vendor"

rm -rf "$vendor"
mkdir -p "$vendor"
npx tree-sitter build --wasm -o "$vendor/tree-sitter-bolt.wasm" "$grammar"
cargo build --quiet --release --target wasm32-unknown-unknown -p bolt-language-server-wasm --manifest-path "$root/Cargo.toml"
wasm-bindgen --target web --out-dir "$vendor/bolt-language-server" \
  "$root/target/wasm32-unknown-unknown/release/bolt_language_server_wasm.wasm"
node "$here/scripts/generate.mjs" "$grammar/queries" "$root/packages/std" "$vendor"

rm -rf "$here/dist"
tsc -p "$here"
mkdir -p "$here/dist/vendor/bolt-language-server"
cp "$vendor/tree-sitter-bolt.wasm" "$here/dist/vendor/"
cp "$vendor/bolt-language-server/bolt_language_server_wasm.js" \
  "$vendor/bolt-language-server/bolt_language_server_wasm_bg.wasm" "$here/dist/vendor/bolt-language-server/"
cp "$here/src/playground.css" "$here/dist/"
