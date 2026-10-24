// The playground as a React component: Bolt code in a textarea, highlighted
// by tree-sitter as it's typed, and checked by the Bolt language server,
// compiled to wasm and run in a worker of its own.
//
// The textarea isn't controlled by React: setting its value would lose
// what undo remembers. React only follows what's typed in it.

import {
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Language, Parser, Query } from "web-tree-sitter";
import { type Queries, layers, runs, toHtml } from "./highlight.js";
import { lineAndColumn, underlines } from "./diagnostics.js";
import { Client, type Diagnostic } from "./lsp.js";
import { highlights, injections } from "./vendor/queries.js";
import { stdFiles } from "./vendor/std.js";

/** Where the code and the files next to it are for the language server. */
const ROOT = "/playground";
const URI = `file://${ROOT}/main.bolt`;
/** Where the server finds the package `std`. */
const STD = `${ROOT}/.bolt/packages/std`;

export interface PlaygroundProps {
  /** The code to start with, and to go back to with the Reset button. */
  defaultValue?: string;
  /** Called with the code after each change. */
  onChange?: (value: string) => void;
  /**
   * Keep the code in local storage under this key, so that it's still
   * there after a reload. Without it, nothing is stored.
   */
  storageKey?: string;
  /**
   * More files that the code can import, by their path relative to it:
   * `{ "shapes.bolt": "pub struct Point;" }` for `import "./shapes"`.
   */
  files?: Record<string, string>;
  /** Whether the code gets the package `std`, which comes with the component. Yes by default. */
  std?: boolean;
  /**
   * The package whose prelude the code gets, without importing it, or
   * `null` for none. By default, it's `std`'s.
   */
  prelude?: string | null;
  /** Shown at the start of the toolbar, like a heading. */
  title?: ReactNode;
  /** The colors to use. By default, they follow the system's. */
  theme?: "light" | "dark";
  className?: string;
  style?: CSSProperties;
}

interface Highlighter {
  parser: Parser;
  queries: Queries;
}

// tree-sitter and the grammar are loaded once, for all playgrounds. A
// parser is only used while highlighting, so they can share one.
let highlighter: Promise<Highlighter> | null = null;

function loadHighlighter(): Promise<Highlighter> {
  if (highlighter) return highlighter;
  const loading = (async () => {
    await Parser.init();
    const language = await Language.load(new URL("./vendor/tree-sitter-bolt.wasm", import.meta.url).href);
    const parser = new Parser();
    parser.setLanguage(language);
    return {
      parser,
      queries: { highlights: new Query(language, highlights), injections: new Query(language, injections) },
    };
  })();
  highlighter = loading;
  // So that a later playground tries again.
  loading.catch(() => {
    if (highlighter === loading) highlighter = null;
  });
  return loading;
}

// Local storage can be unavailable, like in a private window.
function load(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function save(key: string, text: string): void {
  try {
    localStorage.setItem(key, text);
  } catch {
    // Not saved, which is fine.
  }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function Playground({
  defaultValue = "",
  onChange,
  storageKey,
  files,
  std = true,
  prelude,
  title,
  theme,
  className,
  style,
}: PlaygroundProps): ReactNode {
  const [initial] = useState(() => (storageKey === undefined ? null : load(storageKey)) ?? defaultValue);
  const [text, setText] = useState(initial);
  const [hl, setHl] = useState<Highlighter | null>(null);
  const [hlError, setHlError] = useState<string | null>(null);
  const [checkStatus, setCheckStatus] = useState({ text: "", isError: false });
  // The latest diagnostics, and the text they're about.
  const [checked, setChecked] = useState<{ diagnostics: Diagnostic[]; text: string }>({ diagnostics: [], text: "" });

  const source = useRef<HTMLTextAreaElement>(null);
  const highlighted = useRef<HTMLPreElement>(null);
  const underlined = useRef<HTMLPreElement>(null);
  const client = useRef<Client | null>(null);
  const onChangeRef = useRef(onChange);
  useLayoutEffect(() => {
    onChangeRef.current = onChange;
  });

  useEffect(() => {
    let live = true;
    loadHighlighter().then(
      (loaded) => live && setHl(loaded),
      (error: unknown) => live && setHlError(`Can't load tree-sitter: ${message(error)}`),
    );
    return () => {
      live = false;
    };
  }, []);

  // A new server when the files or the prelude change. The key keeps an
  // object written in the props from restarting it at each render.
  const filesKey = JSON.stringify([std, files ?? {}]);
  useEffect(() => {
    const [withStd, extra] = JSON.parse(filesKey) as [boolean, Record<string, string>];
    const all = [
      ...(withStd ? Object.entries(stdFiles).map(([path, text]) => ({ path: `${STD}/${path}`, text })) : []),
      ...Object.entries(extra).map(([path, text]) => ({ path: `${ROOT}/${path.replace(/^\.?\//, "")}`, text })),
    ];
    const started = new Client({
      uri: URI,
      files: all,
      prelude,
      onDiagnostics: (diagnostics, text) => setChecked({ diagnostics, text }),
      onStatus: (text, isError) => setCheckStatus({ text, isError }),
    });
    client.current = started;
    void started.start(source.current?.value ?? initial);
    return () => {
      started.dispose();
      if (client.current === started) client.current = null;
    };
  }, [filesKey, prelude, initial]);

  useEffect(() => {
    if (storageKey !== undefined) save(storageKey, text);
  }, [storageKey, text]);

  const { html, took } = useMemo(() => {
    const before = performance.now();
    const spans = hl ? runs(text, layers(hl.parser, hl.queries, text)) : [{ start: 0, end: text.length, name: null }];
    // The newline gives an empty last line its height, as in the textarea.
    return { html: toHtml(text, spans) + "\n", took: hl ? performance.now() - before : null };
  }, [hl, text]);

  // Underlines for another text would be in the wrong places.
  const current = checked.text === text;
  const underlineHtml = current ? underlines(text, checked.diagnostics) + "\n" : "";

  const syncScroll = (): void => {
    const from = source.current;
    if (!from) return;
    for (const layer of [highlighted.current, underlined.current]) {
      if (!layer) continue;
      layer.scrollTop = from.scrollTop;
      layer.scrollLeft = from.scrollLeft;
    }
  };
  useLayoutEffect(syncScroll, [html, underlineHtml]);

  const changed = (value: string): void => {
    setText(value);
    client.current?.change(value);
    onChangeRef.current?.(value);
  };

  const keyDown = (event: KeyboardEvent<HTMLTextAreaElement>): void => {
    if (event.key === "Escape") {
      event.currentTarget.blur();
      return;
    }
    if (event.key !== "Tab" || event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return;
    // Indent instead of leaving the editor. Escape, then Tab, still leaves.
    // `execCommand` is deprecated, but it's what keeps the edit undoable.
    // It also fires `input`, which goes to `changed`.
    event.preventDefault();
    document.execCommand("insertText", false, "  ");
  };

  const reset = (): void => {
    const element = source.current;
    if (!element) return;
    element.value = defaultValue;
    changed(defaultValue);
    element.focus();
  };

  const select = (d: Diagnostic): void => {
    const element = source.current;
    // Only where the text hasn't changed since.
    if (!element || element.value !== checked.text) return;
    element.focus();
    element.setSelectionRange(d.start, Math.max(d.end, d.start));
  };

  const status = hlError
    ? { text: hlError, isError: true }
    : {
        text: [hl ? checkStatus.text : "Loading tree-sitter…", took === null ? "" : `highlighted in ${took.toFixed(1)} ms`]
          .filter(Boolean)
          .join(" · "),
        isError: checkStatus.isError,
      };

  return (
    <div className={["bolt-playground", className].filter(Boolean).join(" ")} data-theme={theme} style={style}>
      <div className="bp-toolbar">
        {title !== undefined && <div className="bp-title">{title}</div>}
        <span className={status.isError ? "bp-status bp-error" : "bp-status"} role="status">
          {status.text}
        </span>
        <button type="button" className="bp-button" onClick={reset}>
          Reset
        </button>
      </div>
      <div className="bp-editor">
        <pre aria-hidden="true" ref={highlighted}>
          <code dangerouslySetInnerHTML={{ __html: html }} />
        </pre>
        <pre aria-hidden="true" className="bp-underlines" ref={underlined}>
          <code dangerouslySetInnerHTML={{ __html: underlineHtml }} />
        </pre>
        <textarea
          ref={source}
          aria-label="Bolt code"
          spellCheck={false}
          autoCapitalize="off"
          autoComplete="off"
          autoCorrect="off"
          defaultValue={initial}
          onChange={(event) => changed(event.currentTarget.value)}
          onScroll={syncScroll}
          onKeyDown={keyDown}
        />
      </div>
      {checked.diagnostics.length > 0 && (
        <section className="bp-problems" aria-label="Problems">
          <ul className={current ? "bp-problem-list" : "bp-problem-list bp-outdated"}>
            {checked.diagnostics.map((d, i) => {
              const { line, column } = lineAndColumn(checked.text, d.start);
              return (
                <li key={i}>
                  <button type="button" className="bp-problem" onClick={() => select(d)}>
                    <span className="bp-where">
                      {line}:{column}
                    </span>
                    <span className={`bp-severity-${d.severity}`}>{d.severity}</span>: {d.message}
                    {d.code && <span className="bp-code">{d.code}</span>}
                    {d.related.map((r, j) => {
                      const at = lineAndColumn(checked.text, r.start);
                      return (
                        <span key={j} className="bp-related">
                          {at.line}:{at.column} {r.message}
                        </span>
                      );
                    })}
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </div>
  );
}
