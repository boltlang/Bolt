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
see `Regions.lean`). A region becomes `imm` when its block ends. One that
would have to hold `mut` objects, `mut ⊑ r`, is just `mut`: the checker
decides that right away, as `Le.eq_mutable` in `Solver.lean` says (#286). -/
inductive Mu where
  | mutable
  | immutable
  | param (i : Nat)
  | region (r : Nat)
  deriving DecidableEq, Repr

/-- Types. `cell m t` is a shared object with mutability `m` holding a `t`,
`fn m a r` a closure with mutability `m` from `a` to `r`, and `tparam i` the
`i`th type parameter of the function being checked. `gone m` is a closure
with mutability `m` that captured a region that's been freed (#287): it
can't be called any more, since what it changes may now be `imm`. -/
inductive Ty where
  | unit
  | int
  | cell (m : Mu) (t : Ty)
  | fn (m : Mu) (a r : Ty)
  | prod (a b : Ty)
  | tparam (i : Nat)
  | gone (m : Mu)
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

/-- A mutability that isn't a region. -/
def Mu.NoRegion : Mu → Prop
  | .region _ => False
  | _ => True

/-- A type that mentions no region. -/
def Ty.NoRegion : Ty → Prop
  | .unit | .int | .tparam _ => True
  | .gone m => m.NoRegion
  | .cell m t => m.NoRegion ∧ t.NoRegion
  | .fn m a b => m.NoRegion ∧ a.NoRegion ∧ b.NoRegion
  | .prod a b => a.NoRegion ∧ b.NoRegion

/-- `m` is the region `r`. -/
def Mu.mentions (r : Nat) : Mu → Bool
  | .region r' => r' == r
  | _ => false

/-- `t` mentions the region `r`. -/
def Ty.mentions (r : Nat) : Ty → Bool
  | .unit | .int | .tparam _ => false
  | .cell m t => m.mentions r || t.mentions r
  | .fn m a b => m.mentions r || a.mentions r || b.mentions r
  | .prod a b => a.mentions r || b.mentions r
  | .gone m => m.mentions r

/-- The only region `m` can be is `μ`'s: what a closure of mutability `μ`
captures, by rule 5 of the regions page. -/
def Mu.RegionsIn (m μ : Mu) : Prop := ∀ r, m.mentions r = true → μ.mentions r = true

/-- The only region `t` mentions is `μ`'s. -/
def Ty.RegionsIn (t : Ty) (μ : Mu) : Prop := ∀ r, t.mentions r = true → μ.mentions r = true


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
  | .gone m => .gone (m.subst ms)

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

/-- The variable `y` is free in the expression. -/
def Expr.Free : Expr → Nat → Prop
  | .var x, y => x = y
  | .unit, _ | .int _, _ | .loc _, _ => False
  | .new _ e, y => e.Free y
  | .get e, y => e.Free y
  | .call _ _ _ e, y => e.Free y
  | .set e₁ e₂, y => e₁.Free y ∨ e₂.Free y
  | .letE x e₁ e₂, y => e₁.Free y ∨ (x ≠ y ∧ e₂.Free y)
  | .lam _ x body, y => x ≠ y ∧ body.Free y
  | .app e₁ e₂, y => e₁.Free y ∨ e₂.Free y
  | .pair e₁ e₂, y => e₁.Free y ∨ e₂.Free y
  | .fst e, y => e.Free y
  | .snd e, y => e.Free y

/-- Substituting a variable that isn't free changes nothing. -/
theorem Expr.subst_not_free {e : Expr} {x : Nat} (v : Expr) (h : ¬ e.Free x) : e.subst x v = e := by
  induction e with
  | var y =>
    simp only [Expr.Free] at h
    simp [Expr.subst, h]
  | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih =>
    simp only [Expr.subst]; rw [ih h]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.Free, not_or] at h
    simp only [Expr.subst]; rw [ih₁ h.1, ih₂ h.2]
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.Free, not_or, not_and] at h
    simp only [Expr.subst, ih₁ h.1]
    split
    · rfl
    · rename_i hyx; rw [ih₂ (h.2 hyx)]
  | lam b y _ ih =>
    simp only [Expr.Free, not_and] at h
    simp only [Expr.subst]
    split
    · rfl
    · rename_i hyx; rw [ih (h hyx)]

/-- Instantiating the calls of an expression keeps its variables. -/
theorem Expr.free_inst {e : Expr} (ms : List Mu) (τs : List Ty) (y : Nat) : (e.inst ms τs).Free y ↔ e.Free y := by
  induction e with
  | var | unit | int | loc => simp [Expr.inst, Expr.Free]
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => simp only [Expr.inst, Expr.Free, ih]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ =>
    simp only [Expr.inst, Expr.Free, ih₁, ih₂]
  | lam _ _ _ ih => simp only [Expr.inst, Expr.Free, ih]

/-! ## At most -/

/-- What a function assumes about its parameters (#243): `m ⊑ μ` for
mutabilities, like `'v ⊑ 'm`, and `T ⊑ μ` for type parameters, which says
that nothing in `T` is more `mut` than `μ`. -/
structure Assume where
  mus : List (Mu × Mu) := []
  tys : List (Nat × Mu) := []
  /-- The mutability parameters and type parameters that a call can't give
  a region (#285): those that what a closure captures has. Then the closure
  doesn't capture a region's value once the function is called. -/
  noRegionMus : List Nat := []
  noRegionTys : List Nat := []
  /-- The mutability parameters that a call can only give `mut` or a
  region (rule 1 of the regions page): the function can change what has
  them. -/
  changeable : List Nat := []

/-- Nothing assumed, as for the program that's running. -/
def Assume.none : Assume := {}

/-- `m ⊑ μ`: mutabilities are ordered `imm ⊑ mut`, and a parameter is only
below another by an assumption. -/
inductive Le (Δ : Assume) : Mu → Mu → Prop where
  | bot (μ : Mu) : Le Δ .immutable μ
  | top (m : Mu) : Le Δ m .mutable
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
  | .gone m => Le Δ m μ

/-- The parameters in `m` are ones that a call can't give a region. -/
def Mu.ParamsNoRegion (Δ : Assume) : Mu → Prop
  | .param i => i ∈ Δ.noRegionMus
  | _ => True

/-- The parameters in `t` are ones that a call can't give a region: once
the function is called, `t` mentions no region that it doesn't now. -/
def Ty.ParamsNoRegion (Δ : Assume) : Ty → Prop
  | .unit | .int => True
  | .tparam i => i ∈ Δ.noRegionTys
  | .cell m t => m.ParamsNoRegion Δ ∧ t.ParamsNoRegion Δ
  | .fn m a b => m.ParamsNoRegion Δ ∧ a.ParamsNoRegion Δ ∧ b.ParamsNoRegion Δ
  | .prod a b => a.ParamsNoRegion Δ ∧ b.ParamsNoRegion Δ
  | .gone m => m.ParamsNoRegion Δ

/-- A mutability whose objects can be changed: `mut`, a region, or a
parameter that's assumed to be one of them. -/
def Mu.Changeable (Δ : Assume) : Mu → Prop
  | .mutable | .region _ => True
  | .param i => i ∈ Δ.changeable
  | .immutable => False

/-- The arguments `ms` and `τs` meet the assumptions `Δ` of a function,
given what the caller assumes, `Δ'`. Every type argument is at most `mut`:
the objects in it are well-formed. A parameter that can't be given a region
gets an argument that mentions none, and whose parameters can't be given
one either, and a changeable parameter gets a changeable argument. -/
def Sat (Δ' Δ : Assume) (ms : List Mu) (τs : List Ty) : Prop :=
  (∀ p ∈ Δ.mus, Le Δ' (p.1.subst ms) (p.2.subst ms)) ∧
    (∀ p ∈ Δ.tys, AtMost Δ' (p.2.subst ms) (τs[p.1]?.getD .unit)) ∧
    (∀ i : Nat, AtMost Δ' .mutable (τs[i]?.getD .unit)) ∧
    (∀ i ∈ Δ.noRegionMus, (ms[i]?.getD .immutable).NoRegion ∧ (ms[i]?.getD .immutable).ParamsNoRegion Δ') ∧
    (∀ i ∈ Δ.noRegionTys, (τs[i]?.getD .unit).NoRegion ∧ (τs[i]?.getD .unit).ParamsNoRegion Δ') ∧
    (∀ i ∈ Δ.changeable, (ms[i]?.getD .immutable).Changeable Δ')

/-- With nothing assumed, `m ⊑ μ` only by the order itself. -/
theorem Le.none_inv {m μ : Mu} (h : Le .none m μ) : m = .immutable ∨ μ = .mutable ∨ m = μ := by
  induction h with
  | bot => exact .inl rfl
  | top => exact .inr (.inl rfl)
  | refl => exact .inr (.inr rfl)
  | assume h => simp [Assume.none] at h
  | trans _ _ ih₁ ih₂ =>
    rcases ih₁ with h₁ | h₁ | h₁
    · exact .inl h₁
    · subst h₁
      rcases ih₂ with h₂ | h₂ | h₂
      · cases h₂
      · exact .inr (.inl h₂)
      · subst h₂; exact .inr (.inl rfl)
    · subst h₁; exact ih₂

theorem Le.none_immutable {m : Mu} (h : Le .none m .immutable) : m = .immutable := by
  rcases h.none_inv with h | h | h
  · exact h
  · cases h
  · exact h

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
  | gone m' => exact Le.trans h hle

theorem Le.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m μ : Mu}
    (h : Le Δ m μ) : Le Δ' (m.subst ms) (μ.subst ms) := by
  induction h with
  | bot => exact .bot _
  | top => exact .top _
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
    · exact (hs.2.2.1 i).mono (Le.inst hs hle)
    · exact (hs.2.1 (i, μ') hmem).mono (Le.inst hs hle)
  | gone m => exact Le.inst hs h

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
  | region _ => rfl

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
  | gone m => simp [Ty.inst, Mu.subst_subst]

/-- Instantiating a call inside a generic function that's being
instantiated still meets the callee's assumptions. -/
theorem Mu.getD_subst (ms ms' : List Mu) (i : Nat) :
    (ms'[i]?.getD .immutable).subst ms = (ms'.map (Mu.subst ms))[i]?.getD .immutable := by
  simp only [List.getElem?_map]
  cases ms'[i]? <;> rfl

/-- What mentions no region, and has only parameters that can't be given
one, mentions none once they're given. -/
theorem Mu.noRegion_subst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m : Mu}
    (hn : m.NoRegion) (hp : m.ParamsNoRegion Δ) : (m.subst ms).NoRegion ∧ (m.subst ms).ParamsNoRegion Δ' := by
  cases m with
  | param i => exact hs.2.2.2.1 i hp
  | region => exact hn.elim
  | mutable | immutable => exact ⟨trivial, trivial⟩

theorem Ty.noRegion_inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {t : Ty}
    (hn : t.NoRegion) (hp : t.ParamsNoRegion Δ) : (t.inst ms τs).NoRegion ∧ (t.inst ms τs).ParamsNoRegion Δ' := by
  induction t with
  | unit | int => exact ⟨trivial, trivial⟩
  | cell m t ih =>
    obtain ⟨h₁, h₂⟩ := Mu.noRegion_subst hs hn.1 hp.1
    obtain ⟨h₃, h₄⟩ := ih hn.2 hp.2
    exact ⟨⟨h₁, h₃⟩, ⟨h₂, h₄⟩⟩
  | fn m a b iha ihb =>
    obtain ⟨h₁, h₂⟩ := Mu.noRegion_subst hs hn.1 hp.1
    obtain ⟨h₃, h₄⟩ := iha hn.2.1 hp.2.1
    obtain ⟨h₅, h₆⟩ := ihb hn.2.2 hp.2.2
    exact ⟨⟨h₁, h₃, h₅⟩, ⟨h₂, h₄, h₆⟩⟩
  | prod a b iha ihb =>
    obtain ⟨h₁, h₂⟩ := iha hn.1 hp.1
    obtain ⟨h₃, h₄⟩ := ihb hn.2 hp.2
    exact ⟨⟨h₁, h₃⟩, ⟨h₂, h₄⟩⟩
  | tparam i => exact hs.2.2.2.2.1 i hp
  | gone m => exact Mu.noRegion_subst hs hn hp

/-- A changeable mutability stays so with arguments that meet the
assumptions. -/
theorem Mu.Changeable.subst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m : Mu}
    (h : m.Changeable Δ) : (m.subst ms).Changeable Δ' := by
  cases m with
  | param i => exact hs.2.2.2.2.2 i h
  | immutable => exact h.elim
  | _ => exact h

theorem Sat.comp {Δ₀ Δ₁ Δ₂ : Assume} {ms ms' : List Mu} {τs τs' : List Ty}
    (h₁ : Sat Δ₁ Δ₂ ms' τs') (h₀ : Sat Δ₀ Δ₁ ms τs) :
    Sat Δ₀ Δ₂ (ms'.map (Mu.subst ms)) (τs'.map (Ty.inst ms τs)) := by
  refine ⟨fun p hp => ?_, fun p hp => ?_, fun i => ?_, fun i hi => ?_, fun i hi => ?_, fun i hi => ?_⟩
  · have := Le.inst h₀ (h₁.1 p hp)
    rwa [Mu.subst_subst, Mu.subst_subst] at this
  · have := AtMost.inst h₀ (h₁.2.1 p hp)
    rwa [Mu.subst_subst, Ty.getD_inst] at this
  · have := AtMost.inst h₀ (h₁.2.2.1 i)
    rwa [Ty.getD_inst] at this
  · obtain ⟨hn, hp⟩ := h₁.2.2.2.1 i hi
    rw [← Mu.getD_subst]
    exact Mu.noRegion_subst h₀ hn hp
  · obtain ⟨hn, hp⟩ := h₁.2.2.2.2.1 i hi
    rw [← Ty.getD_inst]
    exact Ty.noRegion_inst h₀ hn hp
  · rw [← Mu.getD_subst]
    exact Mu.Changeable.subst h₀ (h₁.2.2.2.2.2 i hi)

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
