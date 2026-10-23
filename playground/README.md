# Bolt playground

A web page that highlights Bolt code with tree-sitter as you type, using
the grammar in `../tree-sitter-bolt` and its `highlights.scm` and
`injections.scm` queries. It only highlights: nothing is type checked.

```sh
npm install
npm run build   # compile the grammar to wasm and copy it to vendor/
npm run serve   # then open http://localhost:8000
```

`npm run build` runs `tree-sitter build --wasm`, which needs the
tree-sitter CLI and downloads the wasi-sdk into `~/.cache/tree-sitter` the
first time. Run it again after changing the grammar or its queries.

The highlighting follows `tree-sitter highlight`: when several patterns
capture a node, the last one wins, and the token trees of macros are
highlighted as Bolt. See `highlight.js`.
