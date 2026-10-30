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
most the region. A region can hold `mut` objects too, if it's marked so
(`mut ⊑ r`, #286): then it becomes `mut` when its block ends, rather than
`imm`, since what it holds needs that. Then, for a whole block:

- `HeapOk.free_all`: freeing a region everywhere at once, in the heap
  typing, always keeps the heap well-typed: what refers to the region's
  objects gets the freed type too.
- `block_frees`: when a block whose region the heap typing didn't mention
  before ends, its region can be frozen without a copy. Its value has the
  freed type, and what comes after it, typed before it, is well-typed.
  What's from before can't hold the region's objects, since its types
  were fixed without the region: so the `leak` of the regions page is
  rejected. The region's objects become `imm`, or `mut` if it holds
  `mut` objects.
- `block_never_changes`: after the block, the objects of a region that
  became `imm` never change.

The blocks aren't in the language: a block is any part of the program
whose region is new, run to its value. For now, a closure captures
nothing of a region, and a call's arguments mention none (`HasType.lam`,
`HasType.call`), so that the program can't change the region afterwards
through a closure. The proofs after this one are #283, for borrowed
parameters, and #284, for unique values at their last use.
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
  | call hf hs hr _ ih => exact .call hf hs hr (ih hn)
  | lam hc himm hrc hrl _ ih =>
    refine .lam hc himm hrc (fun k hk => ?_) (ih hn)
    have hlk : l ≠ k := fun he => hn (he ▸ hk)
    rw [List.getElem?_set_ne hlk]
    exact hrl k hk
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
location needs. A region that holds `mut` objects is always freed to
`mut` instead, so freeing keeps "at most" with nothing to check
(`AtMost.free`). -/

/-- `m` with the region `r` replaced with `μ`, or with `mut` if the region
holds `mut` objects: what it holds needs it (#286). -/
def Mu.free : Mu → Nat → Mu → Mu
  | .region r' b, r, μ => if r' = r then (if b then .mutable else μ) else .region r' b
  | m, _, _ => m

/-- `t` with the region `r` replaced with `μ`. A function type keeps its
parameter and result: a closure captures nothing of a region, so its body
has the same type after, and a value of a type with the region in it
can't be given to it. -/
def Ty.free : Ty → Nat → Mu → Ty
  | .unit, _, _ => .unit
  | .int, _, _ => .int
  | .cell m t, r, μ => .cell (m.free r μ) (t.free r μ)
  | .fn m a b, r, μ => .fn (m.free r μ) a b
  | .prod a b, r, μ => .prod (a.free r μ) (b.free r μ)
  | .tparam i, _, _ => .tparam i

@[simp] theorem Mu.free_mutable (r : Nat) (μ : Mu) : Mu.mutable.free r μ = .mutable := rfl
@[simp] theorem Mu.free_immutable (r : Nat) (μ : Mu) : Mu.immutable.free r μ = .immutable := rfl
@[simp] theorem Mu.free_self (r : Nat) (μ : Mu) : (Mu.region r false).free r μ = μ := by simp [Mu.free]
@[simp] theorem Mu.free_self_mut (r : Nat) (μ : Mu) : (Mu.region r true).free r μ = .mutable := by
  simp [Mu.free]

/-- Freeing a region keeps what's below what, with nothing assumed. -/
theorem Le.free {m μ : Mu} (h : Le .none m μ) (r : Nat) (ν : Mu) : Le .none (m.free r ν) (μ.free r ν) := by
  induction h with
  | bot => exact .bot _
  | top => exact .top _
  | regionMut r' =>
    simp only [Mu.free]
    split
    · exact .refl _
    · exact .regionMut r'
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
theorem AtMost.region_free {r : Nat} {t : Ty} (h : AtMost .none (.region r false) t) :
    AtMost .none .immutable (t.free r .immutable) := by
  simpa using h.free r .immutable

/-- With nothing assumed, only `imm` and the region itself are below a
region that doesn't hold `mut` objects: neither `mut` nor another region. -/
theorem Le.below_region {m : Mu} {r : Nat} (h : Le .none m (.region r false)) :
    m = .immutable ∨ m = .region r false := by
  rcases h.none_inv with h | h | h | ⟨_, h⟩
  · exact .inl h
  · cases h
  · exact .inr h
  · cases h

/-- `let xs = [1] in r; xs[0] = 2`: an object of a region can be changed,
like a `mut` one. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.newR (r := 0) (b := false) .int trivial) (.setR (.var rfl) .int)

/-- A region that becomes `imm` can't hold a `mut` object: `[mut [1]] in r`
is rejected. -/
example : ¬ HasType [] [] .none false Ctx.empty
    (.new true (.new true (.int 1))) (.cell (.region 0 false) (.cell .mutable .int)) := by
  intro h
  cases h with
  | newR _ hd => exact nomatch Le.below_region hd.1

/-- But a region that holds `mut` objects can (#286): `[mut [1]] in r`,
with `mut ⊑ r`. -/
example : HasType [] [] .none false Ctx.empty
    (.new true (.new true (.int 1))) (.cell (.region 0 true) (.cell .mutable .int)) :=
  .newR (.new .int trivial) ⟨.regionMut 0, trivial⟩

/-- And freeing it gives `mut`, even when the block's other regions become
`imm`: what it holds needs it. -/
example : (Ty.cell (.region 0 true) (.cell .mutable .int)).free 0 .immutable =
    .cell .mutable (.cell .mutable .int) := rfl

/-! ## Freeing a region when its block ends

A block is a part of the program whose region nothing before it has: the
heap typing before it doesn't mention the region. When it ends, the region
becomes `imm` everywhere at once (`HeapTy.free`), and so does the type of
what the block gives. That's always fine for the heap (`HeapOk.free_all`):
a location that holds a region's object gets the freed type too. What
could break is a program that still changes the region; but the program
after the block was typed before it, so it can't (`block_frees`). -/

theorem Mu.free_noRegion {m : Mu} (hm : m.NoRegion) (r : Nat) (μ : Mu) : m.free r μ = m := by
  cases m with
  | region => exact hm.elim
  | _ => rfl

theorem Ty.free_noRegion {t : Ty} (ht : t.NoRegion) (r : Nat) (μ : Mu) : t.free r μ = t := by
  induction t with
  | unit | int | tparam => rfl
  | cell m t ih => simp only [Ty.free, Mu.free_noRegion ht.1, ih ht.2]
  | fn m a b => simp only [Ty.free, Mu.free_noRegion ht.1]
  | prod a b iha ihb => simp only [Ty.free, iha ht.1, ihb ht.2]

/-- The heap typing with the region `r` freed to `imm`. -/
def HeapTy.free (r : Nat) (σ : HeapTy) : HeapTy :=
  σ.map fun p => (p.1.free r .immutable, p.2.free r .immutable)

theorem HeapTy.free_getElem? (r : Nat) (σ : HeapTy) (l : Nat) :
    (σ.free r)[l]? = σ[l]?.map fun p => (p.1.free r .immutable, p.2.free r .immutable) := by
  simp [HeapTy.free]

/-- Typing depends on the heap typing only for the locations in the
expression. -/
theorem HasType.congr_heap {fns : List Fn} {σ σ' : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) (hag : ∀ l, e.HasLoc l → σ'[l]? = σ[l]?) : HasType fns σ' Δ io Γ e t := by
  induction ht with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h hio => exact .loc (by rw [hag _ rfl]; exact h) hio
  | new _ hd ih => exact .new (ih hag) hd
  | newR _ hd ih => exact .newR (ih hag) hd
  | get _ ih => exact .get (ih hag)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ fun l h => hag l (.inl h)) (ih₂ fun l h => hag l (.inr h))
  | setR _ _ ih₁ ih₂ => exact .setR (ih₁ fun l h => hag l (.inl h)) (ih₂ fun l h => hag l (.inr h))
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ fun l h => hag l (.inl h)) (ih₂ fun l h => hag l (.inr h))
  | call hf hs hr _ ih => exact .call hf hs hr (ih hag)
  | lam hc himm hrc hrl _ ih =>
    refine .lam hc himm hrc (fun l hl => ?_) (ih hag)
    rw [hag l hl]
    exact hrl l hl
  | app _ _ ih₁ ih₂ => exact .app (ih₁ fun l h => hag l (.inl h)) (ih₂ fun l h => hag l (.inr h))
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ fun l h => hag l (.inl h)) (ih₂ fun l h => hag l (.inr h))
  | fst _ ih => exact .fst (ih hag)
  | snd _ ih => exact .snd (ih hag)

/-- A value has the freed type when the region is freed: a location's type
is freed with it, and a closure, which captures nothing of a region, keeps
its type. -/
theorem HasType.free_value {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {v : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ v t) (hv : v.IsValue) (r : Nat) :
    HasType fns (σ.free r) Δ io Γ v (t.free r .immutable) := by
  induction hv generalizing t with
  | unit => cases ht; exact .unit
  | int => cases ht; exact .int
  | loc => cases ht with
    | loc h hio =>
      exact .loc (by rw [HeapTy.free_getElem?, h]; rfl) (fun h' => by rw [hio h']; rfl)
  | @lam b x body => cases ht with
    | lam hc himm hrc hrl hb =>
      have hag : ∀ l, body.HasLoc l → (σ.free r)[l]? = σ[l]? := by
        intro l hl
        obtain ⟨m, t, h, hm, ht⟩ := hrl l hl
        rw [HeapTy.free_getElem?, h]
        simp [Mu.free_noRegion hm, Ty.free_noRegion ht]
      have hmu : (muOf b).free r .immutable = muOf b := by cases b <;> rfl
      simp only [Ty.free, hmu]
      exact .lam hc himm hrc (fun l hl => by rw [hag l hl]; exact hrl l hl) (hb.congr_heap hag)
  | pair _ _ ih₁ ih₂ => cases ht with
    | pair h₁ h₂ => exact .pair (ih₁ h₁) (ih₂ h₂)

/-- **Freeing a region keeps the heap well-typed**, whatever refers to it:
what refers to it gets the freed type too. What a region holds is at most
the region, so its locations are `imm` all the way down once freed. -/
theorem HeapOk.free_all {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) (r : Nat) :
    HeapOk fns (σ.free r) h := by
  refine ⟨by simp [HeapTy.free, hh.1], ?_⟩
  intro l m t hl
  rw [HeapTy.free_getElem?, Option.map_eq_some_iff] at hl
  obtain ⟨⟨m₀, t₀⟩, h₀, he⟩ := hl
  simp only [Prod.mk.injEq] at he
  obtain ⟨rfl, rfl⟩ := he
  obtain ⟨hm, hd, v, hv, hval, hvt⟩ := hh.2 l m₀ t₀ h₀
  refine ⟨?_, hd.free r .immutable, v, hv, hval, hvt.free_value hval r⟩
  rcases hm with rfl | rfl | ⟨r', b', rfl⟩
  · exact .inl rfl
  · exact .inr (.inl rfl)
  · simp only [Mu.free]
    split
    · split
      · exact .inl rfl
      · exact .inr (.inl rfl)
    · exact .inr (.inr ⟨r', b', rfl⟩)

/-- Preservation for any number of steps. -/
theorem preservation_steps {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ HasType fns σ' .none false Ctx.empty e' t := by
  induction hs with
  | refl => intro σ t hh ht; exact ⟨σ, Extends.refl σ, hh, ht⟩
  | step s _ ih =>
    intro σ t hh ht
    obtain ⟨σ₁, hx₁, hh₁, ht₁⟩ := preservation hf s hh ht
    obtain ⟨σ₂, hx₂, hh₂, ht₂⟩ := ih hh₁ ht₁
    exact ⟨σ₂, hx₁.trans hx₂, hh₂, ht₂⟩

/-- **A block's region can be frozen when it ends.** The heap typing `σ`
before the block doesn't mention its region `r`, and the block `e` gives
the value `v`. Then, with `r` freed to `imm`: the heap is well-typed, `v`
has the freed type, the region's locations are `imm`, or `mut` if the
region holds `mut` objects, and the heap typing
still has `σ`, so what comes after the block, `k` with `x` for `v`, typed
before it, is well-typed. No copy is made: the heap is the same. -/
theorem block_frees {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {e v : Expr} {t : Ty} {r : Nat}
    (hh : HeapOk fns σ h) (hfresh : σ.free r = σ) (ht : HasType fns σ .none false Ctx.empty e t)
    (hs : Steps fns h e h' v) (hv : v.IsValue) :
    ∃ σ' : HeapTy, Extends σ σ' ∧ Extends σ (σ'.free r) ∧ HeapOk fns (σ'.free r) h' ∧
      HasType fns (σ'.free r) .none false Ctx.empty v (t.free r .immutable) ∧
      (∀ (l : Nat) (s : Ty), σ'[l]? = some (.region r false, s) →
        ∃ s' : Ty, (σ'.free r)[l]? = some (.immutable, s')) ∧
      (∀ (l : Nat) (s : Ty), σ'[l]? = some (.region r true, s) →
        ∃ s' : Ty, (σ'.free r)[l]? = some (.mutable, s')) ∧
      ∀ (x : Nat) (k : Expr) (T : Ty), HasType fns σ .none false (Ctx.empty.update x (t.free r .immutable)) k T →
        HasType fns (σ'.free r) .none false Ctx.empty (k.subst x v) T := by
  obtain ⟨σ', hx, hh', hv'⟩ := preservation_steps hf hs hh ht
  have hx' : Extends σ (σ'.free r) := by
    intro l p hl
    have hfix : σ[l]?.map (fun p => (p.1.free r .immutable, p.2.free r .immutable)) = σ[l]? := by
      rw [← HeapTy.free_getElem?, hfresh]
    rw [hl] at hfix
    rw [HeapTy.free_getElem?, hx l p hl]
    exact hfix
  have hvf := hv'.free_value hv r
  refine ⟨σ', hx, hx', hh'.free_all r, hvf, ?_, ?_, ?_⟩
  · intro l s hl
    exact ⟨s.free r .immutable, by rw [HeapTy.free_getElem?, hl]; simp [Mu.free]⟩
  · intro l s hl
    exact ⟨s.free r .immutable, by rw [HeapTy.free_getElem?, hl]; simp [Mu.free]⟩
  · intro x k T hk
    exact (hk.weaken_heap hx').subst_update hv hvf

/-- **After its block, a region never changes.** Whatever runs after the
block, as long as it was typed before it, leaves every location of the
region as the block left it, unless the region holds `mut` objects and so
became `mut`. -/
theorem block_never_changes {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' h'' : Heap}
    {e v k e'' : Expr} {t T : Ty} {r x : Nat}
    (hh : HeapOk fns σ h) (hfresh : σ.free r = σ) (ht : HasType fns σ .none false Ctx.empty e t)
    (hs : Steps fns h e h' v) (hv : v.IsValue)
    (hk : HasType fns σ .none false (Ctx.empty.update x (t.free r .immutable)) k T)
    (hs' : Steps fns h' (k.subst x v) h'' e'') :
    ∃ σ' : HeapTy, Extends σ σ' ∧ ∀ (l : Nat) (s : Ty), σ'[l]? = some (.region r false, s) → h''[l]? = h'[l]? := by
  obtain ⟨σ₁, hx, _, hh₁, _, himm, _, hcont⟩ := block_frees hf hh hfresh ht hs hv
  refine ⟨σ₁, hx, fun l s hl => ?_⟩
  obtain ⟨s', hl'⟩ := himm l s hl
  exact imm_never_changes hf hs' hh₁ (hcont x k T hk) hl'

/-- The `leak` of the regions page: `log.push(xs)`, with `log` from before
the block and `xs` of its region, is rejected. `log`'s type was fixed before
the block, so it can't hold an object of the region: here `log` is location
0, a `mut` cell holding a `mut` cell, and `xs` a cell of region 0. -/
example (b : Bool) (t : Ty) : ¬ HasType [] [(.mutable, .cell .mutable .int)] .none false
    (Ctx.empty.update 0 (.cell (.region 0 b) .int)) (.set (.loc 0) (.var 0)) t := by
  intro h
  cases h with
  | set h₁ h₂ =>
    cases h₁ with
    | loc hσ _ =>
      simp only [List.getElem?_cons_zero, Option.some.injEq, Prod.mk.injEq, true_and] at hσ
      subst hσ
      cases h₂ with
      | var hx => simp [Ctx.update] at hx
  | setR h₁ _ =>
    cases h₁ with
    | loc hσ _ => simp at hσ

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
