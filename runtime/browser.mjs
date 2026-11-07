// The runtime of a Wasm module that Bolt compiled, for browsers:
//
//   import { run } from "./browser.mjs";
//   const code = await run("program.wasm");
//
// Browsers have no WASI, so this is a small WASI preview 1 of its own, as
// `wasi_snapshot_preview1`: what the program writes to stdout goes to
// `console.log`, and what it writes to stderr to `console.error`, a line
// at a time. Stdin is empty, there are no arguments, environment or
// preopened directories, and the clocks and random bytes are the
// browser's. Other WASI functions answer `nosys`, and any other function
// the module imports stops the program, unless `imports` gives it.

/** `errno` values. */
const SUCCESS = 0;
const BADF = 8;
const INVAL = 28;
const NOSYS = 52;

/** Thrown by `proc_exit`, to stop the program. */
class Exit extends Error {
  constructor(code) {
    super(`the program exited with code ${code}`);
    this.code = code;
  }
}

/**
 * Where a file's text goes: whole lines, to `write`. What's left at the
 * end goes with `finish`.
 */
class Lines {
  constructor(write) {
    this.write = write;
    this.decoder = new TextDecoder();
    this.text = "";
  }

  push(bytes) {
    this.text += this.decoder.decode(bytes, { stream: true });
    let end;
    while ((end = this.text.indexOf("\n")) >= 0) {
      this.write(this.text.slice(0, end));
      this.text = this.text.slice(end + 1);
    }
  }

  finish() {
    this.text += this.decoder.decode();
    if (this.text.length > 0) this.write(this.text);
    this.text = "";
  }
}

/** The bytes of `source`: a URL, a `Response`, a module or the bytes. */
async function compile(source) {
  if (source instanceof WebAssembly.Module) return source;
  if (typeof source === "string" || source instanceof URL) source = fetch(source);
  if (source instanceof Promise) source = await source;
  if (typeof Response !== "undefined" && source instanceof Response) {
    if (!source.ok) throw new Error(`can't fetch the module: ${source.status} ${source.statusText}`);
    return WebAssembly.compile(await source.arrayBuffer());
  }
  return WebAssembly.compile(source);
}

/**
 * Instantiate the module of `source`, with stdout going to `stdout` and
 * stderr to `stderr`, a line at a time, and `imports` for the functions
 * of other modules it imports. `call(name)` calls the export `name`, like
 * `main` or a test: it gives 0 when it ends, and 1 when it stops with an
 * error, or exits with a code other than 0.
 */
export async function instantiate(source, options = {}) {
  const { stdout = console.log, stderr = console.error, imports: given = {} } = options;
  const module = await compile(source);
  const files = { 1: new Lines(stdout), 2: new Lines(stderr) };
  const finish = () => {
    files[1].finish();
    files[2].finish();
  };
  let memory;
  const view = () => new DataView(memory.buffer);
  const bytes = (at, len) => new Uint8Array(memory.buffer, at, len);
  // The iovecs at `iovs`, as `(address, length)`.
  const iovecs = (iovs, count) => {
    const out = [];
    for (let i = 0; i < count; i++) {
      out.push([view().getUint32(iovs + 8 * i, true), view().getUint32(iovs + 8 * i + 4, true)]);
    }
    return out;
  };
  const start = performance.now();
  const wasi = {
    fd_write(fd, iovs, count, written) {
      const file = files[fd];
      if (!file) return BADF;
      let n = 0;
      for (const [at, len] of iovecs(iovs, count)) {
        // A copy, since the memory can grow while the text waits.
        file.push(bytes(at, len).slice());
        n += len;
      }
      view().setUint32(written, n, true);
      return SUCCESS;
    },
    // Stdin is empty.
    fd_read(fd, _iovs, _count, read) {
      if (fd !== 0) return BADF;
      view().setUint32(read, 0, true);
      return SUCCESS;
    },
    args_sizes_get(count, size) {
      view().setUint32(count, 0, true);
      view().setUint32(size, 0, true);
      return SUCCESS;
    },
    args_get: () => SUCCESS,
    environ_sizes_get(count, size) {
      view().setUint32(count, 0, true);
      view().setUint32(size, 0, true);
      return SUCCESS;
    },
    environ_get: () => SUCCESS,
    clock_res_get(id, res) {
      if (id < 0 || id > 3) return INVAL;
      view().setBigUint64(res, 1000n, true);
      return SUCCESS;
    },
    clock_time_get(id, _precision, time) {
      let ns;
      if (id === 0) {
        ns = BigInt(Math.round((performance.timeOrigin + performance.now()) * 1e6));
      } else if (id >= 1 && id <= 3) {
        ns = BigInt(Math.round((performance.now() - start) * 1e6));
      } else {
        return INVAL;
      }
      view().setBigUint64(time, ns, true);
      return SUCCESS;
    },
    random_get(buf, len) {
      // `getRandomValues` fills at most 65536 bytes at once.
      for (let at = 0; at < len; at += 65536) {
        crypto.getRandomValues(bytes(buf + at, Math.min(65536, len - at)));
      }
      return SUCCESS;
    },
    sched_yield: () => SUCCESS,
    proc_exit(code) {
      throw new Exit(code);
    },
  };
  const imports = { ...given, wasi_snapshot_preview1: { ...wasi } };
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.kind !== "function") continue;
    if (i.module === "wasi_snapshot_preview1") {
      // Files, sockets and the rest aren't there.
      imports[i.module][i.name] ??= () => NOSYS;
    } else if (!imports[i.module]?.[i.name]) {
      (imports[i.module] ??= {})[i.name] = () => {
        throw new Error(`no host for ${i.module}.${i.name}`);
      };
    }
  }
  const instance = await WebAssembly.instantiate(module, imports);
  memory = instance.exports.memory;
  instance.exports._initialize?.();
  const call = (name) => {
    let code = 0;
    try {
      instance.exports[name]();
    } catch (e) {
      if (e instanceof Exit) {
        code = e.code === 0 ? 0 : 1;
        if (e.code !== 0) files[2].push(new TextEncoder().encode(`${e.message}\n`));
      } else {
        // What was written comes before why it stopped.
        finish();
        stderr(e);
        return 1;
      }
    }
    finish();
    return code;
  };
  return { instance, call };
}

/**
 * Run the export `name` of the module of `source`, `main` by default: 0
 * when it ends, and 1 when it stops with an error. `options` is as for
 * `instantiate`.
 */
export async function run(source, name = "main", options = {}) {
  const { call } = await instantiate(source, options);
  return call(name);
}
