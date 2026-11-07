import fs from "node:fs";
import { WASI } from "node:wasi";
const [, , path, name] = process.argv;
const pending = { 1: [], 2: [] };
const files = { 1: process.stdout, 2: process.stderr };
const flush = (f) => {
  if (pending[f].length > 0) files[f].write(Buffer.from(pending[f]));
  pending[f] = [];
};
const wasi = new WASI({ version: "preview1", args: [], env: {} });
const imports = {
  bolt: { write_file: (f, b) => pending[f].push(b), flush },
  wasi_snapshot_preview1: {
    ...wasi.wasiImport,
    // What `print` wrote goes first, and a code other than 0 fails.
    proc_exit: (code) => {
      flush(1);
      flush(2);
      if (code !== 0) process.stderr.write(`the program exited with code ${code}\n`);
      process.exit(code === 0 ? 0 : 1);
    },
  },
};
let instance;
try {
  const module = await WebAssembly.compile(fs.readFileSync(path));
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.module !== "bolt" && i.module !== "wasi_snapshot_preview1" && i.kind === "function") {
      (imports[i.module] ??= {})[i.name] = () => {
        throw new Error(`no host for ${i.module}.${i.name}`);
      };
    }
  }
  instance = await WebAssembly.instantiate(module, imports);
  wasi.initialize(instance);
} catch (e) {
  process.stderr.write(`can't run the module: ${e.message}
`);
  process.exit(2);
}
try {
  instance.exports[name]();
} catch (e) {
  flush(1);
  flush(2);
  process.stderr.write(`${e.message}
`);
  process.exit(1);
}
