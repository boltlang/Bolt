// The playground: Bolt code in a textarea, highlighted by tree-sitter as
// it's typed. `npm run build` puts the grammar, its queries and
// web-tree-sitter in vendor/, and compiles this to dist/.

import { Language, Parser, Query } from "web-tree-sitter";
import { type Queries, layers, runs, toHtml } from "./highlight.js";

const STORAGE_KEY = "bolt-playground:source";

/** The element with `id`, which index.html has. */
function element<T extends HTMLElement>(id: string, type: new () => T): T {
  const found = document.getElementById(id);
  if (!(found instanceof type)) throw new Error(`index.html has no ${type.name} #${id}`);
  return found;
}

const source = element("source", HTMLTextAreaElement);
const highlighted = element("highlighted", HTMLElement);
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

  const syncScroll = (): void => {
    const pre = highlighted.parentElement;
    if (!pre) return;
    pre.scrollTop = source.scrollTop;
    pre.scrollLeft = source.scrollLeft;
  };

  const render = (): void => {
    const text = source.value;
    const before = performance.now();
    // The newline gives an empty last line its height, as in the textarea.
    highlighted.innerHTML = toHtml(text, runs(text, layers(parser, queries, text))) + "\n";
    const ms = (performance.now() - before).toFixed(1);
    status.textContent = `Highlighted with tree-sitter in ${ms} ms`;
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

  source.addEventListener("input", scheduleRender);
  source.addEventListener("scroll", syncScroll);
  source.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      source.blur();
      return;
    }
    if (event.key !== "Tab" || event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return;
    // Indent instead of leaving the editor. Escape, then Tab, still leaves.
    // `execCommand` is deprecated, but it's what keeps the edit undoable.
    event.preventDefault();
    document.execCommand("insertText", false, "  ");
  });
  reset.addEventListener("click", () => {
    source.value = sample;
    render();
    save(sample);
    source.focus();
  });

  source.value = load() ?? sample;
  source.disabled = false;
  reset.disabled = false;
  render();
}

start().catch((error: unknown) => {
  console.error(error);
  const message = error instanceof Error ? error.message : String(error);
  status.classList.add("error");
  status.textContent = `${message}. Did you run \`npm run build\`?`;
});
