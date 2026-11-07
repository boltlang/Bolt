// What `import ... from './file.bolt'` gives, through `vite-plugin-bolt.ts`.
declare module '*.bolt' {
  interface Options {
    /** Called with each line the program writes to stdout. */
    stdout?: (line: string) => void
    /** Called with each line it writes to stderr, and why it stopped. */
    stderr?: (line: unknown) => void
    /** The functions of other modules it imports. */
    imports?: WebAssembly.Imports
  }
  /** Run the file's top-level code: 0 when it ends, 1 when it stops with an error. */
  export function run(options?: Options): Promise<number>
  /** The instance of the module, and `call(name)` to call one of its exports. */
  export function instantiate(
    options?: Options,
  ): Promise<{ instance: WebAssembly.Instance; call: (name: string) => number }>
  export default run
}
