// Copy web-tree-sitter into src/vendor/, so that the package ships it
// instead of depending on it, with its Node-only imports taken out.
//
//   node scripts/web-tree-sitter.mjs <out dir>
//
// web-tree-sitter runs in browsers and in Node, and takes Node's modules
// with `import("fs/promises")` and `import("module")` only when it runs
// in Node. Bundlers still follow those imports, and webpack and Rspack
// fail on them ("Can't resolve 'fs/promises'"), since they don't provide
// Node's modules in the browser. The component only runs in a browser, so
// they become errors, and apps need no configuration.
//
// The patterns must each be found once: after an update of web-tree-sitter
// that changes them, this fails instead of shipping imports that break.

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const [out] = process.argv.slice(2);
if (!out) throw new Error("usage: web-tree-sitter.mjs <out>");

const require = createRequire(import.meta.url);
// Its package.json isn't exported, but its wasm file is, next to the rest.
const dir = path.dirname(require.resolve("web-tree-sitter/web-tree-sitter.wasm"));

const NODE_ONLY = 'Promise.reject(new Error("@boltlang/playground runs web-tree-sitter in browsers only"))';
const patches = [
  ['await import("fs/promises")', `await ${NODE_ONLY}`],
  ['await import("module")', `await ${NODE_ONLY}`],
];

let js = fs.readFileSync(path.join(dir, "web-tree-sitter.js"), "utf8");
for (const [from, to] of patches) {
  const count = js.split(from).length - 1;
  if (count !== 1) throw new Error(`expected \`${from}\` once in web-tree-sitter.js, found it ${count} times`);
  js = js.replace(from, to);
}
// No other module of Node may be left.
const left = js.match(/import\(\s*["'][^"'.][^"']*["']\s*\)/g);
if (left) throw new Error(`web-tree-sitter.js still imports ${left.join(", ")}`);
// Its source map isn't shipped.
js = js.replace(/\n\/\/# sourceMappingURL=.*\n?$/, "\n");

fs.writeFileSync(path.join(out, "web-tree-sitter.js"), js);
fs.copyFileSync(path.join(dir, "web-tree-sitter.wasm"), path.join(out, "web-tree-sitter.wasm"));
// Its types are in `declare module 'web-tree-sitter' { … }`, which only
// works under that name. Without the wrapper, they're the types of the
// copy. It ends with `export {};`, so what it exports stays the same.
const types = fs.readFileSync(path.join(dir, "web-tree-sitter.d.ts"), "utf8").replace(/\n\/\/# sourceMappingURL=.*\n?$/, "\n");
const wrapped = /^declare module 'web-tree-sitter' \{\n([^]*)\n\}\n*$/.exec(types);
if (!wrapped?.[1]?.includes("export {};")) throw new Error("web-tree-sitter.d.ts isn't one `declare module` anymore");
fs.writeFileSync(path.join(out, "web-tree-sitter.d.ts"), wrapped[1].replace(/^\t/gm, "") + "\n");
