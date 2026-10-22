#!/usr/bin/env python3
"""Compare the structure of our parse trees with tree-sitter's expected trees.

Usage: scripts/compare_corpus.py [-v]   (run from the crate directory)

For every corpus case, both trees are reduced to the pre-order sequence of a
set of corresponding node kinds, and the cases where those differ are printed;
`-v` shows the differences. Some differences are by design:

- Outer attributes are part of the item they precede, as in rust-analyzer,
  instead of siblings like in the grammar.
- Let chains are `&&` binary expressions, as in rust-analyzer, instead of a
  `let_chain` node.
"""
import pathlib
import re
import sys

CORPUS = pathlib.Path.home() / "Projects/tree-sitter-bolt/test/corpus"
SNAPS = pathlib.Path(__file__).resolve().parent.parent / "tests" / "snapshots"

TS_TO_OURS = {
    "binary_expression": "BIN_EXPR", "assignment_expression": "BIN_EXPR",
    "compound_assignment_expr": "BIN_EXPR", "unary_expression": "PREFIX_EXPR",
    "call_expression": "CALL_EXPR", "field_expression": "FIELD_EXPR",
    "index_expression": "INDEX_EXPR", "if_expression": "IF_EXPR",
    "match_expression": "MATCH_EXPR", "match_arm": "MATCH_ARM",
    "closure_expression": "CLOSURE_EXPR", "range_expression": "RANGE_EXPR",
    "type_cast_expression": "CAST_EXPR", "try_expression": "TRY_EXPR",
    "await_expression": "AWAIT_EXPR", "struct_expression": "RECORD_EXPR",
    "array_expression": "ARRAY_EXPR", "tuple_expression": "TUPLE_EXPR",
    "parenthesized_expression": "NEST_EXPR", "function_item": "FUNC_DECL",
    "function_signature_item": "FUNC_DECL", "struct_item": "STRUCT", "enum_item": "ENUM",
    "let_declaration": "LET_STMT", "for_expression": "FOR_EXPR",
    "while_expression": "WHILE_EXPR", "loop_expression": "LOOP_EXPR",
    "impl_item": "IMPL", "trait_item": "TRAIT", "macro_invocation": "MACRO_CALL",
    "tuple_pattern": "TUPLE_PATT", "tuple_struct_pattern": "TUPLE_STRUCT_PATT",
    "struct_pattern": "STRUCT_PATT", "range_pattern": "RANGE_PATT",
    "captured_pattern": "CAPTURED_PATT", "slice_pattern": "SLICE_PATT",
    "function_type": "FN_TYPE", "tuple_type": "TUPLE_TYPE", "array_type": "ARRAY_TYPE",
    "abstract_type": "IMPL_TRAIT_TYPE", "bounded_type": "BOUNDED_TYPE",
    "const_item": "CONST", "type_item": "TYPE_ALIAS", "associated_type": "TYPE_ALIAS",
    "import_declaration": "IMPORT", "foreign_mod_item": "EXTERN_BLOCK",
    "macro_definition": "MACRO_RULES", "attribute_item": "ATTR",
    "inner_attribute_item": "ATTR", "block": "BLOCK", "return_expression": "RET_EXPR",
    "break_expression": "BREAK_EXPR", "continue_expression": "CONTINUE_EXPR",
    "yield_expression": "YIELD_EXPR", "let_condition": "LET_EXPR",
    "where_clause": "WHERE_CLAUSE", "type_parameters": "GENERIC_PARAM_LIST",
    "type_arguments": "GENERIC_ARG_LIST", "const_block": "CONST_BLOCK",
    "async_block": "ASYNC_BLOCK", "unit_expression": "TUPLE_EXPR", "unit_type": "TUPLE_TYPE",
}
OURS = set(TS_TO_OURS.values())


def ts_cases():
    for path in sorted(CORPUS.glob("*.txt")):
        text = path.read_text()
        headers = list(re.finditer(r"^={3,}\n(.*?)\n={3,}\n", text, re.M))
        for i, h in enumerate(headers):
            end = headers[i + 1].start() if i + 1 < len(headers) else len(text)
            body = text[h.end():end]
            _, tree = re.split(r"^-{3,}$", body, maxsplit=1, flags=re.M)
            slug = re.sub(r"[^a-z0-9]+", "_", h.group(1).strip().lower()).strip("_")
            yield f"{path.stem}-{slug}", tree


def ts_sequence(tree):
    names = re.findall(r"\(([a-z_]+)", tree)
    seq = [TS_TO_OURS[n] for n in names if n in TS_TO_OURS]
    # Blocks of async/const blocks are part of our BLOCK_EXPR; drop the marker
    # and keep the block.
    return [k for k in seq if k not in ("CONST_BLOCK", "ASYNC_BLOCK")]


def our_sequence(snap):
    body = snap.split("\n---\n", 1)[1].split("\n---\n", 1)[0]
    kinds = re.findall(r"^ *- ([A-Z_]+)$", body, re.M)
    return [k for k in kinds if k in OURS]


bad = 0
for name, tree in ts_cases():
    group = "err" if re.search(r"\((ERROR|MISSING)\b", tree) else "ok"
    snap = SNAPS / f"data__corpus_{group}@{name}.bolt.snap"
    if group == "err" or not snap.exists():
        continue
    a, b = ts_sequence(tree), our_sequence(snap.read_text())
    if a != b:
        bad += 1
        print(f"== {name}")
        if "-v" in sys.argv:
            import difflib
            print("\n".join(difflib.unified_diff(a, b, "tree-sitter", "ours", lineterm="", n=2)))
print(f"{bad} cases differ")
