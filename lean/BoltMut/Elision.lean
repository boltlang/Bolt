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

/-- **What a freeze's copy corresponds to.** Each copy `c` is of an object
`o` of the region (`π c o`), and is new. With the copies corresponding to
their originals, and what the copy shares to itself, the copy corresponds
to the value, and each copy holds what corresponds to what its original
holds. -/
theorem Copy.map {fns : List Fn} {σ : HeapTy} {h₀ : Heap} (hh : HeapOk fns σ h₀) {r : Nat} {t : Ty}
    {h h' : Heap} {v v' : Expr} (hc : Copy r t h v h' v') :
    (∃ ext, h = h₀ ++ ext) → ∀ {io : Bool} {Γ : Ctx}, HasType fns σ .none io Γ v t → t.Freezable r →
      ∃ π : Nat → Nat → Prop,
        (∀ c o, π c o → h.length ≤ c ∧ c < h'.length ∧ ∃ s, σ[o]? = some (.region r, s)) ∧
        ∀ (ρ : Nat → Nat → Prop) (hfin : Heap), (∃ ext, hfin = h' ++ ext) → (∀ c o, π c o → ρ c o) →
          (∀ l, l < h₀.length → v'.Reaches hfin l → ρ l l) →
          Rel ρ v' v ∧ ∀ c o, π c o → ∃ w w', h₀[o]? = some w ∧ hfin[c]? = some w' ∧ Rel ρ w' w := by
  induction hc with
  | @keep t h v _ =>
    intro _ io Γ ht _
    refine ⟨fun _ _ => False, fun _ _ h => h.elim, fun ρ hfin _ _ hid => ⟨?_, fun _ _ h => h.elim⟩⟩
    -- What's shared is old, and corresponds to itself.
    refine Rel.refl fun l hl => hid l ?_ ⟨l, hl, .refl⟩
    rw [hh.1]
    exact ht.hasLoc_lt hl
  | @cell m s h h' l w w' hmen hw _ ih =>
    intro hpre io Γ ht hf
    cases ht with
    | loc hσ _ =>
      obtain ⟨_, _, w₀, hw₀, _, hwt⟩ := hh.2 l m s hσ
      have hl₀ : l < h₀.length := hh.lt hσ
      obtain ⟨ext, rfl⟩ := hpre
      rw [List.getElem?_append_left hl₀, hw₀] at hw
      obtain rfl : w₀ = w := Option.some.inj hw
      have hreg : m = .region r := by
        simp only [Ty.mentions, Bool.or_eq_true] at hmen
        rcases hmen with hm | hm
        · cases m with
          | region r' => simp only [Mu.mentions, beq_iff_eq] at hm; rw [hm]
          | _ => cases hm
        · exact hf.1 hm
      subst hreg
      obtain ⟨π, hπ, hrel⟩ := ih ⟨ext, rfl⟩ hwt hf.2
      have hle : (h₀ ++ ext).length ≤ h'.length := by
        obtain ⟨e', rfl⟩ := ‹Copy r s (h₀ ++ ext) w₀ h' w'›.prefix
        simp
      refine ⟨fun c o => π c o ∨ (c = h'.length ∧ o = l), ?_, ?_⟩
      · intro c o hco
        rcases hco with hco | ⟨rfl, rfl⟩
        · obtain ⟨h₁, h₂, h₃⟩ := hπ c o hco
          exact ⟨h₁, by simp; omega, h₃⟩
        · exact ⟨hle, by simp, s, hσ⟩
      · intro ρ hfin hfin' hρ hid
        obtain ⟨ext', rfl⟩ := hfin'
        have hc' : (h' ++ [w'] ++ ext')[h'.length]? = some w' := by simp
        -- What the copy holds can be reached from it.
        have hid' : ∀ l', l' < h₀.length → w'.Reaches (h' ++ [w'] ++ ext') l' → ρ l' l' := by
          intro l' hl' ⟨l₁, hl₁, hr⟩
          exact hid l' hl' ⟨h'.length, rfl, .step hc' hl₁ hr⟩
        obtain ⟨hrw, hcon⟩ := hrel ρ _ ⟨[w'] ++ ext', by simp⟩ (fun c o h => hρ c o (.inl h)) hid'
        refine ⟨.loc (hρ _ _ (.inr ⟨rfl, rfl⟩)), ?_⟩
        intro c o hco
        rcases hco with hco | ⟨rfl, rfl⟩
        · exact hcon c o hco
        · exact ⟨w₀, w', hw₀, hc', hrw⟩
  | @pair a b h h₁ h₂ v₁ v₂ v₁' v₂' _ hc₁ hc₂ ih₁ ih₂ =>
    intro hpre io Γ ht hf
    cases ht with
    | pair ht₁ ht₂ =>
      obtain ⟨π₁, hπ₁, hrel₁⟩ := ih₁ hpre ht₁ hf.1
      have hpre₁ : ∃ ext, h₁ = h₀ ++ ext := by
        obtain ⟨e₁, rfl⟩ := hpre
        obtain ⟨e₂, rfl⟩ := hc₁.prefix
        exact ⟨e₁ ++ e₂, by simp⟩
      obtain ⟨π₂, hπ₂, hrel₂⟩ := ih₂ hpre₁ ht₂ hf.2
      have hle₁ : h.length ≤ h₁.length := by
        obtain ⟨e₂, rfl⟩ := hc₁.prefix
        simp
      have hle₂ : h₁.length ≤ h₂.length := by
        obtain ⟨e₂, rfl⟩ := hc₂.prefix
        simp
      refine ⟨fun c o => π₁ c o ∨ π₂ c o, ?_, ?_⟩
      · intro c o hco
        rcases hco with hco | hco
        · obtain ⟨h₁', h₂', h₃'⟩ := hπ₁ c o hco
          exact ⟨h₁', by omega, h₃'⟩
        · obtain ⟨h₁', h₂', h₃'⟩ := hπ₂ c o hco
          exact ⟨by omega, h₂', h₃'⟩
      · intro ρ hfin hfin' hρ hid
        have hfin₁ : ∃ ext, hfin = h₁ ++ ext := by
          obtain ⟨e₁, rfl⟩ := hfin'
          obtain ⟨e₂, rfl⟩ := hc₂.prefix
          exact ⟨e₂ ++ e₁, by simp⟩
        obtain ⟨hr₁, hc₁'⟩ := hrel₁ ρ hfin hfin₁ (fun c o h => hρ c o (.inl h))
          (fun l hl ⟨l₁, hl₁, hr⟩ => hid l hl ⟨l₁, .inl hl₁, hr⟩)
        obtain ⟨hr₂, hc₂'⟩ := hrel₂ ρ hfin hfin' (fun c o h => hρ c o (.inr h))
          (fun l hl ⟨l₁, hl₁, hr⟩ => hid l hl ⟨l₁, .inr hl₁, hr⟩)
        exact ⟨.pair hr₁ hr₂, fun c o hco => hco.elim (hc₁' c o) (hc₂' c o)⟩

/-- **Freezing in place can't be noticed.** At the end of a block whose
region `r` becomes `imm`, the value `v` is copied to `v'`. If nothing of `r`
can be reached from what comes after, `k`, or from the copy, then going on
with `v` itself, in the heap without the copies, computes what going on
with the copy does: whatever result the first gets, the second gets one
that corresponds, and the same integer. -/
theorem freeze_in_place_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' h₁ : Heap}
    {r x : Nat} {t T : Ty} {v v' k e₁ : Expr} (hh : HeapOk fns σ h) (hv : v.IsValue)
    (ht : HasType fns σ .none false Ctx.empty v t) (hfz : t.Freezable r)
    (hk : HasType fns σ .none false (Ctx.empty.update x (t.free r .immutable)) k T)
    (hc : Copy r t h v h' v')
    (hdead : ∀ a s, σ[a]? = some (.region r, s) → ¬ k.Reaches h' a ∧ ¬ v'.Reaches h' a)
    (hs : Steps fns h' (k.subst x v') h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps fns h (k.subst x v) h₂ e₂ ∧ Rel ρ e₁ e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n := by
  obtain ⟨σ', hx, hh', hv', ht', hnew⟩ := hc.typed hh hv ht hfz
  obtain ⟨π, hπ, hrel⟩ := hc.map hh ⟨[], by simp⟩ ht hfz
  -- What can still be reached: from `k`, or from the copy.
  let L : Nat → Prop := fun a => k.Reaches h' a ∨ v'.Reaches h' a
  let ρ : Nat → Nat → Prop := fun a b => (a = b ∧ a < h.length ∧ L a) ∨ π a b
  have hlen : h.length ≤ h'.length := by
    obtain ⟨e, rfl⟩ := hc.prefix
    simp
  obtain ⟨hrv, hcon⟩ := hrel ρ h' ⟨[], by simp⟩ (fun c o h => .inr h)
    (fun l hl hr => .inl ⟨rfl, hl, .inr hr⟩)
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
  have hcorr : Corr M ρ h' h := by
    refine ⟨?_, ?_, ?_⟩
    · intro a b hab
      rcases hab with ⟨rfl, ha, hLa⟩ | hab
      · obtain ⟨w, hw⟩ : ∃ w, h[a]? = some w := ⟨_, List.getElem?_eq_getElem ha⟩
        have hw' : h'[a]? = some w := by rw [hc.keeps ha]; exact hw
        refine ⟨w, w, hw', hw, Rel.refl fun l hl => .inl ⟨rfl, ?_, hL a w l hLa hw' hl⟩⟩
        obtain ⟨p, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem (hh.1 ▸ ha)⟩
        obtain ⟨_, _, w₀, hw₀, _, hwt⟩ := hh.2 a p.1 p.2 hp
        rw [hw] at hw₀
        cases hw₀
        rw [hh.1]
        exact hwt.hasLoc_lt hl
      · obtain ⟨w, w', hw, hw', hr⟩ := hcon a b hab
        exact ⟨w', w, hw', hw, hr⟩
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
  obtain ⟨_, ρ', h₂, e₂, ss, _, hr'⟩ :=
    steps_sim hf hs hh' hte (fun a m t' h hm => ⟨m, t', h, hm⟩) hcorr (hrk.subst hrv x)
  exact ⟨ρ', h₂, e₂, ss, hr', fun n hn => (hn ▸ hr').int_eq⟩

/-! ## The premise, from the typing

A value can only reach an object of a region if its type mentions the
region: a cell holds what its type says, and a closure captures only what
mentions its own region, which its type does (rule 5 of the regions page,
`HasType.lam`). What comes after a block was typed before the block made
its region, so its type doesn't mention it; and the copy's type is the
value's with the region replaced with `imm`. So neither can reach the
region's objects (`freeze_in_place_of_typing`). -/

/-- A value whose type doesn't mention `r` holds only locations whose types
don't. -/
theorem Expr.IsValue.hasLoc_notMentions {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx}
    {v : Expr} {s : Ty} (hv : v.IsValue) (ht : HasType fns σ Δ io Γ v s) {r : Nat} (hs : s.mentions r = false)
    {l : Nat} (hl : v.HasLoc l) : ∃ m t, σ[l]? = some (m, t) ∧ (Ty.cell m t).mentions r = false := by
  induction hv generalizing s with
  | unit | int => exact hl.elim
  | loc => cases ht with
    | loc h _ =>
      cases hl
      exact ⟨_, _, h, hs⟩
  | @lam b x body => cases ht with
    | @lam _ _ _ _ _ _ _ _ μ _ _ _ _ hrl _ =>
      have hl' : body.HasLoc l := hl
      obtain ⟨m, t, h, _, hm, htr⟩ := hrl l hl'
      have hμ : μ.mentions r = false := by
        simp only [Ty.mentions, Bool.or_eq_false_iff] at hs
        exact hs.1.1
      refine ⟨m, t, h, ?_⟩
      simp only [Ty.mentions, Bool.or_eq_false_iff]
      constructor
      · cases e : m.mentions r
        · rfl
        · rw [hm r e] at hμ; cases hμ
      · cases e : t.mentions r
        · rfl
        · rw [htr r e] at hμ; cases hμ
  | pair _ _ ih₁ ih₂ => cases ht with
    | pair h₁ h₂ =>
      simp only [Ty.mentions, Bool.or_eq_false_iff] at hs
      exact hl.elim (ih₁ h₁ hs.1) (ih₂ h₂ hs.2)

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

/-- Replacing `r` with what mentions no region leaves no `r`. -/
theorem Mu.free_mentions_self {m μ : Mu} (hμ : μ.NoRegion) (r : Nat) : (m.free r μ).mentions r = false := by
  cases m with
  | region r' =>
    simp only [Mu.free]
    split
    · exact hμ.mentions r
    · rename_i hne
      simp [Mu.mentions, hne]
  | _ => rfl

theorem Ty.free_mentions_self {t : Ty} {μ : Mu} (hμ : μ.NoRegion) (r : Nat) : (t.free r μ).mentions r = false := by
  induction t with
  | unit | int | tparam => rfl
  | cell m t ih => simp [Ty.free, Ty.mentions, Mu.free_mentions_self hμ r, ih]
  | fn m a b iha ihb => simp [Ty.free, Ty.mentions, Mu.free_mentions_self hμ r, iha, ihb]
  | prod a b iha ihb => simp [Ty.free, Ty.mentions, iha, ihb]

/-- **The typing makes freezing in place unnoticeable.** A block `e`
starts in a heap whose typing `σ₀` mentions its region `r` nowhere, and
runs to the value `v`, whose freeze copies it to `v'`. What comes after
it, `k`, was typed before it. Then going on with `v` itself, in the heap
without the copies, computes what going on with the copy does. -/
theorem freeze_in_place_of_typing {fns : List Fn} (hf : FnsOk fns) {σ₀ : HeapTy} {h₀ h h' h₁ : Heap}
    {r x : Nat} {t T : Ty} {e v v' k e₁ : Expr} (hh₀ : HeapOk fns σ₀ h₀)
    (hfresh : ∀ (l : Nat) (p : Mu × Ty), σ₀[l]? = some p → (Ty.cell p.1 p.2).mentions r = false)
    (he : HasType fns σ₀ .none false Ctx.empty e t) (hfz : t.Freezable r)
    (hk : HasType fns σ₀ .none false (Ctx.empty.update x (t.free r .immutable)) k T)
    (hse : Steps fns h₀ e h v) (hv : v.IsValue) (hc : Copy r t h v h' v')
    (hs : Steps fns h' (k.subst x v') h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps fns h (k.subst x v) h₂ e₂ ∧ Rel ρ e₁ e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n := by
  obtain ⟨σ, hx, hh, hvt⟩ := steps_preservation hf hse hh₀ he
  obtain ⟨σ', hx', hh', hv', ht', _⟩ := hc.typed hh hv hvt hfz
  refine freeze_in_place_changes_nothing hf hh hv hvt hfz (hk.weaken_heap hx) hc ?_ hs
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
