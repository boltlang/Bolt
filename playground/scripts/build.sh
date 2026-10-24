#!/bin/sh
# Put what the page loads in vendor/: the grammar compiled to wasm, its
# queries, web-tree-sitter, the language server compiled to wasm, and the
# package `std` of ../packages/std with a list of its files. Then compile
# src/ to dist/, and put the files the page loads, and nothing else, in
# out/, ready to be served as they are.
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
rm -rf "$here/vendor/std"
cp -r "$root/packages/std" "$here/vendor/std"
node -e '
  const fs = require("fs");
  const dir = process.argv[1];
  const files = fs.readdirSync(dir, { recursive: true }).filter((f) => f.endsWith(".bolt")).sort();
  fs.writeFileSync(dir + "/files.json", JSON.stringify(files, null, 2) + "\n");
' "$here/vendor/std"
cargo build --quiet --release --target wasm32-unknown-unknown -p bolt-language-server-wasm --manifest-path "$root/Cargo.toml"
wasm-bindgen --target web --out-dir "$here/vendor/bolt-language-server" \
  "$root/target/wasm32-unknown-unknown/release/bolt_language_server_wasm.wasm"
rm -rf "$here/dist"
"$here/node_modules/.bin/tsc" -p "$here"
out="$here/out"
rm -rf "$out"
mkdir -p "$out/dist" "$out/vendor/bolt-language-server"
cp "$here/index.html" "$here/style.css" "$here/sample.bolt" "$out/"
cp "$here"/dist/*.js "$out/dist/"
cp "$here/vendor/tree-sitter-bolt.wasm" "$here/vendor/highlights.scm" "$here/vendor/injections.scm" \
  "$here/vendor/web-tree-sitter.js" "$here/vendor/web-tree-sitter.wasm" "$out/vendor/"
cp "$here/vendor/bolt-language-server/bolt_language_server_wasm.js" \
  "$here/vendor/bolt-language-server/bolt_language_server_wasm_bg.wasm" "$out/vendor/bolt-language-server/"
(cd "$here/vendor" && node -e '
  for (const file of require("./std/files.json")) console.log("std/" + file);
' | while read -r file; do
  mkdir -p "$out/vendor/$(dirname "$file")"
  cp "$file" "$out/vendor/$file"
done)
cp "$here/vendor/std/files.json" "$out/vendor/std/"
# The source maps aren't in out/, so the comments that point at them go.
find "$out" -name '*.js' -exec sed -i '/^\/\/# sourceMappingURL=/d' {} +
