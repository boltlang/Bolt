// The runtime of a Wasm module that Bolt compiled, for Node:
//
//   node --experimental-wasm-exnref node.mjs <module> <export>
//
// It calls the export, like `main` or a test, with Node's WASI preview 1
// as `wasi_snapshot_preview1`, without arguments, environment or
// preopened directories, and what `std/js` imports, from `browser.mjs`
// next to it. Any other function the module imports stops the program. It exits with 1 when the export stops with an error, or exits
// with a code other than 0, and with 2 when the module can't be run.
import fs from "node:fs";
import { WASI } from "node:wasi";
import { boltJs, compileOptions, jsString } from "./browser.mjs";
const [, , path, name] = process.argv;
const wasi = new WASI({ version: "preview1", args: [], env: {} });
let instance;
const imports = {
  "wasm:js-string": jsString,
  "bolt:js": boltJs(() => instance.exports.memory),
  wasi_snapshot_preview1: {
    ...wasi.wasiImport,
    // A code other than 0 fails.
    proc_exit: (code) => {
      if (code !== 0) process.stderr.write(`the program exited with code ${code}\n`);
      process.exit(code === 0 ? 0 : 1);
    },
  },
};
try {
  const module = await WebAssembly.compile(fs.readFileSync(path), compileOptions);
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.module !== "wasi_snapshot_preview1" && i.kind === "function") {
      (imports[i.module] ??= {})[i.name] ??= () => {
        throw new Error(`no host for ${i.module}.${i.name}`);
      };
    }
  }
  instance = await WebAssembly.instantiate(module, imports);
  wasi.initialize(instance);
} catch (e) {
  process.stderr.write(`can't run the module: ${e.message}\n`);
  process.exit(2);
}
try {
  instance.exports[name]();
} catch (e) {
  process.stderr.write(`${e.message}\n`);
  process.exit(1);
}
