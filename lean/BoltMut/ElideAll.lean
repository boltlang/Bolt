import BoltMut.Elision

/-!
# Leaving out every freeze's copy (#292)

`freeze_in_place_changes_nothing` leaves out one freeze's copy. Here, all
of them: the program that freezes in place is the same program with every
freeze erased into `keep` (`Expr.erase`, `Fn.erase`), which makes the same
copy but goes on with the value itself. It computes what the program that
copies does (`freezes_in_place_change_nothing`). Making the copy anyway
keeps both heaps as long, so that blocks that start later get the same
regions in both; that the copy, which nothing uses, needn't be made is
the usual fact that garbage can't be noticed, which isn't proven here.

The premise is that when a block ends, nothing that's still to run can
reach its region's objects: what's around a freeze has only locations
whose types don't mention the region (`Expr.Sep`). That has to hold
whenever the program reaches a freeze. Showing that it does, from the
typing, needs a fresh region each time a block runs (the rest of #292).

The proof is `steps_sim` of `Sharing.lean`, with two changes. The
correspondence relates the copying program, erased, to the other. And
at each freeze, it gives up the originals, which nothing can reach any
more, and relates each copy to what its original corresponded to: so it
can shrink, keeping only what the code around the freeze can reach.
-/

namespace BoltMut

/-! ## Erasing freezes -/

/-- `e` with every freeze in place: it makes the same copy, but goes on
with the value itself. -/
def Expr.erase : Expr → Expr
  | .var x => .var x
  | .unit => .unit
  | .int n => .int n
  | .loc l => .loc l
  | .new b e => .new b e.erase
  | .get e => .get e.erase
  | .set a b => .set a.erase b.erase
  | .letE x a b => .letE x a.erase b.erase
  | .call f ms τs e => .call f ms τs e.erase
  | .lam b x e => .lam b x e.erase
  | .app a b => .app a.erase b.erase
  | .pair a b => .pair a.erase b.erase
  | .fst e => .fst e.erase
  | .snd e => .snd e.erase
  | .freeze r t e => .keep r t e.erase
  | .keep r t e => .keep r t e.erase
  | .block k t e => .block k t e.erase

/-- A function whose freezes copy nothing. -/
def Fn.erase (fn : Fn) : Fn := { fn with body := fn.body.erase }

theorem Expr.erase_subst (e : Expr) (x : Nat) (v : Expr) : (e.subst x v).erase = e.erase.subst x v.erase := by
  induction e with
  | var y => simp only [Expr.subst, Expr.erase]; split <;> simp [Expr.erase]
  | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih | block _ _ _ ih =>
    simp only [Expr.subst, Expr.erase, ih]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => simp only [Expr.subst, Expr.erase, ih₁, ih₂]
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst, Expr.erase, ih₁]
    split <;> simp [ih₂]
  | lam b y _ ih =>
    simp only [Expr.subst, Expr.erase]
    split <;> simp [ih]

theorem Expr.erase_inst (e : Expr) (ms : List Mu) (τs : List Ty) : (e.inst ms τs).erase = e.erase.inst ms τs := by
  induction e with
  | var | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih =>
    simp only [Expr.inst, Expr.erase, ih]
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.inst, Expr.erase, ih₁, ih₂]
  | freeze _ _ _ ih | keep _ _ _ ih | block _ _ _ ih => simp only [Expr.inst, Expr.erase, ih]

theorem Expr.IsValue.erase {v : Expr} (hv : v.IsValue) : v.erase.IsValue := by
  induction hv with
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam => exact .lam
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂

theorem Expr.hasLoc_erase (e : Expr) (l : Nat) : e.erase.HasLoc l ↔ e.HasLoc l := by
  induction e with
  | var | unit | int | loc => simp [Expr.erase, Expr.HasLoc]
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih | block _ _ _ ih =>
    simp only [Expr.erase, Expr.HasLoc, ih]
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.erase, Expr.HasLoc, ih₁, ih₂]

theorem Expr.NoLocs.erase {e : Expr} (h : e.NoLocs) : e.erase.NoLocs := by
  induction e with
  | var | unit | int => trivial
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih | block _ _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩

/-- A relation of expressions that only needs the locations of the first
to correspond. -/
theorem Rel.mono_on {ρ ρ' : Nat → Nat → Prop} {e₁ e₂ : Expr} (h : Rel ρ e₁ e₂)
    (hρ : ∀ a c, e₁.HasLoc a → ρ a c → ρ' a c) : Rel ρ' e₁ e₂ := by
  induction h with
  | var => exact .var
  | unit => exact .unit
  | int => exact .int
  | loc h => exact .loc (hρ _ _ rfl h)
  | new _ ih => exact .new (ih hρ)
  | get _ ih => exact .get (ih hρ)
  | call _ ih => exact .call (ih hρ)
  | lam _ ih => exact .lam (ih hρ)
  | fst _ ih => exact .fst (ih hρ)
  | snd _ ih => exact .snd (ih hρ)
  | freeze _ ih => exact .freeze (ih hρ)
  | keep _ ih => exact .keep (ih hρ)
  | block _ ih => exact .block (ih hρ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))
  | app _ _ ih₁ ih₂ => exact .app (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))

/-! ## What's around a freeze -/

/-- `e` has a freeze of the region `r`, maybe in a closure. -/
def Expr.Freezes (r : Nat) : Expr → Prop
  | .var _ | .unit | .int _ | .loc _ => False
  | .new _ e | .get e | .call _ _ _ e | .lam _ _ e | .fst e | .snd e | .block _ _ e => e.Freezes r
  | .set a b | .letE _ a b | .app a b | .pair a b => a.Freezes r ∨ b.Freezes r
  | .freeze r' _ e | .keep r' _ e => r' = r ∨ e.Freezes r

/-- `b`'s locations have types that don't mention a region that `a`
freezes. -/
def Expr.Disj (σ : HeapTy) (a b : Expr) : Prop :=
  ∀ l, b.HasLoc l → ∀ r, a.Freezes r → ∃ m t, σ[l]? = some (m, t) ∧ (Ty.cell m t).mentions r = false

/-- **What's around a freeze can't reach its region's objects.** Whatever
runs before the rest of an expression has no freeze of a region that the
rest's locations mention. -/
def Expr.Sep (σ : HeapTy) : Expr → Prop
  | .set a b | .app a b | .pair a b => Expr.Disj σ a b ∧ Expr.Disj σ b a ∧ a.Sep σ ∧ b.Sep σ
  | .letE _ a b => Expr.Disj σ a b ∧ a.Sep σ ∧ b.Sep σ
  | .new _ e | .get e | .call _ _ _ e | .fst e | .snd e | .freeze _ _ e | .keep _ _ e | .block _ _ e => e.Sep σ
  | _ => True

/-! ## A copy against the value it was made from -/

/-- Two originals aren't copied to the same place. -/
theorem Copy.memo_inj {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') :
    (∀ o c, m.lookup o = some c → c < h.length) → (∀ o o' c, m.lookup o = some c → m.lookup o' = some c → o = o') →
      (∀ o c, m'.lookup o = some c → c < h'.length) ∧ ∀ o o' c, m'.lookup o = some c → m'.lookup o' = some c → o = o' := by
  induction hc with
  | keep | seen => exact fun h₁ h₂ => ⟨h₁, h₂⟩
  | @cell _ _ h h' m m' l _ _ _ _ _ hcw ih =>
    intro hb hi
    obtain ⟨hb', hi'⟩ := ih hb hi
    refine ⟨fun o c hoc => ?_, fun o o' c h₁ h₂ => ?_⟩
    · rw [List.lookup_cons_nat] at hoc
      split at hoc
      · cases hoc; simp
      · have := hb' o c hoc; simp; omega
    · rw [List.lookup_cons_nat] at h₁ h₂
      split at h₁ <;> split at h₂
      · rename_i e₁ e₂; rw [e₁, e₂]
      · cases h₁; have := hb' _ _ h₂; omega
      · cases h₂; have := hb' _ _ h₁; omega
      · exact hi' o o' c h₁ h₂
  | pair _ _ _ ih₁ ih₂ =>
    intro hb hi
    obtain ⟨hb₁, hi₁⟩ := ih₁ hb hi
    exact ih₂ hb₁ hi₁

/-- **What a freeze's copy corresponds to, in the other program.** There,
the value `v₂` corresponds to `v` with its freezes erased, and isn't copied.
Each copy of an original `o` corresponds to what `o` corresponds to, and
what the copy shares to what it corresponds to: then the copy corresponds
to `v₂`, and each copy holds what corresponds to what `o`'s counterpart
holds. -/
theorem Copy.mapRel {fns : List Fn} {σ : HeapTy} {h₀ : Heap} (hh : HeapOk fns σ h₀) {r : Nat} {t : Ty}
    {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr} (hc : Copy r t h m v h' m' v') :
    (∃ ext, h = h₀ ++ ext) → ∀ {io : Bool} {Γ : Ctx}, HasType fns σ .none io Γ v t → t.Freezable r →
      (∀ o c, m.lookup o = some c → h₀.length ≤ c ∧ c < h.length ∧ ∃ s, σ[o]? = some (.region r, s)) →
      ∀ (ρ ρ' : Nat → Nat → Prop) (h₂ : Heap) (v₂ : Expr) (hfin : Heap), (∃ ext, hfin = h' ++ ext) →
        (∀ a c, ρ a c → ∃ u₁ u₂, (h₀.map Expr.erase)[a]? = some u₁ ∧ h₂[c]? = some u₂ ∧ Rel ρ u₁ u₂) →
        Rel ρ v.erase v₂ →
        (∀ o c c₂, m'.lookup o = some c → ρ o c₂ → ρ' c c₂) →
        (∀ a c₂, ρ a c₂ → a < h₀.length → v'.Reaches hfin a → ρ' a c₂) →
        Rel ρ' v'.erase v₂ ∧ ∀ o c c₂, m'.lookup o = some c → m.lookup o = none → ρ o c₂ →
          ∃ w' w₂, hfin[c]? = some w' ∧ h₂[c₂]? = some w₂ ∧ Rel ρ' w'.erase w₂ := by
  induction hc with
  | @keep t h m v _ =>
    intro _ io Γ ht _ _ ρ ρ' h₂ v₂ hfin _ _ hr _ hid
    refine ⟨hr.mono_on fun a c ha hac => hid a c hac ?_ ⟨a, (Expr.hasLoc_erase v a).mp ha, .refl⟩,
      fun o c _ h₁ h₂ => by rw [h₁] at h₂; cases h₂⟩
    rw [hh.1]
    exact ht.hasLoc_lt ((Expr.hasLoc_erase v a).mp ha)
  | @seen μ s h m l c _ hlk =>
    intro _ io Γ _ _ _ ρ ρ' h₂ v₂ hfin _ _ hr hρ _
    simp only [Expr.erase] at hr ⊢
    cases hr with
    | loc hlc => exact ⟨.loc (hρ l c _ hlk hlc), fun o c _ h₁ h₂ => by rw [h₁] at h₂; cases h₂⟩
  | @cell μ s h h' m m' l w w' hmen hnone hw hcw ih =>
    intro hpre io Γ ht hf hm ρ ρ' h₂ v₂ hfin hfin' hcor hr hρ hid
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
      have hnone' : m'.lookup l = none := by
        cases e : m'.lookup l with
        | none => rfl
        | some c =>
          obtain ⟨μ', s', h₁, h₂⟩ := hcw.keys_small hh hpre hwt l c e hnone
          rw [hσ] at h₁
          cases h₁
          simp only [Ty.size] at h₂
          omega
      obtain ⟨ext', rfl⟩ := hfin'
      have hc' : (h' ++ [w'] ++ ext')[h'.length]? = some w' := by simp
      have hρ' : ∀ o c c₂, m'.lookup o = some c → ρ o c₂ → ρ' c c₂ := by
        intro o c c₂ hoc hoc₂
        have : o ≠ l := by rintro rfl; rw [hnone'] at hoc; cases hoc
        apply hρ o c c₂ _ hoc₂
        rw [List.lookup_cons_nat]
        simp only [this, ite_false]
        exact hoc
      have hid' : ∀ a c₂, ρ a c₂ → a < h₀.length → w'.Reaches (h' ++ [w'] ++ ext') a → ρ' a c₂ := by
        intro a c₂ hac ha ⟨l₁, hl₁, hr⟩
        exact hid a c₂ hac ha ⟨h'.length, rfl, .step hc' hl₁ hr⟩
      -- What `l`'s counterpart holds corresponds to what `l` holds, so to
      -- what the copy holds.
      have hcontent : ∀ c₂, ρ l c₂ → ∃ w₂, h₂[c₂]? = some w₂ ∧ Rel ρ' w'.erase w₂ := by
        intro c₂ hlc
        obtain ⟨u₁, u₂, hu₁, hu₂, hru⟩ := hcor l c₂ hlc
        rw [List.getElem?_map, hw₀] at hu₁
        cases hu₁
        exact ⟨u₂, hu₂, (ih hpre hwt hf.2 hm ρ ρ' h₂ u₂ _ ⟨[w'] ++ ext', by simp⟩ hcor hru hρ' hid').1⟩
      simp only [Expr.erase] at hr ⊢
      cases hr with
      | @loc _ l₂ hlc =>
        refine ⟨.loc (hρ l h'.length l₂ (by simp) hlc), ?_⟩
        intro o c c₂ hoc hom hoc₂
        rw [List.lookup_cons_nat] at hoc
        split at hoc
        · rename_i he
          cases hoc
          subst he
          obtain ⟨w₂, hw₂, hrw⟩ := hcontent c₂ hoc₂
          exact ⟨w', w₂, hc', hw₂, hrw⟩
        · obtain ⟨u₁, u₂, hu₁, hu₂, hru⟩ := hcor l l₂ hlc
          rw [List.getElem?_map, hw₀] at hu₁
          cases hu₁
          exact (ih hpre hwt hf.2 hm ρ ρ' h₂ u₂ _ ⟨[w'] ++ ext', by simp⟩ hcor hru hρ' hid').2 o c c₂ hoc hom hoc₂
  | @pair a b h h₁ h₂ m m₁ m₂ v₁ v₂ v₁' v₂' _ hc₁ hc₂ ih₁ ih₂ =>
    intro hpre io Γ ht hf hm ρ ρ' k₂ e₂ hfin hfin' hcor hr hρ hid
    cases ht with
    | pair ht₁ ht₂ =>
      simp only [Expr.erase] at hr ⊢
      cases hr with
      | pair hr₁ hr₂ =>
        have hpre₁ : ∃ ext, h₁ = h₀ ++ ext := by
          obtain ⟨e₁, rfl⟩ := hpre
          obtain ⟨e₂, rfl⟩ := hc₁.prefix
          exact ⟨e₁ ++ e₂, by simp⟩
        have hfin₁ : ∃ ext, hfin = h₁ ++ ext := by
          obtain ⟨e₁, rfl⟩ := hfin'
          obtain ⟨e₂, rfl⟩ := hc₂.prefix
          exact ⟨e₂ ++ e₁, by simp⟩
        obtain ⟨hπ₁, _⟩ := hc₁.map hh hpre ht₁ hf.1 hm
        obtain ⟨r₁, c₁⟩ := ih₁ hpre ht₁ hf.1 hm ρ ρ' k₂ _ hfin hfin₁ hcor hr₁
          (fun o c c₂ h hoc₂ => hρ o c c₂ (hc₂.memo_mono o c h) hoc₂)
          (fun a c₂ hac ha ⟨l₁, hl₁, hr⟩ => hid a c₂ hac ha ⟨l₁, .inl hl₁, hr⟩)
        obtain ⟨r₂, c₂⟩ := ih₂ hpre₁ ht₂ hf.2 hπ₁ ρ ρ' k₂ _ hfin hfin' hcor hr₂ hρ
          (fun a c₂ hac ha ⟨l₁, hl₁, hr⟩ => hid a c₂ hac ha ⟨l₁, .inr hl₁, hr⟩)
        refine ⟨.pair r₁ r₂, fun o c c₂' hoc hom hoc₂ => ?_⟩
        cases h₁o : m₁.lookup o with
        | none => exact c₂ o c c₂' hoc h₁o hoc₂
        | some c' =>
          have e := hc₂.memo_mono o c' h₁o
          rw [hoc] at e
          obtain rfl := Option.some.inj e
          exact c₁ o c c₂' h₁o hom hoc₂

/-- The two programs' copies have copied the objects that correspond, of
those that only one corresponds to. -/
def MemoRelE (M : Nat → Prop) (ρ : Nat → Nat → Prop) (m₁ m₂ : List (Nat × Nat)) : Prop :=
  (∀ l₁ l₂, ρ l₁ l₂ → M l₁ → (m₁.lookup l₁ = none ↔ m₂.lookup l₂ = none)) ∧
  (∀ l₂ c₂, m₂.lookup l₂ = some c₂ → ∃ l₁ c₁, ρ l₁ l₂ ∧ M l₁ ∧ m₁.lookup l₁ = some c₁)

/-- **The other program makes the same copy.** It copies the objects that
correspond to those the first copies, so it adds as many to its heap: the
heaps stay as long, and the blocks that start later get the same regions.
It goes on with the value itself, so its copy isn't used. -/
theorem Copy.simE {fns : List Fn} {σ : HeapTy} {h₀ : Heap} (hh : HeapOk fns σ h₀)
    {r : Nat} {t : Ty} {h₁ h₁' : Heap} {m₁ m₁' : List (Nat × Nat)} {v₁ v₁' : Expr}
    (hcp : Copy r t h₁ m₁ v₁ h₁' m₁' v₁') :
    (∃ ext, h₁ = h₀ ++ ext) → ∀ {io : Bool} {Γ : Ctx}, HasType fns σ .none io Γ v₁ t → t.Freezable r →
    ∀ {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂₀ h₂ : Heap} {m₂ : List (Nat × Nat)} {v₂ : Expr},
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) →
      (∀ a c, ρ a c → ∃ u₁ u₂, (h₀.map Expr.erase)[a]? = some u₁ ∧ h₂₀[c]? = some u₂ ∧ Rel ρ u₁ u₂) →
      (∀ a b c, ρ a c → ρ b c → M a → a = b) → (∀ a c d, ρ a c → ρ a d → M a → c = d) →
      (∃ ext, h₂ = h₂₀ ++ ext) → Rel ρ v₁.erase v₂ → MemoRelE M ρ m₁ m₂ →
      ∃ h₂' m₂' v₂', Copy r t h₂ m₂ v₂ h₂' m₂' v₂' ∧ h₂'.length + h₁.length = h₁'.length + h₂.length ∧
        MemoRelE M ρ m₁' m₂' := by
  induction hcp with
  | keep hm =>
    intro _ io Γ _ _ M ρ h₂₀ h₂ m₂ v₂ _ _ _ _ _ _ hmr
    exact ⟨h₂, m₂, v₂, .keep hm, by omega, hmr⟩
  | @seen μ s h m l c hmen hlk =>
    intro _ io Γ ht hf M ρ h₂₀ h₂ m₂ v₂ hM _ _ _ _ hr hmr
    cases ht with
    | loc hσ _ =>
      have hreg := hf.cell_region hmen
      subst hreg
      simp only [Expr.erase] at hr
      cases hr with
      | @loc _ l₂ hlc =>
        have hn : m₂.lookup l₂ ≠ none := fun h =>
          absurd ((hmr.1 l l₂ hlc (hM l _ _ hσ (by simp))).mpr h) (by rw [hlk]; simp)
        obtain ⟨c₂, hc₂⟩ := Option.ne_none_iff_exists'.mp hn
        exact ⟨h₂, m₂, _, .seen hmen hc₂, by omega, hmr⟩
  | @cell μ s h h' m m' l w w' hmen hnone hw hcw ih =>
    intro hpre io Γ ht hf M ρ h₂₀ h₂ m₂ v₂ hM hcor hinj hfun hpre₂ hr hmr
    cases ht with
    | loc hσ _ =>
      have hreg := hf.cell_region hmen
      subst hreg
      obtain ⟨_, _, w₀, hw₀, _, hwt⟩ := hh.2 l _ s hσ
      have hl₀ : l < h₀.length := hh.lt hσ
      have hw' : w₀ = w := by
        obtain ⟨ext, rfl⟩ := hpre
        rw [List.getElem?_append_left hl₀, hw₀] at hw
        exact Option.some.inj hw
      subst hw'
      have hMl : M l := hM l _ _ hσ (by simp)
      simp only [Expr.erase] at hr
      cases hr with
      | @loc _ l₂ hlc =>
        have hnone₂ : m₂.lookup l₂ = none := (hmr.1 l l₂ hlc hMl).mp hnone
        obtain ⟨u₁, u₂, hu₁, hu₂, hru⟩ := hcor l l₂ hlc
        rw [List.getElem?_map, hw₀] at hu₁
        cases hu₁
        have hu₂' : h₂[l₂]? = some u₂ := by
          obtain ⟨e, rfl⟩ := hpre₂
          rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp hu₂).1, hu₂]
        obtain ⟨h₂c, m₂c, u₂', hc₂, hlen, hmr'⟩ := ih hpre hwt hf.2 hM hcor hinj hfun hpre₂ hru hmr
        refine ⟨h₂c ++ [u₂'], (l₂, h₂c.length) :: m₂c, .loc h₂c.length, .cell hmen hnone₂ hu₂' hc₂,
          by simp; omega, ⟨?_, ?_⟩⟩
        · intro k₁ k₂ hk hMk
          rw [List.lookup_cons_nat, List.lookup_cons_nat]
          by_cases hkl : k₁ = l
          · subst hkl
            have : k₂ = l₂ := hfun k₁ k₂ l₂ hk hlc hMk
            subst this
            simp
          · have hk₂ : k₂ ≠ l₂ := fun he => hkl (hinj k₁ l k₂ hk (he ▸ hlc) hMk)
            simp only [hkl, hk₂, ite_false]
            exact hmr'.1 k₁ k₂ hk hMk
        · intro k₂ c₂ hk
          rw [List.lookup_cons_nat] at hk
          split at hk
          · rename_i he
            subst he
            exact ⟨l, h'.length, hlc, hMl, by simp⟩
          · rename_i hne
            obtain ⟨k₁, c₁, hk₁, hMk, hl₁⟩ := hmr'.2 k₂ c₂ hk
            have hkl : k₁ ≠ l := by
              rintro rfl
              exact hne (hfun k₁ k₂ l₂ hk₁ hlc hMk)
            exact ⟨k₁, c₁, hk₁, hMk, by simp [List.lookup_cons_nat, hkl, hl₁]⟩
  | @pair a b h h₁ h₂ m m₁ m₂ _ _ _ _ hm hc₁ hc₂ ih₁ ih₂ =>
    intro hpre io Γ ht hf M ρ h₂₀ k₂ mm₂ e₂ hM hcor hinj hfun hpre₂ hr hmr
    cases ht with
    | pair ht₁ ht₂ =>
      simp only [Expr.erase] at hr
      cases hr with
      | pair hr₁ hr₂ =>
        have hpre₁ : ∃ ext, h₁ = h₀ ++ ext := by
          obtain ⟨e₁, rfl⟩ := hpre
          obtain ⟨e₂, rfl⟩ := hc₁.prefix
          exact ⟨e₁ ++ e₂, by simp⟩
        obtain ⟨k₂₁, n₂₁, u₁, hk₁, hlen₁, hmr₁⟩ := ih₁ hpre ht₁ hf.1 hM hcor hinj hfun hpre₂ hr₁ hmr
        have hpre₂₁ : ∃ ext, k₂₁ = h₂₀ ++ ext := by
          obtain ⟨e₁, rfl⟩ := hpre₂
          obtain ⟨e₂, rfl⟩ := hk₁.prefix
          exact ⟨e₁ ++ e₂, by simp⟩
        obtain ⟨k₂₂, n₂₂, u₂, hk₂, hlen₂, hmr₂⟩ := ih₂ hpre₁ ht₂ hf.2 hM hcor hinj hfun hpre₂₁ hr₂ hmr₁
        exact ⟨k₂₂, n₂₂, _, .pair hm hk₁ hk₂, by omega, hmr₂⟩

/-! ## One step -/

/-- The context's locations, with those of `b`, don't mention what `a`
freezes. -/
theorem Expr.Disj.ctx {σ : HeapTy} {K : Nat → Prop} {a b e : Expr}
    (hK : ∀ l, K l → ∀ r, e.Freezes r → ∃ m t, σ[l]? = some (m, t) ∧ (Ty.cell m t).mentions r = false)
    (hae : ∀ r, a.Freezes r → e.Freezes r) (hd : Expr.Disj σ a b) :
    ∀ l, (K l ∨ b.HasLoc l) → ∀ r, a.Freezes r → ∃ m t, σ[l]? = some (m, t) ∧ (Ty.cell m t).mentions r = false := by
  intro l hl r hr
  rcases hl with hl | hl
  · exact hK l hl r (hae r hr)
  · exact hd l hl r hr

/-- One step of the program that copies is one of the one that doesn't,
and they still correspond, on everything that the context `K` reaches. -/
theorem step_elide {fns : List Fn} (hf : FnsOk fns) {h₁ h₁' : Heap} {e₁ e₁' : Expr}
    (hs : Step fns h₁ e₁ h₁' e₁') :
    ∀ {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {K : Nat → Prop} {h₂ : Heap} {e₂ : Expr},
      HeapOk fns σ h₁ → HasType fns σ .none false Ctx.empty e₁ t →
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) → Corr M ρ (h₁.map Expr.erase) h₂ →
      Rel ρ e₁.erase e₂ → e₁.Sep σ →
      (∀ l, K l → ∀ r, e₁.Freezes r → ∃ m t', σ[l]? = some (m, t') ∧ (Ty.cell m t').mentions r = false) →
      h₂.length = h₁.length →
      ∃ ρ' h₂' e₂', Step (fns.map Fn.erase) h₂ e₂ h₂' e₂' ∧ (∀ a c, ρ a c → K a → ρ' a c) ∧
        Corr (fun a => M a ∨ h₁.length ≤ a) ρ' (h₁'.map Expr.erase) h₂' ∧ Rel ρ' e₁'.erase e₂' ∧
        h₂'.length = h₁'.length := by
  induction hs with
  | newCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | new he _ | newR he _ =>
      cases hr with
      | new hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he hM hc hr hsep hK hlen
        exact ⟨ρ', h₂', _, .newCong s₂, hρ, hc', .new hr', hl'⟩
  | @newVal e b hv =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | @new _ _ e' hr =>
      have hc' := hc.alloc hr
      simp only [List.length_map] at hc'
      refine ⟨fun a c => ρ a c ∨ (a = h₁.length ∧ c = h₂.length), h₂ ++ [e'], .loc h₂.length,
        .newVal (hr.isValue hv.erase), fun a c h _ => Or.inl h, ?_, .loc (Or.inr ⟨rfl, rfl⟩), by simp [hlen]⟩
      simp only [List.map_append, List.map_cons, List.map_nil]
      exact hc'
  | getCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | get he =>
      cases hr with
      | get hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he hM hc hr hsep hK hlen
        exact ⟨ρ', h₂', _, .getCong s₂, hρ, hc', .get hr', hl'⟩
  | @getLoc l v hl =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | get hr =>
      cases hr with
      | loc hρ =>
        obtain ⟨v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap _ _ hρ
        rw [List.getElem?_map, hl] at h₁v
        cases h₁v
        exact ⟨ρ, h₂, v₂, .getLoc h₂v, fun _ _ h _ => h, (by simpa only [List.length_map] using hc.widen), hrel, hlen⟩
  | setCong₁ _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set he₁ _ | setR he₁ _ =>
      cases hr with
      | set hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₁ hM hc hr₁ hsep.2.2.1
          (hsep.1.ctx hK fun r h => Or.inl h) hlen
        refine ⟨ρ', h₂', _, .setCong₁ s₂, fun a c h hk => hρ a c h (.inl hk), hc', .set hr' ?_, hl'⟩
        exact hr₂.mono_on fun a c ha h => hρ a c h (.inr ((Expr.hasLoc_erase _ a).mp ha))
  | setCong₂ hv _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set _ he₂ | setR _ he₂ =>
      cases hr with
      | set hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₂ hM hc hr₂ hsep.2.2.2
          (hsep.2.1.ctx hK fun r h => Or.inr h) hlen
        refine ⟨ρ', h₂', _, .setCong₂ (hr₁.isValue hv.erase) s₂, fun a c h hk => hρ a c h (.inl hk), hc', .set ?_ hr', hl'⟩
        exact hr₁.mono_on fun c d hc h => hρ c d h (.inr ((Expr.hasLoc_erase _ c).mp hc))
  | @setLoc l₀ v hl hv =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set he₁ _ | setR he₁ _ =>
      cases he₁ with
      | loc hσ _ =>
        have hMl : M l₀ := hM _ _ _ hσ (by intro h; cases h)
        cases hr with
        | @set _ _ _ v' hr₁ hr₂ =>
          cases hr₁ with
          | @loc _ c hρ =>
            have hc₂ : c < h₂.length := hc.lt₂ hρ
            have hl' : l₀ < (h₁.map Expr.erase).length := by simpa using hl
            refine ⟨ρ, h₂.set c v', .unit, .setLoc hc₂ (hr₂.isValue hv.erase), fun _ _ h _ => h, ?_, .unit,
              by simp [hlen]⟩
            have key : Corr M ρ ((h₁.map Expr.erase).set l₀ v.erase) (h₂.set c v') := by
              refine ⟨?_, hc.inj, hc.func⟩
              intro a d had
              obtain ⟨v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap a d had
              by_cases ha : a = l₀
              · subst ha
                have hd : d = c := hc.func _ _ _ had hρ hMl
                subst hd
                exact ⟨v.erase, v', List.getElem?_set_self hl', List.getElem?_set_self hc₂, hr₂⟩
              · have hd : d ≠ c := fun hd => ha (hc.inj l₀ a c hρ (hd ▸ had) hMl).symm
                refine ⟨v₁, v₂, ?_, ?_, hrel⟩
                · rw [List.getElem?_set_ne (Ne.symm ha)]
                  exact h₁v
                · rw [List.getElem?_set_ne (Ne.symm hd)]
                  exact h₂v
            have := key.widen
            simp only [List.length_set, List.length_map] at this
            rw [List.map_set]
            exact this
  | letCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | letE he₁ _ =>
      cases hr with
      | letE hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₁ hM hc hr₁ hsep.2.1
          (hsep.1.ctx hK fun r h => Or.inl h) hlen
        refine ⟨ρ', h₂', _, .letCong s₂, fun a c h hk => hρ a c h (.inl hk), hc', .letE hr' ?_, hl'⟩
        exact hr₂.mono_on fun a c ha h => hρ a c h (.inr ((Expr.hasLoc_erase _ a).mp ha))
  | @letVal x v _ hv =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | letE hr₁ hr₂ =>
      refine ⟨ρ, h₂, _, .letVal (hr₁.isValue hv.erase), fun _ _ h _ => h, (by simpa only [List.length_map] using hc.widen), ?_, hlen⟩
      rw [Expr.erase_subst]
      exact hr₂.subst hr₁ x
  | callCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | call _ _ he =>
      cases hr with
      | call hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he hM hc hr hsep hK hlen
        exact ⟨ρ', h₂', _, .callCong s₂, hρ, hc', .call hr', hl'⟩
  | @callVal f fn ms τs v hfn hv =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | call hr =>
      -- The other program calls the function with its freezes erased.
      have hfn' : (fns.map Fn.erase)[f]? = some fn.erase := by rw [List.getElem?_map, hfn]; rfl
      have hb := (((hf fn (List.mem_of_getElem? hfn)).1).erase).inst ms τs
      have hbody : Rel ρ (fn.body.erase.inst ms τs) (fn.body.erase.inst ms τs) :=
        Rel.refl fun l hl => (hb.not_hasLoc l hl).elim
      refine ⟨ρ, h₂, _, .callVal hfn' (hr.isValue hv.erase), fun _ _ h _ => h, (by simpa only [List.length_map] using hc.widen), ?_, hlen⟩
      rw [Expr.erase_subst, Expr.erase_inst]
      exact hbody.subst hr fn.param
  | appCong₁ _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | app he₁ _ =>
      cases hr with
      | app hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₁ hM hc hr₁ hsep.2.2.1
          (hsep.1.ctx hK fun r h => Or.inl h) hlen
        refine ⟨ρ', h₂', _, .appCong₁ s₂, fun a c h hk => hρ a c h (.inl hk), hc', .app hr' ?_, hl'⟩
        exact hr₂.mono_on fun a c ha h => hρ a c h (.inr ((Expr.hasLoc_erase _ a).mp ha))
  | appCong₂ hv _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | app _ he₂ =>
      cases hr with
      | app hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₂ hM hc hr₂ hsep.2.2.2
          (hsep.2.1.ctx hK fun r h => Or.inr h) hlen
        refine ⟨ρ', h₂', _, .appCong₂ (hr₁.isValue hv.erase) s₂, fun a c h hk => hρ a c h (.inl hk), hc', .app ?_ hr', hl'⟩
        exact hr₁.mono_on fun c d hc h => hρ c d h (.inr ((Expr.hasLoc_erase _ c).mp hc))
  | @appVal b x body v hv =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | app hr₁ hr₂ =>
      cases hr₁ with
      | lam hbody =>
        refine ⟨ρ, h₂, _, .appVal (hr₂.isValue hv.erase), fun _ _ h _ => h, (by simpa only [List.length_map] using hc.widen), ?_, hlen⟩
        rw [Expr.erase_subst]
        exact hbody.subst hr₂ x
  | pairCong₁ _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | pair he₁ _ =>
      cases hr with
      | pair hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₁ hM hc hr₁ hsep.2.2.1
          (hsep.1.ctx hK fun r h => Or.inl h) hlen
        refine ⟨ρ', h₂', _, .pairCong₁ s₂, fun a c h hk => hρ a c h (.inl hk), hc', .pair hr' ?_, hl'⟩
        exact hr₂.mono_on fun a c ha h => hρ a c h (.inr ((Expr.hasLoc_erase _ a).mp ha))
  | pairCong₂ hv _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | pair _ he₂ =>
      cases hr with
      | pair hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he₂ hM hc hr₂ hsep.2.2.2
          (hsep.2.1.ctx hK fun r h => Or.inr h) hlen
        refine ⟨ρ', h₂', _, .pairCong₂ (hr₁.isValue hv.erase) s₂, fun a c h hk => hρ a c h (.inl hk), hc', .pair ?_ hr', hl'⟩
        exact hr₁.mono_on fun c d hc h => hρ c d h (.inr ((Expr.hasLoc_erase _ c).mp hc))
  | fstCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | fst he =>
      cases hr with
      | fst hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he hM hc hr hsep hK hlen
        exact ⟨ρ', h₂', _, .fstCong s₂, hρ, hc', .fst hr', hl'⟩
  | sndCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | snd he =>
      cases hr with
      | snd hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he hM hc hr hsep hK hlen
        exact ⟨ρ', h₂', _, .sndCong s₂, hρ, hc', .snd hr', hl'⟩
  | fstVal hv₁ hv₂ =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | fst hr =>
      cases hr with
      | pair hr₁ hr₂ =>
        exact ⟨ρ, h₂, _, .fstVal (hr₁.isValue hv₁.erase) (hr₂.isValue hv₂.erase), fun _ _ h _ => h, (by simpa only [List.length_map] using hc.widen), hr₁, hlen⟩
  | sndVal hv₁ hv₂ =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr _ _ hlen
    simp only [Expr.erase] at hr
    cases hr with
    | snd hr =>
      cases hr with
      | pair hr₁ hr₂ =>
        exact ⟨ρ, h₂, _, .sndVal (hr₁.isValue hv₁.erase) (hr₂.isValue hv₂.erase), fun _ _ h _ => h, (by simpa only [List.length_map] using hc.widen), hr₂, hlen⟩
  | freezeCong _ ih =>
    intro σ t M ρ K h₂ e₂ hh ht hM hc hr hsep hK hlen
    simp only [Expr.erase] at hr
    cases ht with
    | freeze he _ _ =>
      cases hr with
      | keep hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr', hl'⟩ := ih hh he hM hc hr hsep
          (fun l hl r' hr' => hK l hl r' (.inr hr')) hlen
        exact ⟨ρ', h₂', _, .keepCong s₂, hρ, hc', .keep hr', hl'⟩
  | @freezeVal h₁' r t m v v' hv hcp =>
    intro σ T M ρ K h₂ e₂ hh ht hM hc hr _ hK hlen
    cases ht with
    | freeze he hf _ =>
      simp only [Expr.erase] at hr
      cases hr with
      | @keep _ _ _ v₂ hr =>
        obtain ⟨σ', hx, hh', hv', ht', _, _⟩ := hcp.typed hh hv he hf (MemoOk.nil σ r)
        -- The other program makes the same copy, but goes on with `v₂`.
        obtain ⟨h₂', m₂, v₂', hc₂, hlen₂, _⟩ := hcp.simE hh ⟨[], by simp⟩ he hf (h₂₀ := h₂) (h₂ := h₂) (m₂ := [])
          hM hc.heap hc.inj hc.func ⟨[], by simp⟩ hr ⟨fun _ _ _ _ => by simp, fun _ _ h => by simp at h⟩
        obtain ⟨hπ, _⟩ := hcp.map hh ⟨[], by simp⟩ he hf (fun _ _ h => by simp at h)
        obtain ⟨_, hinj⟩ := hcp.memo_inj (fun _ _ h => by simp at h) (fun _ _ _ h => by simp at h)
        -- What can still be reached: from the context, or from the copy.
        let Live : Nat → Prop := fun a => ∃ l, (K l ∨ v'.HasLoc l) ∧ Reaches h₁' l a
        let ρ' : Nat → Nat → Prop := fun a c => (ρ a c ∧ Live a) ∨ ∃ o, m.lookup o = some a ∧ ρ o c
        -- No object of the region can be reached any more.
        have horig : ∀ o s, σ[o]? = some (.region r, s) → ¬ Live o := by
          intro o s ho ⟨l, hl, hrl⟩
          have hbad : ∀ m' t', σ'[o]? = some (m', t') → (Ty.cell m' t').mentions r = false → False := by
            intro m' t' ho' hm'
            rw [hx o _ ho] at ho'
            cases ho'
            simp [Ty.mentions, Mu.mentions] at hm'
          rcases hl with hl | hl
          · obtain ⟨m₀, t₀, hσl, hm₀⟩ := hK l hl r (.inl rfl)
            obtain ⟨m', t', ho', hm'⟩ := hh'.reaches_notMentions hrl (hx l _ hσl) hm₀
            exact hbad m' t' ho' hm'
          · obtain ⟨m₀, t₀, hσl, hm₀⟩ := hv'.hasLoc_notMentions ht' (Ty.free_mentions_self (μ := .immutable) trivial r) hl
            obtain ⟨m', t', ho', hm'⟩ := hh'.reaches_notMentions hrl hσl hm₀
            exact hbad m' t' ho' hm'
        have hMo : ∀ o s, σ[o]? = some (.region r, s) → M o := fun o s ho => hM o _ _ ho (by simp)
        have hlen : h₁.length ≤ h₁'.length := by
          obtain ⟨e, rfl⟩ := hcp.prefix
          simp
        -- Live old locations hold what's live.
        have hlive : ∀ a w l, Live a → h₁'[a]? = some w → w.HasLoc l → Live l := by
          intro a w l ⟨l₀, hl₀, hr₀⟩ hw hl
          exact ⟨l₀, hl₀, hr₀.trans (.step hw hl .refl)⟩
        obtain ⟨hrv, hcon⟩ := hcp.mapRel hh ⟨[], by simp⟩ he hf (fun _ _ h => by simp at h) ρ ρ' h₂ v₂ h₁'
          ⟨[], by simp⟩ hc.heap hr (fun o c c₂ hoc hoc₂ => .inr ⟨o, hoc, hoc₂⟩)
          (fun a c₂ hac _ ⟨l, hl, hrl⟩ => .inl ⟨hac, l, .inr hl, hrl⟩)
        -- The other program's copy only adds to its heap.
        have hk₂ : ∀ c, c < h₂.length → h₂'[c]? = h₂[c]? := fun c hc' => hc₂.keeps hc'
        refine ⟨ρ', h₂', v₂, .keepVal (hr.isValue hv.erase) hc₂,
          fun a c hac hKa => .inl ⟨hac, a, .inl hKa, .refl⟩, ⟨?_, ?_, ?_⟩, hrv, by omega⟩
        · intro a c hac
          rcases hac with ⟨hac, hLa⟩ | ⟨o, hoa, hoc⟩
          · obtain ⟨u₁, u₂, hu₁, hu₂, hru⟩ := hc.heap a c hac
            have ha : a < h₁.length := by simpa using hc.lt₁ hac
            rw [List.getElem?_map] at hu₁
            obtain ⟨w, hw, rfl⟩ := Option.map_eq_some_iff.mp hu₁
            have hw' : h₁'[a]? = some w := by rw [hcp.keeps ha]; exact hw
            refine ⟨w.erase, u₂, by rw [List.getElem?_map, hw']; rfl, by rw [hk₂ c (hc.lt₂ hac)]; exact hu₂, ?_⟩
            exact hru.mono_on fun l c' hl hlc => .inl ⟨hlc, hlive a w l hLa hw' ((Expr.hasLoc_erase w l).mp hl)⟩
          · obtain ⟨w', w₂, hw', hw₂, hrw⟩ := hcon o a c hoa (by simp) hoc
            exact ⟨w'.erase, w₂, by rw [List.getElem?_map, hw']; rfl, by rw [hk₂ c (hc.lt₂ hoc)]; exact hw₂, hrw⟩
        · -- A copy is of one original, and no original is live, so only
          -- one location corresponds to each.
          intro a b c hac hbc hMa
          have hMa' : a < h₁.length → M a := by
            intro ha
            rcases hMa with hMa | hMa
            · exact hMa
            · omega
          rcases hac with ⟨hac, hLa⟩ | ⟨o, hoa, hoc⟩ <;> rcases hbc with ⟨hbc, hLb⟩ | ⟨o', hob, hoc'⟩
          · have ha : a < h₁.length := by simpa using hc.lt₁ hac
            exact hc.inj a b c hac hbc (hMa' ha)
          · have ha : a < h₁.length := by simpa using hc.lt₁ hac
            obtain ⟨_, _, s', hs'⟩ := hπ o' b hob
            have := hc.inj a o' c hac hoc' (hMa' ha)
            subst this
            exact absurd hLa (horig _ s' hs')
          · obtain ⟨_, _, s', hs'⟩ := hπ o a hoa
            have := hc.inj o b c hoc hbc (hMo o s' hs')
            subst this
            exact absurd hLb (horig _ s' hs')
          · obtain ⟨_, _, s', hs'⟩ := hπ o a hoa
            have := hc.inj o o' c hoc hoc' (hMo o s' hs')
            subst this
            rw [hoa] at hob
            exact Option.some.inj hob
        · intro a c d hac had hMa
          have hMa' : a < h₁.length → M a := by
            intro ha
            rcases hMa with hMa | hMa
            · exact hMa
            · omega
          rcases hac with ⟨hac, _⟩ | ⟨o, hoa, hoc⟩ <;> rcases had with ⟨had, _⟩ | ⟨o', hoa', hod⟩
          · have ha : a < h₁.length := by simpa using hc.lt₁ hac
            exact hc.func a c d hac had (hMa' ha)
          · have ha : a < h₁.length := by simpa using hc.lt₁ hac
            have := (hπ o' a hoa').1
            omega
          · have ha : a < h₁.length := by simpa using hc.lt₁ had
            have := (hπ o a hoa).1
            omega
          · have := hinj o o' a hoa hoa'
            subst this
            obtain ⟨_, _, s', hs'⟩ := hπ o a hoa
            exact hc.func o c d hoc hod (hMo o s' hs')
  | blockEnter => intro σ t M ρ K h₂ e₂ hh ht; cases ht
  | keepCong _ _ => intro σ t M ρ K h₂ e₂ hh ht; cases ht
  | keepVal _ _ => intro σ t M ρ K h₂ e₂ hh ht; cases ht

/-! ## The whole run -/

/-- However many steps the program that copies takes, the one that doesn't
takes them too, and they still correspond, if what's around each freeze
can't reach its region's objects whenever the program gets there. -/
theorem steps_elide {fns : List Fn} (hf : FnsOk fns) {h₁ h₁' : Heap} {e₁ e₁' : Expr}
    (hs : Steps fns h₁ e₁ h₁' e₁') :
    ∀ {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {e₂ : Expr},
      HeapOk fns σ h₁ → HasType fns σ .none false Ctx.empty e₁ t →
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) → Corr M ρ (h₁.map Expr.erase) h₂ →
      Rel ρ e₁.erase e₂ →
      (∀ σ' h e, Steps fns h₁ e₁ h e → HeapOk fns σ' h → HasType fns σ' .none false Ctx.empty e t → e.Sep σ') →
      h₂.length = h₁.length →
      ∃ (M' : Nat → Prop) (ρ' : Nat → Nat → Prop) (h₂' : Heap) (e₂' : Expr),
        Steps (fns.map Fn.erase) h₂ e₂ h₂' e₂' ∧ Corr M' ρ' (h₁'.map Expr.erase) h₂' ∧ Rel ρ' e₁'.erase e₂' := by
  induction hs with
  | refl =>
    intro σ t M ρ h₂ e₂ _ _ _ hc hr _ _
    exact ⟨M, ρ, h₂, e₂, .refl, hc, hr⟩
  | @step h₀ _ _ _ _ _ s _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hsep hlen
    obtain ⟨ρ', h₂', e₂', s₂, _, hc', hr', hl'⟩ :=
      step_elide hf s hh ht hM hc hr (hsep σ _ _ .refl hh ht) (K := fun _ => False) (fun _ h => h.elim) hlen
    obtain ⟨σ', hx, hh', ht'⟩ := preservation hf s hh ht
    -- What isn't `imm` after the step wasn't before, or is new.
    have hM' : ∀ a m t', σ'[a]? = some (m, t') → m ≠ .immutable → M a ∨ h₀.length ≤ a := by
      intro a m t' ha hm
      by_cases hlt : a < h₀.length
      · have hσ : a < σ.length := hh.1 ▸ hlt
        have hp : σ[a]? = some σ[a] := List.getElem?_eq_getElem hσ
        rw [hx a _ hp] at ha
        exact .inl (hM a m t' (hp.trans ha) hm)
      · exact .inr (by omega)
    obtain ⟨M'', ρ'', h₂'', e₂'', ss, hc'', hr''⟩ :=
      ih hh' ht' hM' hc' hr' (fun σ'' h e hs' hh'' ht'' => hsep σ'' h e (.step s hs') hh'' ht'') hl'
    exact ⟨M'', ρ'', h₂'', e₂'', .step s₂ ss, hc'', hr''⟩

/-- **Every freeze can be in place.** The program, with its freezes erased
in it and in its functions, run on the heap with its freezes erased,
computes what it computes: whatever result the program that copies gets,
the other gets one that corresponds, and the same integer. The premise is
that whenever the program reaches a freeze, what's around it can't reach
the region's objects (`Expr.Sep`). -/
theorem freezes_in_place_change_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h₁ : Heap}
    {e e₁ : Expr} {T : Ty} (hh : HeapOk fns σ h) (ht : HasType fns σ .none false Ctx.empty e T)
    (hsep : ∀ σ' h' e', Steps fns h e h' e' → HeapOk fns σ' h' → HasType fns σ' .none false Ctx.empty e' T → e'.Sep σ')
    (hs : Steps fns h e h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps (fns.map Fn.erase) (h.map Expr.erase) e.erase h₂ e₂ ∧ Rel ρ e₁.erase e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n := by
  -- At first, each location corresponds to itself.
  let ρ : Nat → Nat → Prop := fun a c => a = c ∧ a < h.length
  have hself : ∀ a, a < h.length → ρ a a := fun a ha => ⟨rfl, ha⟩
  have hc : Corr (fun a => ∃ m t', σ[a]? = some (m, t') ∧ m ≠ .immutable) ρ (h.map Expr.erase) (h.map Expr.erase) := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c ⟨hac, ha⟩
      subst hac
      obtain ⟨w, hw⟩ : ∃ w, h[a]? = some w := ⟨_, List.getElem?_eq_getElem ha⟩
      have hrw := hh.rel_self hself hw
      refine ⟨w.erase, w.erase, by rw [List.getElem?_map, hw]; rfl, by rw [List.getElem?_map, hw]; rfl, ?_⟩
      exact Rel.refl fun l hl => ⟨rfl, by
        obtain ⟨⟨m, t⟩, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem (hh.1 ▸ ha)⟩
        obtain ⟨_, _, w', hw', _, hwt⟩ := hh.2 a m t hp
        rw [hw] at hw'
        cases hw'
        rw [hh.1]
        exact hwt.hasLoc_lt ((Expr.hasLoc_erase _ l).mp hl)⟩
    · intro a b c ⟨h₁, _⟩ ⟨h₂, _⟩ _
      rw [h₁, h₂]
    · intro a c d ⟨h₁, _⟩ ⟨h₂, _⟩ _
      rw [← h₁, ← h₂]
  have hr : Rel ρ e.erase e.erase :=
    Rel.refl fun l hl => ⟨rfl, by rw [hh.1]; exact ht.hasLoc_lt ((Expr.hasLoc_erase e l).mp hl)⟩
  obtain ⟨_, ρ', h₂, e₂, ss, _, hr'⟩ :=
    steps_elide hf hs hh ht (fun a m t' h hm => ⟨m, t', h, hm⟩) hc hr hsep (by simp)
  refine ⟨ρ', h₂, e₂, ss, hr', fun n hn => ?_⟩
  subst hn
  exact hr'.int_eq

end BoltMut
