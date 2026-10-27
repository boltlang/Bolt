import BoltMut.Typing

/-!
# Semantics

A small-step semantics over a heap of values. Making an object adds a
location; `set` replaces what a location holds, whatever its mutability:
nothing at run time stops writing to an `imm` location, which is what the
type system has to rule out.
-/

namespace BoltMut

abbrev Heap := List Expr

inductive Step (fns : List Fn) : Heap → Expr → Heap → Expr → Prop where
  | newCong {h h' : Heap} {e e' : Expr} {b : Bool} :
      Step fns h e h' e' → Step fns h (.new b e) h' (.new b e')
  | newVal {h : Heap} {e : Expr} {b : Bool} :
      e.IsValue → Step fns h (.new b e) (h ++ [e]) (.loc h.length)
  | getCong {h h' : Heap} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.get e) h' (.get e')
  | getLoc {h : Heap} {l : Nat} {v : Expr} :
      h[l]? = some v → Step fns h (.get (.loc l)) h v
  | setCong₁ {h h' : Heap} {e₁ e₁' e₂ : Expr} :
      Step fns h e₁ h' e₁' → Step fns h (.set e₁ e₂) h' (.set e₁' e₂)
  | setCong₂ {h h' : Heap} {e₁ e₂ e₂' : Expr} :
      e₁.IsValue → Step fns h e₂ h' e₂' → Step fns h (.set e₁ e₂) h' (.set e₁ e₂')
  | setLoc {h : Heap} {l : Nat} {v : Expr} :
      l < h.length → v.IsValue → Step fns h (.set (.loc l) v) (h.set l v) .unit
  | letCong {h h' : Heap} {x : Nat} {e₁ e₁' e₂ : Expr} :
      Step fns h e₁ h' e₁' → Step fns h (.letE x e₁ e₂) h' (.letE x e₁' e₂)
  | letVal {h : Heap} {x : Nat} {v e₂ : Expr} :
      v.IsValue → Step fns h (.letE x v e₂) h (e₂.subst x v)
  | callCong {h h' : Heap} {f : Nat} {ms : List Mu} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.call f ms e) h' (.call f ms e')
  | callVal {h : Heap} {f : Nat} {fn : Fn} {ms : List Mu} {v : Expr} :
      fns[f]? = some fn → v.IsValue →
      Step fns h (.call f ms v) h ((fn.body.msubst ms).subst fn.param v)

/-- Any number of steps. -/
inductive Steps (fns : List Fn) : Heap → Expr → Heap → Expr → Prop where
  | refl : Steps fns h e h e
  | step : Step fns h e h' e' → Steps fns h' e' h'' e'' → Steps fns h e h'' e''

end BoltMut
