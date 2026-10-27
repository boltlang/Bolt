# A proof of Bolt's mutability rules

A Lean 4 proof that the core of Bolt's mutability rules keeps its promise:
**nothing reachable from an `imm` value ever changes**, and a well-typed
program never gets stuck. It uses only Lean's standard library.

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
| an array of arrays, a struct with a struct field | a cell holding a cell |
| `let ys = xs`: another name for the same array | copying a location |
| `mut [x]`, `[x]` | `new true x`, `new false x` |
| `xs[i]`, `xs[i] = y` | `get e`, `set e y` |
| `'m [T]`, or `[T]` in a signature: either mutability | a mutability parameter, `Mu.param i` |
| a generic function, called with the mutabilities the checker finds | a top-level `Fn` with `arity` parameters, called as `call f ms e` |
| a closure, `\|x\| body` | `lam b x body`, of type `fn m a r`: `mut` or `imm`, as #244 proposes |
| a `let mut` variable a closure captures, which it shares | a `mut` cell |

The typing rules (`BoltMut/Typing.lean`) that the proof is about:

- **Deep `imm`**: `new false e` (an `imm` object) can only hold a type that's
  `imm` all the way down (`DeepImm`).
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

The key lemma for generic functions is `HasType.msubst` in `Lemmas.lean`: a
body typed with its mutability parameters unknown keeps its type when they
become `mut` or `imm`. For closures, it's `HasType.hasLoc_imm`: inside an
`imm` closure, every location is `imm`. `BoltMut/Examples.lean` shows
what's accepted and rejected: changing a `mut` object but not an `imm` one,
an `imm` object holding a `mut` one, a generic function that reads but
can't write, and the hole from #244, an `imm` object holding a closure that
changes what it captured.

The theorems depend only on the axioms `propext` and `Quot.sound` (check with
`#print axioms`); there's no `sorry`.

## What's left out

- **Today's closures.** The proof is about closures with a mutability, as
  #244 proposes; with today's rules, an `imm` value can change through a
  closure it holds. An `imm` closure here doesn't capture a value of a
  generic type, which needs the constraint #243 adds.
- **Inference.** The proof is about the typing rules, with the mutability
  arguments of each call given. That the checker finds them, and the most
  general ones, isn't proven.
- **Fields with their own mutability** (`'v`, #243), and bounds checks:
  a cell is one element or field, so an index out of bounds can't happen.
- **Several fields per struct, enums, tuples, classes and `let mut`.** A
  struct's fields share its mutability, like one cell; enums and tuples
  are values, which only matter through the cells they hold.
