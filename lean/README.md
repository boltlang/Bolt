# A proof of Bolt's mutability rules

A Lean 4 proof that the core of Bolt's mutability rules keeps its promise:
**nothing reachable from an `imm` value ever changes**, and a well-typed
program never gets stuck. Also that **calling an `imm` function with `imm`
arguments changes nothing that existed before**, except what a handler it's
given can reach, which is what the reactive design needs from effects. It
uses only Lean's standard library.

```sh
cd lean
lake build
```

The rules are described in
[`site/docs/internals/type-checking/mutability.mdx`](../site/docs/internals/type-checking/mutability.mdx).

## The language

A small language with what matters for mutability:

| Bolt | Here |
|---|---|
| an array or struct, shared by every name for it | a *cell* `cell m t`: an object with mutability `m` holding a `t`, at a location in the heap |
| an array of arrays | a cell holding a cell |
| a tuple `(a, b)`, a value | `pair a b`, of type `prod a b` |
| a struct, whose mutability covers its fields | a cell holding the tuple of its fields: `struct Line { a: Point, n: i32 }` is `cell m (prod (cell m int) int)` |
| `l.n = 5` | `set l (pair (fst (get l)) 5)`: the tuple is replaced |
| `Line { n: 5, ..l }` | `new false (pair (fst (get l)) 5)`: the field it doesn't give is shared with `l` |
| `let ys = xs`: another name for the same array | copying a location |
| `mut [x]`, `[x]` | `new true x`, `new false x` |
| `xs[i]`, `xs[i] = y` | `get e`, `set e y` |
| `'m [T]`, or `[T]` in a signature: either mutability | a mutability parameter, `Mu.param i` |
| a generic function, called with the mutabilities the checker finds | a top-level `Fn` with `arity` parameters, called as `call f ms e` |
| a closure, `\|x\| body` | `lam b x body`, of type `fn m a r`: `mut` or `imm`, as #244 proposes |
| a `let mut` variable a closure captures, which it shares | a `mut` cell |

The typing rules (`BoltMut/Typing.lean`) that the proof is about:

- **Deep `imm`, as "at most"**: an object holds only what's at most its
  mutability (`AtMost`), with `imm ⊑ mut`. For `new false e`, an `imm`
  object, that's a type that's `imm` all the way down (`DeepImm`,
  `atMost_immutable`); a `mut` one can hold `imm` and `mut` things.
- **Only `mut` can change**: `set e₁ e₂` needs `e₁ : cell mutable t`. A
  mutability parameter isn't enough, so a generic function can read what
  it's given but not change it.
- **Aliases share the mutability**: a location has one type in the heap
  typing.
- **Closures have a mutability** (#244): an `imm` closure only sees the
  variables whose types are `imm` all the way down, and only `imm`
  locations. Its type, `fn imm a r`, is `DeepImm`, so an `imm` object can
  hold it; a `mut` closure captures anything, and an `imm` object can't
  hold it. Calling either is allowed.

## What's proven

| Theorem | File | Says |
|---|---|---|
| `progress`, `preservation` | `Soundness.lean` | the usual type safety, with a heap that matches its typing (`HeapOk`) |
| `safety` | `Immutability.lean` | a well-typed program never gets stuck |
| `imm_never_changes` | `Immutability.lean` | however many steps a program takes, an `imm` location holds the same value |
| `reaches_only_imm` | `Immutability.lean` | everything reachable from an `imm` location is `imm`, through what closures captured too |
| `reachable_from_imm_never_changes` | `Immutability.lean` | the two together: nothing reachable from an `imm` location ever changes |
| `program_imm_never_changes` | `Immutability.lean` | the same for a whole program, run from an empty heap |
| `imm_fn_changes_nothing` | `Purity.lean` | calling an `imm` function with an argument that's `imm` all the way down changes no location that existed before the call |
| `imm_fn_changes_only_through_handler` | `Purity.lean` | the same with a handler as an extra argument, as an effect whose operations resume right away compiles: only what the handler can reach changes |

The key lemma for generic functions is `HasType.msubst` in `Lemmas.lean`: a
body typed with its mutability parameters unknown keeps its type when they
become `mut` or `imm`. For closures, it's `HasType.hasLoc_imm`: inside an
`imm` closure, every location is `imm`. `BoltMut/Examples.lean` shows
what's accepted and rejected: changing a `mut` object but not an `imm` one,
an `imm` object holding a `mut` one, a generic function that reads but
can't write, and the hole from #244, an `imm` object holding a closure that
changes what it captured. For structs, it shows #246: `Line { n: 5, ..l }`
with a `mut` `l` is rejected, since it shares `l.a`, a `mut Point`, with an
`imm Line`; giving `a` anew makes it fine.

The theorems depend only on Lean's standard axioms, `propext`,
`Quot.sound` and, in `Purity.lean`, `Classical.choice` (check with
`#print axioms`); there's no `sorry`.

## What's left out

- **Today's closures.** The proof is about closures with a mutability, as
  #244 proposes; with today's rules, an `imm` value can change through a
  closure it holds. An `imm` closure here doesn't capture a value of a
  generic type, which needs the constraint #243 adds.
- **Inference.** The proof is about the typing rules, with the mutability
  arguments of each call given. That the checker finds them, and the most
  general ones, isn't proven.
- **Type parameters**, so generic structs like `Box[T]`, and fields with
  their own mutability (`'v`, #243). `AtMost` is the rule #243 states as
  `T ⊑ μ`; with type parameters, a function's type would carry such
  constraints. That's the next step, and it's where #247 is decided.
- **Struct declarations.** A struct is the tuple of its fields here, with
  the types they have, so a struct update can only share a field at the
  type it has in the base. Bolt's checker gets the same rule from the
  declared field types (#246).
- **Bounds checks, enums, classes and `let mut`**: a cell is one element,
  so an index out of bounds can't happen; enums are values, like tuples.
- **Effects that suspend**, like `Async`. Only operations that resume right
  away are covered, as a handler passed in.
