import BoltMut.Lemmas

/-!
# Renaming a block's region

When a block starts, its name for its region, `2 * k + 1`, becomes the
region `2 * n` it makes, with its marker at location `n` (#292). Here,
that this keeps its body well-typed (`HasType.ren`): nothing that was in
the heap mentions the new region, and nothing from outside the block
mentions its name for it.

Renaming `a` to `b` is freeing `a` to `.region b`, so the lemmas about
freeing are used for both.
-/

namespace BoltMut

/-! ## Freeing, again -/

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

theorem Mu.eq_of_mentions {m : Mu} {r : Nat} (h : m.mentions r = true) : m = .region r := by
  cases m with
  | region r' => simp only [Mu.mentions, beq_iff_eq] at h; rw [h]
  | _ => cases h

/-- What a type mentions once `a` is replaced with `ν`. -/
theorem Mu.mentions_free {m ν : Mu} {a r : Nat} (h : (m.free a ν).mentions r = true) :
    (m.mentions a = true ∧ ν.mentions r = true) ∨ (r ≠ a ∧ m.mentions r = true) := by
  cases m with
  | region r' =>
    by_cases hr : r' = a
    · subst hr
      simp only [Mu.free_self] at h
      exact .inl ⟨by simp [Mu.mentions], h⟩
    · simp only [Mu.free, hr, ite_false, Mu.mentions, beq_iff_eq] at h
      subst h
      exact .inr ⟨hr, by simp [Mu.mentions]⟩
  | _ => cases h

theorem Ty.mentions_free {t : Ty} {ν : Mu} {a r : Nat} (h : (t.free a ν).mentions r = true) :
    (t.mentions a = true ∧ ν.mentions r = true) ∨ (r ≠ a ∧ t.mentions r = true) := by
  induction t with
  | unit | int | tparam => cases h
  | cell m t ih =>
    simp only [Ty.free, Ty.mentions, Bool.or_eq_true] at h ⊢
    rcases h with h | h
    · rcases Mu.mentions_free h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inl h₁, h₂⟩
      · exact .inr ⟨h₁, .inl h₂⟩
    · rcases ih h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inr h₁, h₂⟩
      · exact .inr ⟨h₁, .inr h₂⟩
  | fn m x y ihx ihy =>
    simp only [Ty.free, Ty.mentions, Bool.or_eq_true] at h ⊢
    rcases h with (h | h) | h
    · rcases Mu.mentions_free h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inl (.inl h₁), h₂⟩
      · exact .inr ⟨h₁, .inl (.inl h₂)⟩
    · rcases ihx h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inl (.inr h₁), h₂⟩
      · exact .inr ⟨h₁, .inl (.inr h₂)⟩
    · rcases ihy h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inr h₁, h₂⟩
      · exact .inr ⟨h₁, .inr h₂⟩
  | prod x y ihx ihy =>
    simp only [Ty.free, Ty.mentions, Bool.or_eq_true] at h ⊢
    rcases h with h | h
    · rcases ihx h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inl h₁, h₂⟩
      · exact .inr ⟨h₁, .inl h₂⟩
    · rcases ihy h with ⟨h₁, h₂⟩ | ⟨h₁, h₂⟩
      · exact .inl ⟨.inr h₁, h₂⟩
      · exact .inr ⟨h₁, .inr h₂⟩

theorem Ty.free_keeps_not_mentions {t : Ty} {ν : Mu} {a r : Nat} (h₁ : t.mentions r = false)
    (h₂ : t.mentions a = true → ν.mentions r = false) : (t.free a ν).mentions r = false := by
  cases e : (t.free a ν).mentions r with
  | false => rfl
  | true =>
    rcases Ty.mentions_free e with ⟨h, h'⟩ | ⟨_, h'⟩
    · rw [h₂ h] at h'; cases h'
    · rw [h₁] at h'; cases h'

theorem Mu.free_comm {m μ ν : Mu} {r a : Nat} (hra : r ≠ a) (hμ : μ.mentions a = false)
    (hν : ν.mentions r = false) : (m.free r μ).free a ν = (m.free a ν).free r μ := by
  cases m with
  | region r' =>
    by_cases h₁ : r' = r
    · subst h₁
      have : (Mu.region r').free a ν = .region r' := by simp [Mu.free, hra]
      rw [Mu.free_self, Mu.free_not_mentions hμ, this, Mu.free_self]
    · by_cases h₂ : r' = a
      · subst h₂
        have : (Mu.region r').free r μ = .region r' := by simp [Mu.free, h₁]
        rw [Mu.free_self, Mu.free_not_mentions hν, this, Mu.free_self]
      · simp [Mu.free, h₁, h₂]
  | _ => rfl

theorem Ty.free_comm {t : Ty} {μ ν : Mu} {r a : Nat} (hra : r ≠ a) (hμ : μ.mentions a = false)
    (hν : ν.mentions r = false) : (t.free r μ).free a ν = (t.free a ν).free r μ := by
  induction t with
  | unit | int | tparam => rfl
  | cell m t ih => simp [Ty.free, Mu.free_comm hra hμ hν, ih]
  | fn m x y ihx ihy => simp [Ty.free, Mu.free_comm hra hμ hν, ihx, ihy]
  | prod x y ihx ihy => simp [Ty.free, ihx, ihy]

/-- Renaming `a` to `b` and then freeing `b` is freeing `a`, if `b` wasn't
there. -/
theorem Mu.free_back {m μ : Mu} {a b : Nat} (hb : m.mentions b = false) :
    (m.free a (.region b)).free b μ = m.free a μ := by
  cases m with
  | region r' =>
    by_cases h : r' = a
    · subst h; simp [Mu.free]
    · have : r' ≠ b := by simpa [Mu.mentions] using hb
      simp [Mu.free, h, this]
  | _ => rfl

theorem Ty.free_back {t : Ty} {μ : Mu} {a b : Nat} (hb : t.mentions b = false) :
    (t.free a (.region b)).free b μ = t.free a μ := by
  induction t with
  | unit | int | tparam => rfl
  | cell m t ih =>
    simp only [Ty.mentions, Bool.or_eq_false_iff] at hb
    simp [Ty.free, Mu.free_back hb.1, ih hb.2]
  | fn m x y ihx ihy =>
    simp only [Ty.mentions, Bool.or_eq_false_iff] at hb
    simp [Ty.free, Mu.free_back hb.1.1, ihx hb.1.2, ihy hb.2]
  | prod x y ihx ihy =>
    simp only [Ty.mentions, Bool.or_eq_false_iff] at hb
    simp [Ty.free, ihx hb.1, ihy hb.2]

/-- Freezing another region still can be done once `a` is renamed. -/
theorem Ty.Freezable.ren_other {t : Ty} {r a b : Nat} (h : t.Freezable r) (hra : r ≠ a) (hrb : r ≠ b) :
    (t.free a (.region b)).Freezable r := by
  have hν : (Mu.region b).mentions r = false := by simp [Mu.mentions]; omega
  induction t with
  | unit | int | tparam => trivial
  | cell m t ih =>
    refine ⟨fun hm => ?_, ih h.2⟩
    rcases Ty.mentions_free hm with ⟨_, h'⟩ | ⟨_, h'⟩
    · rw [hν] at h'; cases h'
    · rw [h.1 h']; simp [Mu.free, hra]
  | fn m x y => exact Ty.free_keeps_not_mentions (t := .fn m x y) h (fun _ => hν)
  | prod x y ihx ihy => exact ⟨ihx h.1, ihy h.2⟩

/-- A block's freeze can be done once its region is renamed. -/
theorem Ty.Freezable.ren_self {t : Ty} {a b : Nat} (h : t.Freezable a) (hb : t.mentions b = false) :
    (t.free a (.region b)).Freezable b := by
  induction t with
  | unit | int | tparam => trivial
  | cell m t ih =>
    simp only [Ty.mentions, Bool.or_eq_false_iff] at hb
    refine ⟨fun hm => ?_, ih h.2 hb.2⟩
    rcases Ty.mentions_free hm with ⟨h', _⟩ | ⟨_, h'⟩
    · rw [h.1 h']; simp [Mu.free]
    · rw [hb.2] at h'; cases h'
  | fn m x y =>
    show (Ty.free (.fn m x y) a (.region b)).mentions b = false
    rw [Ty.free_not_mentions h]
    exact hb
  | prod x y ihx ihy =>
    simp only [Ty.mentions, Bool.or_eq_false_iff] at hb
    exact ⟨ihx h.1 hb.1, ihy h.2 hb.2⟩

theorem Ty.NoParams.ren {t : Ty} (h : t.NoParams) (a b : Nat) : (t.free a (.region b)).NoParams := by
  have hi : ∀ i, Mu.region b ≠ .param i := fun _ he => nomatch he
  induction t with
  | unit | int => trivial
  | tparam => exact h.elim
  | cell m t ih => exact ⟨Mu.free_ne_param h.1 a hi, ih h.2⟩
  | fn m x y ihx ihy => exact ⟨Mu.free_ne_param h.1 a hi, ihx h.2.1, ihy h.2.2⟩
  | prod x y ihx ihy => exact ⟨ihx h.1, ihy h.2⟩

theorem Mu.Below.ren {n : Nat} {m : Mu} (h : m.Below n) {a b : Nat} (hb : Made n b) :
    (m.free a (.region b)).Below n := by
  intro r hr
  rcases Mu.mentions_free hr with ⟨_, h'⟩ | ⟨_, h'⟩
  · simp only [Mu.mentions, beq_iff_eq] at h'; subst h'; exact hb
  · exact h r h'

theorem Ty.Below.ren {n : Nat} {t : Ty} (h : t.Below n) {a b : Nat} (hb : Made n b) :
    (t.free a (.region b)).Below n := by
  intro r hr
  rcases Ty.mentions_free hr with ⟨_, h'⟩ | ⟨_, h'⟩
  · simp only [Mu.mentions, beq_iff_eq] at h'; subst h'; exact hb
  · exact h r h'

theorem Mu.RegionsIn.ren {m μ : Mu} (h : m.RegionsIn μ) (a b : Nat) :
    (m.free a (.region b)).RegionsIn (μ.free a (.region b)) := by
  intro r hr
  rcases Mu.mentions_free hr with ⟨hm, h'⟩ | ⟨hra, h'⟩
  · rw [Mu.eq_of_mentions (h a hm)]; simpa [Mu.free] using h'
  · rw [Mu.eq_of_mentions (h r h')]; simp [Mu.free, hra, Mu.mentions]

theorem Ty.RegionsIn.ren {t : Ty} {μ : Mu} (h : t.RegionsIn μ) (a b : Nat) :
    (t.free a (.region b)).RegionsIn (μ.free a (.region b)) := by
  intro r hr
  rcases Ty.mentions_free hr with ⟨hm, h'⟩ | ⟨hra, h'⟩
  · rw [Mu.eq_of_mentions (h a hm)]; simpa [Mu.free] using h'
  · rw [Mu.eq_of_mentions (h r h')]; simp [Mu.free, hra, Mu.mentions]

theorem Mu.ParamsNoRegion.ren {m : Mu} (h : m.ParamsNoRegion .none) (a b : Nat) :
    (m.free a (.region b)).ParamsNoRegion .none := by
  cases m with
  | region r => simp only [Mu.free]; split <;> trivial
  | _ => exact h

theorem Ty.ParamsNoRegion.ren {t : Ty} (h : t.ParamsNoRegion .none) (a b : Nat) :
    (t.free a (.region b)).ParamsNoRegion .none := by
  induction t with
  | unit | int => trivial
  | tparam => exact h
  | cell m t ih => exact ⟨h.1.ren a b, ih h.2⟩
  | fn m x y ihx ihy => exact ⟨h.1.ren a b, ihx h.2.1, ihy h.2.2⟩
  | prod x y ihx ihy => exact ⟨ihx h.1, ihy h.2⟩

theorem Mu.Changeable.ren {m : Mu} (h : m.Changeable .none) (a b : Nat) :
    (m.free a (.region b)).Changeable .none := by
  cases m with
  | region r => simp only [Mu.free]; split <;> trivial
  | mutable => trivial
  | immutable => exact h.elim
  | param i => exact h

theorem Mu.NoRegion.free {m : Mu} (h : m.NoRegion) (a : Nat) (ν : Mu) : m.free a ν = m :=
  Mu.free_not_mentions (h.mentions a) ν

theorem Ty.NoRegion.free {t : Ty} (h : t.NoRegion) (a : Nat) (ν : Mu) : t.free a ν = t :=
  Ty.free_not_mentions (h.mentions a) ν

theorem muOf_free (c : Bool) (a : Nat) (ν : Mu) : (muOf c).free a ν = muOf c := by
  cases c <;> rfl

theorem List.getD_map_of {α : Type} (l : List α) (i : Nat) (f : α → α) {d : α} (hf : f d = d) :
    (l.map f)[i]?.getD d = f (l[i]?.getD d) := by
  rw [List.getElem?_map]
  cases l[i]? <;> simp [hf]

/-- Renaming in the arguments of a function whose signature mentions no
region. -/
theorem Mu.subst_free {m : Mu} (hm : m.NoRegion) (ms : List Mu) (a : Nat) (ν : Mu) :
    (m.subst ms).free a ν = m.subst (ms.map (Mu.free · a ν)) := by
  cases m with
  | param i => simp only [Mu.subst]; rw [List.getD_map_of _ _ _ rfl]
  | region => exact hm.elim
  | _ => rfl

theorem Ty.inst_free {t : Ty} (ht : t.NoRegion) (ms : List Mu) (τs : List Ty) (a : Nat) (ν : Mu) :
    (t.inst ms τs).free a ν = t.inst (ms.map (Mu.free · a ν)) (τs.map (Ty.free · a ν)) := by
  induction t with
  | unit | int => rfl
  | tparam i => simp only [Ty.inst]; rw [List.getD_map_of _ _ _ rfl]
  | cell m t ih => simp [Ty.inst, Ty.free, Mu.subst_free ht.1, ih ht.2]
  | fn m x y ihx ihy => simp [Ty.inst, Ty.free, Mu.subst_free ht.1, ihx ht.2.1, ihy ht.2.2]
  | prod x y ihx ihy => simp [Ty.inst, Ty.free, ihx ht.1, ihy ht.2]

/-- A call's arguments still meet what the function assumes once a region
is renamed in them. -/
theorem Sat.ren {Δ : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat .none Δ ms τs)
    (hΔ : (∀ p ∈ Δ.mus, p.1.NoRegion ∧ p.2.NoRegion) ∧ ∀ p ∈ Δ.tys, p.2.NoRegion) (a b : Nat) :
    Sat .none Δ (ms.map (Mu.free · a (.region b))) (τs.map (Ty.free · a (.region b))) := by
  obtain ⟨h₁, h₂, h₃, h₄, h₅, h₆⟩ := hs
  refine ⟨fun p hp => ?_, fun p hp => ?_, fun i => ?_, fun i hi => ?_, fun i hi => ?_, fun i hi => ?_⟩
  · rw [← Mu.subst_free (hΔ.1 p hp).1, ← Mu.subst_free (hΔ.1 p hp).2]
    exact (h₁ p hp).free a _
  · rw [← Mu.subst_free (hΔ.2 p hp), List.getD_map_of _ _ _ rfl]
    exact (h₂ p hp).free a _
  · rw [List.getD_map_of _ _ _ rfl]
    simpa using (h₃ i).free a (.region b)
  · rw [List.getD_map_of _ _ _ rfl, (h₄ i hi).1.free]
    exact h₄ i hi
  · rw [List.getD_map_of _ _ _ rfl, (h₅ i hi).1.free]
    exact h₅ i hi
  · rw [List.getD_map_of _ _ _ rfl]
    exact (h₆ i hi).ren a b

/-! ## Contexts -/

/-- `Γ` with the region `a` replaced with `ν`. -/
def Ctx.free (Γ : Ctx) (a : Nat) (ν : Mu) : Ctx := fun y => (Γ y).map (Ty.free · a ν)

theorem Ctx.free_update (Γ : Ctx) (x : Nat) (t : Ty) (a : Nat) (ν : Mu) :
    (Γ.update x t).free a ν = (Γ.free a ν).update x (t.free a ν) := by
  funext y
  simp only [Ctx.free, Ctx.update]
  split <;> rfl

theorem Ctx.Sub.free {Γ Γ' : Ctx} (h : Γ.Sub Γ') (a : Nat) (ν : Mu) : (Γ.free a ν).Sub (Γ'.free a ν) := by
  intro y s hy
  simp only [Ctx.free, Option.map_eq_some_iff] at hy ⊢
  obtain ⟨s₀, h₀, rfl⟩ := hy
  exact ⟨s₀, h y s₀ h₀, rfl⟩

theorem Expr.hasLoc_ren {e : Expr} {a b l : Nat} (h : (e.ren a b).HasLoc l) : e.HasLoc l := by
  induction e with
  | var | unit | int | loc => exact h
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih
    | freeze _ _ _ ih | keep _ _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    rcases h with h | h
    · exact .inl (ih₁ h)
    · exact .inr (ih₂ h)
  | block _ k t e ih =>
    simp only [Expr.ren] at h
    split at h
    · exact h
    · exact ih h

/-! ## The renaming lemma -/

/-- **Renaming a block's region keeps its body well-typed.** `a` is renamed
`b`, a region that doesn't exist in `σ` but does in `σ'`; no location in
`e` mentions `a`, nor, since the locations are all in `σ`, `b`. -/
theorem HasType.ren {fns : List Fn} (hfn : ∀ fn ∈ fns, fn.NoRegions) {σ σ' : HeapTy} {io : Bool}
    {Γ : Ctx} {e : Expr} {t : Ty} (ht : HasType fns σ .none io Γ e t) {a b : Nat} (hb2 : b % 2 = 0)
    (hx : Extends σ σ') (hb : Made σ'.length b) (hbn : ¬ Made σ.length b) :
    (∀ l, e.HasLoc l → ∃ m t', σ[l]? = some (m, t') ∧ (Ty.cell m t').mentions a = false) →
    HasType fns σ' .none io (Γ.free a (.region b)) (e.ren a b) (t.free a (.region b)) := by
  -- What exists in `σ` isn't `b`.
  have hne : ∀ {r}, Made σ.length r → r ≠ b := fun hr he => hbn (he ▸ hr)
  have hnb : ∀ {t : Ty}, t.Below σ.length → t.mentions b = false := fun {t} ht' => by
    cases e : t.mentions b with
    | false => rfl
    | true => exact (hbn (ht' b e)).elim
  have hνr : ∀ {r}, r ≠ b → (Mu.region b).mentions r = false := fun h => by
    simp [Mu.mentions]; omega
  induction ht with
  | var hx' => intro _; exact .var (by simp [Ctx.free, hx'])
  | unit => intro _; exact .unit
  | int => intro _; exact .int
  | loc hσ hio =>
    intro hl
    obtain ⟨m, t', hσ', hm⟩ := hl _ rfl
    rw [hσ] at hσ'
    cases hσ'
    rw [Ty.free_not_mentions hm]
    exact .loc (hx _ _ hσ) hio
  | @new _ _ _ _ c _ hd ih =>
    intro hl
    have h := hd.free a (.region b)
    simp only [Ty.free, muOf_free] at h ⊢
    exact .new (ih hl) h
  | get _ ih => intro hl; exact .get (ih hl)
  | set _ _ ih₁ ih₂ =>
    intro hl
    exact .set (ih₁ fun l h => hl l (.inl h)) (ih₂ fun l h => hl l (.inr h))
  | @newR _ _ _ _ r _ hd hr ih =>
    intro hl
    have h := hd.free a (.region b)
    simp only [Ty.free] at h ⊢
    by_cases hra : r = a
    · subst hra
      simp only [Mu.free_self] at h ⊢
      exact .newR (ih hl) h hb
    · simp only [Mu.free, hra, ite_false] at h ⊢
      exact .newR (ih hl) h (hr.mono hx.length_le)
  | newP _ hi => simp [Assume.none] at hi
  | @setR _ _ _ _ _ r _ _ ih₁ ih₂ =>
    intro hl
    have h₁ := ih₁ fun l h => hl l (.inl h)
    have h₂ := ih₂ fun l h => hl l (.inr h)
    simp only [Ty.free, Mu.free] at h₁
    split at h₁
    · exact .setR h₁ h₂
    · exact .setR h₁ h₂
  | setP _ hi => simp [Assume.none] at hi
  | letE _ _ ih₁ ih₂ =>
    intro hl
    have h₂ := ih₂ fun l h => hl l (.inr h)
    rw [Ctx.free_update] at h₂
    exact .letE (ih₁ fun l h => hl l (.inl h)) h₂
  | @call _ _ f fn ms τs _ hfn' hs _ hbm ih =>
    intro hl
    obtain ⟨hna, hnr, hmus, htys⟩ := hfn fn (List.mem_of_getElem? hfn')
    have h := ih hl
    rw [Ty.inst_free hna] at h
    simp only [Expr.ren]
    rw [Ty.inst_free hnr]
    refine .call hfn' (hs.ren ⟨hmus, htys⟩ a b) h fun m hm => ?_
    obtain ⟨m₀, hm₀, rfl⟩ := List.mem_map.mp hm
    exact ((hbm m₀ hm₀).mono hx.length_le).ren hb
  | @lam _ _ Γc c x body ta tr μ hk hsub himm hrc hrl _ hw ih =>
    intro hl
    have hb' := ih fun l h => hl l h
    rw [Ctx.free_update] at hb'
    refine .lam ?_ (hsub.free a _) ?_ ?_ ?_ hb' ((hw.mono hx.length_le).ren hb)
    · rcases hk with rfl | ⟨rfl, r', rfl⟩
      · exact .inl (muOf_free _ _ _)
      · refine .inr ⟨rfl, ?_⟩
        simp only [Mu.free]
        split
        · exact ⟨b, rfl⟩
        · exact ⟨r', rfl⟩
    · intro hμ y s hy
      simp only [Ctx.free, Option.map_eq_some_iff] at hy
      obtain ⟨s₀, h₀, rfl⟩ := hy
      have hμ' : μ ≠ .mutable := fun he => hμ (by rw [he]; rfl)
      exact (himm hμ' y s₀ h₀).free a _
    · intro y s hy
      simp only [Ctx.free, Option.map_eq_some_iff] at hy
      obtain ⟨s₀, h₀, rfl⟩ := hy
      exact ⟨(hrc y s₀ h₀).1.ren a b, (hrc y s₀ h₀).2.ren a b⟩
    · intro l h
      obtain ⟨m, t', hσ, hle, hm, ht'⟩ := hrl l (Expr.hasLoc_ren h)
      obtain ⟨m', t'', hσ', hn⟩ := hl l (Expr.hasLoc_ren h)
      rw [hσ] at hσ'
      cases hσ'
      simp only [Ty.mentions, Bool.or_eq_false_iff] at hn
      refine ⟨m, t', hx _ _ hσ, ?_, ?_, ?_⟩
      · simpa [Mu.free_not_mentions hn.1] using hle.free a (.region b)
      · simpa [Mu.free_not_mentions hn.1] using hm.ren a b
      · simpa [Ty.free_not_mentions hn.2] using ht'.ren a b
  | app _ _ ih₁ ih₂ =>
    intro hl
    exact .app (ih₁ fun l h => hl l (.inl h)) (ih₂ fun l h => hl l (.inr h))
  | pair _ _ ih₁ ih₂ =>
    intro hl
    exact .pair (ih₁ fun l h => hl l (.inl h)) (ih₂ fun l h => hl l (.inr h))
  | fst _ ih => intro hl; exact .fst (ih hl)
  | snd _ ih => intro hl; exact .snd (ih hl)
  | @freeze _ _ _ t r _ hf hp hr hbt ih =>
    intro hl
    have h := ih hl
    have htb := hnb hbt
    simp only [Expr.ren]
    by_cases hra : r = a
    · subst hra
      simp only [ite_true]
      rw [Ty.free_not_mentions (Ty.free_mentions_self (μ := .immutable) trivial r), ← Ty.free_back (μ := .immutable) htb]
      exact .freeze h (hf.ren_self htb) (hp.ren _ _) hb ((hbt.mono hx.length_le).ren hb)
    · simp only [hra, ite_false]
      rw [Ty.free_comm hra rfl (hνr (hne hr))]
      exact .freeze h (hf.ren_other hra (hne hr)) (hp.ren _ _) (hr.mono hx.length_le)
        ((hbt.mono hx.length_le).ren hb)
  | @block _ Γ₀ Γb k t e hsub hnm hle hbody hf hp hbt ih =>
    intro hl
    simp only [Expr.ren]
    have hkb : 2 * k + 1 ≠ b := by omega
    by_cases hka : 2 * k + 1 = a
    · -- An inner block with the same name: its region is its own.
      subst hka
      simp only [ite_true]
      rw [Ty.free_not_mentions (Ty.free_mentions_self (μ := .immutable) trivial _)]
      refine .block (fun y s hy => ?_) hnm (fun l h => ?_) (hbody.weaken_heap hx) hf hp
        (hbt.mono hx.length_le)
      · simp only [Ctx.free, hsub y s hy, Option.map_some, Ty.free_not_mentions (hnm y s hy)]
      · obtain ⟨m, t', hσ, hm⟩ := hle l h
        exact ⟨m, t', hx _ _ hσ, hm⟩
    · simp only [hka, ite_false]
      rw [Ty.free_comm hka rfl (hνr hkb)]
      refine .block (hsub.free a _) (fun y s hy => ?_) (fun l h => ?_) ?_ (hf.ren_other hka hkb)
        (hp.ren _ _) ((hbt.mono hx.length_le).ren hb)
      · simp only [Ctx.free, Option.map_eq_some_iff] at hy
        obtain ⟨s₀, h₀, rfl⟩ := hy
        exact Ty.free_keeps_not_mentions (hnm y s₀ h₀) (fun _ => hνr hkb)
      · obtain ⟨m, t', hσ, hm⟩ := hle l (Expr.hasLoc_ren h)
        exact ⟨m, t', hx _ _ hσ, hm⟩
      · exact ih hl

/-! ## Scoping is kept -/

theorem Expr.Scoped.mono {B B' : Nat → Prop} (hB : ∀ r, B r → B' r) {e : Expr} (h : e.Scoped B) :
    e.Scoped B' := by
  induction e generalizing B B' with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih =>
    exact ih hB h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact ⟨ih₁ hB h.1, ih₂ hB h.2⟩
  | call _ ms τs _ ih =>
    exact ⟨fun m hm r h₁ h₂ => hB r (h.1 m hm r h₁ h₂), fun τ hτ r h₁ h₂ => hB r (h.2.1 τ hτ r h₁ h₂),
      ih hB h.2.2⟩
  | block _ k _ _ ih => exact ih (fun r h' => h'.imp_right (hB r)) h

theorem Expr.Scoped.subst {B : Nat → Prop} {e v : Expr} (h : e.Scoped B) (hv : v.Closed) (x : Nat) :
    (e.subst x v).Scoped B := by
  induction e generalizing B with
  | var y =>
    simp only [Expr.subst]
    split
    · exact hv.mono fun _ h => h.elim
    · trivial
  | unit | int | loc => trivial
  | new _ _ ih | get _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih | block _ _ _ _ ih =>
    exact ih h
  | call _ _ _ _ ih => exact ⟨h.1, h.2.1, ih h.2.2⟩
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst]
    refine ⟨ih₁ h.1, ?_⟩
    split
    · exact h.2
    · exact ih₂ h.2
  | lam _ y _ ih =>
    simp only [Expr.subst]
    split
    · exact h
    · exact ih h

theorem Ty.Scoped.inst {B : Nat → Prop} {t : Ty} (h : t.Scoped B) {ms : List Mu} {τs : List Ty}
    (hms : ∀ m ∈ ms, m.Scoped (fun _ => False)) (hτs : ∀ τ ∈ τs, τ.Scoped (fun _ => False)) :
    (t.inst ms τs).Scoped B := by
  intro r hr ho
  rcases Ty.mentions_inst_or hr with hr | ⟨m, hm, hr⟩ | ⟨τ, hτ, hr⟩
  · exact h r hr ho
  · exact (hms m hm r hr ho).elim
  · exact (hτs τ hτ r hr ho).elim

theorem Mu.Scoped.subst {B : Nat → Prop} {m : Mu} (h : m.Scoped B) {ms : List Mu}
    (hms : ∀ m ∈ ms, m.Scoped (fun _ => False)) : (m.subst ms).Scoped B := by
  intro r hr ho
  rcases Mu.mentions_subst_or hr with hr | ⟨m', hm, hr⟩
  · exact h r hr ho
  · exact (hms m' hm r hr ho).elim

theorem Expr.Scoped.inst {B : Nat → Prop} {e : Expr} (h : e.Scoped B) {ms : List Mu} {τs : List Ty}
    (hms : ∀ m ∈ ms, m.Scoped (fun _ => False)) (hτs : ∀ τ ∈ τs, τ.Scoped (fun _ => False)) :
    (e.inst ms τs).Scoped B := by
  induction e generalizing B with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih
    | block _ _ _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact ⟨ih₁ h.1, ih₂ h.2⟩
  | call _ ms' τs' _ ih =>
    refine ⟨fun m hm => ?_, fun τ hτ => ?_, ih h.2.2⟩
    · obtain ⟨m₀, hm₀, rfl⟩ := List.mem_map.mp hm
      exact (h.1 m₀ hm₀).subst hms
    · obtain ⟨τ₀, hτ₀, rfl⟩ := List.mem_map.mp hτ
      exact (h.2.1 τ₀ hτ₀).inst hms hτs

/-- Renaming a block's name to a region it made leaves one name less. -/
theorem Expr.Scoped.ren {B : Nat → Prop} {a b : Nat} {e : Expr} (h : e.Scoped (fun r => r = a ∨ B r))
    (hb : b % 2 = 0) : (e.ren a b).Scoped B := by
  have hm : ∀ {m : Mu}, m.Scoped (fun r => r = a ∨ B r) → (m.free a (.region b)).Scoped B := by
    intro m h r hr ho
    rcases Mu.mentions_free hr with ⟨_, h'⟩ | ⟨hra, h'⟩
    · simp only [Mu.mentions, beq_iff_eq] at h'; omega
    · exact (h r h' ho).resolve_left hra
  have ht : ∀ {t : Ty}, t.Scoped (fun r => r = a ∨ B r) → (t.free a (.region b)).Scoped B := by
    intro t h r hr ho
    rcases Ty.mentions_free hr with ⟨_, h'⟩ | ⟨hra, h'⟩
    · simp only [Mu.mentions, beq_iff_eq] at h'; omega
    · exact (h r h' ho).resolve_left hra
  induction e generalizing B with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | freeze _ _ _ ih | keep _ _ _ ih =>
    exact ih h hm ht
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact ⟨ih₁ h.1 hm ht, ih₂ h.2 hm ht⟩
  | call _ ms τs _ ih =>
    refine ⟨fun m hm' => ?_, fun τ hτ => ?_, ih h.2.2 hm ht⟩
    · obtain ⟨m₀, hm₀, rfl⟩ := List.mem_map.mp hm'
      exact hm (h.1 m₀ hm₀)
    · obtain ⟨τ₀, hτ₀, rfl⟩ := List.mem_map.mp hτ
      exact ht (h.2.1 τ₀ hτ₀)
  | block _ k t e ih =>
    have h : e.Scoped (fun r => r = 2 * k + 1 ∨ (r = a ∨ B r)) := h
    simp only [Expr.ren]
    split
    · rename_i hka
      refine Expr.Scoped.mono (e := e) (fun r h' => ?_) h
      rcases h' with h' | h' | h'
      · exact .inl h'
      · exact .inl (h'.trans hka.symm)
      · exact .inr h'
    · refine ih (B := fun r => r = 2 * k + 1 ∨ B r) (Expr.Scoped.mono (fun r h' => ?_) h) ?_ ?_
      · rcases h' with h' | h' | h'
        · exact .inr (.inl h')
        · exact .inl h'
        · exact .inr (.inr h')
      · intro m h r hr ho
        rcases Mu.mentions_free hr with ⟨_, h'⟩ | ⟨hra, h'⟩
        · simp only [Mu.mentions, beq_iff_eq] at h'; omega
        · exact (h r h' ho).resolve_left hra
      · intro t h r hr ho
        rcases Ty.mentions_free hr with ⟨_, h'⟩ | ⟨hra, h'⟩
        · simp only [Mu.mentions, beq_iff_eq] at h'; omega
        · exact (h r h' ho).resolve_left hra

/-- Every value in the heap is closed. -/
def HeapClosed (h : Heap) : Prop := ∀ (l : Nat) (v : Expr), h[l]? = some v → v.Closed

theorem HeapClosed.append {h : Heap} (hh : HeapClosed h) {v : Expr} (hv : v.Closed) : HeapClosed (h ++ [v]) := by
  intro l w hl
  by_cases hlt : l < h.length
  · rw [List.getElem?_append_left hlt] at hl; exact hh l w hl
  · rw [List.getElem?_append_right (by omega)] at hl
    rcases hx : l - h.length with _ | n
    · rw [hx] at hl; simp at hl; rw [← hl]; exact hv
    · rw [hx] at hl; simp at hl

theorem HeapClosed.set {h : Heap} (hh : HeapClosed h) {v : Expr} (hv : v.Closed) (l : Nat) : HeapClosed (h.set l v) := by
  intro l' w hl
  rw [List.getElem?_set] at hl
  split at hl
  · split at hl
    · cases hl; exact hv
    · cases hl
  · exact hh l' w hl

/-- A copy is closed, and so is its heap. -/
theorem Copy.closed {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') : HeapClosed h → v.Closed → HeapClosed h' ∧ v'.Closed := by
  induction hc with
  | keep => exact fun hh hv => ⟨hh, hv⟩
  | seen => exact fun hh _ => ⟨hh, trivial⟩
  | cell _ _ hw _ ih =>
    intro hh _
    obtain ⟨hh', hw'⟩ := ih hh (hh _ _ hw)
    exact ⟨hh'.append hw', trivial⟩
  | pair _ _ _ ih₁ ih₂ =>
    intro hh hv
    obtain ⟨hh₁, hv₁⟩ := ih₁ hh hv.1
    obtain ⟨hh₂, hv₂⟩ := ih₂ hh₁ hv.2
    exact ⟨hh₂, hv₁, hv₂⟩

/-- **A step keeps a program closed**, and its heap. -/
theorem step_closed {fns : List Fn} (hf : ∀ fn ∈ fns, fn.body.Closed) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') : HeapClosed h → e.Closed → HeapClosed h' ∧ e'.Closed := by
  induction hs with
  | newCong _ ih | getCong _ ih | fstCong _ ih | sndCong _ ih | freezeCong _ ih | keepCong _ ih =>
    exact ih
  | setCong₁ _ ih | letCong _ ih | appCong₁ _ ih | pairCong₁ _ ih =>
    intro hh he
    obtain ⟨hh', he'⟩ := ih hh he.1
    exact ⟨hh', he', he.2⟩
  | setCong₂ _ _ ih | appCong₂ _ _ ih | pairCong₂ _ _ ih =>
    intro hh he
    obtain ⟨hh', he'⟩ := ih hh he.2
    exact ⟨hh', he.1, he'⟩
  | callCong _ ih =>
    intro hh he
    obtain ⟨hh', he'⟩ := ih hh he.2.2
    exact ⟨hh', he.1, he.2.1, he'⟩
  | newVal _ => exact fun hh he => ⟨hh.append he, trivial⟩
  | getLoc hl => exact fun hh _ => ⟨hh, hh _ _ hl⟩
  | setLoc _ _ => exact fun hh he => ⟨hh.set he.2 _, trivial⟩
  | letVal _ => exact fun hh he => ⟨hh, he.2.subst he.1 _⟩
  | callVal hfn _ =>
    exact fun hh he => ⟨hh, ((hf _ (List.mem_of_getElem? hfn)).inst he.1 he.2.1).subst he.2.2 _⟩
  | @appVal _ x body _ _ =>
    exact fun hh he => ⟨hh, Expr.Scoped.subst (B := fun _ => False) (e := body) he.1 he.2 x⟩
  | fstVal _ _ => exact fun hh he => ⟨hh, he.1⟩
  | sndVal _ _ => exact fun hh he => ⟨hh, he.2⟩
  | freezeVal _ hc => exact fun hh he => hc.closed hh he
  | keepVal _ hc => exact fun hh he => ⟨(hc.closed hh he).1, he⟩
  | blockEnter =>
    exact fun hh he => ⟨hh.append trivial, Expr.Scoped.ren (B := fun _ => False)
      (Expr.Scoped.mono (fun r h' => h'.imp_right id) he) (by omega)⟩
  | blockKeep =>
    exact fun hh he => ⟨hh.append trivial, Expr.Scoped.ren (B := fun _ => False)
      (Expr.Scoped.mono (fun r h' => h'.imp_right id) he) (by omega)⟩

end BoltMut
