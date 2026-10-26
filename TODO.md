# TODO

Everything left to do, grouped by topic. Each item ends with the crates it
touches, without their `boltlang-` prefix: `hir-ty` is `boltlang-hir-ty`.
`tree-sitter-bolt` is the reference grammar in `tree-sitter-bolt/grammar.js`.
Each item links to its issue on GitHub.

## Language

- [ ] The prelude of `std` (its `__prelude__/lib.bolt`) could bring
      `String`, numeric classes for arithmetic and iteration, which also
      lets `for` loop over more than arrays. *(hir-ty, std)* (#91)
- [ ] The prelude is set for the whole database. With a project file, it
      could be set per project. *(base-db, cli, bolt-language-server)* (#92)
- [ ] The visibility of fields isn't checked: a private field can be read
      and set from any file. *(hir-def, hir-ty)* (#93)
- [ ] `G.helper` where `helper` is `pub(package)` in another package says
      "cannot find `helper`"; it could say that `helper` isn't visible, like
      an import does. *(hir-def, hir-ty, hir)* (#94)
- [ ] `pub(super)` is treated like `pub`. *(hir-def)* (#95)
- [ ] `fn -x` and `fn a - b` define the same name, `-`, so defining both is
      "defined more than once". Prefix operators need names of their own.
      *(hir-def)* (#96)
- [ ] Defining a built-in operator, like `fn a + b`, has no effect: `+` in
      expressions is still built in. Only other operators, like `<>`, call
      their definition. *(hir-ty)* (#97)
- [ ] A `std` that comes with the compiler, so that projects don't need a
      copy in `.bolt/packages`. Packages would be looked up in the nearest
      `.bolt/packages` first, then in the bundled ones. *(base-db, hir-def,
      cli, bolt-language-server)* (#98)
- [ ] A global cache of packages in the home directory, looked up after
      `.bolt/packages`. It needs versions, so a project file like
      `bolt.toml` that lists the dependencies. *(hir-def, cli)* (#99)
- [ ] Items in bodies, like a function inside a function, aren't lowered:
      they're `Statement::Item` and can't be referred to. *(hir-def)* (#100)
- [ ] Macro invocations aren't expanded. They're opaque in expressions,
      patterns and types, and the type checker gives them the error type.
      *(hir-def, hir-ty)* (#101)
- [ ] When two glob imports, or imports of variants like `import Option.*;`,
      bring in the same name, the first one wins without a word. Using such
      a name could be reported as ambiguous, as in Rust. *(hir-def, hir,
      ide-diagnostics)* (#102)

- [ ] Kind annotations for parameters whose kind can't be inferred from
      their uses, like unused ones, which are `Type`. `[F: ...]` is already
      a bound, so they need a syntax of their own. *(parser, hir-def,
      hir-ty)* (#103)
- [ ] Kinds are inferred per group (a class and its items, an impl and its
      items, or one item). Definitions of different groups that refer to
      each other, like two structs with fields of each other's type, have
      parameters of kind `Type`, even when one is applied to arguments. Kind
      inference over strongly connected groups, as in Haskell, would fix
      this. A `:todo` case in `tests/checker/higher_kinds.txt` shows it.
      *(hir-ty)* (#104)
- [ ] Only structs, enums and type parameters can be type constructors:
      `Functor[Array]` or a partially applied tuple or function type can't
      be written. *(hir-ty)* (#105)
- [ ] `Functor` impls for `Option`, and `Monad`-like classes, in the prelude
      of `std`. *(std)* (#106)

- [ ] Anonymous records like `{ x: 1, y: 2 }`, as structural types that
      have fields like structs do, so that field constraints work on them
      too. Row polymorphism is only needed for typing record extension and
      update, like `{ r | z: 3 }`, on any record. *(parser, hir-def, hir-ty)* (#74)
- [ ] Decide how field constraints are compiled: the offset of the field as
      a hidden argument, like a class dictionary, or one copy of the
      function per type. *(hir-ty, codegen)* (#6)

## Grammar and parser

The parser covers the whole grammar. The tests of both are in `tests/parser`,
which has the cases of the tree-sitter corpus and the parser's own.
`crates/boltlang-parser/scripts/compare_trees.py` compares our trees with
tree-sitter's for these cases.

- [ ] Give the cases of `tests/parser` expected trees, once both parsers
      give the same trees, and run them against the grammar too. A case
      only says whether the code parses for now. *(tree-sitter-bolt,
      parser)* (#107)
- [ ] The grammar's `test/corpus` is a second copy of its cases. Generate
      it from `tests/parser`, or point `tree-sitter test` at it.
      *(tree-sitter-bolt)* (#108)

### Not in the grammar yet

The parser accepts these, but the grammar doesn't. The grammar probably needs
updating.

- [ ] Haskell-style operators: the lexer joins runs of operator characters,
      so `<$>` is one operator. The grammar has a fixed set of binary
      operators and of operators that `fn (op)` can define.
      *(tree-sitter-bolt, parser)* (#110)
  - [ ] Decide on the precedence of other operators. For now they are
        left-associative and bind tighter than all built-in binary operators
        except `as`, like undeclared operators in Haskell. (#111)
  - [ ] Operator characters next to each other need spaces: `x=-1` lexes
        `=-` as one operator, and `x.+` as `x` followed by `.+`. (#112)

- [ ] `import Option.*;`, which brings the variants of an enum into scope.
      The path can go through files imported with `as`, like
      `import m.Shape.*;`. *(tree-sitter-bolt, parser)* (#113)

### Possible improvements

- [ ] Error recovery reports one error per unexpected token after a missing
      `;`, like rust-analyzer. That can be noisy for badly broken input.
      *(parser)* (#114)
- [ ] A token that can't start a parameter ends the parameter list, so the
      rest of it is parsed as statements: `fn f(=, x) = x;` gives several
      errors. Skipping to the next `,` or `)` would give one. *(parser)* (#115)
- [ ] Validation that the grammar leaves to a later stage: e.g. `..base` in a
      struct literal must come last, a `let` expression only makes sense in a
      condition, and the fragment specifier of `$x:frag` is any identifier.
      *(parser, hir-def)* (#116)

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

- [ ] Expressions: `?`, `await`, ranges and `yield`. *(hir-ty)* (#117)
- [ ] Types: `impl Trait`, bounded types like `A + B`, and types from macro
      invocations. *(hir-ty)* (#118)
- [ ] Associated types: paths like `T.Item`, and arguments like
      `Iterator[Item = T]`. *(hir-ty)* (#119)
- [ ] Const generics: const parameters as values, and const arguments.
      *(hir-ty)* (#120)

### Gaps

- [ ] Matches and `let` patterns aren't checked for missing cases:
      `match s { Shape.Circle(r) => r }` is accepted when `Shape` has more
      variants, and so is `match x {}` on an `i32`. Unreachable arms aren't
      reported either. *(hir-ty, hir, ide-diagnostics)* (#121)
- [ ] `for` only loops over arrays, until there's an iterator class.
      *(hir-ty)* (#91)
- [ ] Assigning to a variable that isn't `mut` isn't reported:
      `let x = 1; x = 2;` is accepted, and so is `x += 1`. Decide whether
      assigning to a field or an element, like `p.x = 1` or `xs[0] = 1`,
      needs the variable to be `mut` too, as in Rust. *(hir-ty, hir,
      ide-diagnostics)* (#44)
- [ ] Array lengths aren't checked: `[T; 3]` is the same type as `[T]`.
      *(hir-ty)* (#122)
      *(hir-ty)* (#123)
- [ ] The bounds a class puts on its associated types, like `type Item:
      Show;`, aren't checked against the impls, which needs solving an
      instance for the impl's type. It would be reported next to the other
      checks of impls against their class. *(hir-ty, hir, ide-diagnostics)* (#124)

### Cleanups

- [ ] `function_scheme` returns `Result<_, ()>`; a type for the cycle error
      would say what went wrong (clippy's `result_unit_err`). *(hir-ty)* (#125)

## Code generation

`boltlang-codegen` compiles scalar types, functions, control flow and
`match` on scalars to WebAssembly; `bolt build` runs it.

- [ ] Structs, tuples, enums, and `match` on them, with WasmGC.
      *(codegen)* (#6)
- [ ] Strings made while the program runs, like `a + b`, which need an
      allocator in linear memory, or WasmGC. Literals are data segments.
      *(codegen)* (#6)
- [ ] More builtins: reading input, the clock, converting numbers to
      strings, and a `debug` that shows a value of any type, which Wasm can
      only do per type, after monomorphization. *(hir-ty, eval, codegen,
      std)* (#6)
- [ ] Closures, and functions as values. *(codegen)* (#6)
- [ ] Class methods: record the impl that the solver chooses at each call,
      which the evaluator needs too. *(hir-ty, codegen, eval)* (#6)
- [ ] Integers wrap around when they overflow, where the evaluator reports
      an error; decide which is right. `300.0 as u8` saturates to the range
      of `i32` and then wraps around, where Rust gives 255.
      *(codegen, eval)* (#6)
- [ ] `i128` and `u128`, which Wasm doesn't have. *(codegen)* (#6)
- [ ] Run the compiled code: `bolt run --wasm`, and in the playground.
      *(cli, playground)* (#6)
- [ ] Debug information: the names of locals, and source maps.
      *(codegen)* (#6)

## Diagnostics

- [ ] "Did you mean `foo`?" for unresolved names, types and imported names,
      from the names in scope by edit distance. *(hir-def, hir,
      ide-diagnostics)* (#126)
- [ ] Labels for "no instance", "ambiguous", "infinite type" and "no
      field" errors, pointing at the call or annotation that introduced the
      constraint. For `get_x(True)`, where `fn get_x(p) = p.x`, the label
      would point at the `p.x`.
      Class constraints from instantiating a scheme only have
      `Reason::None` now. *(hir-ty, hir, ide-diagnostics)* (#127)
- [ ] A label for the arguments of tuple-variant constructors, like `True` in
      `Opt.Some(True)`. The callee isn't a function, so the argument gets
      `Reason::None`. *(hir-ty, hir)* (#128)
- [ ] `fn g(c: Bool) -> i32 = if c { 1 };` gives two errors: the branch
      isn't `()`, and the `if` isn't an `i32`. One error that says the `if`
      has no `else` would be clearer, as in Rust. *(hir-ty, hir,
      ide-diagnostics)* (#129)
- [ ] A literal that doesn't fit an annotation is reported as a whole: for
      `let p: Pair[Bool, i32] = Pair { first: 1, second: True };`, the error
      is "expected `Pair[Bool, i32]`, found `Pair[i32, Bool]`" at the whole
      literal. Passing the expected type down to the fields would report
      `1` and `True` instead, as arrays already do for their elements.
      *(hir-ty)* (#130)
- [ ] Leave out type errors inside the range of a syntax error, which are
      usually caused by it. *(hir)* (#131)
- [ ] When the file of an import can't be found, the names it imports are
      also reported where they're used, as "cannot find `negate`". Name
      resolution could remember them as coming from a broken import, so
      that only the import is reported. *(hir-def, hir-ty)* (#132)
- [ ] `file_diagnostics` isn't a Salsa query, so each call converts every
      diagnostic again (the queries below it are cached). Make it one when
      the language server asks for diagnostics on every change. *(hir)* (#133)
- [ ] A documentation page for each code in `DiagnosticCode`, with an
      example and how to fix it. The language server can link to it with
      `code_description`. *(ide-diagnostics, bolt-language-server)* (#134)
- [ ] Quick fixes: add missing fields to a struct literal, and add the
      missing items of a class to an impl. *(ide-diagnostics,
      bolt-language-server)* (#135)
- [ ] Remove `src/ast.rs`, an empty file from before this crate had a
      purpose. *(hir)* (#136)

## Command line

- [ ] `dump-ast --format=native`, which printed a syntax tree back as Bolt
      code, was removed with the emitter of the `boltlang` crate. The `all`
      target of the `Makefile` still uses it. Bring the format back with a
      new emitter, or change the `Makefile`. *(cli, syntax)* (#137)

## Language server

- [ ] Watch the files on disk (`workspace/didChangeWatchedFiles`). A file
      that isn't open in the editor is read once, so changes to it made
      elsewhere, like by `git checkout`, aren't seen. *(bolt-language-server)* (#138)
- [ ] Clients that can't be asked to refresh diagnostics only pull them for
      the document that changed, so errors that an edit causes or fixes in
      other open documents show up late. Pushing diagnostics with
      `publishDiagnostics` would work for them too. *(bolt-language-server)* (#139)
- [ ] Incremental text sync instead of the whole file on every change.
      *(bolt-language-server)* (#140)
- [ ] In the browser (`bolt-language-server-wasm`), the page has to give
      the server every file that isn't open, like those of
      `.bolt/packages`, with `writeFile`. A request from the server for a
      file it can't find would let the page load files as they're needed.
      *(bolt-language-server)* (#141)
- [ ] A panic ends the server in the browser, since wasm can't catch it,
      and the page has to start a new one. *(bolt-language-server)* (#142)
- [ ] Completion after `x.`: the functions in scope whose first parameter can
      take the type of `x`. *(bolt-language-server, hir, hir-ty)* (#143)
