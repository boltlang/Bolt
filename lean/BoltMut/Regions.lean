import BoltMut.Immutability

/-!
# Freeing a region (#209)

A *region* is the mutability of the values a block makes and that nothing
outside the block has (`site/docs/internals/type-checking/mutability/regions.mdx`).
When the block ends, its values can become `imm` without a copy, since no
other name is left that could change them.

At run time, a location's mutability is only in the heap typing `σ`: the
heap holds values, and nothing stops a write. So making a location `imm`
without a copy isn't a step of the program: it's giving the location
`imm` in `σ`, with the heap as it is. This file proves when that's sound:

- `HasType.retype`: an expression that doesn't mention a location has the
  same types whatever the heap typing says about that location.
- `HeapOk.free`: if nothing in the heap refers to a `mut` location, and
  what it holds is `imm` all the way down, giving it `imm` keeps the heap
  well-typed. Its own value can't refer to it, since an `imm` value only
  refers to `imm` locations (`Expr.IsValue.no_mut_loc`).
- `freed_never_changes`: from then on, the location never changes, by
  `imm_never_changes`, for a program that's well-typed with the location
  `imm`.

A region's objects are made by `new true e` (rule `HasType.newR`) and
changed by `set` (`HasType.setR`), like `mut` ones; what they hold is at
most the region. What's left for regions is the other half: that their
typing makes sure nothing outside a block refers to its values when it
ends, so that `HeapOk.free` applies. The two proofs after this one are #283, for
borrowed parameters, and #284, for unique values at their last use.
-/

namespace BoltMut

/-- An expression that doesn't mention the location `l` has the same types
when `σ` gives `l` anything else. -/
theorem HasType.retype {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) {l : Nat} (hn : ¬ e.HasLoc l) (p : Mu × Ty) :
    HasType fns (σ.set l p) Δ io Γ e t := by
  induction ht with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | @loc _ _ k _ _ h hio =>
    have hk : l ≠ k := fun he => hn (by simp [Expr.HasLoc, he])
    exact .loc (by rw [List.getElem?_set_ne hk]; exact h) hio
  | new _ hd ih => exact .new (ih hn) hd
  | newR _ hd ih => exact .newR (ih hn) hd
  | get _ ih => exact .get (ih hn)
  | set _ _ ih₁ ih₂ =>
    simp only [Expr.HasLoc, not_or] at hn
    exact .set (ih₁ hn.1) (ih₂ hn.2)
  | setR _ _ ih₁ ih₂ =>
    simp only [Expr.HasLoc, not_or] at hn
    exact .setR (ih₁ hn.1) (ih₂ hn.2)
  | letE _ _ ih₁ ih₂ =>
    simp only [Expr.HasLoc, not_or] at hn
    exact .letE (ih₁ hn.1) (ih₂ hn.2)
  | call hf hs _ ih => exact .call hf hs (ih hn)
  | lam hc himm _ ih => exact .lam hc himm (ih hn)
  | app _ _ ih₁ ih₂ =>
    simp only [Expr.HasLoc, not_or] at hn
    exact .app (ih₁ hn.1) (ih₂ hn.2)
  | pair _ _ ih₁ ih₂ =>
    simp only [Expr.HasLoc, not_or] at hn
    exact .pair (ih₁ hn.1) (ih₂ hn.2)
  | fst _ ih => exact .fst (ih hn)
  | snd _ ih => exact .snd (ih hn)

/-- A value whose type is `imm` all the way down doesn't mention a `mut`
location: what it refers to, and what the closures in it captured, is
`imm`. -/
theorem Expr.IsValue.no_mut_loc {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {v : Expr} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ .none io Γ v t) (hd : AtMost .none .immutable t)
    {l : Nat} {t' : Ty} (hl : σ[l]? = some (.mutable, t')) : ¬ v.HasLoc l := by
  intro hloc
  obtain ⟨_, himm⟩ := (HasType.value_imm hv ht hd).hasLoc_imm rfl hloc
  rw [hl] at himm
  cases himm

/-- **Freeing a region.** If nothing in the heap refers to the `mut`
location `l`, and what it holds is `imm` all the way down, the heap is
still well-typed when `l` is `imm`. -/
theorem HeapOk.free {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h)
    {l : Nat} {t : Ty} (hl : σ[l]? = some (.mutable, t)) (himm : AtMost .none .immutable t)
    (hfree : ∀ (k : Nat) (v : Expr), k ≠ l → h[k]? = some v → ¬ v.HasLoc l) :
    HeapOk fns (σ.set l (.immutable, t)) h := by
  obtain ⟨hlen, hok⟩ := hh
  refine ⟨by simp [hlen], ?_⟩
  intro k m t'' hk
  by_cases hkl : l = k
  · subst hkl
    have hlt : l < σ.length := by
      rcases Nat.lt_or_ge l σ.length with hlt | hge
      · exact hlt
      · rw [List.getElem?_eq_none hge] at hl; cases hl
    rw [List.getElem?_set_self hlt] at hk
    cases hk
    obtain ⟨_, _, v, hv, hval, htv⟩ := hok l .mutable t hl
    exact ⟨.inr (.inl rfl), himm, v, hv, hval, htv.retype (hval.no_mut_loc htv himm hl) _⟩
  · rw [List.getElem?_set_ne hkl] at hk
    obtain ⟨hm, hat, v, hv, hval, htv⟩ := hok k m t'' hk
    exact ⟨hm, hat, v, hv, hval, htv.retype (hfree k v (Ne.symm hkl) hv) _⟩

/-- **A freed region never changes.** Once the `mut` location `l` is given
`imm`, as `HeapOk.free` allows, a program that's well-typed with `l` being
`imm` never changes it, however many steps it takes. -/
theorem freed_never_changes {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {e e' : Expr}
    {l : Nat} {t T : Ty} (hh : HeapOk fns σ h) (hl : σ[l]? = some (.mutable, t))
    (himm : AtMost .none .immutable t)
    (hfree : ∀ (k : Nat) (v : Expr), k ≠ l → h[k]? = some v → ¬ v.HasLoc l)
    (ht : HasType fns (σ.set l (.immutable, t)) .none false Ctx.empty e T)
    (hs : Steps fns h e h' e') : h'[l]? = h[l]? := by
  have hlt : l < σ.length := by
    rcases Nat.lt_or_ge l σ.length with hlt | hge
    · exact hlt
    · rw [List.getElem?_eq_none hge] at hl; cases hl
  exact imm_never_changes hf hs (hh.free hl himm hfree) ht (List.getElem?_set_self hlt)

/-! ## Freeing a region in types

When a block ends, its region `r` becomes `imm` everywhere at once: in the
heap typing and in the types of what the block gives. `Mu.free` and
`Ty.free` replace the region with a mutability. What a region's values
hold is at most the region (rule 4 of the regions page), so freeing it to
`imm` makes that `imm` all the way down (`AtMost.region_free`), as an `imm`
location needs. -/

/-- `m` with the region `r` replaced with `μ`. -/
def Mu.free : Mu → Nat → Mu → Mu
  | .region r', r, μ => if r' = r then μ else .region r'
  | m, _, _ => m

/-- `t` with the region `r` replaced with `μ`. -/
def Ty.free : Ty → Nat → Mu → Ty
  | .unit, _, _ => .unit
  | .int, _, _ => .int
  | .cell m t, r, μ => .cell (m.free r μ) (t.free r μ)
  | .fn m a b, r, μ => .fn (m.free r μ) (a.free r μ) (b.free r μ)
  | .prod a b, r, μ => .prod (a.free r μ) (b.free r μ)
  | .tparam i, _, _ => .tparam i

@[simp] theorem Mu.free_mutable (r : Nat) (μ : Mu) : Mu.mutable.free r μ = .mutable := rfl
@[simp] theorem Mu.free_immutable (r : Nat) (μ : Mu) : Mu.immutable.free r μ = .immutable := rfl
@[simp] theorem Mu.free_self (r : Nat) (μ : Mu) : (Mu.region r).free r μ = μ := by simp [Mu.free]

/-- Freeing a region keeps what's below what, with nothing assumed. -/
theorem Le.free {m μ : Mu} (h : Le .none m μ) (r : Nat) (ν : Mu) : Le .none (m.free r ν) (μ.free r ν) := by
  induction h with
  | bot => exact .bot _
  | top => exact .top _
  | refl => exact .refl _
  | assume h => simp [Assume.none] at h
  | trans _ _ ih₁ ih₂ => exact .trans ih₁ ih₂

/-- Freeing a region keeps "at most", with nothing assumed. -/
theorem AtMost.free {μ : Mu} {t : Ty} (h : AtMost .none μ t) (r : Nat) (ν : Mu) :
    AtMost .none (μ.free r ν) (t.free r ν) := by
  induction t generalizing μ with
  | unit => trivial
  | int => trivial
  | cell m t ih => exact ⟨h.1.free r ν, ih h.2⟩
  | fn m a b => exact Le.free h r ν
  | prod a b iha ihb => exact ⟨iha h.1, ihb h.2⟩
  | tparam i =>
    obtain ⟨μ', hμ', hle⟩ := h
    rcases hμ' with rfl | hmem
    · exact ⟨.mutable, .inl rfl, Le.free hle r ν⟩
    · simp [Assume.none] at hmem

/-- What a region holds becomes `imm` all the way down when the region is
freed to `imm`: what an `imm` location of the freed region needs. -/
theorem AtMost.region_free {r : Nat} {t : Ty} (h : AtMost .none (.region r) t) :
    AtMost .none .immutable (t.free r .immutable) := by
  simpa using h.free r .immutable

/-- With nothing assumed, only `imm` and the region itself are below a
region: neither `mut` nor another region. -/
theorem Le.below_region {m : Mu} {r : Nat} (h : Le .none m (.region r)) : m = .immutable ∨ m = .region r := by
  rcases h.none_inv with h | h | h
  · exact .inl h
  · cases h
  · exact .inr h

/-- `let xs = [1] in r; xs[0] = 2`: an object of a region can be changed,
like a `mut` one. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.newR (r := 0) .int trivial) (.setR (.var rfl) .int)

/-- But it can't hold a `mut` object, since it becomes `imm` when its region
is freed: `[mut [1]] in r` is rejected. -/
example : ¬ HasType [] [] .none false Ctx.empty
    (.new true (.new true (.int 1))) (.cell (.region 0) (.cell .mutable .int)) := by
  intro h
  cases h with
  | newR _ hd => exact nomatch Le.below_region hd.1

/-- Freeing needs that nothing in the heap refers to the location. Here
location 1, a `mut` cell, holds location 0, as `log.push(xs)` makes `log`
hold `xs` in the regions page's `leak`. The heap is well-typed with 0 being
`mut`, but not with it being `imm`: 1 would hold an `imm` cell at a `mut`
type, and writing through it would change 0. -/
example (fns : List Fn) :
    HeapOk fns [(.mutable, .int), (.mutable, .cell .mutable .int)] [.int 0, .loc 0] ∧
      ¬ HeapOk fns ([(.mutable, .int), (.mutable, .cell .mutable .int)].set 0 (.immutable, .int))
        [.int 0, .loc 0] := by
  refine ⟨⟨rfl, ?_⟩, ?_⟩
  · intro l m t hl
    match l, hl with
    | 0, hl =>
      cases hl
      exact ⟨.inl rfl, trivial, .int 0, rfl, .int, .int⟩
    | 1, hl =>
      cases hl
      exact ⟨.inl rfl, ⟨.refl _, trivial⟩, .loc 0, rfl, .loc, .loc rfl (fun h => nomatch h)⟩
  · intro ⟨_, hok⟩
    obtain ⟨_, _, v, hv, _, htv⟩ := hok 1 .mutable (.cell .mutable .int) rfl
    cases hv
    cases htv with
    | loc h _ => simp at h

end BoltMut
