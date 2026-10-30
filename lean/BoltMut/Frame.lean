import BoltMut.Purity

/-!
# What a call can change (#255)

`Purity.lean` proves that a call of an `imm` function with `imm` arguments
changes nothing that existed before, and with a handler, only what the
handler can reach. This generalizes it: **a program changes only what's
reachable from the locations in it that aren't `imm`**, its *frame*
(`changes_only_frame`). For a call, that's what's reachable from the
function and its argument (`call_changes_only_what_args_reach`), and what's
`imm` all the way down, like an `imm` closure or an `imm` array, adds
nothing (`Frame.of_deepImm`). So for a call like

```bolt
fill(xs, ys)    // fn fill(xs: mut [i32], ys: [i32]) with `ys` an `imm` array
```

only what `xs` reaches can change (`imm_fn_changes_only_what_it_is_given`).
That's what the reactive design (#200) needs: which calls change what,
from the types alone.

It's `steps_clean` again, with the frame as what's allowed: every location
in the program is `imm` or in the frame, and what a location in the frame
holds is too, since what it reaches, the frame does.
-/

namespace BoltMut

/-- The locations a program can change: those reachable, in the heap, from
a location in it that isn't `imm`. -/
def Frame (σ : HeapTy) (h : Heap) (e : Expr) (l : Nat) : Prop :=
  ∃ l₀, e.HasLoc l₀ ∧ (¬ ∃ t, σ[l₀]? = some (.immutable, t)) ∧ Reaches h l₀ l

/-- However many steps a program takes, it changes nothing that existed
before outside its frame. -/
theorem changes_only_frame {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {e e' : Expr} {t : Ty}
    (hh : HeapOk fns σ h) (ht : HasType fns σ .none false Ctx.empty e t) (hcl : HeapClosed h) (hc : e.Closed)
    (hs : Steps fns h e h' e') :
    ∀ l, l < h.length → ¬ Frame σ h e l → h'[l]? = h[l]? := by
  intro l hl hA
  refine steps_clean hf hs (A := Frame σ h e) hh ht hcl hc (Nat.le_refl _) ?_ ?_ l hl hA
  · intro l₀ h₀
    by_cases himm : ∃ t, σ[l₀]? = some (.immutable, t)
    · exact .inr (.inr himm)
    · exact .inr (.inl ⟨l₀, h₀, himm, .refl⟩)
  · intro l' w hA' hw
    rcases hA' with hA' | ⟨l₀, h₀, himm, hr⟩
    · have := (List.getElem?_eq_some_iff.mp hw).1
      omega
    · exact fun l₁ hl₁ => .inr (.inl ⟨l₀, h₀, himm, hr.tail hw hl₁⟩)

/-- What a program changes that existed before is in its frame, and isn't
`imm`: the two theorems together, as the reactive design would use them. -/
theorem changed_is_in_frame_and_mut {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {e e' : Expr}
    {t : Ty} (hh : HeapOk fns σ h) (ht : HasType fns σ .none false Ctx.empty e t) (hcl : HeapClosed h)
    (hec : e.Closed) (hs : Steps fns h e h' e') {l : Nat} (hl : l < h.length) (hc : h'[l]? ≠ h[l]?) :
    Frame σ h e l ∧ ¬ ∃ ty, σ[l]? = some (.immutable, ty) := by
  refine ⟨Classical.byContradiction fun hn => hc (changes_only_frame hf hh ht hcl hec hs l hl hn), ?_⟩
  rintro ⟨ty, hty⟩
  exact hc (imm_never_changes hf hs hh ht hcl hec hty)

/-- A call changes only what's reachable from the function or its argument,
through a location that isn't `imm`. -/
theorem call_changes_only_what_args_reach {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap}
    {f v e' : Expr} {m : Mu} {a r : Ty} (hh : HeapOk fns σ h)
    (hft : HasType fns σ .none false Ctx.empty f (.fn m a r)) (hvt : HasType fns σ .none false Ctx.empty v a)
    (hcl : HeapClosed h) (hfc : f.Closed) (hvc : v.Closed) (hs : Steps fns h (.app f v) h' e') :
    ∀ l, l < h.length → ¬ Frame σ h f l → ¬ Frame σ h v l → h'[l]? = h[l]? := by
  intro l hl hnf hnv
  refine changes_only_frame hf hh (.app hft hvt) hcl ⟨hfc, hvc⟩ hs l hl ?_
  rintro ⟨l₀, h₀ | h₀, himm, hr⟩
  · exact hnf ⟨l₀, h₀, himm, hr⟩
  · exact hnv ⟨l₀, h₀, himm, hr⟩

/-- A value that's `imm` all the way down has no frame: every location in it
is `imm`. So an `imm` argument adds nothing to what a call can change. -/
theorem Frame.of_deepImm {fns : List Fn} {σ : HeapTy} {h : Heap} {io : Bool} {Γ : Ctx} {v : Expr} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ .none io Γ v t) (hd : AtMost .none .immutable t) (l : Nat) :
    ¬ Frame σ h v l :=
  fun ⟨_, h₀, himm, _⟩ => himm ((ht.value_imm hv hd).hasLoc_imm rfl h₀)

/-- Nor has an `imm` closure: it captured only what's `imm` all the way
down. -/
theorem Frame.of_imm_fn {fns : List Fn} {σ : HeapTy} {h : Heap} {io : Bool} {Γ : Ctx} {f : Expr} {a r : Ty}
    (hv : f.IsValue) (ht : HasType fns σ .none io Γ f (.fn .immutable a r)) (l : Nat) : ¬ Frame σ h f l :=
  Frame.of_deepImm hv ht (.refl _) l

/-- `fill(xs, ys)`: calling an `imm` function with what can change, `w`, and
what's `imm` all the way down, `u`, changes only what `w` reaches. -/
theorem imm_fn_changes_only_what_it_is_given {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap}
    {f w u e' : Expr} {a b r : Ty} (hh : HeapOk fns σ h)
    (hfv : f.IsValue) (hft : HasType fns σ .none false Ctx.empty f (.fn .immutable (.prod a b) r))
    (hwt : HasType fns σ .none false Ctx.empty w a)
    (huv : u.IsValue) (hut : HasType fns σ .none false Ctx.empty u b) (hd : AtMost .none .immutable b)
    (hcl : HeapClosed h) (hfc : f.Closed) (hwc : w.Closed) (huc : u.Closed)
    (hs : Steps fns h (.app f (.pair w u)) h' e') :
    ∀ l, l < h.length → ¬ Frame σ h w l → h'[l]? = h[l]? := by
  intro l hl hnw
  refine call_changes_only_what_args_reach hf hh hft (.pair hwt hut) hcl hfc ⟨hwc, huc⟩ hs l hl (Frame.of_imm_fn hfv hft l) ?_
  rintro ⟨l₀, h₀ | h₀, himm, hr⟩
  · exact hnw ⟨l₀, h₀, himm, hr⟩
  · exact Frame.of_deepImm huv hut hd l ⟨l₀, h₀, himm, hr⟩

end BoltMut
