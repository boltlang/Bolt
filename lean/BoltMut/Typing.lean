import BoltMut.Syntax

/-!
# Typing

The rules that matter for mutability:

* An object holds only what's at most its mutability (`AtMost`). So
  `new false e`, an `imm` object, can only hold a type that's `imm` all the
  way down (`DeepImm`): what an `imm` value holds can't be changed either.
* `set e₁ e₂` needs `e₁` to be exactly `cell mutable t`. A mutability
  parameter isn't enough: a generic function can read what it's given, but
  can't change it, since it may be called with `imm`.
* Aliases share the mutability: a location has one type, `σ[l]`.
* An `imm` closure, `lam false x body`, only captures what's `imm` all the
  way down: its body sees only the variables of such types, and only `imm`
  locations. Its type is `fn imm a r`, and `DeepImm` of that holds, so an
  `imm` object can hold it. A `mut` closure captures anything, and an `imm`
  object can't hold it. Either can be called.

The flag `io` of `HasType` says the expression is in the body of an `imm`
closure, where a location must be `imm`.

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

/-- The variables a closure sees: all of them for a `mut` closure, and those
whose types are `imm` all the way down for an `imm` one. -/
def Ctx.capture (isMut : Bool) (Γ : Ctx) : Ctx :=
  if isMut then Γ else fun y => (Γ y).filter (fun t => decide (DeepImm t))

/-- The mutability and the type held of each location. -/
abbrev HeapTy := List (Mu × Ty)

inductive HasType (fns : List Fn) (σ : HeapTy) : Bool → Ctx → Expr → Ty → Prop where
  | var {io : Bool} {Γ : Ctx} {x : Nat} {t : Ty} : Γ x = some t → HasType fns σ io Γ (.var x) t
  | unit {io : Bool} {Γ : Ctx} : HasType fns σ io Γ .unit .unit
  | int {io : Bool} {Γ : Ctx} {n : Int} : HasType fns σ io Γ (.int n) .int
  | loc {io : Bool} {Γ : Ctx} {l : Nat} {m : Mu} {t : Ty} :
      σ[l]? = some (m, t) → (io = true → m = .immutable) →
      HasType fns σ io Γ (.loc l) (.cell m t)
  | new {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty} {b : Bool} :
      HasType fns σ io Γ e t → AtMost (muOf b) t →
      HasType fns σ io Γ (.new b e) (.cell (muOf b) t)
  | get {io : Bool} {Γ : Ctx} {e : Expr} {m : Mu} {t : Ty} :
      HasType fns σ io Γ e (.cell m t) → HasType fns σ io Γ (.get e) t
  | set {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {t : Ty} :
      HasType fns σ io Γ e₁ (.cell .mutable t) → HasType fns σ io Γ e₂ t →
      HasType fns σ io Γ (.set e₁ e₂) .unit
  | letE {io : Bool} {Γ : Ctx} {x : Nat} {e₁ e₂ : Expr} {t₁ t₂ : Ty} :
      HasType fns σ io Γ e₁ t₁ → HasType fns σ io (Γ.update x t₁) e₂ t₂ →
      HasType fns σ io Γ (.letE x e₁ e₂) t₂
  | call {io : Bool} {Γ : Ctx} {f : Nat} {fn : Fn} {ms : List Mu} {e : Expr} :
      fns[f]? = some fn → ms.length = fn.arity →
      HasType fns σ io Γ e (fn.argTy.subst ms) →
      HasType fns σ io Γ (.call f ms e) (fn.retTy.subst ms)
  | lam {io : Bool} {Γ : Ctx} {b : Bool} {x : Nat} {body : Expr} {a r : Ty} :
      HasType fns σ (io || !b) ((Γ.capture b).update x a) body r →
      HasType fns σ io Γ (.lam b x body) (.fn (muOf b) a r)
  | app {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {m : Mu} {a r : Ty} :
      HasType fns σ io Γ e₁ (.fn m a r) → HasType fns σ io Γ e₂ a →
      HasType fns σ io Γ (.app e₁ e₂) r
  | pair {io : Bool} {Γ : Ctx} {e₁ e₂ : Expr} {a b : Ty} :
      HasType fns σ io Γ e₁ a → HasType fns σ io Γ e₂ b →
      HasType fns σ io Γ (.pair e₁ e₂) (.prod a b)
  | fst {io : Bool} {Γ : Ctx} {e : Expr} {a b : Ty} :
      HasType fns σ io Γ e (.prod a b) → HasType fns σ io Γ (.fst e) a
  | snd {io : Bool} {Γ : Ctx} {e : Expr} {a b : Ty} :
      HasType fns σ io Γ e (.prod a b) → HasType fns σ io Γ (.snd e) b

/-- Each function's body has its declared type, with its mutability
parameters unknown, and no locations. -/
def FnsOk (fns : List Fn) : Prop :=
  ∀ fn ∈ fns, fn.body.NoLocs ∧ HasType fns [] false (Ctx.empty.update fn.param fn.argTy) fn.body fn.retTy

end BoltMut
