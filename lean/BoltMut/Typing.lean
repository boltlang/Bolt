import BoltMut.Syntax

/-!
# Typing

The rules that matter for mutability:

* `new false e`, an `imm` object, can only hold a type that's `imm` all the
  way down (`DeepImm`): what an `imm` value holds can't be changed either.
* `set e₁ e₂` needs `e₁` to be exactly `cell mutable t`. A mutability
  parameter isn't enough: a generic function can read what it's given, but
  can't change it, since it may be called with `imm`.
* Aliases share the mutability: a location has one type, `σ[l]`.

`σ`, the heap typing, gives each location its mutability and the type it
holds. Functions are top-level and generic over their mutability
parameters.
-/

namespace BoltMut

/-- A top-level function: generic over `arity` mutability parameters, with
one parameter, the variable `param`. -/
structure Fn where
  arity : Nat
  param : Nat
  argTy : Ty
  retTy : Ty
  body : Expr

/-- The types of the variables in scope. -/
abbrev Ctx := Nat → Option Ty

def Ctx.empty : Ctx := fun _ => none

def Ctx.update (Γ : Ctx) (x : Nat) (t : Ty) : Ctx := fun y => if y = x then some t else Γ y

/-- The mutability and the type held of each location. -/
abbrev HeapTy := List (Mu × Ty)

inductive HasType (fns : List Fn) (σ : HeapTy) : Ctx → Expr → Ty → Prop where
  | var {Γ : Ctx} {x : Nat} {t : Ty} : Γ x = some t → HasType fns σ Γ (.var x) t
  | unit {Γ : Ctx} : HasType fns σ Γ .unit .unit
  | int {Γ : Ctx} {n : Int} : HasType fns σ Γ (.int n) .int
  | loc {Γ : Ctx} {l : Nat} {m : Mu} {t : Ty} : σ[l]? = some (m, t) → HasType fns σ Γ (.loc l) (.cell m t)
  | new {Γ : Ctx} {e : Expr} {t : Ty} {b : Bool} :
      HasType fns σ Γ e t → (b = false → DeepImm t) →
      HasType fns σ Γ (.new b e) (.cell (muOf b) t)
  | get {Γ : Ctx} {e : Expr} {m : Mu} {t : Ty} :
      HasType fns σ Γ e (.cell m t) → HasType fns σ Γ (.get e) t
  | set {Γ : Ctx} {e₁ e₂ : Expr} {t : Ty} :
      HasType fns σ Γ e₁ (.cell .mutable t) → HasType fns σ Γ e₂ t →
      HasType fns σ Γ (.set e₁ e₂) .unit
  | letE {Γ : Ctx} {x : Nat} {e₁ e₂ : Expr} {t₁ t₂ : Ty} :
      HasType fns σ Γ e₁ t₁ → HasType fns σ (Γ.update x t₁) e₂ t₂ →
      HasType fns σ Γ (.letE x e₁ e₂) t₂
  | call {Γ : Ctx} {f : Nat} {fn : Fn} {ms : List Mu} {e : Expr} :
      fns[f]? = some fn → ms.length = fn.arity →
      HasType fns σ Γ e (fn.argTy.subst ms) →
      HasType fns σ Γ (.call f ms e) (fn.retTy.subst ms)

/-- Each function's body has its declared type, with its mutability
parameters unknown, and no locations. -/
def FnsOk (fns : List Fn) : Prop :=
  ∀ fn ∈ fns, fn.body.NoLocs ∧ HasType fns [] (Ctx.empty.update fn.param fn.argTy) fn.body fn.retTy

end BoltMut
