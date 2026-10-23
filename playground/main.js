// The playground: Bolt code in a textarea, highlighted by tree-sitter as
// it's typed. Run `npm run build` first, which puts the grammar, its queries
// and web-tree-sitter in vendor/.

import { Language, Parser, Query } from "./vendor/web-tree-sitter.js";
import { layers, runs, toHtml } from "./highlight.js";

const STORAGE_KEY = "bolt-playground:source";

const source = document.getElementById("source");
const highlighted = document.getElementById("highlighted");
const status = document.getElementById("status");
const reset = document.getElementById("reset");

async function fetchText(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`can't load ${url}: ${response.status} ${response.statusText}`);
  return response.text();
}

// Local storage can be unavailable, like in a private window.
function load() {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function save(text) {
  try {
    localStorage.setItem(STORAGE_KEY, text);
  } catch {
    // Not saved, which is fine.
  }
}

async function start() {
  await Parser.init();
  const language = await Language.load("vendor/tree-sitter-bolt.wasm");
  const parser = new Parser();
  parser.setLanguage(language);
  const queries = {
    highlights: new Query(language, await fetchText("vendor/highlights.scm")),
    injections: new Query(language, await fetchText("vendor/injections.scm")),
  };
  const sample = await fetchText("sample.bolt");

  function render() {
    const text = source.value;
    const before = performance.now();
    // The newline gives an empty last line its height, as in the textarea.
    highlighted.innerHTML = toHtml(text, runs(text, layers(parser, queries, text))) + "\n";
    const ms = (performance.now() - before).toFixed(1);
    status.textContent = `Highlighted with tree-sitter in ${ms} ms`;
    syncScroll();
  }

  let pending = false;
  function scheduleRender() {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      render();
      save(source.value);
    });
  }

  function syncScroll() {
    highlighted.parentElement.scrollTop = source.scrollTop;
    highlighted.parentElement.scrollLeft = source.scrollLeft;
  }

  source.addEventListener("input", scheduleRender);
  source.addEventListener("scroll", syncScroll);
  source.addEventListener("keydown", (event) => {
    if (event.key !== "Tab" || event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return;
    // Indent instead of leaving the editor. Escape, then Tab, still leaves.
    event.preventDefault();
    document.execCommand("insertText", false, "  ");
  });
  source.addEventListener("keydown", (event) => {
    if (event.key === "Escape") source.blur();
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

start().catch((error) => {
  console.error(error);
  status.classList.add("error");
  status.textContent = `${error.message}. Did you run \`npm run build\`?`;
});
