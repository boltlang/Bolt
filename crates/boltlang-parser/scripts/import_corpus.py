#!/usr/bin/env python3
"""Import the tree-sitter-bolt test corpus as parser test inputs.

Usage: scripts/import_corpus.py [path/to/tree-sitter-bolt/test/corpus]

Each test case in the corpus becomes one `.bolt` file in `tests/data/corpus`:
`ok/<file>-<title>.bolt` if tree-sitter parses it cleanly, or
`err/<file>-<title>.bolt` if its expected tree contains ERROR or MISSING.
Existing files in those directories are replaced. The FAILING list is left
alone; update it by hand or with `cargo test --test data -- --ignored`.
"""

import pathlib
import re
import sys

CRATE = pathlib.Path(__file__).resolve().parent.parent
OUT = CRATE / "tests" / "data" / "corpus"
DEFAULT_CORPUS = pathlib.Path.home() / "Projects" / "tree-sitter-bolt" / "test" / "corpus"

HEADER = re.compile(r"^={3,}\n(.*?)\n={3,}\n", re.MULTILINE)
DIVIDER = re.compile(r"^-{3,}$", re.MULTILINE)


def slugify(title: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", title.lower()).strip("_")


def cases(text: str):
    """Yield (title, input, expected tree) for each case in a corpus file."""
    headers = list(HEADER.finditer(text))
    for i, header in enumerate(headers):
        end = headers[i + 1].start() if i + 1 < len(headers) else len(text)
        body = text[header.end():end]
        source, expected = DIVIDER.split(body, maxsplit=1)
        yield header.group(1).strip(), source.strip("\n") + "\n", expected


def main() -> None:
    corpus = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_CORPUS
    for group in ("ok", "err"):
        (OUT / group).mkdir(parents=True, exist_ok=True)
        for old in (OUT / group).glob("*.bolt"):
            old.unlink()
    counts = {"ok": 0, "err": 0}
    for path in sorted(corpus.glob("*.txt")):
        seen = set()
        for title, source, expected in cases(path.read_text()):
            group = "err" if re.search(r"\((ERROR|MISSING)\b", expected) else "ok"
            name = f"{path.stem}-{slugify(title)}"
            while name in seen:
                name += "_"
            seen.add(name)
            (OUT / group / f"{name}.bolt").write_text(source)
            counts[group] += 1
    print(f"imported {counts['ok']} ok and {counts['err']} err cases into {OUT}")


if __name__ == "__main__":
    main()
