#!/bin/sh
# Put what the page loads in vendor/: the grammar compiled to wasm, its
# highlights query, and web-tree-sitter. Run `npm install` first.
# `tree-sitter build --wasm` downloads the wasi-sdk the first time.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
grammar="$here/../tree-sitter-bolt"
mkdir -p "$here/vendor"
tree-sitter build --wasm -o "$here/vendor/tree-sitter-bolt.wasm" "$grammar"
cp "$grammar/queries/highlights.scm" "$grammar/queries/injections.scm" "$here/vendor/"
cp "$here/node_modules/web-tree-sitter/web-tree-sitter.js" "$here/node_modules/web-tree-sitter/web-tree-sitter.wasm" "$here/vendor/"
