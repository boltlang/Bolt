// Highlighting Bolt with tree-sitter, like `tree-sitter highlight`.
//
// - When several patterns capture the same node, the last one wins, so
//   that a pattern for a special case comes after the general one. In
//   `Shape.Circle(_)`, `Shape` is an uppercase name, so a constructor, and
//   then the start of a path, so a type.
// - A node inside another is painted over it, like an escape sequence
//   inside a string.
// - The token trees of macros are parsed again as Bolt, as the injections
//   query says, and painted over the rest, like `println!` in a
//   `macro_rules!`.

import type { Parser, Query, QueryCapture, Range } from "./vendor/web-tree-sitter.js";

/** How deep injections can be inside each other. */
const MAX_DEPTH = 4;

export interface Queries {
  highlights: Query;
  injections?: Query;
}

/**
 * A capture of a highlights query, with the indices of JavaScript strings,
 * as tree-sitter gives them.
 */
export interface Capture {
  name: string;
  start: number;
  end: number;
}

/** A stretch of text with one capture name, or `null` for none. */
export interface Run {
  start: number;
  end: number;
  name: string | null;
}

/**
 * The captures of `text`, as layers to paint in order: those of the whole
 * text, then those of each part injected into it.
 */
export function layers(parser: Parser, queries: Queries, text: string, ranges?: Range[], depth = 0): Capture[][] {
  const tree = parser.parse(text, null, ranges ? { includedRanges: ranges } : undefined);
  if (!tree) return [];
  try {
    const plain = (capture: QueryCapture): Capture => ({
      name: capture.name,
      start: capture.node.startIndex,
      end: capture.node.endIndex,
    });
    const out = [queries.highlights.captures(tree.rootNode).map(plain)];
    if (queries.injections && depth < MAX_DEPTH) {
      for (const capture of queries.injections.captures(tree.rootNode)) {
        const language = capture.setProperties?.["injection.language"];
        if (capture.name !== "injection.content" || language !== "bolt") continue;
        const { startIndex, endIndex, startPosition, endPosition } = capture.node;
        const range = { startIndex, endIndex, startPosition, endPosition };
        out.push(...layers(parser, queries, text, [range], depth + 1));
      }
    }
    return out;
  } finally {
    // Trees live in the memory of the wasm module, which isn't collected.
    tree.delete();
  }
}

/** The highlighted runs of `text`, from layers of captures in the order to paint them. */
export function runs(text: string, layers: Capture[][]): Run[] {
  const names: (string | null)[] = new Array(text.length).fill(null);
  for (const captures of layers) {
    // The last capture of each node.
    const byNode = new Map<string, Capture>();
    for (const capture of captures) {
      if (capture.start < capture.end) byNode.set(`${capture.start}:${capture.end}`, capture);
    }
    // Outer nodes first, so that the nodes inside them paint over them.
    const ranges = [...byNode.values()].sort((a, b) => a.start - b.start || b.end - a.end);
    for (const { start, end, name } of ranges) {
      names.fill(name, start, Math.min(end, text.length));
    }
  }
  const out: Run[] = [];
  let start = 0;
  for (let i = 1; i <= text.length; i++) {
    if (i === text.length || names[i] !== names[start]) {
      out.push({ start, end: i, name: names[start] ?? null });
      start = i;
    }
  }
  return out;
}

/**
 * HTML for `text` with each run of a capture in a `<span>`. A name like
 * `type.builtin` gives the classes `hl-type hl-type-builtin`, so that the
 * style of `type` applies unless `type.builtin` has its own.
 */
export function toHtml(text: string, runs: Run[]): string {
  let html = "";
  for (const { start, end, name } of runs) {
    const part = escape(text.slice(start, end));
    html += name === null ? part : `<span class="${classes(name)}">${part}</span>`;
  }
  return html;
}

function classes(name: string): string {
  const parts = name.split(".");
  return parts.map((_, i) => "hl-" + parts.slice(0, i + 1).join("-")).join(" ");
}

const ENTITIES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;" };

function escape(text: string): string {
  return text.replace(/[&<>]/g, (c) => ENTITIES[c] ?? c);
}
