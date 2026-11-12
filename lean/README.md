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
[`site/docs/internals/designs/mutability/`](../site/docs/internals/designs/mutability/),
and how the checker implements them in
[`site/docs/internals/compiler/type-checking/mutability/`](../site/docs/internals/compiler/type-checking/mutability/).
[`site/docs/internals/proofs/`](../site/docs/internals/proofs/) documents
this proof: the model, each theorem with Bolt code that shows what it
means, and how to work on it.

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
| `ro [T]`, a read-only view (#320) | `cell readonly t`; a cell of any mutability can be seen as one (`Sub`) |
| `ro T`, a type parameter seen as `ro` (#320) | `Ty.roParam i`; a `T` can be seen as one, and instantiated it's the argument seen as `ro` (`Ty.ro`) |
| a type parameter `T` | `Ty.tparam i` |
| `struct Box[T] { item: T }` | `cell m (tparam i)` |
| a field with a mutability of its own, `'v Vec[T]` (#243) | a cell with a mutability parameter |
| what a function assumes, like `T ⊑ imm` or `'v ⊑ 'm` (#243) | `Fn.assume`: an `Assume` |
| a generic function, called with the arguments the checker finds | a top-level `Fn`, called as `call f ms τs e` with mutability and type arguments |
| a closure, `\|x\| body` | `lam b x body`, of type `fn m a r`: `mut` or `imm`, as #244 proposes |
| a `let mut` variable a closure captures, which it shares | a `mut` cell |
| `imm e`: `e`'s value with each `mut` array and struct it reaches copied into an `imm` one (#335) | `toImm false e`, a deep copy (`Freeze`) that copies each `mut` object once and shares what's `imm`; `toImm true e` makes the same copy but goes on with the value, and makes what it copied `imm` where it is; it isn't typed |
| an object knows whether it's `mut`, as in the evaluator | each heap object is a pair: whether it's `mut`, and what it holds |

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
- **Only `mut` can change**: `set e₁ e₂` needs `e₁ : cell mutable t`. A
  mutability parameter isn't enough: a generic function can read what it's
  given but not change it.
- **`imm e`** (#335): `toImm false e` has the type `ImmTy` gives: each cell
  becomes `imm`, a tuple is done part by part, and what's `imm` all the
  way down may stay as it is, which is how closures and type parameters
  are done, since they can't be copied. So `imm` of a `mut` closure is
  rejected, and `imm x` with `x : T` and `T ⊑ imm` is a `T`.
- **Read-only views** (#320): `ro` is a fifth mutability. For holding it
  sits between the two, `imm ⊑ ro ⊑ mut`, so an `imm` object can't hold a
  `ro` one, which may change. For subtyping it's above both: `Sub`
  (`BoltMut/Subtyping.lean`) sees a `mut`, `imm` or `ro` cell as `ro`, and
  its contents too, as long as they are at most `ro`, so the view is deep.
  No object is made `ro`, and `set` never takes one. A cell of any
  mutability, a parameter too, can be seen as `ro`. A value of a type
  parameter `T` can be seen as `ro T`, which is at most `ro`, so `ro [T]`
  needs nothing assumed about `T`. A view leaves a closure's own
  mutability as it is: a closure never changes once it's made, and its
  mutability doesn't keep it from being called, so an object at least
  `ro` can hold any closure. A `ro` literal, like `(xs, 1)` for a `mut`
  `xs`, is a new `mut` object used at a `ro` type from the start, so it
  needs no rule of its own (`BoltMut/ReadOnly.lean`).
- **Subtyping** (#444): any expression can be used at a supertype of its
  type (`sub`). A function fits where one that takes less and gives more
  is expected, like one that takes `ro` where one that takes `mut` is
  (`Sub.fn`), and what an `imm` object holds can be seen as a supertype of
  its type, since it never changes (`Sub.immCell`). The proofs take a
  typing apart up to `sub` (`HasType.app_inv` and the others in
  `Lemmas.lean`).
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
| `imm_fn_changes_nothing` | `Frame.lean` | calling an `imm` function with an argument that's `imm` all the way down changes no location that existed before the call |
| `imm_fn_changes_only_through_handler` | `Frame.lean` | the same with a handler as an extra argument, as an effect whose operations resume right away compiles: only what the handler can reach changes |
| `changes_only_frame` | `Frame.lean` | a program changes only what's reachable from the locations in it that aren't `imm`, its frame (#255) |
| `call_changes_only_what_args_reach` | `Frame.lean` | for a call, that's what the function and its argument reach; what's `imm` all the way down, like an `imm` closure or array, adds nothing |
| `changed_is_in_frame_and_mut` | `Frame.lean` | what a program changes is in its frame and isn't `imm` |
| `Freeze.typed`, `Freeze.exists` | `Soundness.lean` | `imm e`'s copy has `e`'s type with each cell `imm` (`Ty.imm`), and can always be made: so preservation and progress hold for `toImm` |
| `steps_sim` | `Sharing.lean` | two runs whose heaps differ only in how `imm` values are stored, each `mut` location being one in both, take the same steps, with results that correspond (#254). Both copy the same way when they reach `imm e` (`Freeze.sim`) |
| `imm_copy_changes_nothing` | `Sharing.lean` | using a copy of an `imm` value in place of it computes the same |
| `imm_merge_changes_nothing` | `Sharing.lean` | using one of two `imm` locations that hold the same value for both computes the same |
| `HasType.newRo`, `HasType.newRo_tparam` | `ReadOnly.lean` | a `ro` literal, `new true e` used at a `ro` type, has the type `ro t'` for what `e`'s type can be seen as (#320). One holding a value of a type parameter is `ro (ro T)`, with nothing assumed, as #415's `match (a, b)` needs |
| `Sub.trans`, `Sub.imm` | `Subtyping.lean`, `Soundness.lean` | subtyping composes, also through a function's parameters, which are related the other way round; and the copy `imm e` makes of a value seen at a supertype fits the copy of that type |
| `Ty.sub_var`, `StructDecl.sub_view`, `StructDecl.sub_imm` | `Variance.lean` | a struct type with its arguments changed is a subtype of it when each changed parameter is only where it can change that way: covariant in tuples and in `imm` and `ro` objects, contravariant in a function's parameters, invariant in `mut` objects (#440). So `mut Box[mut [i32]]` can be seen as `ro Box[ro [i32]]`, and an `imm Handler[ro [i32]]` used as an `imm Handler[mut [i32]]` |
| `toImms_in_place_change_nothing` | `Unique.lean` | every `toImm` of a program can be in place: the program with every `toImm` in place, in it and its functions, computes what it does, if whenever it gets to a `toImm`, the region of what it makes `imm`, the `mut` objects it reaches (`Region`), is its own: the program has them only there, and no object outside the region holds one (`Expr.UniqToImm`). Freezing in place makes the region `imm` where it is. The simulation gives up the originals and relates each copy to its original (`region_freeze_step`), using what the copy is (`Freeze.shape`): exactly the region, each object copied to its own location (`Freeze.keys`, `Freeze.fresh`). No closure may have a `toImm` |
| `checked_toImms_in_place_change_nothing` | `UniqueCheck.lean` | a check of the program as written gives that premise (#284): every `toImm` is of a variable that a `let` binds to a new `mut` object, `let x = mut [..] in b`, where `b` only reads and writes `x` before making it `imm` last, and doesn't give it another name, put it in an object or a closure, or pass it to a function that keeps it (`Ok`, `Con`, `Bor`). A function borrows its argument when its body only reads and writes it, or passes it to functions that borrow it (`Borrows`), like `fill(xs)`; or a part of a tuple, like `push((xs, 1))`, which the body reaches with `fst` or `snd`. A variable can also hold what a function gives back new, like `make()`, when the function's body is `mut [e]` (`Returns`). Then the program with every `toImm` in place computes what it does. That what each `toImm` makes `imm` holds only what's `imm` already is a premise (`Shallow`): then its region is just the object (`Shallow.region`). The proof keeps an invariant of the running program (`Inv`): each object it owns is `mut`, consumed or gone from it, and no object holds it |
| `atMost_iff_split` | `Solver.lean` | the solver's split of "at most" into the mutabilities a type holds, and its type parameters, says exactly what `AtMost` does |
| `Le.eq_immutable`, `Le.eq_mutable` | `Solver.lean` | deciding an unknown below `imm` to be `imm`, or one above `mut` to be `mut`, loses no solution, unless an assumption ends at `imm` or starts at `mut` |
| `le_iff_leSearch` | `Solver.lean` | following assumptions from `m`, and from `mut`, finds every `m ⊑ μ` within as many steps as there are assumptions |
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
`'v ⊑ imm`. For #320, it shows `fn peek(xs: ro [i32])` taking a `mut`
array and an `imm` one, `fn total(xs: 'm [i32]) = peek(xs)` passing what
it's given on as `ro`, `fn first[T](xs: ro [T]) -> ro T` taking a
`mut [mut [i32]]` with nothing assumed, `fn both[T](p: (T, T))` called with a `mut` and an
`imm` array as `T = ro [i32]`, and that a `ro` view can't be changed or put
in an `imm` object.

The theorems depend only on Lean's standard axioms, `propext`,
`Quot.sound` and `Classical.choice` (check with `#print axioms`); there's
no `sorry`.

## What's left out

- **Inference.** The proof is about the typing rules, with the arguments
  of each call and the assumptions of each function given. That the
  checker finds them, and the most general ones, isn't proven, beyond the
  solver's shortcuts for "at most" in `Solver.lean`. In particular, a closure's mutability is written here; in Bolt, it's a
  parameter `'f` that the checker infers, with assumptions like `T ⊑ 'f`.
- **Generic closures.** Closures are monomorphic within a function: a
  closure can't take a type argument.
- **Struct declarations**, mostly. A struct is the tuple of its fields in
  the language here, with the types they have, so a struct update can
  only share a field at the type it has in the base; Bolt's checker gets
  the same rule from the declared field types (#246). `Structs.lean` adds
  declarations with type and mutability parameters, but only to say which
  of their types are well-formed, and `Variance.lean` how their arguments
  can change, not to type programs with them.
- **Bounds checks, enums, classes and `let mut`**: a cell is one element,
  so an index out of bounds can't happen; enums are values, like tuples.
- **Borrowed parameters** (#283).
- **The rest of `ro`** (#320): a closure's own mutability isn't seen as
  `ro`, so a struct whose closure field has the struct's mutability can't
  be seen as `ro` in the model, though the checker allows it; and there's
  no rule for other places, like `if` branches, which the core doesn't
  have. Impls and inference aren't
  covered either. `imm e` of a `ro T` can't be typed: it has no type
  like the checker's `imm T`.
- **The rest of uniqueness** (#284): a unique object can go to a function
  that borrows it as the whole argument or as one part of a pair, with
  the other part a value. A function that borrows part of a tuple reaches
  it with `fst` or `snd` each time, not by naming it with `let`. A
  function can give back a new unique object, like `StringBuilder.new()`,
  only if its body is `mut [e]` once it's called: one that changes the
  object before it gives it back, like `let b = mut []; b.push(1); b`,
  isn't covered, and left out on purpose for now: the check would have to
  remember that the call gives back an object it owns, and every
  substitution would then need to know what each owned object is used
  for. Freezing where the object is built is covered. A closure can't
  make an `imm` copy in place, and a `toImm`'s argument must be a
  variable. Freezing in place is only covered for an object whose
  contents are `imm` already, and the check doesn't show that: it's a
  premise of `checked_toImms_in_place_change_nothing` (#335).
- **Leaving out the copy** (#293): freezing in place with `toImm`
  (`toImmKeep`) still makes the copy and only goes on with the original.
  That a run that also skips the copy
  computes the same, since nothing uses it, isn't proven.
- **Effects that suspend**, like `Async`. Only operations that resume right
  away are covered, as a handler passed in.
