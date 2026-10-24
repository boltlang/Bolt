#!/bin/sh
# Put what the page loads in vendor/: the grammar compiled to wasm, its
# queries, web-tree-sitter, and the language server compiled to wasm. Then
# compile src/ to dist/.
#
# Needs `npm install` first, the tree-sitter CLI, which downloads the
# wasi-sdk the first time, and for the language server the wasm32 target
# of Rust (`rustup target add wasm32-unknown-unknown`) and wasm-bindgen-cli
# at the version of wasm-bindgen in Cargo.lock.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/.." && pwd)
grammar="$root/tree-sitter-bolt"
mkdir -p "$here/vendor"
npx tree-sitter build --wasm -o "$here/vendor/tree-sitter-bolt.wasm" "$grammar"
cp "$grammar/queries/highlights.scm" "$grammar/queries/injections.scm" "$here/vendor/"
cp "$here/node_modules/web-tree-sitter/web-tree-sitter.js" "$here/node_modules/web-tree-sitter/web-tree-sitter.wasm" "$here/vendor/"
cargo build --quiet --release --target wasm32-unknown-unknown -p bolt-language-server-wasm --manifest-path "$root/Cargo.toml"
wasm-bindgen --target web --out-dir "$here/vendor/bolt-language-server" \
  "$root/target/wasm32-unknown-unknown/release/bolt_language_server_wasm.wasm"
"$here/node_modules/.bin/tsc" -p "$here"
