# hello-world

A Vite app that runs Bolt in the browser. `src/main.ts` imports
`src/hello.bolt`:

```ts
import { run } from './hello.bolt'
run({ stdout: (line) => console.log(line) })
```

`vite-plugin-bolt.ts` compiles a `.bolt` file with `bolt build` when it's
imported, and gives a module that runs it with Bolt's runtime for
browsers, `runtime/browser.mjs`, which it adds as `virtual:bolt-runtime`.
What the compiler reports shows in Vite's error overlay.

```bash
cargo build -p boltlang-cli   # in the repository, once
npm install
npm run dev
```

The compiler is `$BOLT`, or the one built in this repository, or `bolt`
on the `PATH`.
