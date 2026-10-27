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

A tuple is a value, as in Bolt: `prod a b`. A struct is a cell that holds
the tuple of its fields, so one mutability covers all of them, and a field
that's a struct is a cell in that tuple. Changing a field replaces the
tuple, and `S { a: x, ..base }` is `new b (pair x (snd (get base)))`: the
fields it doesn't give are shared with `base`, at the types they have
there.

Closures have a mutability too, as #244 proposes: `fn m a r`. A closure is
`imm` only if everything it captures is `imm` all the way down. In Bolt, a
closure shares the `let mut` variables it captures, which the evaluator
keeps in shared slots; here, such a variable is a `mut` cell, so capturing
one makes a closure `mut` by the same rule.
-/

namespace BoltMut

/-- A mutability: `mut` (`mutable`, since `mut` is a keyword of Lean),
`imm`, or the `i`th mutability parameter of the function being checked. -/
inductive Mu where
  | mutable
  | immutable
  | param (i : Nat)
  deriving DecidableEq, Repr

/-- Types. `cell m t` is a shared object with mutability `m` holding a `t`,
and `fn m a r` a closure with mutability `m` from `a` to `r`. -/
inductive Ty where
  | unit
  | int
  | cell (m : Mu) (t : Ty)
  | fn (m : Mu) (a r : Ty)
  | prod (a b : Ty)
  deriving DecidableEq, Repr

/-- Expressions. `loc` only appears while running: it's a location in the
heap. `new true e` is `mut` and `new false e` is `imm`, like `mut [e]` and
`[e]`; a literal can't have a parameter as its mutability. A call gives the
mutability arguments of the function explicitly, as the type checker finds
them. -/
inductive Expr where
  | var (x : Nat)
  | unit
  | int (n : Int)
  | loc (l : Nat)
  | new (isMut : Bool) (e : Expr)
  | get (e : Expr)
  | set (e₁ e₂ : Expr)
  | letE (x : Nat) (e₁ e₂ : Expr)
  | call (f : Nat) (ms : List Mu) (e : Expr)
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
  | .call _ _ e => e.NoLocs
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
  | .call _ _ e, l => e.HasLoc l
  | .set e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .letE _ e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .lam _ _ body, l => body.HasLoc l
  | .app e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .pair e₁ e₂, l => e₁.HasLoc l ∨ e₂.HasLoc l
  | .fst e, l => e.HasLoc l
  | .snd e, l => e.HasLoc l

/-- Replace the mutability parameters with `ms`; one without an argument
becomes `imm`. -/
def Mu.subst (ms : List Mu) : Mu → Mu
  | .param i => ms[i]?.getD .immutable
  | m => m

def Ty.subst (ms : List Mu) : Ty → Ty
  | .unit => .unit
  | .int => .int
  | .cell m t => .cell (m.subst ms) (t.subst ms)
  | .fn m a r => .fn (m.subst ms) (a.subst ms) (r.subst ms)
  | .prod a b => .prod (a.subst ms) (b.subst ms)

def Expr.msubst (ms : List Mu) : Expr → Expr
  | .var x => .var x
  | .unit => .unit
  | .int n => .int n
  | .loc l => .loc l
  | .new b e => .new b (e.msubst ms)
  | .get e => .get (e.msubst ms)
  | .set e₁ e₂ => .set (e₁.msubst ms) (e₂.msubst ms)
  | .letE x e₁ e₂ => .letE x (e₁.msubst ms) (e₂.msubst ms)
  | .call f ms' e => .call f (ms'.map (Mu.subst ms)) (e.msubst ms)
  | .lam b x body => .lam b x (body.msubst ms)
  | .app e₁ e₂ => .app (e₁.msubst ms) (e₂.msubst ms)
  | .pair e₁ e₂ => .pair (e₁.msubst ms) (e₂.msubst ms)
  | .fst e => .fst (e.msubst ms)
  | .snd e => .snd (e.msubst ms)

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
  | .call f ms e, x, v => .call f ms (e.subst x v)
  | .lam b y body, x, v => .lam b y (if y = x then body else body.subst x v)
  | .app e₁ e₂, x, v => .app (e₁.subst x v) (e₂.subst x v)
  | .pair e₁ e₂, x, v => .pair (e₁.subst x v) (e₂.subst x v)
  | .fst e, x, v => .fst (e.subst x v)
  | .snd e, x, v => .snd (e.subst x v)

/-- `imm` is deep: the type of something that never changes, and nothing it
holds changes either. An `imm` closure only captures such things, so its
argument and result types don't matter. -/
def DeepImm : Ty → Prop
  | .unit | .int => True
  | .cell m t => m = .immutable ∧ DeepImm t
  | .fn m _ _ => m = .immutable
  | .prod a b => DeepImm a ∧ DeepImm b

instance DeepImm.dec : (t : Ty) → Decidable (DeepImm t)
  | .unit => .isTrue trivial
  | .int => .isTrue trivial
  | .cell m t =>
    match decEq m .immutable, DeepImm.dec t with
    | .isTrue hm, .isTrue ht => .isTrue ⟨hm, ht⟩
    | .isFalse hm, _ => .isFalse (fun h => hm h.1)
    | _, .isFalse ht => .isFalse (fun h => ht h.2)
  | .fn m _ _ => decEq m .immutable
  | .prod a b =>
    match DeepImm.dec a, DeepImm.dec b with
    | .isTrue ha, .isTrue hb => .isTrue ⟨ha, hb⟩
    | .isFalse ha, _ => .isFalse (fun h => ha h.1)
    | _, .isFalse hb => .isFalse (fun h => hb h.2)

/-- Mutabilities are ordered `imm ⊑ mut`. A parameter is only below itself
and `mut`, since it may be either. -/
def Mu.Le (m μ : Mu) : Prop := m = .immutable ∨ μ = .mutable ∨ m = μ

/-- Nothing in a value of type `t` is more `mut` than `μ`: what an object of
mutability `μ` can hold. A cell or a closure in it is at most `μ`, and what
a cell holds is at most its own mutability. At most `imm` is `imm` all the
way down (`atMost_immutable`); at most `mut` only asks that the objects
inside are well-formed. -/
def AtMost (μ : Mu) : Ty → Prop
  | .unit | .int => True
  | .cell m t => Mu.Le m μ ∧ AtMost m t
  | .fn m _ _ => Mu.Le m μ
  | .prod a b => AtMost μ a ∧ AtMost μ b

theorem Mu.le_immutable {m : Mu} (h : Mu.Le m .immutable) : m = .immutable := by
  rcases h with h | h | h
  · exact h
  · cases h
  · exact h

theorem atMost_immutable {t : Ty} : AtMost .immutable t ↔ DeepImm t := by
  induction t with
  | unit | int => exact ⟨fun _ => trivial, fun _ => trivial⟩
  | cell m t ih =>
    constructor
    · intro ⟨hm, ht⟩
      have hm := Mu.le_immutable hm
      subst hm
      exact ⟨rfl, ih.mp ht⟩
    · intro ⟨hm, ht⟩
      subst hm
      exact ⟨.inl rfl, ih.mpr ht⟩
  | fn m a r _ _ =>
    constructor
    · exact Mu.le_immutable
    · intro h; exact .inl h
  | prod a b iha ihb =>
    exact ⟨fun ⟨ha, hb⟩ => ⟨iha.mp ha, ihb.mp hb⟩, fun ⟨ha, hb⟩ => ⟨iha.mpr ha, ihb.mpr hb⟩⟩

theorem Mu.Le.subst {m μ : Mu} (ms : List Mu) (h : Mu.Le m μ) : Mu.Le (m.subst ms) (μ.subst ms) := by
  rcases h with h | h | h
  · subst h; exact .inl rfl
  · subst h; exact .inr (.inl rfl)
  · subst h; exact .inr (.inr rfl)

theorem AtMost.subst {μ : Mu} {t : Ty} (ms : List Mu) (h : AtMost μ t) : AtMost (μ.subst ms) (t.subst ms) := by
  induction t generalizing μ with
  | unit | int => trivial
  | cell m t ih => exact ⟨h.1.subst ms, ih h.2⟩
  | fn m a r _ _ => exact Mu.Le.subst ms h
  | prod a b iha ihb => exact ⟨iha h.1, ihb h.2⟩

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

theorem Ty.subst_subst (ms ms' : List Mu) (t : Ty) :
    (t.subst ms').subst ms = t.subst (ms'.map (Mu.subst ms)) := by
  induction t with
  | unit => rfl
  | int => rfl
  | cell m t ih => simp [Ty.subst, ih, Mu.subst_subst]
  | fn m a r iha ihr => simp [Ty.subst, iha, ihr, Mu.subst_subst]
  | prod a b iha ihb => simp [Ty.subst, iha, ihb]

theorem DeepImm.subst {t : Ty} (ms : List Mu) (h : DeepImm t) : DeepImm (t.subst ms) := by
  induction t with
  | unit => trivial
  | int => trivial
  | cell m t ih =>
    obtain ⟨rfl, ht⟩ := h
    exact ⟨rfl, ih ht⟩
  | fn m a r _ _ =>
    have hm : m = .immutable := h
    subst hm
    rfl
  | prod a b iha ihb => exact ⟨iha h.1, ihb h.2⟩

theorem Expr.NoLocs.msubst {e : Expr} (ms : List Mu) (h : e.NoLocs) : (e.msubst ms).NoLocs := by
  induction e with
  | var | unit | int => trivial
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | lam _ _ _ ih => exact ih h

/-- An expression without locations has none. -/
theorem Expr.NoLocs.not_hasLoc {e : Expr} (h : e.NoLocs) (l : Nat) : ¬ e.HasLoc l := by
  induction e with
  | var | unit | int => exact id
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih => exact ih h
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
  | new _ _ ih | get _ ih | call _ _ _ ih | fst _ ih | snd _ ih => exact ih h
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
