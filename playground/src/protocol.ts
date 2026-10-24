// The messages between the page and the worker that runs the language
// server.

/** From the page to the worker. */
export type ToWorker =
  /** A JSON-RPC message for the server. */
  | { type: "lsp"; json: string }
  /** A file that isn't open, like one of a package, at an absolute path. */
  | { type: "file"; path: string; text: string };

/** From the worker to the page. */
export type FromWorker =
  /** A JSON-RPC message from the server. */
  | { type: "lsp"; json: string }
  /** Something went wrong outside the protocol, like giving a file. */
  | { type: "error"; message: string }
  /**
   * The server panicked. A panic can't be caught in wasm, and the module
   * can't be used after it, so the worker stops, and the page starts a new
   * one.
   */
  | { type: "crashed"; message: string };
