import BoltMut.Structs

/-!
# Seeing a struct's arguments as `ro` (#440)

A struct type gives its arguments as they are, so `mut Box[mut [i32]]`
for `struct Box[T] { item: T }` can only be seen as `ro Box[mut [i32]]`,
which can't exist: a `ro` value can't hold a `mut` one (E0223). What it
should be seen as is `ro Box[ro [i32]]`. Whether that's sound depends on
where the parameter is in the fields: a `T` in a closure's parameters
can't be seen as `ro T`, since the closure may change what it's given.

A view changes some of the parameters (`Changed`): type parameters, which
are seen as `ro`, and mutability parameters, which become `ro`. The struct
type with those arguments changed is a view of it (`StructDecl.sub_view`)
when each changed parameter is only reached through tuples and objects
whose mutability is a changed parameter (`Ty.coIn`), since those are the
objects that become `ro`. Elsewhere, a changed parameter can't be
(`Ty.fixedIn`):

- in a function type, as `Sub` has nothing for functions but `refl`
  (#444);
- in an object whose mutability is written, like `imm [T]`, since only an
  object seen as `ro` can have what it holds seen as `ro`;
- in an object whose mutability is a parameter that isn't changed.

`ro T` can be anywhere: `T` seen as `ro`, seen as `ro` again, is the same
(`Ty.ro_ro`).

The checker infers the largest set of parameters it can change, once for
each struct; any set for which `Ty.coIn` holds is sound. The view must
still be well-formed, at most `ro`: for an invariant `T` held as it is, a
`ro Box[mut [i32]]` stays an error.
-/

namespace BoltMut

/-- The parameters a view changes: the type parameters `tys` are seen as
`ro`, and the mutability parameters `mus` become `ro`. -/
structure Changed where
  tys : Nat → Bool
  mus : Nat → Bool

/-- A mutability that the changed parameters don't change. -/
def Mu.fixedIn (c : Changed) : Mu → Bool
  | .param j => !c.mus j
  | _ => true

/-- A type that the changed parameters don't change: none of them is in it,
except as `ro T`, which they leave as it is. -/
def Ty.fixedIn (c : Changed) : Ty → Bool
  | .unit | .int | .roParam _ => true
  | .cell m t => m.fixedIn c && t.fixedIn c
  | .fn m a r => m.fixedIn c && a.fixedIn c && r.fixedIn c
  | .prod a b => a.fixedIn c && b.fixedIn c
  | .tparam i => !c.tys i

/-- The changed parameters can be changed in a type: each is reached only
through tuples and objects whose mutability is a changed parameter. -/
def Ty.coIn (c : Changed) : Ty → Bool
  | .unit | .int | .roParam _ | .tparam _ => true
  | .cell m t => match m with
    | .param j => if c.mus j then t.coIn c else t.fixedIn c
    | _ => t.fixedIn c
  | .fn m a r => (Ty.fn m a r).fixedIn c
  | .prod a b => a.coIn c && b.coIn c

/-- `ms'` and `τs'` are the arguments `ms` and `τs` with the changed
parameters changed: a mutability to `ro`, and a type to its view. -/
structure Changed.Views (c : Changed) (ms ms' : List Mu) (τs τs' : List Ty) : Prop where
  mus : ∀ j, ms'[j]?.getD .immutable = if c.mus j then .readonly else ms[j]?.getD .immutable
  tys : ∀ i, τs'[i]?.getD .unit = if c.tys i then (τs[i]?.getD .unit).ro else τs[i]?.getD .unit

theorem Mu.subst_fixed {c : Changed} {ms ms' : List Mu} {τs τs' : List Ty}
    (hv : c.Views ms ms' τs τs') {m : Mu} (h : m.fixedIn c = true) : m.subst ms' = m.subst ms := by
  cases m with
  | param j =>
    simp only [Mu.fixedIn, Bool.not_eq_eq_eq_not, Bool.not_true] at h
    simp [Mu.subst, hv.mus j, h]
  | _ => rfl

/-- A type that the changed parameters don't change is the same with either
arguments. -/
theorem Ty.inst_fixed {c : Changed} {ms ms' : List Mu} {τs τs' : List Ty}
    (hv : c.Views ms ms' τs τs') {t : Ty} (h : t.fixedIn c = true) :
    t.inst ms' τs' = t.inst ms τs := by
  induction t with
  | unit | int => rfl
  | cell m t ih =>
    simp only [Ty.fixedIn, Bool.and_eq_true] at h
    simp [Ty.inst, Mu.subst_fixed hv h.1, ih h.2]
  | fn m a r iha ihr =>
    simp only [Ty.fixedIn, Bool.and_eq_true] at h
    simp [Ty.inst, Mu.subst_fixed hv h.1.1, iha h.1.2, ihr h.2]
  | prod a b iha ihb =>
    simp only [Ty.fixedIn, Bool.and_eq_true] at h
    simp [Ty.inst, iha h.1, ihb h.2]
  | tparam i =>
    simp only [Ty.fixedIn, Bool.not_eq_eq_eq_not, Bool.not_true] at h
    simp [Ty.inst, hv.tys i, h]
  | roParam i =>
    simp only [Ty.inst, hv.tys i]
    split <;> simp [Ty.ro_ro]

/-- A type with the changed parameters changed is a view of it, if that's
at most `ro`. -/
theorem Ty.sub_view {Δ : Assume} {c : Changed} {ms ms' : List Mu} {τs τs' : List Ty}
    (hv : c.Views ms ms' τs τs') {t : Ty} (hc : t.coIn c = true)
    (ha : AtMost Δ .readonly (t.inst ms' τs')) : Sub Δ (t.inst ms τs) (t.inst ms' τs') := by
  induction t with
  | unit | int => exact .refl _
  | tparam i =>
    simp only [Ty.inst, hv.tys i]
    split
    · exact Sub.to_ro Δ _
    · exact .refl _
  | roParam i =>
    rw [Ty.inst_fixed hv (t := .roParam i) rfl]
    exact .refl _
  | fn m a r =>
    rw [Ty.inst_fixed hv (t := .fn m a r) hc]
    exact .refl _
  | prod a b iha ihb =>
    simp only [Ty.coIn, Bool.and_eq_true] at hc
    simp only [Ty.inst, AtMost] at ha ⊢
    exact .prod (iha hc.1 ha.1) (ihb hc.2 ha.2)
  | cell m t ih =>
    have fixed (hf : t.fixedIn c = true) (hm : m.fixedIn c = true) :
        Sub Δ ((Ty.cell m t).inst ms τs) ((Ty.cell m t).inst ms' τs') := by
      rw [Ty.inst_fixed hv (t := .cell m t) (by simp [Ty.fixedIn, hf, hm])]
      exact .refl _
    cases m with
    | param j =>
      by_cases hj : c.mus j = true
      · simp only [Ty.coIn, hj, ite_true] at hc
        simp only [Ty.inst, Mu.subst, hv.mus j, hj, ite_true, AtMost] at ha ⊢
        exact .ro (ih hc ha.2) ha.2
      · simp only [Ty.coIn, hj] at hc
        exact fixed hc (by simpa [Mu.fixedIn] using hj)
    | _ => exact fixed hc rfl

/-- A struct type with the changed parameters changed, and its own
mutability `ro`, is a view of it, if that's well-formed: at most `ro`
(`StructDecl.atMost_ty_iff`). -/
theorem StructDecl.sub_view (d : StructDecl) {Δ : Assume} {c : Changed} {τs τs' : List Ty}
    {vs vs' : List Mu} {m : Mu} (hc : d.fields.coIn c = true)
    (hv : c.Views (vs ++ [m]) (vs' ++ [.readonly]) τs τs')
    (ha : AtMost Δ .readonly (d.ty τs' vs' .readonly)) :
    Sub Δ (d.ty τs vs m) (d.ty τs' vs' .readonly) := by
  simp only [StructDecl.ty, AtMost] at ha ⊢
  exact .ro (Ty.sub_view hv hc ha.2) ha.2

/-! ## Examples -/

/-- `struct Box[T] { item: T }`. -/
def box : StructDecl := { types := 1, muts := 0, fields := .tparam 0 }

/-- What `Box` can change: its `T` and its own mutability. -/
def box.co : Changed := { tys := fun i => i == 0, mus := fun j => j == 0 }

example : box.fields.coIn box.co = true := rfl

/-- `mut Box[mut [i32]]` can be seen as `ro Box[ro [i32]]`. -/
example : Sub .none (box.ty [.cell .mutable .int] [] .mutable)
    (box.ty [.cell .readonly .int] [] .readonly) := by
  refine box.sub_view (c := box.co) rfl ⟨?_, ?_⟩ ⟨.refl _, .refl _, trivial⟩
  · intro j; rcases j with _ | j <;> simp [box.co]
  · intro i; rcases i with _ | i <;> simp [box.co, Ty.ro]

/-- `VecIter[T, 'v]` can change `T`, `'v` and its own mutability:
`mut VecIter[mut [i32], mut]` can be seen as `ro VecIter[ro [i32], ro]`. -/
def vecIter.co : Changed := { tys := fun i => i == 0, mus := fun j => j == 0 || j == 1 }

example : vecIter.fields.coIn vecIter.co = true := rfl

example : Sub .none (vecIter.ty [.cell .mutable .int] [.mutable] .mutable)
    (vecIter.ty [.cell .readonly .int] [.readonly] .readonly) := by
  refine vecIter.sub_view (c := vecIter.co) rfl ⟨?_, ?_⟩ ?_
  · intro j; rcases j with _ | _ | j <;> simp [vecIter.co]
  · intro i; rcases i with _ | i <;> simp [vecIter.co, Ty.ro]
  · exact ⟨.refl _, ⟨.refl _, .refl _, trivial⟩, trivial⟩

/-- `Handler[T]`'s `T` is in a closure's parameters, so it can't be
changed, and neither can its own mutability, the closure's. -/
example : handler.fields.coIn { tys := fun i => i == 0, mus := fun _ => false } = false := rfl
example : handler.fields.coIn { tys := fun _ => false, mus := fun j => j == 0 } = false := rfl

/-- `struct Frozen[T] { xs: imm [T] }`: `T` is in an object whose mutability
is written, which isn't seen as `ro`. -/
def frozen : StructDecl := { types := 1, muts := 0, fields := .cell .immutable (.tparam 0) }

example : frozen.fields.coIn { tys := fun i => i == 0, mus := fun j => j == 0 } = false := rfl

/-- `struct Pair[A, B] { a: A, f: fn(B) }`, its own mutability left as it is:
`A` can be changed, `B` can't. -/
def pairFn : StructDecl :=
  { types := 2, muts := 0, fields := .prod (.tparam 0) (.fn .immutable (.tparam 1) .unit) }

example : pairFn.fields.coIn { tys := fun i => i == 0, mus := fun j => j == 0 } = true := rfl
example : pairFn.fields.coIn { tys := fun i => i == 1, mus := fun j => j == 0 } = false := rfl

end BoltMut
