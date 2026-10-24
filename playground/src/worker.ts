// The worker that runs the Bolt language server, compiled to wasm, so that
// checking doesn't hold up typing. It passes JSON-RPC messages between the
// page and the server.

import init, { LanguageServer } from "../vendor/bolt-language-server/bolt_language_server_wasm.js";
import type { FromWorker, ToWorker } from "./protocol.js";

// The DOM types of the rest of the playground don't describe a worker.
const scope = self as unknown as {
  onmessage: ((event: MessageEvent<ToWorker>) => void) | null;
  postMessage(message: FromWorker): void;
  close(): void;
};

function send(messages: string[]): void {
  for (const json of messages) scope.postMessage({ type: "lsp", json });
}

// Messages that come before the module is loaded wait for it, in order.
const server = init().then(() => new LanguageServer());
server.catch((error: unknown) => {
  const text = error instanceof Error ? error.message : String(error);
  scope.postMessage({ type: "error", message: `can't load the language server: ${text}` });
});

scope.onmessage = (event) => {
  const message = event.data;
  void server.then((server) => {
    try {
      switch (message.type) {
        case "lsp":
          send(server.handle(message.json));
          break;
        case "file":
          send(server.writeFile(message.path, message.text));
          break;
      }
    } catch (error) {
      const text = error instanceof Error ? error.message : String(error);
      if (error instanceof WebAssembly.RuntimeError) {
        scope.postMessage({ type: "crashed", message: text });
        scope.close();
      } else {
        scope.postMessage({ type: "error", message: text });
      }
    }
  });
};
