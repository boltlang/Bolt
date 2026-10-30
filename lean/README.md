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
[`site/docs/internals/type-checking/mutability/`](../site/docs/internals/type-checking/mutability/).

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
| a type parameter `T` | `Ty.tparam i` |
| `struct Box[T] { item: T }` | `cell m (tparam i)` |
| a field with a mutability of its own, `'v Vec[T]` (#243) | a cell with a mutability parameter |
| what a function assumes, like `T ⊑ imm` or `'v ⊑ 'm` (#243) | `Fn.assume`: an `Assume` |
| a generic function, called with the arguments the checker finds | a top-level `Fn`, called as `call f ms τs e` with mutability and type arguments |
| a closure, `\|x\| body` | `lam b x body`, of type `fn m a r`: `mut` or `imm`, as #244 proposes |
| a `let mut` variable a closure captures, which it shares | a `mut` cell |
| a value a block makes, that nothing outside it has (#209) | a cell of the region `r`: `new true x` of type `cell (region r) t` |

The typing rules (`BoltMut/Typing.lean`) that the proof is about:

- **Deep `imm`, as "at most"**: an object holds only what's at most its
  mutability (`AtMost`), with `imm ⊑ mut`. For `new false e`, an `imm`
  object, that's a type that's `imm` all the way down; a `mut` one can hold
  `imm` and `mut` things.
- **Assumptions** (#243): a parameter is below something only by what the
  function assumes (`Le`, `AtMost`). A type parameter is at most `mut`,
  since every type is, and at most `imm` only if the function assumes
  `T ⊑ imm`. So `fn wrap[T](x: T) = Box { item: x }`, an `imm` box holding
  a `T`, is rejected where it's made (#247), and is fine with that
  assumption. A call must meet the callee's assumptions with its arguments
  (`Sat`).
- **Only `mut` can change**: `set e₁ e₂` needs `e₁ : cell mutable t`, or a
  cell of a region. A mutability parameter is only enough if the function
  assumes it's changeable (`Assume.changeable`, #285): then a call gives it
  `mut` or a region, as rule 1 of the regions page says. Otherwise a
  generic function can read what it's given but not change it.
- **Regions** (#209): `new true e` can make a cell of a region `r` instead
  of a `mut` one, which can be changed like one. What it holds is at most
  `r`: `imm`, or of the same region. A value that would have to hold `mut`
  objects is just `mut`, as the checker decides (#286). A call can give a
  region to a generic function (#285), which can change and make what has
  a changeable mutability parameter. A region that becomes `imm` is copied
  (#292).
- **Aliases share the mutability**: a location has one type in the heap
  typing.
- **Closures have a mutability** (#244): an `imm` closure only sees the
  variables whose types are `imm` all the way down, and only `imm`
  locations. Its type, `fn imm a r`, is at most `imm`, so an `imm` object
  can hold it; a `mut` closure captures anything, and an `imm` object
  can't hold it. Calling either is allowed.

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
| `changes_only_frame` | `Frame.lean` | a program changes only what's reachable from the locations in it that aren't `imm`, its frame (#255) |
| `call_changes_only_what_args_reach` | `Frame.lean` | for a call, that's what the function and its argument reach; what's `imm` all the way down, like an `imm` closure or array, adds nothing |
| `changed_is_in_frame_and_mut` | `Frame.lean` | what a program changes is in its frame and isn't `imm` |
| `steps_sim` | `Sharing.lean` | two runs whose heaps differ only in how `imm` values are stored, each `mut` location being one in both, take the same steps, with results that correspond (#254) |
| `imm_copy_changes_nothing` | `Sharing.lean` | using a copy of an `imm` value in place of it computes the same |
| `imm_merge_changes_nothing` | `Sharing.lean` | using one of two `imm` locations that hold the same value for both computes the same |
| `atMost_iff_split` | `Solver.lean` | the solver's split of "at most" into the mutabilities a type holds, and its type parameters, says exactly what `AtMost` does |
| `Le.eq_immutable`, `Le.eq_mutable` | `Solver.lean` | deciding an unknown below `imm` to be `imm`, or one above `mut` to be `mut`, loses no solution, unless an assumption ends at `imm` or starts at `mut` |
| `le_iff_leSearch` | `Solver.lean` | following assumptions from `m`, and from `mut`, finds every `m ⊑ μ` within as many steps as there are assumptions |
| `AtMost.free`, `AtMost.region_free` | `Syntax.lean` | replacing a region with a mutability keeps "at most"; and what a region holds is `imm` all the way down with the region replaced with `imm` |
| `StructDecl.atMost_ty_iff` | `Structs.lean` | a struct type like `mut VecIter[T, 'v]` is well-formed exactly when its arguments meet what its fields imply, like `T ⊑ 'v` |

The key lemma for generic functions, and for #243, is `HasType.inst` in
`Lemmas.lean`: a body typed under its assumptions, with its parameters
unknown, keeps its type for any arguments that meet them. It rests on
`AtMost.inst` and `Sat.comp` in `Syntax.lean`: "at most" survives
instantiating, and a call inside a generic function still meets its
callee's assumptions once the outer function is called. For closures, it's `HasType.hasLoc_imm`: inside an
`imm` closure, every location is `imm`. `BoltMut/Examples.lean` shows
what's accepted and rejected: changing a `mut` object but not an `imm` one,
an `imm` object holding a `mut` one, a generic function that reads but
can't write, and the hole from #244, an `imm` object holding a closure that
changes what it captured. For structs, it shows #246: `Line { n: 5, ..l }`
with a `mut` `l` is rejected, since it shares `l.a`, a `mut Point`, with an
`imm Line`; giving `a` anew makes it fine. For #243, it shows an `imm Box`
holding a `T` rejected without `T ⊑ imm` and accepted with it, `wrap` called
with an `imm` array but not a `mut` one, and an iterator whose array has a
mutability `'v` of its own: `mut` with any `'v`, `imm` only with
`'v ⊑ imm`.

The theorems depend only on Lean's standard axioms, `propext`,
`Quot.sound` and, in `Purity.lean`, `Frame.lean` and for `le_iff_leSearch`, `Classical.choice` (check with
`#print axioms`); there's no `sorry`.

## What's left out

- **Inference.** The proof is about the typing rules, with the arguments
  of each call and the assumptions of each function given. That the
  checker finds them, and the most general ones, isn't proven, beyond the
  solver's shortcuts for "at most" in `Solver.lean`. `leSearchOld` there is
  how `Solver::mutability_le` followed assumptions before: from `m` only,
  at most 8 steps, which missed chains of 9 and those through `mut`. In
  particular, a closure's mutability is written here; in Bolt, it's a
  parameter `'f` that the checker infers, with assumptions like `T ⊑ 'f`.
- **Generic closures.** Closures are monomorphic within a function: a
  closure can't take a type argument.
- **Struct declarations**, mostly. A struct is the tuple of its fields in
  the language here, with the types they have, so a struct update can
  only share a field at the type it has in the base; Bolt's checker gets
  the same rule from the declared field types (#246). `Structs.lean` adds
  declarations with type and mutability parameters, but only to say which
  of their types are well-formed, not to type programs with them.
- **Bounds checks, enums, classes and `let mut`**: a cell is one element,
  so an index out of bounds can't happen; enums are values, like tuples.
- **The rest of regions** (#209, #292): the freeze that copies a region
  that becomes `imm`, and that leaving the copy out can't be noticed, with
  rules 2, 3 and 5 of the regions page, which is also what #284 needs.
  Also borrowed parameters (#283).
- **Effects that suspend**, like `Async`. Only operations that resume right
  away are covered, as a handler passed in.
