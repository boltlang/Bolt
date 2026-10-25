#!/usr/bin/env python3
"""Compare our parse trees with tree-sitter's, for the cases in tests/parser.

Usage: scripts/compare_trees.py   (needs `cargo build --bin bolt` and the
`tree-sitter` command)

Both parsers parse every case that parses as a file without errors. The two
trees are lined up by the text each node covers: nodes on both sides that
cover the same text are listed as tree-sitter's chain of kinds next to ours,
and nodes that only one side has are listed separately. That shows the
renames that would make the trees the same, and the structural differences
that renaming can't fix.

Some differences are by design:

- Outer attributes are part of the item they precede, as in rust-analyzer,
  instead of siblings like in the grammar.
- Let chains are `&&` binary expressions, as in rust-analyzer, instead of a
  `let_chain` node.
"""
import collections
import json
import pathlib
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[3]
CASES = ROOT / "tests/parser"
GRAMMAR = ROOT / "tree-sitter-bolt"
BOLT = ROOT / "target/debug/bolt"
TRIVIA = {"WHITESPACE", "COMMENT"}
SEPARATOR = re.compile(r"^={3,}$")


def cases():
    """The (name, input) of each case that parses as a file without errors."""
    for path in sorted(CASES.glob("*.txt")):
        lines = path.read_text().splitlines()
        i = 0
        while i < len(lines):
            if not SEPARATOR.match(lines[i]):
                i += 1
                continue
            j = i + 1
            while j < len(lines) and not SEPARATOR.match(lines[j]):
                j += 1
            head = lines[i + 1:j]
            k = j + 1
            while k < len(lines) and not SEPARATOR.match(lines[k]):
                k += 1
            attributes = [line.strip() for line in head if line.startswith(":")]
            title = " ".join(line for line in head if not line.startswith(":")).strip()
            if not attributes:
                yield f"{path.stem}: {title}", "\n".join(lines[j + 1:k]).strip("\n") + "\n"
            i = k


def ts_nodes(file, text):
    out = subprocess.run(["tree-sitter", "parse", str(file)], cwd=GRAMMAR, capture_output=True, text=True).stdout
    if "ERROR" in out or "MISSING" in out:
        return None
    raw = text.encode()
    line_starts = [0] + [i + 1 for i, b in enumerate(raw) if b == 10]
    nodes = []
    for m in re.finditer(r"\(([a-z_]+) \[(\d+), (\d+)\] - \[(\d+), (\d+)\]", out):
        kind, r1, c1, r2, c2 = m.group(1), *map(int, m.groups()[1:])
        end = line_starts[r2] + c2 if r2 < len(line_starts) else len(raw)
        nodes.append((kind, line_starts[r1] + c1, end))
    return nodes


def our_nodes(file):
    out = subprocess.run([str(BOLT), "dump-ast", "--no-prelude", "--format", "json", str(file)], capture_output=True, text=True)
    if out.returncode != 0:
        return None
    nodes = []

    def content(n):
        """The range of the tokens of `n` that aren't trivia."""
        if "children" not in n:
            return None if n["kind"] in TRIVIA else tuple(n["text_range"])
        ranges = [r for c in n["children"] if (r := content(c))]
        return (ranges[0][0], ranges[-1][1]) if ranges else None

    def walk(n):
        if r := content(n):
            nodes.append((n["kind"], r[0], r[1], "children" in n))
        for c in n.get("children", []):
            walk(c)

    walk(json.loads(out.stdout))
    return nodes


def main():
    pairs, ts_only, ours_only = collections.Counter(), collections.Counter(), collections.Counter()
    examples = {}
    compared = 0
    with tempfile.TemporaryDirectory() as tmp:
        file = pathlib.Path(tmp) / "case.bolt"
        for name, text in cases():
            file.write_text(text)
            ts, ours = ts_nodes(file, text), our_nodes(file)
            if ts is None or ours is None:
                continue
            compared += 1
            by_ts, by_ours = collections.defaultdict(list), collections.defaultdict(list)
            for kind, s, e in ts:
                if kind != "source_file":
                    by_ts[(s, e)].append(kind)
            for kind, s, e, is_node in ours:
                if kind != "SOURCE_FILE":
                    by_ours[(s, e)].append((kind, is_node))
            raw = text.encode()
            for r in set(by_ts) | set(by_ours):
                t = tuple(by_ts.get(r, []))
                nodes = tuple(k for k, is_node in by_ours.get(r, []) if is_node)
                tokens = tuple(k for k, is_node in by_ours.get(r, []) if not is_node)
                example = (name, raw[r[0]:r[1]].decode(errors="replace")[:50])
                if t and (nodes or tokens):
                    key = (t, nodes or (f"token {tokens[0]}",))
                    pairs[key] += 1
                    examples.setdefault(key, example)
                elif t:
                    for k in t:
                        ts_only[k] += 1
                        examples.setdefault(("ts", k), example)
                elif nodes:
                    for k in nodes:
                        ours_only[k] += 1
                        examples.setdefault(("ours", k), example)

    print(f"{compared} cases compared\n")
    print("== The same text on both sides: tree-sitter's kinds => ours")
    for (t, o), n in sorted(pairs.items(), key=lambda kv: (kv[0][0], -kv[1])):
        print(f"{n:5}  {' > '.join(t):45} => {' > '.join(o):55} e.g. {examples[(t, o)][1]!r}")
    print("\n== tree-sitter nodes without one of ours on the same text")
    for k, n in ts_only.most_common():
        print(f"{n:5}  {k:35} e.g. {examples[('ts', k)][1]!r}  ({examples[('ts', k)][0]})")
    print("\n== Our nodes without one of tree-sitter's on the same text")
    for k, n in ours_only.most_common():
        print(f"{n:5}  {k:35} e.g. {examples[('ours', k)][1]!r}  ({examples[('ours', k)][0]})")


if __name__ == "__main__":
    main()
