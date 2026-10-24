// The playground: Bolt code in a textarea, highlighted by tree-sitter as
// it's typed, and checked by the Bolt language server, compiled to wasm and
// run in a worker. `npm run build` puts the grammar, its queries,
// web-tree-sitter and the language server in vendor/, and compiles this to
// dist/.

import { Language, Parser, Query } from "web-tree-sitter";
import { type Queries, layers, runs, toHtml } from "./highlight.js";
import { lineAndColumn, underlines } from "./diagnostics.js";
import { Client, type Diagnostic } from "./lsp.js";

const STORAGE_KEY = "bolt-playground:source";

/** Where the document is for the language server, which finds imports from it. */
const URI = "file:///playground/main.bolt";

/**
 * Where the server finds the package `std`, whose prelude the code gets
 * and which it can import. `npm run build` copies it to vendor/std, with
 * the list of its files in vendor/std/files.json.
 */
const STD_PATH = "/playground/.bolt/packages/std";

/** The element with `id`, which index.html has. */
function element<T extends HTMLElement>(id: string, type: new () => T): T {
  const found = document.getElementById(id);
  if (!(found instanceof type)) throw new Error(`index.html has no ${type.name} #${id}`);
  return found;
}

const source = element("source", HTMLTextAreaElement);
const highlighted = element("highlighted", HTMLElement);
const underlined = element("underlines", HTMLElement);
const problemList = element("problem-list", HTMLUListElement);
const status = element("status", HTMLElement);
const reset = element("reset", HTMLButtonElement);

async function fetchText(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`can't load ${url}: ${response.status} ${response.statusText}`);
  return response.text();
}

// Local storage can be unavailable, like in a private window.
function load(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function save(text: string): void {
  try {
    localStorage.setItem(STORAGE_KEY, text);
  } catch {
    // Not saved, which is fine.
  }
}

// The status line: what the checker says, then how long highlighting took.
let checkStatus = { text: "", isError: false };
let highlightStatus = "";

function showStatus(): void {
  status.textContent = [checkStatus.text, highlightStatus].filter(Boolean).join(" · ");
  status.classList.toggle("error", checkStatus.isError);
}

// The latest diagnostics, and the text they're about.
let checked: { diagnostics: Diagnostic[]; text: string } = { diagnostics: [], text: "" };

/** Show the diagnostics, if they're about the text as it is now. */
function showDiagnostics(): void {
  const text = source.value;
  const current = checked.text === text;
  // Underlines for another text would be in the wrong places.
  underlined.innerHTML = current ? underlines(text, checked.diagnostics) + "\n" : "";
  problemList.classList.toggle("outdated", !current);
}

function listProblems(): void {
  const { diagnostics, text } = checked;
  problemList.replaceChildren(
    ...diagnostics.map((d) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.className = "problem";
      const { line, column } = lineAndColumn(text, d.start);
      button.append(
        span("where", `${line}:${column}`),
        span(`severity-${d.severity}`, d.severity),
        ": ",
        d.message,
        ...(d.code ? [span("code", d.code)] : []),
        ...d.related.map((r) => {
          const at = lineAndColumn(text, r.start);
          return span("related", `${at.line}:${at.column} ${r.message}`);
        }),
      );
      button.addEventListener("click", () => {
        // Only where the text hasn't changed since.
        if (source.value !== text) return;
        source.focus();
        source.setSelectionRange(d.start, Math.max(d.end, d.start));
      });
      item.append(button);
      return item;
    }),
  );
}

function span(className: string, text: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.className = className;
  span.textContent = text;
  return span;
}

async function start(): Promise<void> {
  await Parser.init();
  const language = await Language.load("vendor/tree-sitter-bolt.wasm");
  const parser = new Parser();
  parser.setLanguage(language);
  const queries: Queries = {
    highlights: new Query(language, await fetchText("vendor/highlights.scm")),
    injections: new Query(language, await fetchText("vendor/injections.scm")),
  };
  const sample = await fetchText("sample.bolt");
  const stdFiles: string[] = JSON.parse(await fetchText("vendor/std/files.json"));
  const files = await Promise.all(
    stdFiles.map(async (file) => ({ path: `${STD_PATH}/${file}`, text: await fetchText(`vendor/std/${file}`) })),
  );

  const client = new Client({
    worker: new URL("./worker.js", import.meta.url),
    uri: URI,
    files,
    onDiagnostics(diagnostics, text) {
      checked = { diagnostics, text };
      listProblems();
      showDiagnostics();
    },
    onStatus(text, isError) {
      checkStatus = { text, isError };
      showStatus();
    },
  });

  const syncScroll = (): void => {
    for (const layer of [highlighted, underlined]) {
      const pre = layer.parentElement;
      if (!pre) continue;
      pre.scrollTop = source.scrollTop;
      pre.scrollLeft = source.scrollLeft;
    }
  };

  const render = (): void => {
    const text = source.value;
    const before = performance.now();
    // The newline gives an empty last line its height, as in the textarea.
    highlighted.innerHTML = toHtml(text, runs(text, layers(parser, queries, text))) + "\n";
    highlightStatus = `highlighted in ${(performance.now() - before).toFixed(1)} ms`;
    showStatus();
    showDiagnostics();
    syncScroll();
  };

  let pending = false;
  const scheduleRender = (): void => {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      render();
      save(source.value);
    });
  };

  source.addEventListener("input", () => {
    scheduleRender();
    client.change(source.value);
  });
  source.addEventListener("scroll", syncScroll);
  source.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      source.blur();
      return;
    }
    if (event.key !== "Tab" || event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return;
    // Indent instead of leaving the editor. Escape, then Tab, still leaves.
    // `execCommand` is deprecated, but it's what keeps the edit undoable.
    // It also fires `input`, which renders and checks.
    event.preventDefault();
    document.execCommand("insertText", false, "  ");
  });
  reset.addEventListener("click", () => {
    source.value = sample;
    render();
    save(sample);
    client.change(sample);
    source.focus();
  });

  source.value = load() ?? sample;
  source.disabled = false;
  reset.disabled = false;
  render();
  // Highlighting doesn't wait for the language server.
  void client.start(source.value);
}

start().catch((error: unknown) => {
  console.error(error);
  const message = error instanceof Error ? error.message : String(error);
  checkStatus = { text: `${message}. Did you run \`npm run build\`?`, isError: true };
  showStatus();
});
