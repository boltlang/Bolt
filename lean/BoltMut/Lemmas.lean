import BoltMut.Semantics

/-!
# Lemmas about typing

Weakening, canonical forms, and the two substitutions a call makes: the
mutability and type arguments for the function's parameters, and the
argument for its variable.
-/

namespace BoltMut

/-- `σ'` has every location of `σ`, with the same type: the heap typing only
grows. -/
def Extends (σ σ' : HeapTy) : Prop := ∀ (l : Nat) (p : Mu × Ty), σ[l]? = some p → σ'[l]? = some p

theorem Extends.refl (σ : HeapTy) : Extends σ σ := fun _ _ h => h

theorem Extends.trans {σ₁ σ₂ σ₃ : HeapTy} (h₁ : Extends σ₁ σ₂) (h₂ : Extends σ₂ σ₃) : Extends σ₁ σ₃ :=
  fun l p h => h₂ l p (h₁ l p h)

theorem Extends.append (σ : HeapTy) (p : Mu × Ty) : Extends σ (σ ++ [p]) := by
  intro l q h
  rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp h).1]
  exact h

theorem Extends.nil (σ : HeapTy) : Extends [] σ := by
  intro l p h
  simp at h

theorem HasType.weaken_heap {fns : List Fn} {σ σ' : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr}
    {t : Ty} (ht : HasType fns σ Δ io Γ e t) (hx : Extends σ σ') : HasType fns σ' Δ io Γ e t := by
  induction ht with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h hio => exact .loc (hx _ _ h) hio
  | new _ hd ih => exact .new ih hd
  | newR _ hd ih => exact .newR ih hd
  | newP _ hi hd ih => exact .newP ih hi hd
  | get _ ih => exact .get ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | setR _ _ ih₁ ih₂ => exact .setR ih₁ ih₂
  | setP _ hi _ ih₁ ih₂ => exact .setP ih₁ hi ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | call hf hs _ ih => exact .call hf hs ih
  | lam hk hc himm hrc hrl _ ih =>
    refine .lam hk hc himm hrc (fun l hl => ?_) ih
    obtain ⟨m, t, h, hm⟩ := hrl l hl
    exact ⟨m, t, hx _ _ h, hm⟩
  | gone hf hl =>
    refine .gone hf (fun l h' => ?_)
    obtain ⟨m, t, h, hm⟩ := hl l h'
    exact ⟨m, t, hx _ _ h, hm⟩
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih

theorem Ctx.Sub.update {Γ Γ' : Ctx} (h : Γ.Sub Γ') (x : Nat) (t : Ty) :
    (Γ.update x t).Sub (Γ'.update x t) := by
  intro y s hy
  simp only [Ctx.update] at hy ⊢
  by_cases hyx : y = x
  · simpa [hyx] using hy
  · simp only [hyx] at hy ⊢
    exact h _ _ hy

theorem Ctx.Sub.trans {Γ₁ Γ₂ Γ₃ : Ctx} (h₁ : Γ₁.Sub Γ₂) (h₂ : Γ₂.Sub Γ₃) : Γ₁.Sub Γ₃ :=
  fun x s h => h₂ x s (h₁ x s h)

theorem Ctx.Sub.empty (Γ : Ctx) : Ctx.empty.Sub Γ := fun _ _ h => nomatch h

/-- Typing only depends on the variables that are in scope. A closure keeps
the variables it sees. -/
theorem HasType.weaken_ctx {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ Γ' : Ctx} {e : Expr}
    {t : Ty} (ht : HasType fns σ Δ io Γ e t) (hΓ : Γ.Sub Γ') : HasType fns σ Δ io Γ' e t := by
  induction ht generalizing Γ' with
  | var h => exact .var (hΓ _ _ h)
  | unit => exact .unit
  | int => exact .int
  | loc h hio => exact .loc h hio
  | new _ hd ih => exact .new (ih hΓ) hd
  | newR _ hd ih => exact .newR (ih hΓ) hd
  | newP _ hi hd ih => exact .newP (ih hΓ) hi hd
  | get _ ih => exact .get (ih hΓ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ) (ih₂ hΓ)
  | setR _ _ ih₁ ih₂ => exact .setR (ih₁ hΓ) (ih₂ hΓ)
  | setP _ hi _ ih₁ ih₂ => exact .setP (ih₁ hΓ) hi (ih₂ hΓ)
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ hΓ) (ih₂ (hΓ.update _ _))
  | call hf hs _ ih => exact .call hf hs (ih hΓ)
  | lam hk hc himm hrc hrl hb _ => exact .lam hk (hc.trans hΓ) himm hrc hrl hb
  | gone hf hl => exact .gone hf hl
  | app _ _ ih₁ ih₂ => exact .app (ih₁ hΓ) (ih₂ hΓ)
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ hΓ) (ih₂ hΓ)
  | fst _ ih => exact .fst (ih hΓ)
  | snd _ ih => exact .snd (ih hΓ)

/-- Outside an `imm` closure, anything typed inside one has its type too. -/
theorem HasType.weaken_imm {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) {io' : Bool} (hio : io' = true → io = true) :
    HasType fns σ Δ io' Γ e t := by
  induction ht generalizing io' with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h hm => exact .loc h (fun h' => hm (hio h'))
  | new _ hd ih => exact .new (ih hio) hd
  | newR _ hd ih => exact .newR (ih hio) hd
  | newP _ hi hd ih => exact .newP (ih hio) hi hd
  | get _ ih => exact .get (ih hio)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hio) (ih₂ hio)
  | setR _ _ ih₁ ih₂ => exact .setR (ih₁ hio) (ih₂ hio)
  | setP _ hi _ ih₁ ih₂ => exact .setP (ih₁ hio) hi (ih₂ hio)
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ hio) (ih₂ hio)
  | call hf hs _ ih => exact .call hf hs (ih hio)
  | lam hk hc himm hrc hrl _ ih =>
    refine .lam hk hc himm hrc hrl (ih ?_)
    intro h
    simp only [Bool.or_eq_true] at h ⊢
    exact h.imp_left hio
  | gone hf hl =>
    refine .gone hf (fun l h' => ?_)
    obtain ⟨m, t, h, hle, hm, ht, hi⟩ := hl l h'
    exact ⟨m, t, h, hle, hm, ht, fun h'' => hi (hio h'')⟩
  | app _ _ ih₁ ih₂ => exact .app (ih₁ hio) (ih₂ hio)
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ hio) (ih₂ hio)
  | fst _ ih => exact .fst (ih hio)
  | snd _ ih => exact .snd (ih hio)

theorem muOf_cases (b : Bool) : muOf b = .mutable ∨ muOf b = .immutable := by
  cases b
  · exact .inr rfl
  · exact .inl rfl

theorem muOf_immutable {b : Bool} (h : muOf b = .immutable) : b = false := by
  cases b
  · rfl
  · cases h

/-- A value whose type is `imm` all the way down can be captured by an `imm`
closure. Values only run in the program, which assumes nothing. -/
theorem HasType.value_imm {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {v : Expr} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ .none io Γ v t) (hd : AtMost .none .immutable t) :
    HasType fns σ .none true Γ v t := by
  induction hv generalizing t with
  | pair _ _ ih₁ ih₂ => cases ht with
    | pair h₁ h₂ => exact .pair (ih₁ h₁ hd.1) (ih₂ h₂ hd.2)
  | unit => cases ht; exact .unit
  | int => cases ht; exact .int
  | loc => cases ht with
    | loc h _ => exact .loc h (fun _ => Le.none_immutable hd.1)
  | @lam b x body => cases ht with
    | lam hk hc himm hrc hrl hb =>
      have hμ := Le.none_immutable hd
      subst hμ
      have hb' : b = false := by
        rcases hk with h | ⟨_, _, h⟩
        · exact muOf_immutable h.symm
        · cases h
      subst hb'
      exact .lam hk hc himm hrc hrl (hb.weaken_imm (fun _ => by simp))
    | gone hf hl =>
      have hμ := Le.none_immutable hd
      subst hμ
      refine .gone hf (fun l h' => ?_)
      obtain ⟨m, t, h, hle, hm, ht, _⟩ := hl l h'
      exact ⟨m, t, h, hle, hm, ht, fun _ => Le.none_immutable hle⟩

/-- A value of a cell type is a location of that type. -/
theorem canonical_cell {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {v : Expr} {m : Mu}
    {t : Ty} (hv : v.IsValue) (ht : HasType fns σ Δ io Γ v (.cell m t)) :
    ∃ l, v = .loc l ∧ σ[l]? = some (m, t) := by
  cases hv with
  | unit => cases ht
  | int => cases ht
  | loc => cases ht with
    | loc h _ => exact ⟨_, rfl, h⟩
  | lam => cases ht
  | pair => cases ht

/-- A value of a function type is a closure. -/
theorem canonical_fn {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {v : Expr} {m : Mu}
    {a r : Ty} (hv : v.IsValue) (ht : HasType fns σ Δ io Γ v (.fn m a r)) : ∃ b x body, v = .lam b x body := by
  cases hv with
  | unit => cases ht
  | int => cases ht
  | loc => cases ht
  | lam => exact ⟨_, _, _, rfl⟩
  | pair => cases ht

/-- A value of a tuple type is a pair of values. -/
theorem canonical_prod {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {v : Expr} {a b : Ty}
    (hv : v.IsValue) (ht : HasType fns σ Δ io Γ v (.prod a b)) :
    ∃ v₁ v₂, v = .pair v₁ v₂ ∧ v₁.IsValue ∧ v₂.IsValue := by
  cases hv with
  | unit => cases ht
  | int => cases ht
  | loc => cases ht
  | lam => cases ht
  | pair h₁ h₂ => exact ⟨_, _, rfl, h₁, h₂⟩

/-- `Γ` without `x`. -/
def Ctx.drop (Γ : Ctx) (x : Nat) : Ctx := fun z => if z = x then none else Γ z

theorem Ctx.drop_sub {Γc Γ' Γ : Ctx} {x : Nat} (hc : Γc.Sub Γ') (hΓ : ∀ y, y ≠ x → Γ' y = Γ y) :
    (Γc.drop x).Sub Γ := by
  intro z s hz
  simp only [Ctx.drop] at hz
  split at hz
  · cases hz
  · rename_i hzx
    rw [← hΓ z hzx]
    exact hc _ _ hz

theorem Ctx.drop_some {Γ : Ctx} {x z : Nat} {s : Ty} (h : Γ.drop x z = some s) : Γ z = some s := by
  simp only [Ctx.drop] at h
  split at h
  · cases h
  · exact h

/-- A location of `e.subst x v` is one of `e`, or one of `v` when `x` is in
scope in `e`. -/
theorem HasType.hasLoc_subst {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr}
    {t : Ty} (ht : HasType fns σ Δ io Γ e t) {x : Nat} {v : Expr} {l : Nat} (h : (e.subst x v).HasLoc l) :
    e.HasLoc l ∨ (∃ s, Γ x = some s) ∧ v.HasLoc l := by
  induction ht with
  | @var _ _ y s hy =>
    simp only [Expr.subst] at h
    split at h
    · rename_i hyx
      subst hyx
      exact .inr ⟨⟨s, hy⟩, h⟩
    · exact h.elim
  | unit | int => exact h.elim
  | loc => exact .inl h
  | new _ _ ih | newR _ _ ih | newP _ _ _ ih | get _ ih | call _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | setR _ _ ih₁ ih₂ | setP _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    rcases h with h | h
    · exact (ih₁ h).imp .inl id
    · exact (ih₂ h).imp .inr id
  | @letE _ _ y _ _ _ _ _ _ ih₁ ih₂ =>
    simp only [Expr.subst, Expr.HasLoc] at h
    rcases h with h | h
    · exact (ih₁ h).imp .inl id
    · by_cases hyx : y = x
      · simp only [hyx, ite_true] at h
        exact .inl (.inr h)
      · simp only [hyx, ite_false] at h
        rcases ih₂ h with h | ⟨⟨s, hs⟩, h⟩
        · exact .inl (.inr h)
        · simp only [Ctx.update, Ne.symm hyx, ite_false] at hs
          exact .inr ⟨⟨s, hs⟩, h⟩
  | gone hf =>
    rw [Expr.subst_not_free _ (hf x)] at h
    exact .inl h
  | @lam _ _ _ _ y _ _ _ _ _ hc _ _ _ _ ih =>
    simp only [Expr.subst, Expr.HasLoc] at h
    by_cases hyx : y = x
    · simp only [hyx, ite_true] at h
      exact .inl h
    · simp only [hyx, ite_false] at h
      rcases ih h with h | ⟨⟨s, hs⟩, h⟩
      · exact .inl h
      · simp only [Ctx.update, Ne.symm hyx, ite_false] at hs
        exact .inr ⟨⟨s, hc _ _ hs⟩, h⟩

theorem Mu.RegionsIn.trans {m μ ν : Mu} (h₁ : m.RegionsIn μ) (h₂ : μ.RegionsIn ν) : m.RegionsIn ν :=
  fun r h => h₂ r (h₁ r h)

theorem Ty.RegionsIn.trans {t : Ty} {μ ν : Mu} (h₁ : t.RegionsIn μ) (h₂ : μ.RegionsIn ν) : t.RegionsIn ν :=
  fun r h => h₂ r (h₁ r h)

theorem Mu.NoRegion.mentions {m : Mu} (h : m.NoRegion) (r : Nat) : m.mentions r = false := by
  cases m with
  | region => exact h.elim
  | _ => rfl

theorem Ty.NoRegion.mentions {t : Ty} (h : t.NoRegion) (r : Nat) : t.mentions r = false := by
  induction t with
  | unit | int | tparam => rfl
  | cell m t ih => simp [Ty.mentions, h.1.mentions, ih h.2]
  | fn m a b iha ihb => simp [Ty.mentions, h.1.mentions, iha h.2.1, ihb h.2.2]
  | prod a b iha ihb => simp [Ty.mentions, iha h.1, ihb h.2]
  | gone m => simp [Ty.mentions, Mu.NoRegion.mentions h]

theorem Ty.NoRegion.regionsIn {t : Ty} (h : t.NoRegion) (μ : Mu) : t.RegionsIn μ := by
  intro r hr
  rw [h.mentions r] at hr
  cases hr

/-- Only `μ`'s region is in a value whose type mentions only that region,
and so in what the closures in it captured. If the type is at most `μ`,
so are its locations. -/
theorem Expr.IsValue.hasLoc_regionsIn {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {v : Expr}
    {s : Ty} (hv : v.IsValue) (ht : HasType fns σ Δ io Γ v s) {μ : Mu} (hs : s.RegionsIn μ)
    (hle : μ ≠ .mutable → AtMost Δ μ s) {l : Nat} (hl : v.HasLoc l) :
    ∃ m t, σ[l]? = some (m, t) ∧ Le Δ m μ ∧ m.RegionsIn μ ∧ t.RegionsIn μ := by
  have top : ∀ {m : Mu}, (μ ≠ .mutable → Le Δ m μ) → Le Δ m μ := by
    intro m h
    by_cases hμ : μ = .mutable
    · subst hμ; exact .top m
    · exact h hμ
  induction hv generalizing s with
  | unit | int => exact hl.elim
  | loc => cases ht with
    | loc h _ =>
      cases hl
      exact ⟨_, _, h, top fun hμ => (hle hμ).1, fun r h => hs r (by simp [Ty.mentions, h]),
        fun r h => hs r (by simp [Ty.mentions, h])⟩
  | @lam b x body => cases ht with
    | @lam _ _ _ _ _ _ _ _ μc _ _ _ _ hrl _ =>
      have hl' : body.HasLoc l := hl
      obtain ⟨m, t, hσ, hm, hmr, htr⟩ := hrl l hl'
      have hμc : μc.RegionsIn μ := fun r hr => hs r (by simp [Ty.mentions, hr])
      exact ⟨m, t, hσ, top fun hμ => hm.trans (hle hμ), hmr.trans hμc, htr.trans hμc⟩
    | @gone _ _ _ _ _ μc _ hgl =>
      have hl' : body.HasLoc l := hl
      obtain ⟨m, t, hσ, hm, hmr, htr, _⟩ := hgl l hl'
      have hμc : μc.RegionsIn μ := fun r hr => hs r (by simp [Ty.mentions, hr])
      exact ⟨m, t, hσ, top fun hμ => hm.trans (hle hμ), hmr.trans hμc, htr.trans hμc⟩
  | pair _ _ ih₁ ih₂ => cases ht with
    | pair h₁ h₂ =>
      exact hl.elim (ih₁ h₁ (fun r h => hs r (by simp [Ty.mentions, h])) (fun hμ => (hle hμ).1))
        (ih₂ h₂ (fun r h => hs r (by simp [Ty.mentions, h])) (fun hμ => (hle hμ).2))

/-- Substituting a value for `x`, where `x` has the value's type or isn't in
scope (in the body of an `imm` closure that didn't capture it). -/
theorem HasType.subst {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ' : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ .none io Γ' e t) {Γ : Ctx} {x : Nat} {v : Expr} (hΓ : ∀ y, y ≠ x → Γ' y = Γ y)
    (hx : ∀ s, Γ' x = some s → HasType fns σ .none io Ctx.empty v s) (hv : v.IsValue) :
    HasType fns σ .none io Γ (e.subst x v) t := by
  induction ht generalizing Γ with
  | @var _ _ y s h =>
    simp only [Expr.subst]
    by_cases hyx : y = x
    · subst hyx
      simp only [ite_true]
      exact (hx _ h).weaken_ctx (Ctx.Sub.empty Γ)
    · simp only [hyx, ite_false]
      exact .var (hΓ y hyx ▸ h)
  | unit => exact .unit
  | int => exact .int
  | loc h hio => exact .loc h hio
  | new _ hd ih => exact .new (ih hΓ hx) hd
  | newR _ hd ih => exact .newR (ih hΓ hx) hd
  | newP _ hi hd ih => exact .newP (ih hΓ hx) hi hd
  | get _ ih => exact .get (ih hΓ hx)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ hx) (ih₂ hΓ hx)
  | setR _ _ ih₁ ih₂ => exact .setR (ih₁ hΓ hx) (ih₂ hΓ hx)
  | setP _ hi _ ih₁ ih₂ => exact .setP (ih₁ hΓ hx) hi (ih₂ hΓ hx)
  | @letE _ Γ₀ y e₁ e₂ s t₂ _ he₂ ih₁ ih₂ =>
    simp only [Expr.subst]
    by_cases hyx : y = x
    · subst hyx
      simp only [ite_true]
      refine .letE (ih₁ hΓ hx) (he₂.weaken_ctx ?_)
      intro z s' hz
      simp only [Ctx.update] at hz ⊢
      by_cases hzy : z = y
      · simpa [hzy] using hz
      · simp only [hzy, ite_false] at hz ⊢
        exact hΓ z hzy ▸ hz
    · simp only [hyx, ite_false]
      refine .letE (ih₁ hΓ hx) (ih₂ ?_ ?_)
      · intro z hzx
        simp only [Ctx.update]
        split
        · rfl
        · exact hΓ z hzx
      · intro s' hs'
        simp only [Ctx.update, Ne.symm hyx, ite_false] at hs'
        exact hx s' hs'
  | call hf hs _ ih => exact .call hf hs (ih hΓ hx)
  | gone hf hl =>
    rw [Expr.subst_not_free _ (hf x)]
    exact .gone hf hl
  | @lam io₀ Γ₀ Γc b y body a r μ hk hc himm hrc hrl hbody ih =>
    -- The closure sees what it saw, without `x`.
    have hsub := Ctx.drop_sub (x := x) hc hΓ
    have himm' : μ ≠ .mutable → ∀ z s, (Γc.drop x) z = some s → AtMost .none μ s :=
      fun hb z s hz => himm hb z s (Ctx.drop_some hz)
    have hrc' : ∀ z s, (Γc.drop x) z = some s → s.RegionsIn μ ∧ s.ParamsNoRegion .none :=
      fun z s hz => hrc z s (Ctx.drop_some hz)
    simp only [Expr.subst]
    by_cases hyx : y = x
    · subst hyx
      simp only [ite_true]
      refine .lam hk hsub himm' hrc' hrl (hbody.weaken_ctx ?_)
      intro z s' hz
      simp only [Ctx.update, Ctx.drop] at hz ⊢
      by_cases hzy : z = y
      · simpa [hzy] using hz
      · simpa [hzy] using hz
    · simp only [hyx, ite_false]
      -- What it captures of `v` has only its region, since its type does.
      have hrl' : ∀ l, (body.subst x v).HasLoc l →
          ∃ m t, σ[l]? = some (m, t) ∧ Le .none m μ ∧ m.RegionsIn μ ∧ t.RegionsIn μ := by
        intro l hl
        rcases hbody.hasLoc_subst hl with hl | ⟨⟨s, hs⟩, hl⟩
        · exact hrl l hl
        · simp only [Ctx.update, Ne.symm hyx, ite_false] at hs
          exact hv.hasLoc_regionsIn (hx s (hc _ _ hs)) (hrc _ _ hs).1 (fun hμ => himm hμ _ _ hs) hl
      refine .lam hk hsub himm' hrc' hrl' (ih ?_ ?_)
      · intro z hzx
        simp only [Ctx.update, Ctx.drop]
        split
        · rfl
        · simp
      · intro s' hs'
        simp only [Ctx.update, Ne.symm hyx, ite_false] at hs'
        have hv' := hx s' (hc _ _ hs')
        cases b
        · -- An `imm` closure only saw `x` if it's `imm` all the way down.
          have hμ : μ = .immutable := by
            rcases hk with h | ⟨h, _⟩
            · exact h
            · cases h
          subst hμ
          exact (hv'.value_imm hv (himm (fun h => nomatch h) x s' hs')).weaken_imm (fun _ => rfl)
        · simpa using hv'
  | app _ _ ih₁ ih₂ => exact .app (ih₁ hΓ hx) (ih₂ hΓ hx)
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ hΓ hx) (ih₂ hΓ hx)
  | fst _ ih => exact .fst (ih hΓ hx)
  | snd _ ih => exact .snd (ih hΓ hx)

/-- Substituting a closed value for a variable just put in scope. -/
theorem HasType.subst_update {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {x : Nat} {t₁ t : Ty}
    {e v : Expr} (ht : HasType fns σ .none io (Γ.update x t₁) e t) (hv : v.IsValue)
    (hvt : HasType fns σ .none io Ctx.empty v t₁) : HasType fns σ .none io Γ (e.subst x v) t := by
  refine ht.subst (fun y hyx => by simp [Ctx.update, hyx]) (fun s hs => ?_) hv
  simp only [Ctx.update, ite_true, Option.some.injEq] at hs
  exact hs ▸ hvt

/-- The types of the variables, with the arguments for the parameters. -/
def Ctx.inst (ms : List Mu) (τs : List Ty) (Γ : Ctx) : Ctx := fun y => (Γ y).map (Ty.inst ms τs)

theorem Ctx.inst_update (ms : List Mu) (τs : List Ty) (Γ : Ctx) (x : Nat) (t : Ty) :
    (Γ.update x t).inst ms τs = (Γ.inst ms τs).update x (t.inst ms τs) := by
  funext y
  simp only [Ctx.inst, Ctx.update]
  split <;> rfl

theorem Ctx.inst_empty (ms : List Mu) (τs : List Ty) : Ctx.empty.inst ms τs = Ctx.empty := rfl

theorem Ctx.Sub.inst {Γ Γ' : Ctx} (h : Γ.Sub Γ') (ms : List Mu) (τs : List Ty) :
    (Γ.inst ms τs).Sub (Γ'.inst ms τs) := by
  intro y s hy
  simp only [Ctx.inst] at hy ⊢
  rcases hΓ : Γ y with _ | t
  · simp [hΓ] at hy
  · rw [hΓ] at hy
    rw [h _ _ hΓ]
    exact hy

/-- Arguments for parameters that can't be given a region add none. -/
theorem Mu.mentions_subst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m : Mu}
    (hp : m.ParamsNoRegion Δ) {r : Nat} (h : (m.subst ms).mentions r = true) : m.mentions r = true := by
  cases m with
  | param i =>
    simp only [Mu.subst] at h
    rw [(hs.2.2.2.1 i hp).1.mentions r] at h
    cases h
  | _ => exact h

theorem Ty.mentions_inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {t : Ty}
    (hp : t.ParamsNoRegion Δ) {r : Nat} (h : (t.inst ms τs).mentions r = true) : t.mentions r = true := by
  induction t with
  | unit | int => cases h
  | cell m t ih =>
    simp only [Ty.inst, Ty.mentions, Bool.or_eq_true] at h ⊢
    exact h.imp (Mu.mentions_subst hs hp.1) (ih hp.2)
  | fn m a b iha ihb =>
    simp only [Ty.inst, Ty.mentions, Bool.or_eq_true] at h ⊢
    rcases h with (h | h) | h
    · exact .inl (.inl (Mu.mentions_subst hs hp.1 h))
    · exact .inl (.inr (iha hp.2.1 h))
    · exact .inr (ihb hp.2.2 h)
  | prod a b iha ihb =>
    simp only [Ty.inst, Ty.mentions, Bool.or_eq_true] at h ⊢
    exact h.imp (iha hp.1) (ihb hp.2)
  | tparam i =>
    simp only [Ty.inst] at h
    rw [(hs.2.2.2.2.1 i hp).1.mentions r] at h
    cases h
  | gone m => exact Mu.mentions_subst hs hp h

/-- And their parameters can't be given one either. -/
theorem Mu.paramsNoRegion_subst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {m : Mu}
    (hp : m.ParamsNoRegion Δ) : (m.subst ms).ParamsNoRegion Δ' := by
  cases m with
  | param i => exact (hs.2.2.2.1 i hp).2
  | _ => trivial

theorem Ty.paramsNoRegion_inst {Δ Δ' : Assume} {ms : List Mu} {τs : List Ty} (hs : Sat Δ' Δ ms τs) {t : Ty}
    (hp : t.ParamsNoRegion Δ) : (t.inst ms τs).ParamsNoRegion Δ' := by
  induction t with
  | unit | int => trivial
  | cell m t ih => exact ⟨Mu.paramsNoRegion_subst hs hp.1, ih hp.2⟩
  | fn m a b iha ihb => exact ⟨Mu.paramsNoRegion_subst hs hp.1, iha hp.2.1, ihb hp.2.2⟩
  | prod a b iha ihb => exact ⟨iha hp.1, ihb hp.2⟩
  | tparam i => exact (hs.2.2.2.2.1 i hp).2
  | gone m => exact Mu.paramsNoRegion_subst hs hp

theorem Mu.mentions_subst_of {ms : List Mu} {m : Mu} {r : Nat} (h : m.mentions r = true) :
    (m.subst ms).mentions r = true := by
  cases m with
  | region => exact h
  | _ => cases h

/-- The key to #243: a body typed under its assumptions, with its
parameters unknown, has the type with any arguments that meet them. This
is where a generic function can't change what it's given (`set` needs
`mutable` itself, not a parameter that may become `immutable`), unless it
assumes the parameter is changeable, so that it becomes `mut` or a region,
and can
only put a value of a type parameter in an `imm` object if it assumes that
the parameter is `imm`. -/
theorem HasType.inst {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) (hn : e.NoLocs) {Δ' : Assume} {ms : List Mu} {τs : List Ty}
    (hs : Sat Δ' Δ ms τs) :
    HasType fns σ Δ' io (Γ.inst ms τs) (e.inst ms τs) (t.inst ms τs) := by
  induction ht with
  | var h => exact .var (by simp [Ctx.inst, h])
  | unit => exact .unit
  | int => exact .int
  | loc _ _ => exact hn.elim
  | @new _ _ _ s b _ hd ih =>
    simp only [Expr.inst, Ty.inst, Mu.subst_muOf]
    have h := AtMost.inst hs hd
    rw [Mu.subst_muOf] at h
    exact .new (ih hn) h
  | newR _ hd ih =>
    simp only [Expr.inst, Ty.inst, Mu.subst]
    have h := AtMost.inst hs hd
    simp only [Mu.subst] at h
    exact .newR (ih hn) h
  | @newP _ _ _ _ i _ hi hd ih =>
    -- The argument is `mut`, a region, or a changeable parameter of the
    -- caller.
    have h := AtMost.inst hs hd
    have hc := hs.2.2.2.2.2 i hi
    simp only [Expr.inst, Ty.inst, Mu.subst] at h ⊢
    revert h hc
    cases ms[i]?.getD .immutable with
    | mutable => intro h _; exact .new (b := true) (ih hn) h
    | region r => intro h _; exact .newR (ih hn) h
    | param j => intro h hc; exact .newP (ih hn) hc h
    | immutable => intro _ hc; exact hc.elim
  | get _ ih =>
    have := ih hn
    simp only [Ty.inst] at this
    exact .get this
  | set _ _ ih₁ ih₂ =>
    have h₁ := ih₁ hn.1
    simp only [Ty.inst, Mu.subst] at h₁
    exact .set h₁ (ih₂ hn.2)
  | setR _ _ ih₁ ih₂ =>
    have h₁ := ih₁ hn.1
    simp only [Ty.inst, Mu.subst] at h₁
    exact .setR h₁ (ih₂ hn.2)
  | @setP _ _ _ _ _ i _ hi _ ih₁ ih₂ =>
    -- The argument is `mut`, a region, or a changeable parameter of the
    -- caller.
    have h₁ := ih₁ hn.1
    have hc := hs.2.2.2.2.2 i hi
    simp only [Ty.inst, Mu.subst] at h₁
    revert h₁ hc
    cases ms[i]?.getD .immutable with
    | mutable => intro h₁ _; exact .set h₁ (ih₂ hn.2)
    | region r => intro h₁ _; exact .setR h₁ (ih₂ hn.2)
    | param j => intro h₁ hc; exact .setP h₁ hc (ih₂ hn.2)
    | immutable => intro _ hc; exact hc.elim
  | letE _ _ ih₁ ih₂ =>
    have h₂ := ih₂ hn.2
    rw [Ctx.inst_update] at h₂
    exact .letE (ih₁ hn.1) h₂
  | @call _ _ f fn ms' τs' _ hf hsat _ ih =>
    simp only [Expr.inst]
    rw [Ty.inst_inst]
    have h := ih hn
    rw [Ty.inst_inst] at h
    exact .call hf (hsat.comp hs) h
  | @lam _ Γ₀ Γc b x _ a r μ hk hc himm hrc _ _ ih =>
    simp only [Expr.inst, Ty.inst]
    have h := ih hn
    rw [Ctx.inst_update] at h
    have hk' : μ.subst ms = muOf b ∨ b = true ∧ ∃ r', μ.subst ms = .region r' := by
      rcases hk with rfl | ⟨hb, r', rfl⟩
      · exact .inl (Mu.subst_muOf ms b)
      · exact .inr ⟨hb, r', rfl⟩
    refine .lam hk' (hc.inst ms τs) (fun hb y s hy => ?_) (fun y s hy => ?_)
      (fun l hl => ((Expr.NoLocs.inst ms τs hn).not_hasLoc l hl).elim) h
    · simp only [Ctx.inst, Option.map_eq_some_iff] at hy
      obtain ⟨s₀, h₀, rfl⟩ := hy
      have hμ : μ ≠ .mutable := fun he => hb (by rw [he]; rfl)
      exact AtMost.inst hs (himm hμ y s₀ h₀)
    · simp only [Ctx.inst, Option.map_eq_some_iff] at hy
      obtain ⟨s₀, h₀, rfl⟩ := hy
      obtain ⟨hr₀, hp₀⟩ := hrc y s₀ h₀
      exact ⟨fun r' h' => Mu.mentions_subst_of (hr₀ r' (Ty.mentions_inst hs hp₀ h')),
        Ty.paramsNoRegion_inst hs hp₀⟩
  | @gone _ _ b x body _ hf _ =>
    simp only [Expr.inst, Ty.inst]
    refine .gone (fun y hy => ?_) (fun l hl => ((Expr.NoLocs.inst ms τs hn).not_hasLoc l hl).elim)
    have hy' : (Expr.lam b x body).Free y := by
      simp only [Expr.Free] at hy ⊢
      exact ⟨hy.1, (Expr.free_inst ms τs y).mp hy.2⟩
    exact hf y hy'
  | app _ _ ih₁ ih₂ =>
    have h₁ := ih₁ hn.1
    simp only [Ty.inst] at h₁
    exact .app h₁ (ih₂ hn.2)
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ hn.1) (ih₂ hn.2)
  | fst _ ih =>
    have h := ih hn
    simp only [Ty.inst] at h
    exact .fst h
  | snd _ ih =>
    have h := ih hn
    simp only [Ty.inst] at h
    exact .snd h

/-- Inside an `imm` closure, every location is `imm`. -/
theorem HasType.hasLoc_imm {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) (hio : io = true) {l : Nat} (hl : e.HasLoc l) :
    ∃ t', σ[l]? = some (.immutable, t') := by
  induction ht with
  | var | unit | int => exact hl.elim
  | loc h hm =>
    cases hl
    exact ⟨_, hm hio ▸ h⟩
  | new _ _ ih | newR _ _ ih | newP _ _ _ ih | get _ ih | call _ _ _ ih | fst _ ih | snd _ ih => exact ih hio hl
  | set _ _ ih₁ ih₂ | setR _ _ ih₁ ih₂ | setP _ _ _ ih₁ ih₂ | letE _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact hl.elim (ih₁ hio) (ih₂ hio)
  | lam _ _ _ _ _ _ ih => exact ih (by simp [hio]) hl
  | gone _ hgl =>
    obtain ⟨m, t, h, _, _, _, hi⟩ := hgl l hl
    exact ⟨t, hi hio ▸ h⟩

end BoltMut
