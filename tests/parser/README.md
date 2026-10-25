# Parser tests

The test cases of the parser (`crates/boltlang-parser`) and of the
tree-sitter grammar (`tree-sitter-bolt`), one file per area of the language.
Each case is a title between two lines of `=`, then the code:

```text
================================================================================
Precedence of binary operators
:rule expr
================================================================================
a + b * c
```

Lines starting with `:` after the title are attributes:

- `:rule <rule>` parses the code with `file` (the default), `item`, `expr`,
  `pattern` or `type_expr`.
- `:error`: the code must not parse.
- `:todo`: the parser doesn't handle the case yet; it's ignored.
- `:skip`: the case is never run.

There are no expected trees yet, so a case only says whether the code
parses. `cargo test -p boltlang-parser --test parser` runs the cases; see
`crates/boltlang-parser/tests/parser.rs` for the details.
