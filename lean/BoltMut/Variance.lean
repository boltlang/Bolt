import BoltMut.Structs
import BoltMut.Subtyping

/-!
# Variance: how a struct's arguments can change (#440)

A struct type gives its arguments as they are, so `mut Box[mut [i32]]` for
`struct Box[T] { item: T }` can only be seen as `ro Box[mut [i32]]`,
which can't exist: a `ro` value can't hold a `mut` one (E0223). What it
should be seen as is `ro Box[ro [i32]]`. And an `imm Handler[ro [i32]]`
for `struct Handler[T] { f: fn(T) }`, whose closure takes `ro`, should be
usable as an `imm Handler[mut [i32]]`, one whose closure takes `mut`.

Whether that's sound depends on where a parameter is in the fields. Each
type parameter gets a variance (`Var`): `co`, changed to a supertype;
`contra`, changed to a subtype; or `fixed`, kept as it is. A mutability
parameter can be changed to `ro`. The struct type with its arguments
changed so is a subtype of the original, or a supertype for a position
seen the other way round (`Ty.sub_var`), when each changed parameter is
only where it may change that way (`Ty.varIn`):

- in a tuple, or in an object that's `imm`, `ro`, or whose mutability is a
  parameter that becomes `ro`, as it is: what they hold can be seen as a
  supertype (`Sub.ro`, `Sub.immCell`);
- in a function's parameters the other way round, and in what it gives as
  it is (`Sub.fn`, #444);
- not in a `mut` object, nor one whose mutability is a parameter that's
  kept, since what they hold is read and written.

A mutability parameter that becomes `ro` can only be in objects that are
seen as it is, not in a function's parameters, nor as a closure's
mutability.

The checker infers the variance of each parameter once for each struct;
any variance for which `Ty.varIn` holds is sound. The types must still be
well-formed (`Ty.WF`): a `ro` view must be at most `ro` inside, so an
invariant `T` held as it is leaves `ro Box[mut [i32]]` an error.
-/

namespace BoltMut

/-- How a type parameter's argument may change. -/
inductive Var where
  | fixed
  | co
  | contra
  deriving DecidableEq, Repr

/-- The arguments `τ` and `τ'` are related as `v` says. -/
def Var.Rel (Δ : Assume) : Var → Ty → Ty → Prop
  | .fixed, τ, τ' => τ' = τ
  | .co, τ, τ' => Sub Δ τ τ'
  | .contra, τ, τ' => Sub Δ τ' τ

/-- How each parameter may change: type parameters by their variance, and
the mutability parameters in `mus` to `ro`. -/
structure Changed where
  tys : Nat → Var
  mus : Nat → Bool

/-- The arguments `ms'` and `τs'` are `ms` and `τs` changed as `c` says. -/
structure Changed.Rel (c : Changed) (Δ : Assume) (ms ms' : List Mu) (τs τs' : List Ty) : Prop where
  mus : ∀ j, ms'[j]?.getD .immutable = if c.mus j then .readonly else ms[j]?.getD .immutable
  tys : ∀ i, (c.tys i).Rel Δ (τs[i]?.getD .unit) (τs'[i]?.getD .unit)

/-- A mutability that `c` doesn't change. -/
def Mu.fixedIn (c : Changed) : Mu → Bool
  | .param j => !c.mus j
  | _ => true

/-- A type that `c` doesn't change: none of the parameters it changes is in
it. -/
def Ty.fixedIn (c : Changed) : Ty → Bool
  | .unit | .int => true
  | .cell m t => m.fixedIn c && t.fixedIn c
  | .fn m a r => m.fixedIn c && a.fixedIn c && r.fixedIn c
  | .prod a b => a.fixedIn c && b.fixedIn c
  | .tparam i | .roParam i => c.tys i == .fixed

/-- Each parameter that `c` changes is where it can change that way: in a
place that's seen as it is when `pos`, and the other way round when not. -/
def Ty.varIn (c : Changed) (pos : Bool) : Ty → Bool
  | .unit | .int => true
  | .tparam i | .roParam i => match c.tys i with
    | .fixed => true
    | .co => pos
    | .contra => !pos
  | .cell m t => match m with
    | .param j => if c.mus j then pos && t.varIn c pos else t.fixedIn c
    | .immutable | .readonly => t.varIn c pos
    | .mutable => t.fixedIn c
  | .fn m a r => m.fixedIn c && a.varIn c (!pos) && r.varIn c pos
  | .prod a b => a.varIn c pos && b.varIn c pos

/-- A well-formed type: each object in it, also in a function's parameters
and what it gives, holds what's at most its mutability. -/
def Ty.WF (Δ : Assume) : Ty → Prop
  | .cell m t => AtMost Δ m t ∧ t.WF Δ
  | .fn _ a r => a.WF Δ ∧ r.WF Δ
  | .prod a b => a.WF Δ ∧ b.WF Δ
  | _ => True

/-- `Sub` in the direction `pos` says: `x ⊑ y` when it's true, `y ⊑ x` when
it's false. -/
def Sub.dir (Δ : Assume) : Bool → Ty → Ty → Prop
  | true, x, y => Sub Δ x y
  | false, x, y => Sub Δ y x

theorem Sub.dir_refl (Δ : Assume) (pos : Bool) (t : Ty) : Sub.dir Δ pos t t := by
  cases pos <;> exact .refl _

theorem Mu.subst_fixed {c : Changed} {Δ : Assume} {ms ms' : List Mu} {τs τs' : List Ty}
    (hv : c.Rel Δ ms ms' τs τs') {m : Mu} (h : m.fixedIn c = true) : m.subst ms' = m.subst ms := by
  cases m with
  | param j =>
    simp only [Mu.fixedIn, Bool.not_eq_eq_eq_not, Bool.not_true] at h
    simp [Mu.subst, hv.mus j, h]
  | _ => rfl

/-- A type that `c` doesn't change is the same with either arguments. -/
theorem Ty.inst_fixed {c : Changed} {Δ : Assume} {ms ms' : List Mu} {τs τs' : List Ty}
    (hv : c.Rel Δ ms ms' τs τs') {t : Ty} (h : t.fixedIn c = true) :
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
    simp only [Ty.fixedIn, beq_iff_eq] at h
    have := hv.tys i
    rw [h] at this
    simp only [Var.Rel] at this
    simp only [Ty.inst, this]
  | roParam i =>
    simp only [Ty.fixedIn, beq_iff_eq] at h
    have := hv.tys i
    rw [h] at this
    simp only [Var.Rel] at this
    simp only [Ty.inst, this]

/-- **A type with its arguments changed as `c` says is a subtype of it**, in
the direction `pos`, when each changed parameter is where it can change
that way, and both are well-formed. -/
theorem Ty.sub_var {Δ : Assume} {c : Changed} {ms ms' : List Mu} {τs τs' : List Ty}
    (hv : c.Rel Δ ms ms' τs τs') (t : Ty) :
    ∀ (pos : Bool), t.varIn c pos = true → (t.inst ms τs).WF Δ → (t.inst ms' τs').WF Δ →
      Sub.dir Δ pos (t.inst ms τs) (t.inst ms' τs') := by
  have fixed : ∀ (pos : Bool) (t : Ty), t.fixedIn c = true → Sub.dir Δ pos (t.inst ms τs) (t.inst ms' τs') :=
    fun pos t h => by rw [Ty.inst_fixed hv h]; exact Sub.dir_refl _ _ _
  induction t with
  | unit | int => intro pos _ _ _; exact Sub.dir_refl _ _ _
  | tparam i =>
    intro pos hc _ _
    have hr := hv.tys i
    simp only [Ty.inst]
    simp only [Ty.varIn] at hc
    cases hci : c.tys i <;> rw [hci] at hr hc <;> simp only [Var.Rel] at hr <;> simp at hc
    · rw [hr]; exact Sub.dir_refl _ _ _
    · subst hc; exact hr
    · subst hc; exact hr
  | roParam i =>
    intro pos hc _ _
    have hr := hv.tys i
    simp only [Ty.inst]
    simp only [Ty.varIn] at hc
    cases hci : c.tys i <;> rw [hci] at hr hc <;> simp only [Var.Rel] at hr <;> simp at hc
    · rw [hr]; exact Sub.dir_refl _ _ _
    · subst hc; exact hr.ro_mono
    · subst hc; exact hr.ro_mono
  | prod a b iha ihb =>
    intro pos hc hw hw'
    simp only [Ty.varIn, Bool.and_eq_true] at hc
    simp only [Ty.inst, Ty.WF] at hw hw' ⊢
    have h₁ := iha pos hc.1 hw.1 hw'.1
    have h₂ := ihb pos hc.2 hw.2 hw'.2
    cases pos
    · exact .prod h₁ h₂
    · exact .prod h₁ h₂
  | fn m a r iha ihr =>
    intro pos hc hw hw'
    simp only [Ty.varIn, Bool.and_eq_true] at hc
    simp only [Ty.inst, Ty.WF] at hw hw'
    have ha := iha (!pos) hc.1.2 hw.1 hw'.1
    have hr := ihr pos hc.2 hw.2 hw'.2
    simp only [Ty.inst, Mu.subst_fixed hv hc.1.1]
    cases pos
    · exact .fn ha hr
    · exact .fn ha hr
  | cell m t ih =>
    intro pos hc hw hw'
    cases m with
    | param j =>
      by_cases hj : c.mus j = true
      · simp only [Ty.varIn, hj, ite_true, Bool.and_eq_true] at hc
        obtain ⟨rfl, hc⟩ := hc
        simp only [Ty.inst, Mu.subst, hv.mus j, hj, ite_true, Ty.WF] at hw hw' ⊢
        exact .ro (ih true hc hw.2 hw'.2) hw'.1
      · simp only [Bool.not_eq_true] at hj
        simp only [Ty.varIn, hj, Bool.false_eq_true, ite_false] at hc
        exact fixed pos _ (by simp [Ty.fixedIn, Mu.fixedIn, hj, hc])
    | immutable =>
      simp only [Ty.varIn] at hc
      simp only [Ty.inst, Mu.subst, Ty.WF] at hw hw' ⊢
      have h := ih pos hc hw.2 hw'.2
      cases pos
      · exact .immCell h
      · exact .immCell h
    | readonly =>
      simp only [Ty.varIn] at hc
      simp only [Ty.inst, Mu.subst, Ty.WF] at hw hw' ⊢
      have h := ih pos hc hw.2 hw'.2
      cases pos
      · exact .ro h hw.1
      · exact .ro h hw'.1
    | mutable =>
      simp only [Ty.varIn] at hc
      exact fixed pos _ (by simp [Ty.fixedIn, Mu.fixedIn, hc])

/-- A struct type with its arguments changed as `c` says, and its own
mutability `ro`, is a view of it, if both are well-formed. -/
theorem StructDecl.sub_view (d : StructDecl) {Δ : Assume} {c : Changed} {τs τs' : List Ty}
    {vs vs' : List Mu} {m : Mu} (hc : d.fields.varIn c true = true)
    (hv : c.Rel Δ (vs ++ [m]) (vs' ++ [.readonly]) τs τs')
    (hw : (d.ty τs vs m).WF Δ) (hw' : (d.ty τs' vs' .readonly).WF Δ) :
    Sub Δ (d.ty τs vs m) (d.ty τs' vs' .readonly) := by
  simp only [StructDecl.ty, Ty.WF] at hw hw' ⊢
  exact .ro (Ty.sub_var hv _ true hc hw.2 hw'.2) hw'.1

/-- An `imm` struct type with its arguments changed as `c` says is a subtype
of it, if both are well-formed. -/
theorem StructDecl.sub_imm (d : StructDecl) {Δ : Assume} {c : Changed} {τs τs' : List Ty}
    {vs vs' : List Mu} (hc : d.fields.varIn c true = true)
    (hv : c.Rel Δ (vs ++ [.immutable]) (vs' ++ [.immutable]) τs τs')
    (hw : (d.ty τs vs .immutable).WF Δ) (hw' : (d.ty τs' vs' .immutable).WF Δ) :
    Sub Δ (d.ty τs vs .immutable) (d.ty τs' vs' .immutable) := by
  simp only [StructDecl.ty, Ty.WF] at hw hw' ⊢
  exact .immCell (Ty.sub_var hv _ true hc hw.2 hw'.2)

/-! ## Examples -/

/-- `struct Box[T] { item: T }`. -/
def box : StructDecl := { types := 1, muts := 0, fields := .tparam 0 }

/-- `Box` can see its `T` as a supertype, and its own mutability as `ro`. -/
def box.co : Changed := { tys := fun _ => .co, mus := fun j => j == 0 }

example : box.fields.varIn box.co true = true := rfl

/-- `mut Box[mut [i32]]` can be seen as `ro Box[ro [i32]]`. -/
example : Sub .none (box.ty [.cell .mutable .int] [] .mutable)
    (box.ty [.cell .readonly .int] [] .readonly) := by
  refine box.sub_view (c := box.co) rfl ⟨?_, ?_⟩ ?_ ?_
  · intro j; rcases j with _ | j <;> simp [box.co]
  · intro i
    rcases i with _ | i
    · exact .ro (.refl _) trivial
    · exact .refl _
  · exact ⟨⟨.top _, trivial⟩, trivial, trivial⟩
  · exact ⟨⟨.refl _, trivial⟩, trivial, trivial⟩

/-- `VecIter[T, 'v]` can change `T`, `'v` and its own mutability:
`mut VecIter[mut [i32], mut]` can be seen as `ro VecIter[ro [i32], ro]`. -/
def vecIter.co : Changed := { tys := fun _ => .co, mus := fun j => j == 0 || j == 1 }

example : vecIter.fields.varIn vecIter.co true = true := rfl

/-- But keeping `'v`, it can't change `T`: the array may be `mut`. -/
example : vecIter.fields.varIn { tys := fun _ => .co, mus := fun j => j == 1 } true = false := rfl

/-- `Handler[T]`'s `T` is in a closure's parameters: it changes the other
way round. -/
def handler.contra : Changed := { tys := fun _ => .contra, mus := fun _ => false }

example : handler.fields.varIn handler.contra true = true := rfl
example : handler.fields.varIn { tys := fun _ => .co, mus := fun _ => false } true = false := rfl

/-- An `imm Handler[ro [i32]]`, whose closure takes `ro`, can be used as an
`imm Handler[mut [i32]]`. -/
example : Sub .none (handler.ty [.cell .readonly .int] [] .immutable)
    (handler.ty [.cell .mutable .int] [] .immutable) := by
  refine handler.sub_imm (c := handler.contra) rfl ⟨?_, ?_⟩ ?_ ?_
  · intro j; rcases j with _ | j <;> simp [handler.contra]
  · intro i
    rcases i with _ | i
    · exact .ro (.refl _) trivial
    · exact .refl _
  · exact ⟨.inl (.refl _), ⟨trivial, trivial⟩, trivial⟩
  · exact ⟨.inl (.refl _), ⟨trivial, trivial⟩, trivial⟩

/-- `struct Pair[A, B] { a: A, f: imm fn(B) }`: `A` changes as it is, and
`B` the other way round. -/
def pairFn : StructDecl :=
  { types := 2, muts := 0, fields := .prod (.tparam 0) (.fn .immutable (.tparam 1) .unit) }

example : pairFn.fields.varIn
    { tys := fun i => if i == 0 then .co else .contra, mus := fun j => j == 0 } true = true := rfl
example : pairFn.fields.varIn { tys := fun _ => .co, mus := fun j => j == 0 } true = false := rfl

/-- `struct Cell[T] { xs: imm [T] }`: what an `imm` object holds can change
too. -/
def frozen : StructDecl := { types := 1, muts := 0, fields := .cell .immutable (.tparam 0) }

example : frozen.fields.varIn { tys := fun _ => .co, mus := fun j => j == 0 } true = true := rfl

end BoltMut
