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

/-- The copy that a freeze of the region `r` makes of a value of type `t`
(#292): an object whose type mentions `r` is copied, into a new one with
what it holds copied, and the rest is shared. An object of `r` itself only
holds what's `imm` or of `r`, so everything copied is `imm` once `r` is.
`m` says which objects are copied already, and to where: an object reached
twice is copied once, so the copy shares what the value shares. A copy
only adds to the heap. -/
inductive Copy (r : Nat) : Ty → Heap → List (Nat × Nat) → Expr → Heap → List (Nat × Nat) → Expr → Prop where
  | keep {t : Ty} {h : Heap} {m : List (Nat × Nat)} {v : Expr} : t.mentions r = false → Copy r t h m v h m v
  | seen {μ : Mu} {s : Ty} {h : Heap} {m : List (Nat × Nat)} {l c : Nat} :
      (Ty.cell μ s).mentions r = true → m.lookup l = some c → Copy r (.cell μ s) h m (.loc l) h m (.loc c)
  | cell {μ : Mu} {s : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {l : Nat} {w w' : Expr} :
      (Ty.cell μ s).mentions r = true → m.lookup l = none → h[l]? = some w → Copy r s h m w h' m' w' →
      Copy r (.cell μ s) h m (.loc l) (h' ++ [w']) ((l, h'.length) :: m') (.loc h'.length)
  | pair {a b : Ty} {h h₁ h₂ : Heap} {m m₁ m₂ : List (Nat × Nat)} {v₁ v₂ v₁' v₂' : Expr} :
      (Ty.prod a b).mentions r = true → Copy r a h m v₁ h₁ m₁ v₁' → Copy r b h₁ m₁ v₂ h₂ m₂ v₂' →
      Copy r (.prod a b) h m (.pair v₁ v₂) h₂ m₂ (.pair v₁' v₂')

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
  | freezeCong {h h' : Heap} {r : Nat} {t : Ty} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.freeze r t e) h' (.freeze r t e')
  | freezeVal {h h' : Heap} {r : Nat} {t : Ty} {m : List (Nat × Nat)} {v v' : Expr} :
      v.IsValue → Copy r t h [] v h' m v' → Step fns h (.freeze r t v) h' v'
  | keepCong {h h' : Heap} {r : Nat} {t : Ty} {e e' : Expr} :
      Step fns h e h' e' → Step fns h (.keep r t e) h' (.keep r t e')
  | keepVal {h h' : Heap} {r : Nat} {t : Ty} {m : List (Nat × Nat)} {v v' : Expr} :
      v.IsValue → Copy r t h [] v h' m v' → Step fns h (.keep r t v) h' v

/-- Any number of steps. -/
inductive Steps (fns : List Fn) : Heap → Expr → Heap → Expr → Prop where
  | refl : Steps fns h e h e
  | step : Step fns h e h' e' → Steps fns h' e' h'' e'' → Steps fns h e h'' e''

end BoltMut
