import BoltMut.Solver

/-!
# Struct declarations

Elsewhere, a struct is the tuple of its fields, at the types they have. A
struct in Bolt is a declaration with parameters, like
`struct VecIter[T, 'v] { data: 'v [T], index: i32 }`: type parameters, the
mutabilities it declares, and its own mutability, which a field without
`mut` or `imm` has. A type like `mut VecIter[Point, imm]` gives them all,
and its fields are the declared ones with the arguments in place
(`struct_args` in `crates/boltlang-hir-ty/src/lower.rs`).

`StructDecl.atMost_ty_iff`: such a type is well-formed, at most `μ`,
exactly when its mutability is at most `μ` and its arguments meet what its
fields imply (`StructDecl.implied`): the atoms that "the fields are at most
the struct's own mutability" splits into. For `VecIter` that's `'v ⊑ m`
and `T ⊑ 'v`. It's what Rust does for `T: 'a`, which it infers from a
struct's fields.

The checker asks something else where a value is made (`struct_holds` in
`infer.rs`, `ArgsHold` here): that each type argument, and each mutability
argument, is at most the struct's mutability. That's neither enough nor
needed, as the examples at the end show: it rejects an `imm Handler[mut
[i32]]` whose `T` is only the parameter of a closure, and accepts
`mut VecIter[mut [i32], imm]`, whose `data` would be an `imm` array of
`mut` arrays.
-/

namespace BoltMut

/-- A struct declaration: `types` type parameters, `tparam 0` to
`tparam (types - 1)`; `muts` mutabilities it declares, `param 0` to
`param (muts - 1)`; and its own mutability, `param muts`. `fields` is the
tuple of its fields, in terms of those. -/
structure StructDecl where
  types : Nat
  muts : Nat
  fields : Ty

namespace StructDecl

/-- The struct's own mutability, as its fields see it. -/
def own (d : StructDecl) : Mu := .param d.muts

/-- The struct type `m S[τs, vs]`: a cell with mutability `m` holding the
fields, with the arguments in place, `m` for the struct's own mutability. -/
def ty (d : StructDecl) (τs : List Ty) (vs : List Mu) (m : Mu) : Ty :=
  .cell m (d.fields.inst (vs ++ [m]) τs)

/-- What the fields imply of the arguments: the atoms of "the fields are at
most the struct's own mutability". -/
def implied (d : StructDecl) : List Atom := split d.own d.fields

/-- What the checker asks of the arguments where a value is made: each type
argument and each mutability argument is at most `m`. -/
def ArgsHold (Δ : Assume) (τs : List Ty) (vs : List Mu) (m : Mu) : Prop :=
  (∀ τ ∈ τs, AtMost Δ m τ) ∧ (∀ v ∈ vs, Le Δ v m)

end StructDecl

/-- An atom with arguments in place of the parameters, as `Sat` says them. -/
def Atom.HoldsInst (Δ : Assume) (ms : List Mu) (τs : List Ty) : Atom → Prop
  | .le m μ => Le Δ (m.subst ms) (μ.subst ms)
  | .tparam i μ => AtMost Δ (μ.subst ms) (τs[i]?.getD .unit)

/-- With arguments in place, a type is at most `μ` exactly when the atoms it
splits into hold for them. -/
theorem atMost_inst_iff (Δ : Assume) (ms : List Mu) (τs : List Ty) (μ : Mu) (t : Ty) :
    AtMost Δ (μ.subst ms) (t.inst ms τs) ↔ ∀ a ∈ split μ t, a.HoldsInst Δ ms τs := by
  induction t generalizing μ with
  | unit => simp [AtMost, Ty.inst, split]
  | int => simp [AtMost, Ty.inst, split]
  | cell m t ih => simp [AtMost, Ty.inst, split, Atom.HoldsInst, ih]
  | fn m a r => simp [AtMost, Ty.inst, split, Atom.HoldsInst]
  | prod a b iha ihb => simp [AtMost, Ty.inst, split, iha, ihb, or_imp, forall_and]
  | tparam i => simp [Ty.inst, split, Atom.HoldsInst]

namespace StructDecl

/-- With one mutability argument for each the struct declares, its own
mutability is the struct's: `struct_args` puts it last. -/
theorem own_subst (d : StructDecl) {vs : List Mu} (hv : vs.length = d.muts) (m : Mu) :
    d.own.subst (vs ++ [m]) = m := by
  simp [own, Mu.subst, ← hv]

/-- A struct type is at most `μ` exactly when its mutability is, and its
arguments meet what its fields imply. With `μ = mut`, that's when the type
is well-formed: when values of it can exist. -/
theorem atMost_ty_iff (d : StructDecl) (Δ : Assume) (τs : List Ty) {vs : List Mu} (hv : vs.length = d.muts)
    (m μ : Mu) :
    AtMost Δ μ (d.ty τs vs m) ↔ Le Δ m μ ∧ ∀ a ∈ d.implied, a.HoldsInst Δ (vs ++ [m]) τs := by
  have h := atMost_inst_iff Δ (vs ++ [m]) τs d.own d.fields
  rw [d.own_subst hv] at h
  simp only [ty, AtMost, implied, h]

end StructDecl

/-! ## Examples

`struct VecIter[T, 'v] { data: 'v [T], index: i32 }`: the fields imply
`'v ⊑ m` and `T ⊑ 'v`. -/

def vecIter : StructDecl := { types := 1, muts := 1, fields := .prod (.cell (.param 0) (.tparam 0)) .int }

example : vecIter.implied = [.le (.param 0) (.param 1), .tparam 0 (.param 0)] := rfl

/-- `mut VecIter[mut [i32], imm]` meets what the checker asks... -/
example : StructDecl.ArgsHold .none [.cell .mutable .int] [.immutable] .mutable :=
  ⟨by simp [AtMost]; exact .refl _, by simp; exact .bot _⟩

/-- ...but has no values: its `data` would be an `imm` array of `mut` ones. -/
example : ¬ AtMost .none .mutable (vecIter.ty [.cell .mutable .int] [.immutable] .mutable) := by
  rw [vecIter.atMost_ty_iff .none _ rfl]
  intro ⟨_, h⟩
  have := h (.tparam 0 (.param 0)) (by simp [vecIter, StructDecl.implied, StructDecl.own, split])
  simp [Atom.HoldsInst, Mu.subst, AtMost] at this
  exact nomatch Le.none_immutable this

/-- `struct Handler[T] { f: fn(T) -> () }`: a closure's parameters aren't
held, so the fields imply nothing about `T`. -/
def handler : StructDecl := { types := 1, muts := 0, fields := .fn (.param 0) (.tparam 0) .unit }

example : handler.implied = [.le (.param 0) (.param 0)] := rfl

/-- An `imm Handler[mut [i32]]` is fine... -/
example : AtMost .none .mutable (handler.ty [.cell .mutable .int] [] .immutable) := by
  rw [handler.atMost_ty_iff .none _ rfl]
  refine ⟨.top _, ?_⟩
  intro a ha
  simp [handler, StructDecl.implied, StructDecl.own, split] at ha
  subst ha
  exact .refl _

/-- ...but the checker rejects it, since `mut [i32]` isn't at most `imm`. -/
example : ¬ StructDecl.ArgsHold .none [.cell .mutable .int] [] .immutable := by
  intro ⟨h, _⟩
  have h₁ : Le .none .mutable .immutable := by simpa [AtMost] using h (.cell .mutable .int) (by simp)
  exact nomatch Le.none_immutable h₁

/-- `struct Tag[T] { id: i32 }` holds no `T` at all. -/
def tag : StructDecl := { types := 1, muts := 0, fields := .int }

example : tag.implied = [] := rfl

example : AtMost .none .mutable (tag.ty [.cell .mutable .int] [] .immutable) := by
  rw [tag.atMost_ty_iff .none _ rfl]
  exact ⟨.top _, by simp [StructDecl.implied, tag, split]⟩

example : ¬ StructDecl.ArgsHold .none [.cell .mutable .int] [] .immutable := by
  intro ⟨h, _⟩
  have h₁ : Le .none .mutable .immutable := by simpa [AtMost] using h (.cell .mutable .int) (by simp)
  exact nomatch Le.none_immutable h₁

end BoltMut
