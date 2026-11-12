import BoltMut.Syntax

/-!
# Subtyping

`ro` (#320) sits between `imm` and `mut` in what an object can hold. It's
also above both in subtyping: a `mut` or an `imm` object can be seen as
`ro`, which can read it but not change it. The two orders differ: an `imm`
object can't hold a `ro` one, since that may change, but an `imm` object
can be seen as `ro`.

A `ro` view is deep: what it holds is at most `ro`, so what's read through
it can't be changed either. An object of any mutability can be seen as
`ro`, one whose mutability is a parameter too, and a value of a type
parameter `T` as `ro T`.

Two more rules make functions fit where others are expected (#444): a
function that takes less and gives more fits where one that takes more
and gives less is expected, and what an `imm` object holds can be seen as
a supertype of its type, since it never changes. A closure's own
mutability stays as it is.

The lemmas that take a subtyping apart are named after the side that's
known: `Sub.of_cell` says what a cell can be seen as, `Sub.to_cell` what
can be seen as one.
-/

namespace BoltMut

/-- `t` can be used where `t'` is expected. -/
inductive Sub (Δ : Assume) : Ty → Ty → Prop where
  | refl (t : Ty) : Sub Δ t t
  /-- Any object seen as `ro`, with what it holds seen as a supertype that's
  at most `ro`. A `mut` object's contents can be seen as `ro` too, since
  nothing can be stored through the view. -/
  | ro {m : Mu} {t t' : Ty} : Sub Δ t t' → AtMost Δ .readonly t' →
      Sub Δ (.cell m t) (.cell .readonly t')
  /-- What an `imm` object holds never changes, so it can be seen as any
  supertype of its type, like a closure it holds at another function type. -/
  | immCell {t t' : Ty} : Sub Δ t t' → Sub Δ (.cell .immutable t) (.cell .immutable t')
  | prod {a a' b b' : Ty} : Sub Δ a a' → Sub Δ b b' → Sub Δ (.prod a b) (.prod a' b')
  /-- A function can be used where one that takes less and gives more is
  expected, like one that takes `ro` where one that takes `mut` is. -/
  | fn {m : Mu} {a a' r r' : Ty} : Sub Δ a' a → Sub Δ r r' → Sub Δ (.fn m a r) (.fn m a' r')
  /-- A value of a type parameter can be seen as `ro T`. -/
  | tparam (i : Nat) : Sub Δ (.tparam i) (.roParam i)

/-- Any type can be seen as `ro`. -/
theorem Sub.to_ro (Δ : Assume) (t : Ty) : Sub Δ t t.ro := by
  induction t with
  | unit | int => exact .refl _
  | cell m t ih => exact .ro ih (AtMost.of_ro Δ t)
  | fn m a r _ _ => exact .refl _
  | prod a b iha ihb => exact .prod iha ihb
  | tparam i => exact .tparam i
  | roParam i => exact .refl _

/-- By induction on the type in the middle, since a function's parameters
are related the other way round. -/
theorem Sub.trans {Δ : Assume} {t₁ t₂ t₃ : Ty} (h₁ : Sub Δ t₁ t₂) (h₂ : Sub Δ t₂ t₃) : Sub Δ t₁ t₃ := by
  induction t₂ generalizing t₁ t₃ with
  | unit | int => cases h₁; exact h₂
  | cell m t ih =>
    cases h₁ with
    | refl => exact h₂
    | ro h ha =>
      cases h₂ with
      | refl => exact .ro h ha
      | ro h' ha' => exact .ro (ih h h') ha'
    | immCell h =>
      cases h₂ with
      | refl => exact .immCell h
      | ro h' ha' => exact .ro (ih h h') ha'
      | immCell h' => exact .immCell (ih h h')
  | fn m a r iha ihr =>
    cases h₁ with
    | refl => exact h₂
    | fn ha hr =>
      cases h₂ with
      | refl => exact .fn ha hr
      | fn ha' hr' => exact .fn (iha ha' ha) (ihr hr hr')
  | prod a b iha ihb =>
    cases h₁ with
    | refl => exact h₂
    | prod ha hb =>
      cases h₂ with
      | refl => exact .prod ha hb
      | prod ha' hb' => exact .prod (iha ha ha') (ihb hb hb')
  | tparam i => cases h₁; exact h₂
  | roParam i => cases h₂; exact h₁

/-- Subtyping survives instantiating. -/
theorem Sub.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {t t' : Ty}
    (h : Sub Δ t t') : Sub Δ' (t.inst ms τs) (t'.inst ms τs) := by
  induction h with
  | refl => exact .refl _
  | ro _ ha ih =>
    simp only [Ty.inst]
    have := AtMost.inst hs ha
    simp only [Mu.subst] at this
    exact .ro ih this
  | immCell _ ih =>
    simp only [Ty.inst, Mu.subst]
    exact .immCell ih
  | prod _ _ iha ihb => exact .prod iha ihb
  | fn _ _ iha ihr => exact .fn iha ihr
  | tparam i => exact Sub.to_ro Δ' _

/-- Seeing two types as `ro` keeps them related. -/
theorem Sub.ro_mono {Δ : Assume} {t t' : Ty} (h : Sub Δ t t') : Sub Δ t.ro t'.ro := by
  induction h with
  | refl => exact .refl _
  | ro _ _ ih | immCell _ ih => exact .ro ih (AtMost.of_ro _ _)
  | prod _ _ iha ihb => exact .prod iha ihb
  | fn ha hr => exact .fn ha hr
  | tparam => exact .refl _

/-! ## Taking a subtyping apart -/

theorem Sub.of_unit {Δ : Assume} {t : Ty} (h : Sub Δ .unit t) : t = .unit := by
  cases h; rfl

theorem Sub.of_int {Δ : Assume} {t : Ty} (h : Sub Δ .int t) : t = .int := by
  cases h; rfl

/-- What a cell can be seen as: a cell of the same mutability or a `ro`
one, holding a supertype of what it holds. -/
theorem Sub.of_cell {Δ : Assume} {m : Mu} {t t' : Ty} (h : Sub Δ (.cell m t) t') :
    ∃ m' s, t' = .cell m' s ∧ Sub Δ t s ∧ (m' = m ∨ m' = .readonly) := by
  cases h with
  | refl => exact ⟨_, _, rfl, .refl _, .inl rfl⟩
  | ro h _ => exact ⟨_, _, rfl, h, .inr rfl⟩
  | immCell h => exact ⟨_, _, rfl, h, .inl rfl⟩

/-- What can be seen as a cell: a cell. -/
theorem Sub.to_cell {Δ : Assume} {t t' : Ty} {m : Mu} (h : Sub Δ t (.cell m t')) :
    ∃ m₀ t₀, t = .cell m₀ t₀ := by
  cases h with
  | refl | ro | immCell => exact ⟨_, _, rfl⟩

/-- What can be seen as a `mut` cell: only that cell. -/
theorem Sub.to_mut_cell {Δ : Assume} {t t' : Ty} (h : Sub Δ t (.cell .mutable t')) :
    t = .cell .mutable t' := by
  cases h; rfl

theorem Sub.of_prod {Δ : Assume} {a b t' : Ty} (h : Sub Δ (.prod a b) t') :
    ∃ a' b', t' = .prod a' b' ∧ Sub Δ a a' ∧ Sub Δ b b' := by
  cases h with
  | refl => exact ⟨a, b, rfl, .refl _, .refl _⟩
  | prod ha hb => exact ⟨_, _, rfl, ha, hb⟩

theorem Sub.to_prod {Δ : Assume} {t a b : Ty} (h : Sub Δ t (.prod a b)) :
    ∃ a₀ b₀, t = .prod a₀ b₀ ∧ Sub Δ a₀ a ∧ Sub Δ b₀ b := by
  cases h with
  | refl => exact ⟨a, b, rfl, .refl _, .refl _⟩
  | prod ha hb => exact ⟨_, _, rfl, ha, hb⟩

/-- What a function type can be seen as: one with the same mutability that
takes less and gives more. -/
theorem Sub.of_fn {Δ : Assume} {m : Mu} {a r t : Ty} (h : Sub Δ (.fn m a r) t) :
    ∃ a' r', t = .fn m a' r' ∧ Sub Δ a' a ∧ Sub Δ r r' := by
  cases h with
  | refl => exact ⟨_, _, rfl, .refl _, .refl _⟩
  | fn ha hr => exact ⟨_, _, rfl, ha, hr⟩

theorem Sub.to_fn {Δ : Assume} {t a r : Ty} {m : Mu} (h : Sub Δ t (.fn m a r)) :
    ∃ a₀ r₀, t = .fn m a₀ r₀ := by
  cases h with
  | refl | fn => exact ⟨_, _, rfl⟩

/-- Nothing is seen as `ro` in a type that's `imm` all the way down, so what
it's a supertype of is `imm` all the way down too. -/
theorem Sub.atMost_imm {t t' : Ty} (h : Sub .none t t') (hd : AtMost .none .immutable t') :
    AtMost .none .immutable t := by
  induction h with
  | refl => exact hd
  | ro _ _ => exact nomatch Le.none_immutable hd.1
  | immCell _ ih => exact ⟨hd.1, ih hd.2⟩
  | prod _ _ iha ihb => exact ⟨iha hd.1, ihb hd.2⟩
  | fn => exact hd
  | tparam => exact nomatch Le.none_immutable hd

end BoltMut
