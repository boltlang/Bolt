import BoltMut.Typing

/-!
# Semantics

A small-step semantics over a heap of objects. Making an object adds a
location; `set` replaces what a location holds, whatever its mutability:
nothing at run time stops writing to an `imm` location, which is what the
type system has to rule out. Each object knows whether it's `mut`, as in
the evaluator, so that `toImm` copies only what's `mut`.
-/

namespace BoltMut

/-- An object: whether it's `mut`, and what it holds. -/
abbrev Obj := Bool × Expr

abbrev Heap := List Obj

/-- `toImm` of a value (#335), like `imm e`: each `mut` object it reaches is
copied into a new `imm` one, once, and what's `imm` is shared, as are
closures. `m` says which objects are copied already, and to where, so the
copy shares what the value shares. A copy only adds to the heap. -/
inductive Freeze : Heap → List (Nat × Nat) → Expr → Heap → List (Nat × Nat) → Expr → Prop where
  | unit {h : Heap} {m : List (Nat × Nat)} : Freeze h m .unit h m .unit
  | int {h : Heap} {m : List (Nat × Nat)} {n : Int} : Freeze h m (.int n) h m (.int n)
  | lam {h : Heap} {m : List (Nat × Nat)} {b : Bool} {x : Nat} {e : Expr} :
      Freeze h m (.lam b x e) h m (.lam b x e)
  | imm {h : Heap} {m : List (Nat × Nat)} {l : Nat} {w : Expr} :
      h[l]? = some (false, w) → Freeze h m (.loc l) h m (.loc l)
  | seen {h : Heap} {m : List (Nat × Nat)} {l c : Nat} {w : Expr} :
      h[l]? = some (true, w) → m.lookup l = some c → Freeze h m (.loc l) h m (.loc c)
  | cell {h h' : Heap} {m m' : List (Nat × Nat)} {l : Nat} {w w' : Expr} :
      h[l]? = some (true, w) → m.lookup l = none → Freeze h m w h' m' w' →
      Freeze h m (.loc l) (h' ++ [(false, w')]) ((l, h'.length) :: m') (.loc h'.length)
  | pair {h h₁ h₂ : Heap} {m m₁ m₂ : List (Nat × Nat)} {v₁ v₂ v₁' v₂' : Expr} :
      Freeze h m v₁ h₁ m₁ v₁' → Freeze h₁ m₁ v₂ h₂ m₂ v₂' →
      Freeze h m (.pair v₁ v₂) h₂ m₂ (.pair v₁' v₂')

/-- The heap with each object a copy copied made `imm`, where it is: what
freezing in place does to them. `m` is what the copy copied. -/
def Heap.freezeAt (h : Heap) (m : List (Nat × Nat)) : Heap :=
  h.mapIdx fun i o => if (m.lookup i).isSome then (false, o.2) else o

theorem Heap.freezeAt_get (h : Heap) (m : List (Nat × Nat)) (i : Nat) :
    (h.freezeAt m)[i]? = h[i]?.map fun o => if (m.lookup i).isSome then (false, o.2) else o := by
  simp [Heap.freezeAt, List.getElem?_mapIdx]

inductive Step (fns : List Fn) : Heap → Expr → Heap → Expr → Prop where
  | newCong {h h' : Heap} {e e' : Expr} {m : Mu} :
      Step fns h e h' e' → Step fns h (.new m e) h' (.new m e')
  /-- A `ro` literal is a new `mut` object (`Mu.allocMut`). -/
  | newVal {h : Heap} {e : Expr} {m : Mu} :
      e.IsValue → Step fns h (.new m e) (h ++ [(m.allocMut, e)]) (.loc h.length)
  | getCong {h h' : Heap} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.get e) h' (.get e')
  | getLoc {h : Heap} {l : Nat} {b : Bool} {v : Expr} :
      h[l]? = some (b, v) → Step fns h (.get (.loc l)) h v
  | setCong₁ {h h' : Heap} {e₁ e₁' e₂ : Expr} :
      Step fns h e₁ h' e₁' → Step fns h (.set e₁ e₂) h' (.set e₁' e₂)
  | setCong₂ {h h' : Heap} {e₁ e₂ e₂' : Expr} :
      e₁.IsValue → Step fns h e₂ h' e₂' → Step fns h (.set e₁ e₂) h' (.set e₁ e₂')
  | setLoc {h : Heap} {l : Nat} {b : Bool} {w v : Expr} :
      h[l]? = some (b, w) → v.IsValue → Step fns h (.set (.loc l) v) (h.set l (b, v)) .unit
  | letCong {h h' : Heap} {x : Nat} {e₁ e₁' e₂ : Expr} :
      Step fns h e₁ h' e₁' → Step fns h (.letE x e₁ e₂) h' (.letE x e₁' e₂)
  | letVal {h : Heap} {x : Nat} {v e₂ : Expr} :
      v.IsValue → Step fns h (.letE x v e₂) h (e₂.subst x v)
  | callCong {h h' : Heap} {f : Nat} {ms : List Mu} {τs : List Ty} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.call f ms τs e) h' (.call f ms τs e')
  | callVal {h : Heap} {f : Nat} {fn : Fn} {ms : List Mu} {τs : List Ty} {v : Expr} :
      fns[f]? = some fn → v.IsValue →
      Step fns h (.call f ms τs v) h ((fn.body.inst ms τs).subst fn.param v)
  | appCong₁ {h h' : Heap} {e₁ e₁' e₂ : Expr} :
      Step fns h e₁ h' e₁' → Step fns h (.app e₁ e₂) h' (.app e₁' e₂)
  | appCong₂ {h h' : Heap} {e₁ e₂ e₂' : Expr} :
      e₁.IsValue → Step fns h e₂ h' e₂' → Step fns h (.app e₁ e₂) h' (.app e₁ e₂')
  | appVal {h : Heap} {b : Bool} {x : Nat} {body v : Expr} :
      v.IsValue → Step fns h (.app (.lam b x body) v) h (body.subst x v)
  | pairCong₁ {h h' : Heap} {e₁ e₁' e₂ : Expr} :
      Step fns h e₁ h' e₁' → Step fns h (.pair e₁ e₂) h' (.pair e₁' e₂)
  | pairCong₂ {h h' : Heap} {e₁ e₂ e₂' : Expr} :
      e₁.IsValue → Step fns h e₂ h' e₂' → Step fns h (.pair e₁ e₂) h' (.pair e₁ e₂')
  | fstCong {h h' : Heap} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.fst e) h' (.fst e')
  | fstVal {h : Heap} {v₁ v₂ : Expr} :
      v₁.IsValue → v₂.IsValue → Step fns h (.fst (.pair v₁ v₂)) h v₁
  | sndCong {h h' : Heap} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.snd e) h' (.snd e')
  | sndVal {h : Heap} {v₁ v₂ : Expr} :
      v₁.IsValue → v₂.IsValue → Step fns h (.snd (.pair v₁ v₂)) h v₂
  | toImmCong {h h' : Heap} {b : Bool} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.toImm b e) h' (.toImm b e')
  /-- `toImm` copies what's `mut` in the value. -/
  | toImmVal {h h' : Heap} {m : List (Nat × Nat)} {v v' : Expr} :
      v.IsValue → Freeze h [] v h' m v' → Step fns h (.toImm false v) h' v'
  /-- In place, it makes the same copy, but goes on with the value, and
  what it copied becomes `imm` instead. -/
  | toImmKeep {h h' : Heap} {m : List (Nat × Nat)} {v v' : Expr} :
      v.IsValue → Freeze h [] v h' m v' → Step fns h (.toImm true v) (h'.freezeAt m) v

/-- Any number of steps. -/
inductive Steps (fns : List Fn) : Heap → Expr → Heap → Expr → Prop where
  | refl : Steps fns h e h e
  | step : Step fns h e h' e' → Steps fns h' e' h'' e'' → Steps fns h e h'' e''

/-- A value takes no step. -/
theorem Expr.IsValue.no_step {fns : List Fn} {v : Expr} (hv : v.IsValue) {h h' : Heap} {e' : Expr}
    (hs : Step fns h v h' e') : False := by
  induction hv generalizing h' e' with
  | unit | int | loc | lam => cases hs
  | pair _ _ ih₁ ih₂ =>
    cases hs with
    | pairCong₁ s => exact ih₁ s
    | pairCong₂ _ s => exact ih₂ s


end BoltMut
