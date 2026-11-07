// A small Vite plugin for Bolt: `import { run } from "./hello.bolt"`
// compiles the file to WebAssembly with `bolt build`, and gives a module
// that runs it with Bolt's runtime for browsers, `runtime/browser.mjs`,
// which the plugin adds to the project as `virtual:bolt-runtime`.
//
// The compiler is `$BOLT`, or the one built in this repository, or `bolt`
// on the `PATH`. What it reports goes to Vite's error overlay.

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import type { Plugin } from "vite";

/** The root of the Bolt repository this example is in. */
const repository = resolve(import.meta.dirname, "../..");

const RUNTIME = "virtual:bolt-runtime";
const RESOLVED_RUNTIME = "\0" + RUNTIME;

export interface BoltOptions {
  /** The `bolt` to compile with. */
  bolt?: string;
  /** Pass `--no-opt` to `bolt build`. */
  optimize?: boolean;
}

/** The compiler to use, as the module docs say. */
function findBolt(): string {
  if (process.env.BOLT) return process.env.BOLT;
  for (const profile of ["debug", "release"]) {
    const built = join(repository, "target", profile, "bolt");
    if (existsSync(built)) return built;
  }
  return "bolt";
}

/** The Wasm module of the Bolt file at `path`, or an error with what the
 * compiler said. */
function compile(bolt: string, path: string, optimize: boolean): Buffer {
  const dir = mkdtempSync(join(tmpdir(), "vite-plugin-bolt-"));
  const out = join(dir, "module.wasm");
  try {
    const args = ["build", path, "-o", out, ...(optimize ? [] : ["--no-opt"])];
    execFileSync(bolt, args, { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8" });
    return readFileSync(out);
  } catch (e) {
    const error = e as { stderr?: string; stdout?: string; message: string };
    const said = `${error.stdout ?? ""}${error.stderr ?? ""}`.trim();
    throw new Error(said || `can't run \`${bolt}\`: ${error.message}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

export default function bolt(options: BoltOptions = {}): Plugin {
  const compiler = options.bolt ?? findBolt();
  const optimize = options.optimize ?? true;
  return {
    name: "bolt",
    resolveId(id) {
      if (id === RUNTIME) return RESOLVED_RUNTIME;
    },
    load(id) {
      // Bolt's runtime for browsers, as it is in the repository.
      if (id === RESOLVED_RUNTIME) {
        const runtime = join(repository, "runtime", "browser.mjs");
        this.addWatchFile(runtime);
        return readFileSync(runtime, "utf8");
      }
      if (!id.endsWith(".bolt")) return;
      const wasm = compile(compiler, id, optimize);
      // The module is small, so it's in the JavaScript, which works the
      // same in `vite dev` and `vite build`.
      return `import { instantiate as instantiateWith, run as runWith } from ${JSON.stringify(RUNTIME)};
const bytes = Uint8Array.from(atob(${JSON.stringify(wasm.toString("base64"))}), (c) => c.charCodeAt(0));
/** Run the file's top-level code: 0 when it ends, 1 when it stops with an error. */
export function run(options) { return runWith(bytes, "main", options); }
/** The instance of the module, and \`call(name)\` to call one of its exports. */
export function instantiate(options) { return instantiateWith(bytes, options); }
export default run;
`;
    },
  };
}
