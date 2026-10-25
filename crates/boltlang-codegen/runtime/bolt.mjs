// The imports of the module `bolt` that compiled Bolt programs need, for
// browsers and Node.
//
//     import { instantiate } from "./bolt.mjs";
//
//     const bytes = await (await fetch("program.wasm")).arrayBuffer();
//     const program = await instantiate(bytes, { print: (text) => console.log(text) });
//     program.exports.main();
//
// A string is the address in the module's memory of its length, as a
// little-endian `u32`, followed by its UTF-8 bytes.

const decoder = new TextDecoder();

/// The string at `address` in `memory`.
export function readString(memory, address) {
  const length = new DataView(memory.buffer).getUint32(address, true);
  return decoder.decode(new Uint8Array(memory.buffer, address + 4, length));
}

/// The imports of the module `bolt`. `print` gets the text to show, and
/// defaults to `console.log`; `getMemory` gives the memory of the instance
/// once it exists.
export function boltImports(getMemory, { print = (text) => console.log(text) } = {}) {
  return {
    print: (address) => print(readString(getMemory(), address)),
    panic: (address) => {
      throw new Error(`panicked: ${readString(getMemory(), address)}`);
    },
  };
}

/// Instantiate a compiled program. `env` gives the functions of its
/// `extern` blocks.
export async function instantiate(bytes, { print, env = {} } = {}) {
  let instance;
  const memory = () => instance.exports.memory;
  const result = await WebAssembly.instantiate(bytes, { bolt: boltImports(memory, { print }), env });
  instance = result.instance;
  return instance;
}
