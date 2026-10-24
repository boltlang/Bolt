// Showing the diagnostics of the language server: wavy lines under the code,
// in a layer over it, and where they are, for the list under the editor.

import type { Diagnostic, Severity } from "./lsp.js";

const RANK: Record<Severity, number> = { hint: 1, information: 2, warning: 3, error: 4 };

/**
 * HTML for `text` with the ranges of the diagnostics in spans, to be shown
 * transparent with only their underlines. Where they overlap, the most
 * severe one shows. One of no width, like a missing `;`, is reported where
 * the next token starts: it covers the last character before the space in
 * front of that token, like the `1` of `let x = 1` before the next line, or
 * else the character it's at.
 */
export function underlines(text: string, diagnostics: Diagnostic[]): string {
  const severities: (Severity | null)[] = new Array(text.length).fill(null);
  for (const d of diagnostics) {
    let { start, end } = d;
    if (start === end) {
      let before = start;
      while (before > 0 && /\s/.test(text[before - 1] ?? "")) before -= 1;
      if (before < start && before > 0) [start, end] = [before - 1, before];
      else if (start < text.length && text[start] !== "\n") end = start + 1;
      else if (start > 0) start -= 1;
    }
    for (let i = start; i < Math.min(end, text.length); i++) {
      const current = severities[i];
      // A line break can't show an underline.
      if (text[i] !== "\n" && (current == null || RANK[d.severity] > RANK[current])) severities[i] = d.severity;
    }
  }
  let html = "";
  let start = 0;
  for (let i = 1; i <= text.length; i++) {
    if (i < text.length && severities[i] === severities[start]) continue;
    const part = escape(text.slice(start, i));
    const severity = severities[start];
    html += severity == null ? part : `<span class="diagnostic diagnostic-${severity}">${part}</span>`;
    start = i;
  }
  return html;
}

/** The line and column of `offset`, from 1, as editors show them. */
export function lineAndColumn(text: string, offset: number): { line: number; column: number } {
  let line = 1;
  let lineStart = 0;
  for (let i = 0; i < offset && i < text.length; i++) {
    if (text[i] === "\n") {
      line += 1;
      lineStart = i + 1;
    }
  }
  return { line, column: offset - lineStart + 1 };
}

const ENTITIES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;" };

function escape(text: string): string {
  return text.replace(/[&<>]/g, (c) => ENTITIES[c] ?? c);
}
