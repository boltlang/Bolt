# Bolt playground

A React component where you type Bolt code and see it highlighted and
checked, all in the browser:

- tree-sitter highlights it, with the grammar in `../tree-sitter-bolt` and
  its `highlights.scm` and `injections.scm` queries;
- the Bolt language server, compiled to wasm (`bolt-language-server-wasm`)
  and run in a web worker, reports its errors, as wavy lines in the code
  and in a list under it.

The package `std` of `../packages/std` comes with it, so the code gets its
prelude and can import its files. The playground always has the `std` it
was built with.

## Using it

```sh
npm install @boltlang/playground
```

```tsx
import { Playground } from "@boltlang/playground";
import "@boltlang/playground/style.css";

<Playground defaultValue={'print("hello");\n'} style={{ height: 400 }} />;
```

It fills the height of its parent, or what its `style` or `className`
gives it. React 18 or later is needed.

| Prop           | What it does                                                                                            |
| -------------- | ------------------------------------------------------------------------------------------------------- |
| `defaultValue` | The code to start with, and to go back to with the Reset button.                                         |
| `onChange`     | Called with the code after each change.                                                                  |
| `storageKey`   | Keep the code in local storage under this key, so that it survives a reload. Nothing is stored without it. |
| `files`        | More files the code can import, by path relative to it: `{ "shapes.bolt": "…" }` for `import "./shapes"`. |
| `std`          | Whether the code gets the package `std`. Yes by default.                                                  |
| `prelude`      | The package whose prelude the code gets, or `null` for none. `std`'s by default.                          |
| `title`        | Shown at the start of the toolbar, like `<h1>Bolt playground</h1>`.                                       |
| `theme`        | `"light"` or `"dark"`. By default, the colors follow the system's.                                       |
| `className`, `style` | For the outer element, which has the class `bolt-playground`.                                     |

The styles are all under `.bolt-playground`, and its colors are custom
properties, like `--bg` and `--keyword`, which a page can override.

Each playground has its own worker, and so its own language server. The
component loads the worker and its wasm files with `new URL("./…",
import.meta.url)`, which bundlers like Vite and webpack follow, so they
need no configuration: the files end up in the app's assets. The package
isn't bundled itself, so it also works in a browser as it is, with an
import map for `react` and `web-tree-sitter`. web-tree-sitter imports
some of Node's modules only when it runs in Node; its `browser` field in
package.json, which bundlers follow for the browser, leaves them out.

## Working on it

```sh
npm install
npm run build   # src/vendor/, then the package in dist/ and the demo in out/
npm run dev     # the demo, served by Vite, with the component of dist/
npm run serve   # the built demo of out/, on http://localhost:8000
```

`npm run build` needs:

- the tree-sitter CLI, for `tree-sitter build --wasm`, which downloads the
  wasi-sdk into `~/.cache/tree-sitter` the first time;
- the wasm32 target of Rust, `rustup target add wasm32-unknown-unknown`;
- `wasm-bindgen-cli`, at the version of `wasm-bindgen` in `../Cargo.lock`:
  `cargo install wasm-bindgen-cli --version <version>`.

Run it again after changing the grammar, its queries, the compiler or
`std`. It goes in three steps (see `scripts/build.sh`):

1. `src/vendor/`, not in git: the grammar and the language server compiled
   to wasm, and, written by `scripts/generate.mjs`, the queries and the
   files of `std` as modules, so that apps don't have to serve them.
2. `dist/`: the package, compiled from `src/` by `tsc`, one module per
   file, with the wasm files and the style sheet.
3. `out/`: the demo page of `demo/`, built by Vite. It uses the package of
   `dist/` as an app would, and it's what the Pages workflow deploys.

`npm run watch` compiles `src/` to `dist/` as it changes, which the demo
of `npm run dev` picks up. `npm run check` type-checks the component and
the demo.

The code, in `src/`:

- `Playground.tsx`: the component. The textarea isn't controlled by
  React, so that undo keeps working.
- `highlight.ts`: highlighting with tree-sitter.
- `lsp.ts`: a small LSP client, which runs the server in a worker, sends it
  the code as it changes, and pulls its diagnostics. A panic of the server
  can't be caught in wasm, so the client starts a new worker then.
- `worker.ts`: the worker, which passes messages to the server.
- `diagnostics.ts`: the wavy lines and positions of diagnostics.

The highlighting follows `tree-sitter highlight`: when several patterns
capture a node, the last one wins, and the token trees of macros are
highlighted as Bolt. See `src/highlight.ts`.
