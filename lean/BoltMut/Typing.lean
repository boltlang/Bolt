import BoltMut.Syntax

/-!
# Typing

The rules that matter for mutability:

* An object holds only what's at most its mutability (`AtMost`). So
  `new false e`, an `imm` object, can only hold a type that's `imm` all the
  way down: what an `imm` value holds can't be changed either.
* `set e₁ e₂` needs `e₁` to be exactly `cell mutable t`, or a cell of a
  region. A mutability parameter isn't enough: a generic function can read
  what it's given, but can't change it, since it may be called with `imm`.
* `new true e` can make a cell of a region `r` (#209) instead of a `mut`
  one. What it holds is at most `r`, so it's `imm` all the way down once
  the region is freed (`Regions.lean`). A region that holds `mut` objects
  (#286) becomes `mut` instead.
* Aliases share the mutability: a location has one type, `σ[l]`.
* An `imm` closure, `lam false x body`, only captures what's `imm` all the
  way down: its body sees only the variables of such types, and only `imm`
  locations. Its type is `fn imm a r`, so an `imm` object can hold it. A
  `mut` closure captures anything, and an `imm` object can't hold it.
  Either can be called.
* A `mut` closure can have a region as its mutability (#287), and then
  capture what's of that region. Once the region is freed, its type is
  `gone`, which can't be called. Other closures capture nothing whose
  type mentions a region, and the arguments of a call mention none, for
  now.
* A generic function is typed under what it assumes about its parameters
  (`Fn.assume`, #243), like `T ⊑ imm`, and a call must meet that with its
  arguments (`Sat`). Without an assumption, a value of type `T` can't be put
  in an `imm` object, since `T` may be `mut` (#247).

The flag `io` of `HasType` says the expression is in the body of an `imm`
closure, where a location must be `imm`. `Δ` is what the function being
checked assumes; the running program assumes nothing.

`σ`, the heap typing, gives each location its mutability and the type it
holds. Functions are top-level and generic over their mutability and type
parameters.
-/

namespace BoltMut

/-- A top-level function: generic over mutability and type parameters, with
what it assumes about them, and one parameter, the variable `param`. -/
structure Fn where
  assume : Assume
  param : Nat
  argTy : Ty
  retTy : Ty
  body : Expr

/-- The types of the variables in scope. -/
abbrev Ctx := Nat → Option Ty

def Ctx.empty : Ctx := fun _ => none

def Ctx.update (Γ : Ctx) (x : Nat) (t : Ty) : Ctx := fun y => if y = x then some t else Γ y

/-- `Γ'` has every variable of `Γ`, with the same type. -/
def Ctx.Sub (Γ Γ' : Ctx) : Prop := ∀ (x : Nat) (s : Ty), Γ x = some s → Γ' x = some s

/-- The mutability and the type held of each location. -/
abbrev HeapTy := List (Mu × Ty)

inductive HasType (fns : List Fn) (σ : HeapTy) (Δ : Assume) : Bool → Ctx → Expr → Ty → Prop where
  | var {io : Bool} {Γ : Ctx} {x : Nat} {t : Ty} : Γ x = some t → HasType fns σ Δ io Γ (.var x) t
  | unit {io : Bool} {Γ : Ctx} : HasType fns σ Δ io Γ .unit .unit
  | int {io : Bool} {Γ : Ctx} {n : Int} : HasType fns σ Δ io Γ (.int n) .int
  | loc {io : Bool} {Γ : Ctx} {l : Nat} {m : Mu} {t : Ty} :
      σ[l]? = some (m, t) → (io = true → m = .immutable) →
      HasType fns σ Δ io Γ (.loc l) (.cell m t)
  | new {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty} {b : Bool} :
      HasType fns σ Δ io Γ e t → AtMost Δ (muOf b) t →
      HasType fns σ Δ io Γ (.new b e) (.cell (muOf b) t)
  | get {io : Bool} {Γ : Ctx} {e : Expr} {m : Mu} {t : Ty} :
      HasType fns σ Δ io Γ e (.cell m t) → HasType fns σ Δ io Γ (.get e) t
  | set {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {t : Ty} :
      HasType fns σ Δ io Γ e₁ (.cell .mutable t) → HasType fns σ Δ io Γ e₂ t →
      HasType fns σ Δ io Γ (.set e₁ e₂) .unit
  /-- A `mut` object can be made in the region `r` instead: what it holds
  is at most the region. So it's `imm` once the region is freed, or `mut`
  if the region holds `mut` objects. -/
  | newR {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty} {r : Nat} {b : Bool} :
      HasType fns σ Δ io Γ e t → AtMost Δ (.region r b) t →
      HasType fns σ Δ io Γ (.new true e) (.cell (.region r b) t)
  /-- A region's objects can be changed, like `mut` ones. -/
  | setR {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {t : Ty} {r : Nat} {b : Bool} :
      HasType fns σ Δ io Γ e₁ (.cell (.region r b) t) → HasType fns σ Δ io Γ e₂ t →
      HasType fns σ Δ io Γ (.set e₁ e₂) .unit
  | letE {io : Bool} {Γ : Ctx} {x : Nat} {e₁ e₂ : Expr} {t₁ t₂ : Ty} :
      HasType fns σ Δ io Γ e₁ t₁ → HasType fns σ Δ io (Γ.update x t₁) e₂ t₂ →
      HasType fns σ Δ io Γ (.letE x e₁ e₂) t₂
  | call {io : Bool} {Γ : Ctx} {f : Nat} {fn : Fn} {ms : List Mu} {τs : List Ty} {e : Expr} :
      fns[f]? = some fn → Sat Δ fn.assume ms τs → ArgsNoRegion ms τs →
      HasType fns σ Δ io Γ e (fn.argTy.inst ms τs) →
      HasType fns σ Δ io Γ (.call f ms τs e) (fn.retTy.inst ms τs)
  /-- A closure sees the variables `Γc` of those in scope. Its mutability
  `μ` is `muOf b`, or, for a `mut` one, a region (rule 5 of the regions
  page, #287). Unless it's `mut`, what it captures is at most `μ`: for an
  `imm` one, `imm` all the way down. The only region what it captures
  mentions is `μ`'s, so a closure of no region captures nothing of one, and
  freeing a region that isn't the closure's leaves it as it is. -/
  | lam {io : Bool} {Γ Γc : Ctx} {b : Bool} {x : Nat} {body : Expr} {a r : Ty} {μ : Mu} :
      (μ = muOf b ∨ b = true ∧ ∃ r' hm, μ = .region r' hm) →
      Γc.Sub Γ → (μ ≠ .mutable → ∀ y s, Γc y = some s → AtMost Δ μ s) →
      (∀ y s, Γc y = some s → s.RegionsIn μ) →
      (∀ l, body.HasLoc l → ∃ m t, σ[l]? = some (m, t) ∧ Le Δ m μ ∧ m.RegionsIn μ ∧ t.RegionsIn μ) →
      HasType fns σ Δ (io || !b) (Γc.update x a) body r →
      HasType fns σ Δ io Γ (.lam b x body) (.fn μ a r)
  /-- A closure that captured a region that's been freed: its body isn't
  typed any more, so it can't be called. It has no free variables, and what it captured is still at most its mutability, and
  `imm` inside an `imm` closure. -/
  | gone {io : Bool} {Γ : Ctx} {b : Bool} {x : Nat} {body : Expr} {μ : Mu} :
      (∀ y, ¬ (Expr.lam b x body).Free y) →
      (∀ l, body.HasLoc l → ∃ m t, σ[l]? = some (m, t) ∧ Le Δ m μ ∧ m.RegionsIn μ ∧ t.RegionsIn μ ∧
        (io = true → m = .immutable)) →
      HasType fns σ Δ io Γ (.lam b x body) (.gone μ)
  | app {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {m : Mu} {a r : Ty} :
      HasType fns σ Δ io Γ e₁ (.fn m a r) → HasType fns σ Δ io Γ e₂ a →
      HasType fns σ Δ io Γ (.app e₁ e₂) r
  | pair {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {a b : Ty} :
      HasType fns σ Δ io Γ e₁ a → HasType fns σ Δ io Γ e₂ b →
      HasType fns σ Δ io Γ (.pair e₁ e₂) (.prod a b)
  | fst {io : Bool} {Γ : Ctx} {e : Expr} {a b : Ty} :
      HasType fns σ Δ io Γ e (.prod a b) → HasType fns σ Δ io Γ (.fst e) a
  | snd {io : Bool} {Γ : Ctx} {e : Expr} {a b : Ty} :
      HasType fns σ Δ io Γ e (.prod a b) → HasType fns σ Δ io Γ (.snd e) b

/-- Each function's body has its declared type, under what the function
assumes, with its parameters unknown, and no locations. -/
def FnsOk (fns : List Fn) : Prop :=
  ∀ fn ∈ fns, fn.body.NoLocs ∧ HasType fns [] fn.assume false (Ctx.empty.update fn.param fn.argTy) fn.body fn.retTy

end BoltMut
