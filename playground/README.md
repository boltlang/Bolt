# Bolt playground

A web page where you type Bolt code and see it highlighted and checked:

- tree-sitter highlights it, with the grammar in `../tree-sitter-bolt` and
  its `highlights.scm` and `injections.scm` queries;
- the Bolt language server, compiled to wasm (`bolt-language-server-wasm`)
  and run in a web worker, reports its errors, as wavy lines in the code
  and in a list under it.

The code is `file:///playground/main.bolt` for the server. It can import
the tiny `std` package in `std/`, which the page gives the server as
`/playground/.bolt/packages/std`.

```sh
npm install
npm run build   # compile the grammar to wasm into vendor/, and src/ to dist/
npm run serve   # then open http://localhost:8000
```

The code is TypeScript, in `src/`. `npm run watch` compiles it as it
changes, and `npm run check` only type-checks it; both need the files of
`npm run build` in `vendor/`, for the types of the language server.

- `src/highlight.ts`: highlighting with tree-sitter.
- `src/lsp.ts`: a small LSP client, which runs the server in a worker,
  sends it the code as it changes, and pulls its diagnostics. A panic of
  the server can't be caught in wasm, so the client starts a new worker
  then.
- `src/worker.ts`: the worker, which passes messages to the server.
- `src/diagnostics.ts`: the wavy lines and positions of diagnostics.

`npm run build` needs:

- the tree-sitter CLI, for `tree-sitter build --wasm`, which downloads the
  wasi-sdk into `~/.cache/tree-sitter` the first time;
- the wasm32 target of Rust, `rustup target add wasm32-unknown-unknown`;
- `wasm-bindgen-cli`, at the version of `wasm-bindgen` in `../Cargo.lock`:
  `cargo install wasm-bindgen-cli --version <version>`.

Run it again after changing the grammar, its queries or the compiler.

The highlighting follows `tree-sitter highlight`: when several patterns
capture a node, the last one wins, and the token trees of macros are
highlighted as Bolt. See `src/highlight.ts`.
