# TODO

Everything left to do, grouped by topic. Each item ends with the crates it
touches, without their `boltlang-` prefix: `hir-ty` is `boltlang-hir-ty`.
`tree-sitter-bolt` is the reference grammar in
`~/Projects/tree-sitter-bolt/grammar.js`, outside this repository.

## Language

- [ ] A prelude. It would bring `String`, numeric classes for arithmetic and
      iteration, which also lets `for` loop over more than arrays.
      *(base-db, hir-def, hir-ty)*
- [ ] The visibility of fields isn't checked: a private field can be read
      and set from any file. *(hir-def, hir-ty)*
- [ ] `G.helper` where `helper` is `pub(package)` in another package says
      "cannot find `helper`"; it could say that `helper` isn't visible, like
      an import does. *(hir-def, hir-ty, hir)*
- [ ] `pub(super)` is treated like `pub`. *(hir-def)*
- [ ] `fn -x` and `fn a - b` define the same name, `-`, so defining both is
      "defined more than once". Prefix operators need names of their own.
      *(hir-def)*
- [ ] Defining a built-in operator, like `fn a + b`, has no effect: `+` in
      expressions is still built in. Only other operators, like `<>`, call
      their definition. *(hir-ty)*
- [ ] A `std` that comes with the compiler, so that projects don't need a
      copy in `.bolt/packages`. Packages would be looked up in the nearest
      `.bolt/packages` first, then in the bundled ones. *(base-db, hir-def,
      cli, bolt-language-server)*
- [ ] A global cache of packages in the home directory, looked up after
      `.bolt/packages`. It needs versions, so a project file like
      `bolt.toml` that lists the dependencies. *(hir-def, cli)*
- [ ] Items in bodies, like a function inside a function, aren't lowered:
      they're `Statement::Item` and can't be referred to. *(hir-def)*
- [ ] Macro invocations aren't expanded. They're opaque in expressions,
      patterns and types, and the type checker gives them the error type.
      *(hir-def, hir-ty)*
- [ ] `pub import` doesn't re-export: an import only sees the items defined
      in the file it imports from, not what that file imports. *(hir-def)*

- [ ] Higher-kinded types: a class parameter applied to arguments, like the
      `F[A]` of `class Functor[F] { fn fmap[A, B](f: fn (A) -> B, x: F[A])
      -> F[B]; }`, with impls for a type constructor like `Functor[Option]`.
      For now `F[A]` is "expected 0 type arguments". *(hir-ty)*

- [ ] Anonymous records like `{ x: 1, y: 2 }`, as structural types that
      have fields like structs do, so that field constraints work on them
      too. Row polymorphism is only needed for typing record extension and
      update, like `{ r | z: 3 }`, on any record. *(parser, hir-def, hir-ty)*
- [ ] Decide how field constraints are compiled: the offset of the field as
      a hidden argument, like a class dictionary, or one copy of the
      function per type. *(hir-ty, codegen)*

## Grammar and parser

The parser covers the whole grammar: the cases of the tree-sitter corpus
parse without errors (`crates/boltlang-parser/tests/data/corpus`), except the
13 that use syntax the parser no longer accepts (see below).
`scripts/compare_corpus.py` compares our trees with tree-sitter's expected
ones; the only differences left are the deliberate ones listed at the end of
this section.

### Removed from the parser, still in the grammar

- [ ] `self` parameters are gone: functions for a type are top-level
      functions called like `x.f()`. Remove them from `grammar.js` too, then
      re-import the corpus with `scripts/import_corpus.py` and remove the 13
      entries at the end of `tests/data/corpus/FAILING`.
      *(tree-sitter-bolt, parser)*

### Not in the grammar yet

The parser accepts these, but the grammar doesn't. The grammar probably needs
updating.

- [ ] Haskell-style operators: the lexer joins runs of operator characters,
      so `<$>` is one operator. The grammar has a fixed set of binary
      operators and of operators that `fn (op)` can define.
      *(tree-sitter-bolt, parser)*
  - [ ] Decide on the precedence of other operators. For now they are
        left-associative and bind tighter than all built-in binary operators
        except `as`, like undeclared operators in Haskell.
  - [ ] Operator characters next to each other need spaces: `x=-1` lexes
        `=-` as one operator, and `x.+` as `x` followed by `.+`.

- [ ] `import Option.*;`, which brings the variants of an enum into scope.
      The path can go through files imported with `as`, like
      `import m.Shape.*;`. *(tree-sitter-bolt, parser)*

### Possible improvements

- [ ] Error recovery reports one error per unexpected token after a missing
      `;`, like rust-analyzer. That can be noisy for badly broken input.
      *(parser)*
- [ ] A token that can't start a parameter ends the parameter list, so the
      rest of it is parsed as statements: `fn f(self, x) = x;` gives several
      errors (`tests/data/err/self_param.bolt`). Skipping to the next `,` or
      `)` would give one. *(parser)*
- [ ] Validation that the grammar leaves to a later stage: e.g. `..base` in a
      struct literal must come last, a `let` expression only makes sense in a
      condition, and the fragment specifier of `$x:frag` is any identifier.
      *(parser, hir-def)*

### Deliberate differences in tree structure

- Outer attributes are part of the item, statement, expression, field or
  parameter they precede, as in rust-analyzer. In the grammar they are
  siblings. A lone attribute (e.g. at the end of a file) stays a sibling.
- Let chains (`let a = b && c`) are `&&` binary expressions with LET_EXPR
  operands, as in rust-analyzer, instead of a `let_chain` node.
- Method calls are calls of a field access (`x.f()`), like in the grammar.
- Doc comments are ordinary comment tokens; `ast::Comment` and
  `ast::HasDocs` tell them apart.

## Type checker

### Not supported yet

These give the warning `W0001` and have the error type, so they don't cause
other errors.

- [ ] Expressions: `?`, `await`, ranges and `yield`. *(hir-ty)*
- [ ] Types: `impl Trait`, bounded types like `A + B`, and types from macro
      invocations. *(hir-ty)*
- [ ] Associated types: paths like `T.Item`, and arguments like
      `Iterator[Item = T]`. *(hir-ty)*
- [ ] Const generics: const parameters as values, and const arguments.
      *(hir-ty)*

### Gaps

- [ ] `for` only loops over arrays, until there's an iterator class.
      *(hir-ty)*
- [ ] Array lengths aren't checked: `[T; 3]` is the same type as `[T]`.
      *(hir-ty)*
- [ ] `FnOnce(A) -> B` and other function classes are plain function types.
      *(hir-ty)*
- [ ] The bounds a class puts on its associated types, like `type Item:
      Show;`, aren't checked against the impls, which needs solving an
      instance for the impl's type. It would be reported next to the other
      checks of impls against their class. *(hir-ty, hir, ide-diagnostics)*

### Cleanups

- [ ] `function_scheme` returns `Result<_, ()>`; a type for the cycle error
      would say what went wrong (clippy's `result_unit_err`). *(hir-ty)*

## Diagnostics

- [ ] "Did you mean `foo`?" for unresolved names, types and imported names,
      from the names in scope by edit distance. *(hir-def, hir,
      ide-diagnostics)*
- [ ] Labels for "no instance", "ambiguous", "infinite type" and "no
      field" errors, pointing at the call or annotation that introduced the
      constraint. For `get_x(true)`, where `fn get_x(p) = p.x`, the label
      would point at the `p.x`.
      Class constraints from instantiating a scheme only have
      `Reason::None` now. *(hir-ty, hir, ide-diagnostics)*
- [ ] A label for the arguments of tuple-variant constructors, like `true` in
      `Opt.Some(true)`. The callee isn't a function, so the argument gets
      `Reason::None`. *(hir-ty, hir)*
- [ ] Leave out type errors inside the range of a syntax error, which are
      usually caused by it. *(hir)*
- [ ] When the file of an import can't be found, the names it imports are
      also reported where they're used, as "cannot find `negate`". Name
      resolution could remember them as coming from a broken import, so
      that only the import is reported. *(hir-def, hir-ty)*
- [ ] `file_diagnostics` isn't a Salsa query, so each call converts every
      diagnostic again (the queries below it are cached). Make it one when
      the language server asks for diagnostics on every change. *(hir)*
- [ ] A documentation page for each code in `DiagnosticCode`, with an
      example and how to fix it. The language server can link to it with
      `code_description`. *(ide-diagnostics, bolt-language-server)*
- [ ] Quick fixes: add missing fields to a struct literal, and add the
      missing items of a class to an impl. *(ide-diagnostics,
      bolt-language-server)*
- [ ] Remove `src/ast.rs`, an empty file from before this crate had a
      purpose. *(hir)*

## Command line

- [ ] `dump-ast --format=native`, which printed a syntax tree back as Bolt
      code, was removed with the emitter of the `boltlang` crate. The `all`
      target of the `Makefile` still uses it. Bring the format back with a
      new emitter, or change the `Makefile`. *(cli, syntax)*

## Language server

- [ ] Watch the files on disk (`workspace/didChangeWatchedFiles`). A file
      that isn't open in the editor is read once, so changes to it made
      elsewhere, like by `git checkout`, aren't seen. *(bolt-language-server)*
- [ ] Clients that can't be asked to refresh diagnostics only pull them for
      the document that changed, so errors that an edit causes or fixes in
      other open documents show up late. Pushing diagnostics with
      `publishDiagnostics` would work for them too. *(bolt-language-server)*
- [ ] Incremental text sync instead of the whole file on every change.
      *(bolt-language-server)*
- [ ] Completion after `x.`: the functions in scope whose first parameter can
      take the type of `x`. *(bolt-language-server, hir, hir-ty)*
