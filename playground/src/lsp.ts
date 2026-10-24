// A small LSP client for the playground: it runs the Bolt language server
// in a worker, keeps it up to date with one document, and pulls the
// diagnostics of the document after it changes.

import type { FromWorker, ToWorker } from "./protocol.js";

export type Severity = "error" | "warning" | "information" | "hint";

/** A diagnostic, with offsets into the text it's about. */
export interface Diagnostic {
  start: number;
  end: number;
  severity: Severity;
  code: string | null;
  message: string;
  /** Labels, like the annotation that says which type was expected. */
  related: { start: number; end: number; message: string }[];
}

export interface ClientOptions {
  /** The URL of the worker script, `dist/worker.js`. */
  worker: URL;
  /** The `file:` URI of the document. */
  uri: string;
  /** Files that aren't open, like those of packages. */
  files: { path: string; text: string }[];
  /** New diagnostics for `text`, the text they're about. */
  onDiagnostics(diagnostics: Diagnostic[], text: string): void;
  onStatus(status: string, isError: boolean): void;
}

/** How long to wait after a change before checking, in milliseconds. */
const DELAY = 150;
/**
 * How many times in a row the server can crash before it isn't restarted
 * anymore: when the same text crashes it each time.
 */
const MAX_RESTARTS = 3;

// The error codes of JSON-RPC.
const METHOD_NOT_FOUND = -32601;

interface Pending {
  resolve(result: unknown): void;
  reject(error: Error): void;
}

// What the server sends, loosely: a response, a request or a notification.
interface Message {
  id?: number | string | null;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code: number; message: string };
}

// The parts of the diagnostics of the protocol that the playground uses.
interface LspPosition {
  line: number;
  character: number;
}
interface LspRange {
  start: LspPosition;
  end: LspPosition;
}
interface LspDiagnostic {
  range: LspRange;
  severity?: number;
  code?: string | number;
  message: string;
  relatedInformation?: { location: { uri: string; range: LspRange }; message: string }[];
}

const SEVERITIES: Severity[] = ["error", "warning", "information", "hint"];

export class Client {
  private worker: Worker | null = null;
  private nextId = 1;
  private pending = new Map<number, Pending>();
  private text = "";
  private version = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private restarts = 0;
  private ready = false;
  /** Counts starts, so that a start that was replaced stays quiet. */
  private generation = 0;

  constructor(private readonly options: ClientOptions) {}

  /** Start the server, with `text` as the document. */
  async start(text: string): Promise<void> {
    const generation = ++this.generation;
    try {
      await this.startServer(text);
    } catch (error) {
      if (generation === this.generation) this.options.onStatus((error as Error).message, true);
    }
  }

  private async startServer(text: string): Promise<void> {
    this.text = text;
    this.version += 1;
    this.ready = false;
    this.spawn();
    this.options.onStatus("Starting the language server…", false);
    await this.request("initialize", {
      processId: null,
      clientInfo: { name: "bolt-playground" },
      rootUri: null,
      // JavaScript strings count UTF-16 units, like the protocol by default.
      capabilities: { general: { positionEncodings: ["utf-16"] }, textDocument: { diagnostic: {} } },
    });
    this.notify("initialized", {});
    for (const file of this.options.files) this.post({ type: "file", path: file.path, text: file.text });
    this.notify("textDocument/didOpen", {
      textDocument: { uri: this.options.uri, languageId: "bolt", version: this.version, text: this.text },
    });
    this.ready = true;
    await this.check();
  }

  /** The document is now `text`. */
  change(text: string): void {
    this.text = text;
    this.version += 1;
    if (!this.ready) return;
    this.notify("textDocument/didChange", {
      textDocument: { uri: this.options.uri, version: this.version },
      contentChanges: [{ text }],
    });
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.check(), DELAY);
  }

  /** Pull the diagnostics of the document, unless it changes meanwhile. */
  private async check(): Promise<void> {
    const version = this.version;
    const text = this.text;
    this.options.onStatus("Checking…", false);
    let report: { items?: LspDiagnostic[] };
    try {
      report = (await this.request("textDocument/diagnostic", { textDocument: { uri: this.options.uri } })) as {
        items?: LspDiagnostic[];
      };
    } catch (error) {
      // A restart checks again.
      if (version === this.version) this.options.onStatus(`Checking failed: ${(error as Error).message}`, true);
      return;
    }
    if (version !== this.version) return;
    this.restarts = 0;
    const diagnostics = (report.items ?? []).map((d) => toDiagnostic(d, text, this.options.uri));
    this.options.onDiagnostics(diagnostics, text);
    const errors = diagnostics.filter((d) => d.severity === "error").length;
    const others = diagnostics.length - errors;
    const parts = [errors && plural(errors, "error"), others && plural(others, "warning")].filter(Boolean);
    this.options.onStatus(parts.length ? `${parts.join(", ")}` : "No problems", false);
  }

  private spawn(): void {
    this.worker?.terminate();
    const worker = new Worker(this.options.worker, { type: "module" });
    worker.onmessage = (event: MessageEvent<FromWorker>) => this.receive(event.data);
    worker.onerror = (event) => {
      event.preventDefault();
      this.crashed(event.message || "the worker failed");
    };
    this.worker = worker;
  }

  private receive(message: FromWorker): void {
    switch (message.type) {
      case "lsp":
        this.receiveLsp(JSON.parse(message.json) as Message);
        break;
      case "error":
        console.error(message.message);
        this.options.onStatus(message.message, true);
        break;
      case "crashed":
        this.crashed(message.message);
        break;
    }
  }

  private receiveLsp(message: Message): void {
    if (message.method === undefined) {
      // A response to a request of ours.
      if (typeof message.id !== "number") return;
      const pending = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) pending?.reject(new Error(message.error.message));
      else pending?.resolve(message.result);
    } else if (message.id !== undefined) {
      // A request of the server's. Diagnostics are pulled after every change
      // anyway, so a refresh needs nothing.
      const known = message.method === "workspace/diagnostic/refresh";
      this.send(
        known
          ? { jsonrpc: "2.0", id: message.id, result: null }
          : { jsonrpc: "2.0", id: message.id, error: { code: METHOD_NOT_FOUND, message: `unsupported: ${message.method}` } },
      );
    } else if (message.method === "window/logMessage") {
      const { type, message: text } = message.params as { type: number; message: string };
      (type === 1 ? console.error : type === 2 ? console.warn : console.info)(`[bolt-language-server] ${text}`);
    }
  }

  /** The server crashed: start a new one, unless it keeps crashing. */
  private crashed(message: string): void {
    console.error(`the language server crashed: ${message}`);
    this.worker?.terminate();
    this.worker = null;
    this.ready = false;
    for (const pending of this.pending.values()) pending.reject(new Error("the language server crashed"));
    this.pending.clear();
    if (this.restarts >= MAX_RESTARTS) {
      this.options.onStatus("The language server keeps crashing; reload the page to try again", true);
      return;
    }
    this.restarts += 1;
    void this.start(this.text);
  }

  private request(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  private notify(method: string, params: unknown): void {
    this.send({ jsonrpc: "2.0", method, params });
  }

  private send(message: object): void {
    this.post({ type: "lsp", json: JSON.stringify(message) });
  }

  private post(message: ToWorker): void {
    this.worker?.postMessage(message);
  }
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

function toDiagnostic(d: LspDiagnostic, text: string, uri: string): Diagnostic {
  const lines = lineStarts(text);
  const offset = (p: LspPosition) => offsetOf(lines, text, p);
  return {
    start: offset(d.range.start),
    end: offset(d.range.end),
    severity: SEVERITIES[(d.severity ?? 1) - 1] ?? "error",
    code: d.code === undefined ? null : String(d.code),
    message: d.message,
    related: (d.relatedInformation ?? [])
      .filter((r) => r.location.uri === uri)
      .map((r) => ({ start: offset(r.location.range.start), end: offset(r.location.range.end), message: r.message })),
  };
}

/** Where each line of `text` starts. */
function lineStarts(text: string): number[] {
  const starts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === "\n") starts.push(i + 1);
  return starts;
}

/** The offset of a position in UTF-16 units, as in JavaScript strings. */
function offsetOf(lines: number[], text: string, position: LspPosition): number {
  const start = lines[position.line];
  if (start === undefined) return text.length;
  const end = lines[position.line + 1] ?? text.length;
  return Math.min(start + position.character, end);
}
