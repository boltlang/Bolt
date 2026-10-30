import BoltMut.Sharing

/-!
# Leaving out a freeze's copy (#292)

A block whose region becomes `imm` ends with a freeze, which copies the
region's part of its value (`Copy`). The compiler would rather freeze the
objects in place: give the value as it is, and treat its objects as `imm`
from then on. Nothing can tell, if nothing after the block can reach the
region's objects: then the originals are as good as the copies.

`freeze_in_place_changes_nothing` proves it, for one freeze. The program
that copies goes on with the copy, `k` with `x` for `v'`; the one that
doesn't goes on with the value itself, `k` with `x` for `v`, in the heap
without the copies. Whatever the first computes, the second does too, with
a result that corresponds, and the same integer. The premise is that
nothing of the region can be reached from `k`, or from the copy, which
shares what it doesn't copy, like a closure.

The typing gives that premise (`freeze_in_place_of_typing`): what comes
after the block was typed before the block made its region (rules 2 and
3 of the regions page), and a closure that captures a region's objects has
the region in its type (rule 5). Leaving out every freeze of a program,
rather than one, is still to do.

The proof relates each copy to its original, and every other location that
can still be reached to itself (`steps_sim` in `Sharing.lean` does the
rest). The originals, and whatever else can't be reached, are left out:
the first program never looks at them again.
-/

namespace BoltMut

/-- `a` can be reached through the heap `h` from a location in `e`. -/
def Expr.Reaches (h : Heap) (e : Expr) (a : Nat) : Prop := ∃ l, e.HasLoc l ∧ BoltMut.Reaches h l a

theorem Reaches.trans {h : Heap} {a b c : Nat} (h₁ : Reaches h a b) (h₂ : Reaches h b c) : Reaches h a c := by
  induction h₁ with
  | refl => exact h₂
  | step hv hl _ ih => exact .step hv hl (ih h₂)

/-- A longer heap reaches what a shorter one does. -/
theorem Reaches.append {h : Heap} {a b : Nat} (hr : Reaches h a b) (ext : Heap) : Reaches (h ++ ext) a b := by
  induction hr with
  | refl => exact .refl
  | step hv hl _ ih =>
    refine .step ?_ hl ih
    rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp hv).1]
    exact hv

/-- The size of a type. -/
def Ty.size : Ty → Nat
  | .unit | .int | .tparam _ => 1
  | .cell _ t => t.size + 1
  | .fn _ a b => a.size + b.size + 1
  | .prod a b => a.size + b.size + 1

/-- What a copy copies has a type no bigger than the value's: so copying
what an object holds never copies the object itself again. -/
theorem Copy.keys_small {fns : List Fn} {σ : HeapTy} {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)}
    {v v' : Expr} (hc : Copy r t h m v h' m' v') {h₀ : Heap} (hh : HeapOk fns σ h₀) :
    (∃ ext, h = h₀ ++ ext) → ∀ {io : Bool} {Γ : Ctx}, HasType fns σ .none io Γ v t →
      ∀ k c, m'.lookup k = some c → m.lookup k = none → ∃ μ s, σ[k]? = some (μ, s) ∧ (Ty.cell μ s).size ≤ t.size := by
  induction hc with
  | keep | seen => intro _ io Γ _ k c h₁ h₂; rw [h₁] at h₂; cases h₂
  | @cell μ s h h' m m' l w w' _ _ hw _ ih =>
    intro hpre io Γ ht k c hk hkm
    cases ht with
    | loc hσ _ =>
      obtain ⟨_, _, w₀, hw₀, _, hwt⟩ := hh.2 l μ s hσ
      obtain ⟨ext, rfl⟩ := hpre
      rw [List.getElem?_append_left (hh.lt hσ), hw₀] at hw
      cases hw
      simp only [List.lookup_cons_nat] at hk
      split at hk
      · rename_i he
        subst he
        exact ⟨μ, s, hσ, Nat.le_refl _⟩
      · obtain ⟨μ', s', h₁, h₂⟩ := ih ⟨ext, rfl⟩ hwt k c hk hkm
        exact ⟨μ', s', h₁, by simp only [Ty.size] at h₂ ⊢; omega⟩
  | @pair a b h h₁ h₂ m m₁ m₂ _ _ _ _ _ hc₁ _ ih₁ ih₂ =>
    intro hpre io Γ ht k c hk hkm
    cases ht with
    | pair ht₁ ht₂ =>
      have hpre₁ : ∃ ext, h₁ = h₀ ++ ext := by
        obtain ⟨e₁, rfl⟩ := hpre
        obtain ⟨e₂, rfl⟩ := hc₁.prefix
        exact ⟨e₁ ++ e₂, by simp⟩
      cases h₁k : m₁.lookup k with
      | none =>
        obtain ⟨μ', s', e₁, e₂⟩ := ih₂ hpre₁ ht₂ k c hk h₁k
        exact ⟨μ', s', e₁, by simp only [Ty.size] at e₂ ⊢; omega⟩
      | some c' =>
        obtain ⟨μ', s', e₁, e₂⟩ := ih₁ hpre ht₁ k c' h₁k hkm
        exact ⟨μ', s', e₁, by simp only [Ty.size] at e₂ ⊢; omega⟩

/-- What a copy copied stays copied, to the same place. -/
theorem Copy.memo_mono {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') : ∀ o c, m.lookup o = some c → m'.lookup o = some c := by
  induction hc with
  | keep | seen => exact fun _ _ h => h
  | @cell _ _ _ _ _ _ l _ _ _ hnone _ _ ih =>
    intro o c ho
    rw [List.lookup_cons_nat]
    have : o ≠ l := by rintro rfl; rw [hnone] at ho; cases ho
    simp only [this, ite_false]
    exact ih o c ho
  | pair _ _ _ ih₁ ih₂ => exact fun o c ho => ih₂ o c (ih₁ o c ho)

/-- **What a freeze's copy corresponds to.** Each object `o` of the region
it copies, it copies once, to a new `c` (`m'.lookup o = some c`). With
the copies corresponding to their originals, and what the copy shares to
itself, the copy corresponds to the value, and each copy holds what
corresponds to what its original holds. -/
theorem Copy.map {fns : List Fn} {σ : HeapTy} {h₀ : Heap} (hh : HeapOk fns σ h₀) {r : Nat} {t : Ty}
    {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr} (hc : Copy r t h m v h' m' v') :
    (∃ ext, h = h₀ ++ ext) → ∀ {io : Bool} {Γ : Ctx}, HasType fns σ .none io Γ v t → t.Freezable r →
      (∀ o c, m.lookup o = some c → h₀.length ≤ c ∧ c < h.length ∧ ∃ s, σ[o]? = some (.region r, s)) →
      (∀ o c, m'.lookup o = some c → h₀.length ≤ c ∧ c < h'.length ∧ ∃ s, σ[o]? = some (.region r, s)) ∧
      ∀ (ρ : Nat → Nat → Prop) (hfin : Heap), (∃ ext, hfin = h' ++ ext) → (∀ o c, m'.lookup o = some c → ρ c o) →
        (∀ l, l < h₀.length → v'.Reaches hfin l → ρ l l) →
        Rel ρ v' v ∧ ∀ o c, m'.lookup o = some c → m.lookup o = none →
          ∃ w w', h₀[o]? = some w ∧ hfin[c]? = some w' ∧ Rel ρ w' w := by
  induction hc with
  | @keep t h m v _ =>
    intro _ io Γ ht _ hm
    refine ⟨hm, fun ρ hfin _ _ hid => ⟨?_, fun o c h₁ h₂ => by rw [h₁] at h₂; cases h₂⟩⟩
    -- What's shared is old, and corresponds to itself.
    refine Rel.refl fun l hl => hid l ?_ ⟨l, hl, .refl⟩
    rw [hh.1]
    exact ht.hasLoc_lt hl
  | @seen μ s h m l c _ hlk =>
    intro _ io Γ _ _ hm
    exact ⟨hm, fun ρ hfin _ hρ _ => ⟨.loc (hρ l c hlk), fun o c h₁ h₂ => by rw [h₁] at h₂; cases h₂⟩⟩
  | @cell μ s h h' m m' l w w' hmen hnone hw hcw ih =>
    intro hpre io Γ ht hf hm
    cases ht with
    | loc hσ _ =>
      obtain ⟨_, _, w₀, hw₀, _, hwt⟩ := hh.2 l μ s hσ
      have hl₀ : l < h₀.length := hh.lt hσ
      have hw' : w₀ = w := by
        obtain ⟨ext, rfl⟩ := hpre
        rw [List.getElem?_append_left hl₀, hw₀] at hw
        exact Option.some.inj hw
      subst hw'
      have hreg := hf.cell_region hmen
      subst hreg
      have hle : h.length ≤ h'.length := by
        obtain ⟨e', rfl⟩ := hcw.prefix
        simp
      have hh₀ : h₀.length ≤ h.length := by
        obtain ⟨e, rfl⟩ := hpre
        simp
      -- Copying what `l` holds didn't copy `l`: its type is smaller.
      have hnone' : m'.lookup l = none := by
        cases e : m'.lookup l with
        | none => rfl
        | some c =>
          obtain ⟨μ', s', h₁, h₂⟩ := hcw.keys_small hh hpre hwt l c e hnone
          rw [hσ] at h₁
          cases h₁
          simp only [Ty.size] at h₂
          omega
      obtain ⟨hπ, hrel⟩ := ih hpre hwt hf.2 hm
      refine ⟨?_, ?_⟩
      · intro o c hoc
        rw [List.lookup_cons_nat] at hoc
        split at hoc
        · rename_i he
          cases hoc
          subst he
          exact ⟨by omega, by simp, s, hσ⟩
        · obtain ⟨h₁, h₂, h₃⟩ := hπ o c hoc
          exact ⟨h₁, by simp; omega, h₃⟩
      · intro ρ hfin hfin' hρ hid
        obtain ⟨ext', rfl⟩ := hfin'
        have hc' : (h' ++ [w'] ++ ext')[h'.length]? = some w' := by simp
        have hρ' : ∀ o c, m'.lookup o = some c → ρ c o := by
          intro o c hoc
          have : o ≠ l := by rintro rfl; rw [hnone'] at hoc; cases hoc
          apply hρ o c
          rw [List.lookup_cons_nat]
          simp only [this, ite_false]
          exact hoc
        -- What the copy holds can be reached from it.
        have hid' : ∀ l', l' < h₀.length → w'.Reaches (h' ++ [w'] ++ ext') l' → ρ l' l' := by
          intro l' hl' ⟨l₁, hl₁, hr⟩
          exact hid l' hl' ⟨h'.length, rfl, .step hc' hl₁ hr⟩
        obtain ⟨hrw, hcon⟩ := hrel ρ _ ⟨[w'] ++ ext', by simp⟩ hρ' hid'
        refine ⟨.loc (hρ l h'.length (by simp)), ?_⟩
        intro o c hoc hom
        rw [List.lookup_cons_nat] at hoc
        split at hoc
        · rename_i he
          cases hoc
          subst he
          exact ⟨w₀, w', hw₀, hc', hrw⟩
        · exact hcon o c hoc hom
  | @pair a b h h₁ h₂ m m₁ m₂ v₁ v₂ v₁' v₂' _ hc₁ hc₂ ih₁ ih₂ =>
    intro hpre io Γ ht hf hm
    cases ht with
    | pair ht₁ ht₂ =>
      obtain ⟨hπ₁, hrel₁⟩ := ih₁ hpre ht₁ hf.1 hm
      have hpre₁ : ∃ ext, h₁ = h₀ ++ ext := by
        obtain ⟨e₁, rfl⟩ := hpre
        obtain ⟨e₂, rfl⟩ := hc₁.prefix
        exact ⟨e₁ ++ e₂, by simp⟩
      obtain ⟨hπ₂, hrel₂⟩ := ih₂ hpre₁ ht₂ hf.2 hπ₁
      refine ⟨hπ₂, fun ρ hfin hfin' hρ hid => ?_⟩
      have hfin₁ : ∃ ext, hfin = h₁ ++ ext := by
        obtain ⟨e₁, rfl⟩ := hfin'
        obtain ⟨e₂, rfl⟩ := hc₂.prefix
        exact ⟨e₂ ++ e₁, by simp⟩
      obtain ⟨hr₁, hc₁'⟩ := hrel₁ ρ hfin hfin₁ (fun o c h => hρ o c (hc₂.memo_mono o c h))
        (fun l hl ⟨l₁, hl₁, hr⟩ => hid l hl ⟨l₁, .inl hl₁, hr⟩)
      obtain ⟨hr₂, hc₂'⟩ := hrel₂ ρ hfin hfin' hρ (fun l hl ⟨l₁, hl₁, hr⟩ => hid l hl ⟨l₁, .inr hl₁, hr⟩)
      refine ⟨.pair hr₁ hr₂, fun o c hoc hom => ?_⟩
      cases h₁o : m₁.lookup o with
      | none => exact hc₂' o c hoc h₁o
      | some c' =>
        have e := hc₂.memo_mono o c' h₁o
        rw [hoc] at e
        obtain rfl := Option.some.inj e
        exact hc₁' o c h₁o hom

/-- **Freezing in place can't be noticed.** At the end of a block whose
region `r` becomes `imm`, the value `v` is copied to `v'`. If nothing of `r`
can be reached from what comes after, `k`, or from the copy, then going on
with `v` itself computes what going on with the copy does: whatever result
the first gets, the second gets one that corresponds, and the same
integer. Both go on in the heap with the copies, so that blocks that start
later make the same regions in both; the second just doesn't use them. -/
theorem freeze_in_place_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' h₁ : Heap}
    {m' : List (Nat × Nat)} {r x : Nat} {t T : Ty} {v v' k e₁ : Expr} (hh : HeapOk fns σ h) (hv : v.IsValue)
    (ht : HasType fns σ .none false Ctx.empty v t) (hfz : t.Freezable r)
    (hk : HasType fns σ .none false (Ctx.empty.update x (t.free r .immutable)) k T)
    (hc : Copy r t h [] v h' m' v')
    (hdead : ∀ a s, σ[a]? = some (.region r, s) → ¬ k.Reaches h' a ∧ ¬ v'.Reaches h' a)
    (hcl : HeapClosed h) (hvc : v.Closed) (hkc : k.Closed)
    (hs : Steps fns h' (k.subst x v') h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps fns h' (k.subst x v) h₂ e₂ ∧ Rel ρ e₁ e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n := by
  obtain ⟨σ', hx, hh', hv', ht', hnew, _⟩ := hc.typed hh hv ht hfz (MemoOk.nil σ r)
  obtain ⟨hπ₀, hrel⟩ := hc.map hh ⟨[], by simp⟩ ht hfz (fun _ _ h => by simp at h)
  -- Each copy `a` is of the original `b`.
  let π : Nat → Nat → Prop := fun a b => m'.lookup b = some a
  have hπ : ∀ c o, π c o → h.length ≤ c ∧ c < h'.length ∧ ∃ s, σ[o]? = some (.region r, s) := by
    intro c o hco
    obtain ⟨h₁, h₂, h₃⟩ := hπ₀ o c hco
    exact ⟨h₁, h₂, h₃⟩
  -- What can still be reached: from `k`, or from the copy.
  let L : Nat → Prop := fun a => k.Reaches h' a ∨ v'.Reaches h' a
  let ρ : Nat → Nat → Prop := fun a b => (a = b ∧ a < h.length ∧ L a) ∨ π a b
  have hlen : h.length ≤ h'.length := by
    obtain ⟨e, rfl⟩ := hc.prefix
    simp
  obtain ⟨hrv, hcon₀⟩ := hrel ρ h' ⟨[], by simp⟩ (fun o c h => .inr h)
    (fun l hl hr => .inl ⟨rfl, by rw [hh.1]; rw [hh.1] at hl; exact hl, .inr hr⟩)
  have hcon : ∀ c o, π c o → ∃ w w', h[o]? = some w ∧ h'[c]? = some w' ∧ Rel ρ w' w :=
    fun c o hco => hcon₀ o c hco (by simp)
  -- An old location that can be reached holds what can be reached.
  have hL : ∀ a w l, L a → h'[a]? = some w → w.HasLoc l → L l := by
    intro a w l ha hw hl
    rcases ha with ⟨l₀, hl₀, hr⟩ | ⟨l₀, hl₀, hr⟩
    · exact .inl ⟨l₀, hl₀, hr.trans (.step hw hl .refl)⟩
    · exact .inr ⟨l₀, hl₀, hr.trans (.step hw hl .refl)⟩
  -- No original can still be reached.
  have horig : ∀ c o, π c o → ¬ L o := by
    intro c o hco ho
    obtain ⟨_, _, s, hs⟩ := hπ c o hco
    rcases ho with ho | ho
    · exact (hdead o s hs).1 ho
    · exact (hdead o s hs).2 ho
  let M : Nat → Prop := fun a => ∃ m t', σ'[a]? = some (m, t') ∧ m ≠ .immutable
  have hcorr : Corr M ρ h' h' := by
    refine ⟨?_, ?_, ?_⟩
    · intro a b hab
      rcases hab with ⟨rfl, ha, hLa⟩ | hab
      · obtain ⟨w, hw⟩ : ∃ w, h[a]? = some w := ⟨_, List.getElem?_eq_getElem ha⟩
        have hw' : h'[a]? = some w := by rw [hc.keeps ha]; exact hw
        refine ⟨w, w, hw', hw', Rel.refl fun l hl => .inl ⟨rfl, ?_, hL a w l hLa hw' hl⟩⟩
        obtain ⟨p, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem (hh.1 ▸ ha)⟩
        obtain ⟨_, _, w₀, hw₀, _, hwt⟩ := hh.2 a p.1 p.2 hp
        rw [hw] at hw₀
        cases hw₀
        rw [hh.1]
        exact hwt.hasLoc_lt hl
      · obtain ⟨w, w', hw, hw', hr⟩ := hcon a b hab
        obtain ⟨_, _, s, hs⟩ := hπ a b hab
        exact ⟨w', w, hw', by rw [hc.keeps (hh.lt hs)]; exact hw, hr⟩
    · -- A copy is `imm`, so only a location that corresponds to itself
      -- counts, and no original can still be reached.
      intro a b c hac hbc hMa
      have hcopy : ∀ a c, π a c → ¬ M a := by
        intro a c hac ⟨m, t', ha, hm⟩
        obtain ⟨hle, _, _⟩ := hπ a c hac
        exact hm (hnew a (m, t') (by rw [← hh.1]; exact hle) ha)
      rcases hac with ⟨rfl, _, _⟩ | hac
      · rcases hbc with ⟨rfl, _, _⟩ | hbc
        · rfl
        · exact absurd ‹L a› (horig b a hbc)
      · exact absurd hMa (hcopy a c hac)
    · intro a c d hac had hMa
      have hcopy : ∀ a c, π a c → ¬ M a := by
        intro a c hac ⟨m, t', ha, hm⟩
        obtain ⟨hle, _, _⟩ := hπ a c hac
        exact hm (hnew a (m, t') (by rw [← hh.1]; exact hle) ha)
      rcases hac with ⟨rfl, ha, _⟩ | hac
      · rcases had with ⟨rfl, _, _⟩ | had
        · rfl
        · obtain ⟨hle, _, _⟩ := hπ a d had
          omega
      · exact absurd hMa (hcopy a c hac)
  -- `k` corresponds to itself, and the copy to the value.
  have hrk : Rel ρ k k := by
    refine Rel.refl fun l hl => .inl ⟨rfl, ?_, .inl ⟨l, hl, .refl⟩⟩
    rw [hh.1]
    exact hk.hasLoc_lt hl
  have hte : HasType fns σ' .none false Ctx.empty (k.subst x v') T :=
    (hk.weaken_heap hx).subst_update hv' ht'
  obtain ⟨hcl', hvc'⟩ := hc.closed hcl hvc
  obtain ⟨_, ρ', h₂, e₂, ss, _, hr'⟩ :=
    steps_sim hf hs hh' hte (fun a m t' h hm => ⟨m, t', h, hm⟩) hcorr (hrk.subst hrv x)
      hcl' (hkc.subst hvc' x) rfl
  exact ⟨ρ', h₂, e₂, ss, hr', fun n hn => (hn ▸ hr').int_eq⟩

/-! ## The premise, from the typing

A value can only reach an object of a region if its type mentions the
region: a cell holds what its type says, and a closure captures only what
mentions its own region, which its type does (rule 5 of the regions page,
`HasType.lam`). What comes after a block was typed before the block made
its region, so its type doesn't mention it; and the copy's type is the
value's with the region replaced with `imm`. So neither can reach the
region's objects (`freeze_in_place_of_typing`). -/

/-- In a well-typed heap, nothing that a location whose type doesn't mention
`r` reaches is of `r`. -/
theorem HeapOk.reaches_notMentions {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {r a b : Nat}
    (hr : Reaches h a b) :
    ∀ {m : Mu} {t : Ty}, σ[a]? = some (m, t) → (Ty.cell m t).mentions r = false →
      ∃ m' t', σ[b]? = some (m', t') ∧ (Ty.cell m' t').mentions r = false := by
  induction hr with
  | refl => intro m t ha hm; exact ⟨m, t, ha, hm⟩
  | step hv hl _ ih =>
    intro m t ha hm
    obtain ⟨_, _, w, hw, hwv, hwt⟩ := hh.2 _ _ _ ha
    rw [hv] at hw
    cases hw
    have ht : t.mentions r = false := by
      simp only [Ty.mentions, Bool.or_eq_false_iff] at hm
      exact hm.2
    obtain ⟨m₁, t₁, h₁, hm₁⟩ := hwv.hasLoc_notMentions hwt ht hl
    exact ih h₁ hm₁

/-- **The typing makes freezing in place unnoticeable.** A block `e`
starts in a heap whose typing `σ₀` mentions its region `r` nowhere, and
runs to the value `v`, whose freeze copies it to `v'`. What comes after
it, `k`, was typed before it. Then going on with `v` itself, in the heap
with the copies unused, computes what going on with the copy does. -/
theorem freeze_in_place_of_typing {fns : List Fn} (hf : FnsOk fns) {σ₀ : HeapTy} {h₀ h h' h₁ : Heap}
    {r x : Nat} {t T : Ty} {e v v' k e₁ : Expr} (hh₀ : HeapOk fns σ₀ h₀)
    (hfresh : ∀ (l : Nat) (p : Mu × Ty), σ₀[l]? = some p → (Ty.cell p.1 p.2).mentions r = false)
    (he : HasType fns σ₀ .none false Ctx.empty e t) (hfz : t.Freezable r)
    (hk : HasType fns σ₀ .none false (Ctx.empty.update x (t.free r .immutable)) k T)
    (hcl₀ : HeapClosed h₀) (hec : e.Closed) (hkc : k.Closed)
    (hse : Steps fns h₀ e h v) (hv : v.IsValue) {m' : List (Nat × Nat)} (hc : Copy r t h [] v h' m' v')
    (hs : Steps fns h' (k.subst x v') h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps fns h' (k.subst x v) h₂ e₂ ∧ Rel ρ e₁ e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n := by
  obtain ⟨σ, hx, hh, hvt, hcl, hvc⟩ := steps_preservation hf hse hh₀ he hcl₀ hec
  obtain ⟨σ', hx', hh', hv', ht', _, _⟩ := hc.typed hh hv hvt hfz (MemoOk.nil σ r)
  refine freeze_in_place_changes_nothing hf hh hv hvt hfz (hk.weaken_heap hx) hc ?_ hcl hvc hkc hs
  intro a s ha
  -- An object of `r` has a type that mentions `r`.
  have hbad : ∀ m' t', σ'[a]? = some (m', t') → (Ty.cell m' t').mentions r = false → False := by
    intro m' t' ha' hm'
    rw [hx' a _ ha] at ha'
    cases ha'
    simp [Ty.mentions, Mu.mentions] at hm'
  constructor
  · rintro ⟨l, hl, hr⟩
    obtain ⟨p, hp⟩ : ∃ p, σ₀[l]? = some p := ⟨_, List.getElem?_eq_getElem (hk.hasLoc_lt hl)⟩
    obtain ⟨m', t', ha', hm'⟩ := hh'.reaches_notMentions hr (hx' l p (hx l p hp)) (hfresh l p hp)
    exact hbad m' t' ha' hm'
  · rintro ⟨l, hl, hr⟩
    obtain ⟨m, t₀, hl', hm⟩ := hv'.hasLoc_notMentions ht' (Ty.free_mentions_self (μ := .immutable) trivial r) hl
    obtain ⟨m', t', ha', hm'⟩ := hh'.reaches_notMentions hr hl' hm
    exact hbad m' t' ha' hm'

end BoltMut
