/-!
# Syntax

The core of Bolt's mutability rules, as a small language.

Bolt's arrays and structs are shared objects: `let ys = xs` makes another
name for the same array. Here, a *cell* stands for such an object, or one
element or field of it: `Ty.cell m t` is an object with mutability `m` that
holds a `t`. Cells in cells give nested arrays and structs, and copying a
location gives aliases.

A mutability is `mut`, `imm`, or a parameter of a generic function, like the
hidden or named mutability of a parameter written `[T]` or `'m [T]` in Bolt.
A type can be a type parameter of a generic function too, like `T`.

A tuple is a value, as in Bolt: `prod a b`. A struct is a cell that holds
the tuple of its fields, so one mutability covers all of them, and a field
that's a struct is a cell in that tuple. Changing a field replaces the
tuple, and `S { a: x, ..base }` is `new b (pair x (snd (get base)))`: the
fields it doesn't give are shared with `base`, at the types they have
there. A generic struct like `Box[T]` is a cell holding a `T`, and a field
with a mutability of its own (`'v`, #243) is a cell with a mutability
parameter.

Closures have a mutability too, as #244 proposes: `fn m a r`. A closure is
`imm` only if everything it captures is `imm` all the way down. In Bolt, a
closure shares the `let mut` variables it captures, which the evaluator
keeps in shared slots; here, such a variable is a `mut` cell, so capturing
one makes a closure `mut` by the same rule.

What an object holds must be at most as `mut` as the object, with
`imm ⊑ mut` (`AtMost`). For a type parameter or a mutability parameter,
that's only known from what the function assumes, like `T ⊑ imm` or
`'v ⊑ 'm` (#243): a call must meet those assumptions (`Sat`).
-/

namespace BoltMut

/-- A mutability: `mut` (`mutable`, since `mut` is a keyword of Lean),
`imm`, the `i`th mutability parameter of the function being checked, or
the region `r`: the mutability of the values a block makes and that nothing
outside the block has, which can be changed while the block runs (#209;
see `Regions.lean`). `holdsMut` says the region can hold `mut` objects,
`mut ⊑ r`, so that it becomes `mut` when its block ends rather than `imm`
(#286). -/
inductive Mu where
  | mutable
  | immutable
  | param (i : Nat)
  | region (r : Nat) (holdsMut : Bool)
  deriving DecidableEq, Repr

/-- Types. `cell m t` is a shared object with mutability `m` holding a `t`,
`fn m a r` a closure with mutability `m` from `a` to `r`, and `tparam i` the
`i`th type parameter of the function being checked. -/
inductive Ty where
  | unit
  | int
  | cell (m : Mu) (t : Ty)
  | fn (m : Mu) (a r : Ty)
  | prod (a b : Ty)
  | tparam (i : Nat)
  deriving DecidableEq, Repr

/-- Expressions. `loc` only appears while running: it's a location in the
heap. `new true e` is `mut` and `new false e` is `imm`, like `mut [e]` and
`[e]`; a literal can't have a parameter as its mutability. A call gives the
mutability and type arguments of the function explicitly, as the type
checker finds them. -/
inductive Expr where
  | var (x : Nat)
  | unit
  | int (n : Int)
  | loc (l : Nat)
  | new (isMut : Bool) (e : Expr)
  | get (e : Expr)
  | set (e₁ e₂ : Expr)
  | letE (x : Nat) (e₁ e₂ : Expr)
  | call (f : Nat) (ms : List Mu) (τs : List Ty) (e : Expr)
  /-- A closure, `mut` or `imm`, with the parameter `x`. -/
  | lam (isMut : Bool) (x : Nat) (body : Expr)
  | app (e₁ e₂ : Expr)
  | pair (e₁ e₂ : Expr)
  | fst (e : Expr)
  | snd (e : Expr)
  deriving Repr

/-- The mutability of `new isMut _`. -/
def muOf (isMut : Bool) : Mu := if isMut then .mutable else .immutable

/-- Values: what an expression evaluates to. -/
inductive Expr.IsValue : Expr → Prop where
  | unit : IsValue .unit
  | int : IsValue (.int n)
  | loc : IsValue (.loc l)
  | lam : IsValue (.lam b x body)
  | pair : IsValue v₁ → IsValue v₂ → IsValue (.pair v₁ v₂)

/-- An expression without locations, like the body of a function. -/
def Expr.NoLocs : Expr → Prop
  | .var _ | .unit | .int _ => True
  | .loc _ => False
  | .new _ e => e.NoLocs
  | .get e => e.NoLocs
  | .call _ _ _ e => e.NoLocs
  | .set e₁ e₂ => e₁.NoLocs ∧ e₂.NoLocs
  | .letE _ e₁ e₂ => e₁.NoLocs ∧ e₂.NoLocs
  | .lam _ _ body => body.NoLocs
  | .app e₁ e₂ => e₁.NoLocs ∧ e₂.NoLocs
  | .pair e₁ e₂ => e₁.NoLocs ∧ e₂.NoLocs
  | .fst e => e.NoLocs
  | .snd e => e.NoLocs

/-- The location `l` is in the expression: what a closure captured is in its
body. -/
def Expr.HasLoc : Expr → Nat → Prop
  | .var _, _ | .unit, _ | .int _, _ => False
  | .loc l', l => l' = l
  | .new _ e, l => e.HasLoc l
  | .get e, l => e.HasLoc l
  | .call _ _ _ e, l => e.HasLoc l
  | .set e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .letE _ e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .lam _ _ body, l => body.HasLoc l
  | .app e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .pair e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .fst e, l => e.HasLoc l
  | .snd e, l => e.HasLoc l

/-- `mut`, or a region that holds `mut` objects: everything is below it. -/
def Mu.isTop : Mu → Bool
  | .mutable => true
  | .region _ b => b
  | _ => false

/-- A mutability that isn't a region. -/
def Mu.NoRegion : Mu → Prop
  | .region _ _ => False
  | _ => True

/-- A type that mentions no region. -/
def Ty.NoRegion : Ty → Prop
  | .unit | .int | .tparam _ => True
  | .cell m t => m.NoRegion ∧ t.NoRegion
  | .fn m a b => m.NoRegion ∧ a.NoRegion ∧ b.NoRegion
  | .prod a b => a.NoRegion ∧ b.NoRegion

/-- Arguments for a function's parameters that mention no region. -/
def ArgsNoRegion (ms : List Mu) (τs : List Ty) : Prop := (∀ m ∈ ms, m.NoRegion) ∧ ∀ τ ∈ τs, τ.NoRegion

/-- Replace the mutability parameters with `ms`; one without an argument
becomes `imm`. -/
def Mu.subst (ms : List Mu) : Mu → Mu
  | .param i => ms[i]?.getD .immutable
  | m => m

/-- Replace the mutability parameters with `ms` and the type parameters with
`τs`; one without an argument becomes `()`. -/
def Ty.inst (ms : List Mu) (τs : List Ty) : Ty → Ty
  | .unit => .unit
  | .int => .int
  | .cell m t => .cell (m.subst ms) (t.inst ms τs)
  | .fn m a r => .fn (m.subst ms) (a.inst ms τs) (r.inst ms τs)
  | .prod a b => .prod (a.inst ms τs) (b.inst ms τs)
  | .tparam i => τs[i]?.getD .unit

/-- The same for the arguments of the calls in an expression. -/
def Expr.inst (ms : List Mu) (τs : List Ty) : Expr → Expr
  | .var x => .var x
  | .unit => .unit
  | .int n => .int n
  | .loc l => .loc l
  | .new b e => .new b (e.inst ms τs)
  | .get e => .get (e.inst ms τs)
  | .set e₁ e₂ => .set (e₁.inst ms τs) (e₂.inst ms τs)
  | .letE x e₁ e₂ => .letE x (e₁.inst ms τs) (e₂.inst ms τs)
  | .call f ms' τs' e => .call f (ms'.map (Mu.subst ms)) (τs'.map (Ty.inst ms τs)) (e.inst ms τs)
  | .lam b x body => .lam b x (body.inst ms τs)
  | .app e₁ e₂ => .app (e₁.inst ms τs) (e₂.inst ms τs)
  | .pair e₁ e₂ => .pair (e₁.inst ms τs) (e₂.inst ms τs)
  | .fst e => .fst (e.inst ms τs)
  | .snd e => .snd (e.inst ms τs)

/-- Replace the variable `x` with the value `v`, which has no variables, so
nothing can be captured. -/
def Expr.subst : Expr → Nat → Expr → Expr
  | .var y, x, v => if y = x then v else .var y
  | .unit, _, _ => .unit
  | .int n, _, _ => .int n
  | .loc l, _, _ => .loc l
  | .new b e, x, v => .new b (e.subst x v)
  | .get e, x, v => .get (e.subst x v)
  | .set e₁ e₂, x, v => .set (e₁.subst x v) (e₂.subst x v)
  | .letE y e₁ e₂, x, v => .letE y (e₁.subst x v) (if y = x then e₂ else e₂.subst x v)
  | .call f ms τs e, x, v => .call f ms τs (e.subst x v)
  | .lam b y body, x, v => .lam b y (if y = x then body else body.subst x v)
  | .app e₁ e₂, x, v => .app (e₁.subst x v) (e₂.subst x v)
  | .pair e₁ e₂, x, v => .pair (e₁.subst x v) (e₂.subst x v)
  | .fst e, x, v => .fst (e.subst x v)
  | .snd e, x, v => .snd (e.subst x v)

/-! ## At most -/

/-- What a function assumes about its parameters (#243): `m ⊑ μ` for
mutabilities, like `'v ⊑ 'm`, and `T ⊑ μ` for type parameters, which says
that nothing in `T` is more `mut` than `μ`. -/
structure Assume where
  mus : List (Mu × Mu) := []
  tys : List (Nat × Mu) := []

/-- Nothing assumed, as for the program that's running. -/
def Assume.none : Assume := {}

/-- `m ⊑ μ`: mutabilities are ordered `imm ⊑ mut`, and a parameter is only
below another by an assumption. A region that holds `mut` objects is above
`mut` (#286), so it's as high as `mut`. -/
inductive Le (Δ : Assume) : Mu → Mu → Prop where
  | bot (μ : Mu) : Le Δ .immutable μ
  | top (m : Mu) : Le Δ m .mutable
  | regionMut (r : Nat) : Le Δ .mutable (.region r true)
  | refl (m : Mu) : Le Δ m m
  | assume {m μ : Mu} : (m, μ) ∈ Δ.mus → Le Δ m μ
  | trans {m₁ m₂ m₃ : Mu} : Le Δ m₁ m₂ → Le Δ m₂ m₃ → Le Δ m₁ m₃

/-- Nothing in a value of type `t` is more `mut` than `μ`: what an object of
mutability `μ` can hold. A cell or a closure in it is at most `μ`, what a
cell holds is at most its own mutability, and a type parameter is at most
what's assumed, or `mut`, which every type is. At most `imm` is `imm` all
the way down. -/
def AtMost (Δ : Assume) (μ : Mu) : Ty → Prop
  | .unit | .int => True
  | .cell m t => Le Δ m μ ∧ AtMost Δ m t
  | .fn m _ _ => Le Δ m μ
  | .prod a b => AtMost Δ μ a ∧ AtMost Δ μ b
  | .tparam i => ∃ μ', (μ' = .mutable ∨ (i, μ') ∈ Δ.tys) ∧ Le Δ μ' μ

/-- The arguments `ms` and `τs` meet the assumptions `Δ` of a function,
given what the caller assumes, `Δ'`. Every type argument is at most `mut`:
the objects in it are well-formed. -/
def Sat (Δ' Δ : Assume) (ms : List Mu) (τs : List Ty) : Prop :=
  (∀ p ∈ Δ.mus, Le Δ' (p.1.subst ms) (p.2.subst ms)) ∧
    (∀ p ∈ Δ.tys, AtMost Δ' (p.2.subst ms) (τs[p.1]?.getD .unit)) ∧
    (∀ i : Nat, AtMost Δ' .mutable (τs[i]?.getD .unit))

/-- With nothing assumed, `m ⊑ μ` only by the order itself: `μ` is `mut`
or a region that holds `mut` objects, which everything is below, or `m` is
`imm` or `μ`. -/
theorem Le.none_inv {m μ : Mu} (h : Le .none m μ) :
    m = .immutable ∨ μ = .mutable ∨ m = μ ∨ ∃ r, μ = .region r true := by
  induction h with
  | bot => exact .inl rfl
  | top => exact .inr (.inl rfl)
  | regionMut r => exact .inr (.inr (.inr ⟨r, rfl⟩))
  | refl => exact .inr (.inr (.inl rfl))
  | assume h => simp [Assume.none] at h
  | trans _ _ ih₁ ih₂ =>
    rcases ih₁ with h₁ | h₁ | h₁ | ⟨r, h₁⟩
    · exact .inl h₁
    · subst h₁
      rcases ih₂ with h₂ | h₂ | h₂ | h₂
      · cases h₂
      · exact .inr (.inl h₂)
      · subst h₂; exact .inr (.inl rfl)
      · exact .inr (.inr (.inr h₂))
    · subst h₁; exact ih₂
    · subst h₁
      rcases ih₂ with h₂ | h₂ | h₂ | h₂
      · cases h₂
      · exact .inr (.inl h₂)
      · subst h₂; exact .inr (.inr (.inr ⟨r, rfl⟩))
      · exact .inr (.inr (.inr h₂))

theorem Le.none_immutable {m : Mu} (h : Le .none m .immutable) : m = .immutable := by
  rcases h.none_inv with h | h | h | ⟨_, h⟩
  · exact h
  · cases h
  · exact h
  · cases h

/-- Something at most `m` is at most anything above `m`. -/
theorem AtMost.mono {Δ : Assume} {m μ : Mu} {t : Ty} (h : AtMost Δ m t) (hle : Le Δ m μ) : AtMost Δ μ t := by
  induction t generalizing m μ with
  | unit | int => trivial
  | cell m' t _ => exact ⟨h.1.trans hle, h.2⟩
  | fn m' a r _ _ => exact Le.trans h hle
  | prod a b iha ihb => exact ⟨iha h.1 hle, ihb h.2 hle⟩
  | tparam i =>
    obtain ⟨μ', hmem, hle'⟩ := h
    exact ⟨μ', hmem, hle'.trans hle⟩

theorem Le.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m μ : Mu}
    (h : Le Δ m μ) : Le Δ' (m.subst ms) (μ.subst ms) := by
  induction h with
  | bot => exact .bot _
  | top => exact .top _
  | regionMut r => exact .regionMut r
  | refl => exact .refl _
  | assume h => exact hs.1 _ h
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

/-- What's at most `μ` stays so for arguments that meet the assumptions. -/
theorem AtMost.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {μ : Mu} {t : Ty}
    (h : AtMost Δ μ t) : AtMost Δ' (μ.subst ms) (t.inst ms τs) := by
  induction t generalizing μ with
  | unit | int => trivial
  | cell m t ih => exact ⟨Le.inst hs h.1, ih h.2⟩
  | fn m a r _ _ => exact Le.inst hs h
  | prod a b iha ihb => exact ⟨iha h.1, ihb h.2⟩
  | tparam i =>
    obtain ⟨μ', hmem, hle⟩ := h
    rcases hmem with rfl | hmem
    · exact (hs.2.2 i).mono (Le.inst hs hle)
    · exact (hs.2.1 (i, μ') hmem).mono (Le.inst hs hle)

theorem Mu.subst_muOf (ms : List Mu) (b : Bool) : (muOf b).subst ms = muOf b := by
  cases b <;> rfl

theorem Mu.subst_subst (ms ms' : List Mu) (m : Mu) :
    (m.subst ms').subst ms = m.subst (ms'.map (Mu.subst ms)) := by
  cases m with
  | mutable => rfl
  | immutable => rfl
  | param i =>
    simp only [Mu.subst, List.getElem?_map]
    cases ms'[i]? <;> rfl
  | region _ _ => rfl

theorem Ty.getD_inst (ms : List Mu) (τs τs' : List Ty) (i : Nat) :
    (τs'[i]?.getD .unit).inst ms τs = (τs'.map (Ty.inst ms τs))[i]?.getD .unit := by
  simp only [List.getElem?_map]
  cases τs'[i]? <;> rfl

theorem Ty.inst_inst (ms ms' : List Mu) (τs τs' : List Ty) (t : Ty) :
    (t.inst ms' τs').inst ms τs = t.inst (ms'.map (Mu.subst ms)) (τs'.map (Ty.inst ms τs)) := by
  induction t with
  | unit => rfl
  | int => rfl
  | cell m t ih => simp [Ty.inst, ih, Mu.subst_subst]
  | fn m a r iha ihr => simp [Ty.inst, iha, ihr, Mu.subst_subst]
  | prod a b iha ihb => simp [Ty.inst, iha, ihb]
  | tparam i => exact Ty.getD_inst ms τs τs' i

/-- Instantiating a call inside a generic function that's being
instantiated still meets the callee's assumptions. -/
theorem Sat.comp {Δ₀ Δ₁ Δ₂ : Assume} {ms ms' : List Mu} {τs τs' : List Ty}
    (h₁ : Sat Δ₁ Δ₂ ms' τs') (h₀ : Sat Δ₀ Δ₁ ms τs) :
    Sat Δ₀ Δ₂ (ms'.map (Mu.subst ms)) (τs'.map (Ty.inst ms τs)) := by
  refine ⟨fun p hp => ?_, fun p hp => ?_, fun i => ?_⟩
  · have := Le.inst h₀ (h₁.1 p hp)
    rwa [Mu.subst_subst, Mu.subst_subst] at this
  · have := AtMost.inst h₀ (h₁.2.1 p hp)
    rwa [Mu.subst_subst, Ty.getD_inst] at this
  · have := AtMost.inst h₀ (h₁.2.2 i)
    rwa [Ty.getD_inst] at this

theorem Expr.NoLocs.inst {e : Expr} (ms : List Mu) (τs : List Ty) (h : e.NoLocs) : (e.inst ms τs).NoLocs := by
  induction e with
  | var | unit | int => trivial
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | lam _ _ _ ih => exact ih h

/-- An expression without locations has none. -/
theorem Expr.NoLocs.not_hasLoc {e : Expr} (h : e.NoLocs) (l : Nat) : ¬ e.HasLoc l := by
  induction e with
  | var | unit | int => exact id
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact fun hl => hl.elim (ih₁ h.1) (ih₂ h.2)

/-- The locations of `e.subst x v` are those of `e` and of `v`. -/
theorem Expr.hasLoc_subst {e : Expr} {x : Nat} {v : Expr} {l : Nat} (h : (e.subst x v).HasLoc l) :
    e.HasLoc l ∨ v.HasLoc l := by
  induction e with
  | var y =>
    simp only [Expr.subst] at h
    split at h
    · exact .inr h
    · exact h.elim
  | unit | int => exact h.elim
  | loc => exact .inl h
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    rcases h with h | h
    · exact (ih₁ h).imp .inl id
    · exact (ih₂ h).imp .inr id
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst, Expr.HasLoc] at h
    rcases h with h | h
    · exact (ih₁ h).imp .inl id
    · by_cases hyx : y = x
      · simp only [hyx, ite_true] at h
        exact .inl (.inr h)
      · simp only [hyx, ite_false] at h
        exact (ih₂ h).imp .inr id
  | lam b y body ih =>
    simp only [Expr.subst, Expr.HasLoc] at h
    split at h
    · exact .inl h
    · exact ih h

end BoltMut
