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

A tuple is a value: `prod a b`. A struct is a cell that holds the tuple
of its fields, so one mutability covers all of them, and a field
that's a struct is a cell in that tuple. Changing a field replaces the
tuple, and `S { a: x, ..base }` is `new b (pair x (snd (get base)))`: the
fields it doesn't give are shared with `base`, at the types they have
there. A generic struct like `Box[T]` is a cell holding a `T`, and a field
with a mutability of its own (`'v`, #243) is a cell with a mutability
parameter.

A tuple of Bolt, like `mut (1, 2)`, has a mutability too: it's a struct
with numeric fields, so a cell holding a `prod`.

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
`imm`, `ro`, or the `i`th mutability parameter of the function being
checked. -/
inductive Mu where
  | mutable
  | immutable
  /-- `ro` (#320): a view that can read an object but not change it, and
  that doesn't promise it won't change. No object is made `ro`: a `mut`
  or `imm` one is seen as `ro` through subtyping (`Sub`). -/
  | readonly
  | param (i : Nat)
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
  /-- `ro T` (#320): the `i`th type parameter seen as `ro`. Instantiated, it's
  the argument seen as `ro` (`Ty.ro`). -/
  | roParam (i : Nat)
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
  /-- `imm e` (#335): `e`'s value with each `mut` object it reaches copied
  into a new `imm` one. One `inPlace` makes the same copy but goes on with
  the value itself, which can't be noticed when nothing else has what it
  reaches; only the program that leaves copies out has it, so it isn't
  typed. -/
  | toImm (inPlace : Bool) (e : Expr)
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
  | .toImm _ e => e.NoLocs

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
  | .toImm _ e, l => e.HasLoc l

/-- Replace the mutability parameters with `ms`; one without an argument
becomes `imm`. -/
def Mu.subst (ms : List Mu) : Mu → Mu
  | .param i => ms[i]?.getD .immutable
  | m => m

/-- `t` seen as `ro` (#320): each cell `ro`, with what it holds seen the
same way, and each type parameter `ro T`. A closure keeps its mutability:
it never changes once it's made, and its mutability doesn't keep it from
being called, so a view has nothing to take from it. -/
def Ty.ro : Ty → Ty
  | .unit => .unit
  | .int => .int
  | .cell _ t => .cell .readonly t.ro
  | .fn m a r => .fn m a r
  | .prod a b => .prod a.ro b.ro
  | .tparam i => .roParam i
  | .roParam i => .roParam i

/-- Replace the mutability parameters with `ms` and the type parameters with
`τs`; one without an argument becomes `()`. -/
def Ty.inst (ms : List Mu) (τs : List Ty) : Ty → Ty
  | .unit => .unit
  | .int => .int
  | .cell m t => .cell (m.subst ms) (t.inst ms τs)
  | .fn m a r => .fn (m.subst ms) (a.inst ms τs) (r.inst ms τs)
  | .prod a b => .prod (a.inst ms τs) (b.inst ms τs)
  | .tparam i => τs[i]?.getD .unit
  | .roParam i => (τs[i]?.getD .unit).ro

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
  | .toImm b e => .toImm b (e.inst ms τs)

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
  | .toImm b e, x, v => .toImm b (e.subst x v)

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
  | .fn m _ _ => Le Δ m μ ∨ Le Δ .readonly μ
  | .prod a b => AtMost Δ μ a ∧ AtMost Δ μ b
  | .tparam i => ∃ μ', (μ' = .mutable ∨ (i, μ') ∈ Δ.tys) ∧ Le Δ μ' μ
  | .roParam _ => Le Δ .readonly μ

/-- The arguments `ms` and `τs` meet the assumptions `Δ` of a function,
given what the caller assumes, `Δ'`. Every type argument is at most `mut`:
the objects in it are well-formed. -/
def Sat (Δ' Δ : Assume) (ms : List Mu) (τs : List Ty) : Prop :=
  (∀ p ∈ Δ.mus, Le Δ' (p.1.subst ms) (p.2.subst ms)) ∧
    (∀ p ∈ Δ.tys, AtMost Δ' (p.2.subst ms) (τs[p.1]?.getD .unit)) ∧
    (∀ i : Nat, AtMost Δ' .mutable (τs[i]?.getD .unit))

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
  | fn m' a r _ _ => exact h.elim (fun h => .inl (h.trans hle)) (fun h => .inr (h.trans hle))
  | prod a b iha ihb => exact ⟨iha h.1 hle, ihb h.2 hle⟩
  | tparam i =>
    obtain ⟨μ', hmem, hle'⟩ := h
    exact ⟨μ', hmem, hle'.trans hle⟩
  | roParam i => exact Le.trans h hle

theorem Le.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m μ : Mu}
    (h : Le Δ m μ) : Le Δ' (m.subst ms) (μ.subst ms) := by
  induction h with
  | bot => exact .bot _
  | top => exact .top _
  | refl => exact .refl _
  | assume h => exact hs.1 _ h
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

/-- With nothing assumed, a closure at most `imm` is an `imm` one: `ro`
isn't at most `imm`. -/
theorem AtMost.none_fn {m : Mu} {a r : Ty} (h : AtMost .none .immutable (.fn m a r)) :
    Le .none m .immutable :=
  h.elim id (fun h => nomatch Le.none_immutable h)

/-- A type seen as `ro` is at most `ro`. -/
theorem AtMost.of_ro (Δ : Assume) (t : Ty) : AtMost Δ .readonly t.ro := by
  induction t with
  | unit | int => trivial
  | cell m t ih => exact ⟨.refl _, ih⟩
  | fn m a r _ _ => exact .inr (.refl _)
  | prod a b iha ihb => exact ⟨iha, ihb⟩
  | tparam i => exact .refl _
  | roParam i => exact .refl _

/-- What's at most `μ` stays so for arguments that meet the assumptions. -/
theorem AtMost.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {μ : Mu} {t : Ty}
    (h : AtMost Δ μ t) : AtMost Δ' (μ.subst ms) (t.inst ms τs) := by
  induction t generalizing μ with
  | unit | int => trivial
  | cell m t ih => exact ⟨Le.inst hs h.1, ih h.2⟩
  | fn m a r _ _ => exact h.elim (fun h => .inl (Le.inst hs h)) (fun h => .inr (Le.inst hs h))
  | prod a b iha ihb => exact ⟨iha h.1, ihb h.2⟩
  | tparam i =>
    obtain ⟨μ', hmem, hle⟩ := h
    rcases hmem with rfl | hmem
    · exact (hs.2.2 i).mono (Le.inst hs hle)
    · exact (hs.2.1 (i, μ') hmem).mono (Le.inst hs hle)
  | roParam i => exact (AtMost.of_ro Δ' _).mono (Le.inst hs h)

theorem Mu.subst_muOf (ms : List Mu) (b : Bool) : (muOf b).subst ms = muOf b := by
  cases b <;> rfl

theorem Mu.subst_subst (ms ms' : List Mu) (m : Mu) :
    (m.subst ms').subst ms = m.subst (ms'.map (Mu.subst ms)) := by
  cases m with
  | mutable => rfl
  | immutable => rfl
  | readonly => rfl
  | param i =>
    simp only [Mu.subst, List.getElem?_map]
    cases ms'[i]? <;> rfl

theorem Ty.getD_inst (ms : List Mu) (τs τs' : List Ty) (i : Nat) :
    (τs'[i]?.getD .unit).inst ms τs = (τs'.map (Ty.inst ms τs))[i]?.getD .unit := by
  simp only [List.getElem?_map]
  cases τs'[i]? <;> rfl

/-- Seeing a type as `ro` twice is seeing it once. -/
theorem Ty.ro_ro (t : Ty) : t.ro.ro = t.ro := by
  induction t with
  | unit | int | tparam | roParam => rfl
  | cell m t ih => simp [Ty.ro, ih]
  | fn m a r => rfl
  | prod a b iha ihb => simp [Ty.ro, iha, ihb]

/-- Seeing a type as `ro` and instantiating it go either way. -/
theorem Ty.ro_inst (ms : List Mu) (τs : List Ty) (t : Ty) : t.ro.inst ms τs = (t.inst ms τs).ro := by
  induction t with
  | unit | int | tparam => rfl
  | roParam i => simp [Ty.ro, Ty.inst, Ty.ro_ro]
  | cell m t ih => simp [Ty.ro, Ty.inst, Mu.subst, ih]
  | fn m a r => rfl
  | prod a b iha ihb => simp [Ty.ro, Ty.inst, iha, ihb]

theorem Ty.inst_inst (ms ms' : List Mu) (τs τs' : List Ty) (t : Ty) :
    (t.inst ms' τs').inst ms τs = t.inst (ms'.map (Mu.subst ms)) (τs'.map (Ty.inst ms τs)) := by
  induction t with
  | unit => rfl
  | int => rfl
  | cell m t ih => simp [Ty.inst, ih, Mu.subst_subst]
  | fn m a r iha ihr => simp [Ty.inst, iha, ihr, Mu.subst_subst]
  | prod a b iha ihb => simp [Ty.inst, iha, ihb]
  | tparam i => exact Ty.getD_inst ms τs τs' i
  | roParam i => simp only [Ty.inst, Ty.ro_inst, Ty.getD_inst]

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
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | lam _ _ _ ih => exact ih h

/-- An expression without locations has none. -/
theorem Expr.NoLocs.not_hasLoc {e : Expr} (h : e.NoLocs) (l : Nat) : ¬ e.HasLoc l := by
  induction e with
  | var | unit | int => exact id
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
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
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
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

/-! ## The type of `imm e` -/

/-- `t'` is the type of `imm e` for an `e` of type `t` (#335): each cell
becomes `imm`, and a tuple is done part by part. What's `imm` all the way
down can stay as it is: that's how a closure and a type parameter are
done, which can't be copied, and it keeps `imm x` of a `T ⊑ imm` a `T`. -/
inductive ImmTy (Δ : Assume) : Ty → Ty → Prop where
  | asIs {t : Ty} : AtMost Δ .immutable t → ImmTy Δ t t
  | cell {m : Mu} {t t' : Ty} : ImmTy Δ t t' → ImmTy Δ (.cell m t) (.cell .immutable t')
  | prod {a b a' b' : Ty} : ImmTy Δ a a' → ImmTy Δ b b' → ImmTy Δ (.prod a b) (.prod a' b')

/-- What `imm e` gives is `imm` all the way down. -/
theorem ImmTy.atMost {Δ : Assume} {t t' : Ty} (h : ImmTy Δ t t') : AtMost Δ .immutable t' := by
  induction h with
  | asIs h => exact h
  | cell _ ih => exact ⟨.refl _, ih⟩
  | prod _ _ iha ihb => exact ⟨iha, ihb⟩

/-- It stays so for arguments that meet the assumptions. -/
theorem ImmTy.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {t t' : Ty}
    (h : ImmTy Δ t t') : ImmTy Δ' (t.inst ms τs) (t'.inst ms τs) := by
  induction h with
  | asIs h => exact .asIs (by simpa [Mu.subst] using AtMost.inst hs h)
  | cell _ ih => exact .cell ih
  | prod _ _ iha ihb => exact .prod iha ihb

/-! ## Read-only views

`ro` (#320) sits between `imm` and `mut` in what an object can hold. It's
also above both in subtyping: a `mut` or an `imm` object can be seen as
`ro`, which can read it but not change it (`Sub`). The two orders differ:
an `imm` object can't hold a `ro` one, since that may change, but an `imm`
object can be seen as `ro`.

A `ro` view is deep: what it holds is at most `ro`, so what's read through
it can't be changed either. An object of any mutability can be seen as
`ro`, one whose mutability is a parameter too, and a value of a type
parameter `T` as `ro T`. A closure stays as it is: it never changes once
it's made, so something at least `ro` can hold any closure. -/

/-- `t` can be used where `t'` is expected: the same type, with some of its
`mut`, `imm` and `ro` objects seen as `ro`. A `mut` object's contents can
be seen as `ro` too, since nothing can be stored through the view. -/
inductive Sub (Δ : Assume) : Ty → Ty → Prop where
  | refl (t : Ty) : Sub Δ t t
  | ro {m : Mu} {t t' : Ty} : Sub Δ t t' → AtMost Δ .readonly t' →
      Sub Δ (.cell m t) (.cell .readonly t')
  | prod {a a' b b' : Ty} : Sub Δ a a' → Sub Δ b b' → Sub Δ (.prod a b) (.prod a' b')
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

theorem Sub.trans {Δ : Assume} {t₁ t₂ t₃ : Ty} (h₁ : Sub Δ t₁ t₂) (h₂ : Sub Δ t₂ t₃) : Sub Δ t₁ t₃ := by
  induction h₁ generalizing t₃ with
  | refl => exact h₂
  | ro _ _ ih =>
    cases h₂ with
    | refl => exact .ro (by assumption) (by assumption)
    | ro h₂ ha => exact .ro (ih h₂) ha
  | prod _ _ iha ihb =>
    cases h₂ with
    | refl => exact .prod (by assumption) (by assumption)
    | prod ha hb => exact .prod (iha ha) (ihb hb)
  | tparam i => cases h₂; exact .tparam i

/-- Seeing an object as `ro` survives instantiating. -/
theorem Sub.inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {t t' : Ty}
    (h : Sub Δ t t') : Sub Δ' (t.inst ms τs) (t'.inst ms τs) := by
  induction h with
  | refl => exact .refl _
  | ro _ ha ih =>
    simp only [Ty.inst]
    have := AtMost.inst hs ha
    simp only [Mu.subst] at this
    exact .ro ih this
  | prod _ _ iha ihb => exact .prod iha ihb
  | tparam i => exact Sub.to_ro Δ' _

/-- What `t` can be used as, when `t` isn't a `ro` view or a tuple: only `t`. -/
theorem Sub.cell_inv {Δ : Assume} {m : Mu} {t t' : Ty} (h : Sub Δ (.cell m t) t') :
    t' = .cell m t ∨ ∃ s, t' = .cell .readonly s ∧ Sub Δ t s ∧ AtMost Δ .readonly s := by
  cases h with
  | refl => exact .inl rfl
  | ro h ha => exact .inr ⟨_, rfl, h, ha⟩

theorem Sub.to_cell_inv {Δ : Assume} {m : Mu} {t t' : Ty} (h : Sub Δ t (.cell m t')) (hm : m ≠ .readonly) :
    t = .cell m t' := by
  cases h with
  | refl => rfl
  | ro => exact (hm rfl).elim

theorem Sub.prod_inv {Δ : Assume} {a b t' : Ty} (h : Sub Δ (.prod a b) t') :
    ∃ a' b', t' = .prod a' b' ∧ Sub Δ a a' ∧ Sub Δ b b' := by
  cases h with
  | refl => exact ⟨a, b, rfl, .refl _, .refl _⟩
  | prod ha hb => exact ⟨_, _, rfl, ha, hb⟩

/-- Other types are only subtypes of themselves. -/
theorem Sub.eq_of_not {Δ : Assume} {t t' : Ty} (h : Sub Δ t t')
    (hc : ∀ m s, t ≠ .cell m s) (hp : ∀ a b, t ≠ .prod a b) (hq : ∀ i, t ≠ .tparam i) : t' = t := by
  cases h with
  | refl => rfl
  | ro => exact (hc _ _ rfl).elim
  | prod => exact (hp _ _ rfl).elim
  | tparam => exact (hq _ rfl).elim

/-- What's a subtype of a tuple: a tuple. -/
theorem Sub.to_prod_inv {Δ : Assume} {t a b : Ty} (h : Sub Δ t (.prod a b)) :
    ∃ a₀ b₀, t = .prod a₀ b₀ ∧ Sub Δ a₀ a ∧ Sub Δ b₀ b := by
  cases h with
  | refl => exact ⟨a, b, rfl, .refl _, .refl _⟩
  | prod ha hb => exact ⟨_, _, rfl, ha, hb⟩

/-- What's a subtype of a cell: a cell. -/
theorem Sub.to_cell {Δ : Assume} {t t' : Ty} {m : Mu} (h : Sub Δ t (.cell m t')) :
    ∃ m₀ t₀, t = .cell m₀ t₀ := by
  cases h with
  | refl => exact ⟨_, _, rfl⟩
  | ro => exact ⟨_, _, rfl⟩

/-- What's a subtype of a function type: a function type. -/
theorem Sub.to_fn {Δ : Assume} {t a r : Ty} {m : Mu} (h : Sub Δ t (.fn m a r)) :
    ∃ a₀ r₀, t = .fn m a₀ r₀ := by
  cases h with
  | refl => exact ⟨_, _, rfl⟩

/-- What a function type is a subtype of: a function type. -/
theorem Sub.of_fn {Δ : Assume} {m : Mu} {a r t : Ty} (h : Sub Δ (.fn m a r) t) :
    ∃ a' r', t = .fn m a' r' := by
  cases h with
  | refl => exact ⟨_, _, rfl⟩

/-- A function type is a subtype of another when they're the same. -/
theorem Sub.fn_inv {Δ : Assume} {m m' : Mu} {a a' r r' : Ty} (h : Sub Δ (.fn m a r) (.fn m' a' r')) :
    m = m' ∧ a = a' ∧ r = r' := by
  cases h with
  | refl => exact ⟨rfl, rfl, rfl⟩

/-- What's a subtype of a type with nothing to see as `ro` in it: that type. -/
theorem Sub.to_eq_of_not {Δ : Assume} {t t' : Ty} (h : Sub Δ t t')
    (hc : ∀ s, t' ≠ .cell .readonly s) (hp : ∀ a b, t' ≠ .prod a b) (hq : ∀ i, t' ≠ .roParam i) :
    t = t' := by
  cases h with
  | refl => rfl
  | ro => exact (hc _ rfl).elim
  | prod => exact (hp _ _ rfl).elim
  | tparam => exact (hq _ rfl).elim

/-- Nothing is seen as `ro` in a type that's `imm` all the way down, so only
that type is a subtype of it. -/
theorem Sub.eq_of_imm {t t' : Ty} (h : Sub .none t t') (hd : AtMost .none .immutable t') : t = t' := by
  induction h with
  | refl => rfl
  | ro _ _ => exact nomatch Le.none_immutable hd.1
  | prod _ _ iha ihb => rw [iha hd.1, ihb hd.2]
  | tparam => exact nomatch Le.none_immutable hd

end BoltMut
