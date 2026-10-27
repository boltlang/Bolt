import BoltMut.Semantics

/-!
# Lemmas about typing

Weakening, canonical forms, and the two substitutions a call makes: the
mutability arguments for the function's parameters, and the argument for
its variable.
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

theorem HasType.weaken_heap {fns : List Fn} {σ σ' : HeapTy} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ io Γ e t) (hx : Extends σ σ') : HasType fns σ' io Γ e t := by
  induction ht with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h hio => exact .loc (hx _ _ h) hio
  | new _ hd ih => exact .new ih hd
  | get _ ih => exact .get ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | call hf hl _ ih => exact .call hf hl ih
  | lam _ ih => exact .lam ih
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂

/-- `Γ'` has every variable of `Γ`, with the same type. -/
def Ctx.Sub (Γ Γ' : Ctx) : Prop := ∀ (x : Nat) (s : Ty), Γ x = some s → Γ' x = some s

theorem Ctx.Sub.update {Γ Γ' : Ctx} (h : Γ.Sub Γ') (x : Nat) (t : Ty) :
    (Γ.update x t).Sub (Γ'.update x t) := by
  intro y s hy
  simp only [Ctx.update] at hy ⊢
  by_cases hyx : y = x
  · simpa [hyx] using hy
  · simp only [hyx] at hy ⊢
    exact h _ _ hy

theorem Ctx.capture_apply (b : Bool) (Γ : Ctx) (y : Nat) :
    Γ.capture b y = if b then Γ y else (Γ y).filter (fun t => decide (DeepImm t)) := by
  cases b <;> rfl

theorem Ctx.Sub.capture {Γ Γ' : Ctx} (h : Γ.Sub Γ') (b : Bool) : (Γ.capture b).Sub (Γ'.capture b) := by
  intro y s hy
  rw [Ctx.capture_apply] at hy ⊢
  cases b
  · simp only [Bool.false_eq_true, ite_false] at hy ⊢
    rcases hΓ : Γ y with _ | t
    · simp [hΓ] at hy
    · rw [hΓ] at hy
      rw [h _ _ hΓ]
      exact hy
  · exact h _ _ hy

theorem Ctx.capture_congr {Γ Γ' : Ctx} (b : Bool) {y : Nat} (h : Γ y = Γ' y) :
    Γ.capture b y = Γ'.capture b y := by
  rw [Ctx.capture_apply, Ctx.capture_apply, h]

theorem Ctx.capture_empty (b : Bool) : Ctx.empty.capture b = Ctx.empty := by
  funext y
  rw [Ctx.capture_apply]
  cases b <;> rfl

theorem Ctx.Sub.empty (Γ : Ctx) : Ctx.empty.Sub Γ := fun _ _ h => nomatch h

/-- Typing only depends on the variables that are in scope. -/
theorem HasType.weaken_ctx {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ Γ' : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ io Γ e t) (hΓ : Γ.Sub Γ') : HasType fns σ io Γ' e t := by
  induction ht generalizing Γ' with
  | var h => exact .var (hΓ _ _ h)
  | unit => exact .unit
  | int => exact .int
  | loc h hio => exact .loc h hio
  | new _ hd ih => exact .new (ih hΓ) hd
  | get _ ih => exact .get (ih hΓ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ) (ih₂ hΓ)
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ hΓ) (ih₂ (hΓ.update _ _))
  | call hf hl _ ih => exact .call hf hl (ih hΓ)
  | lam _ ih => exact .lam (ih ((hΓ.capture _).update _ _))
  | app _ _ ih₁ ih₂ => exact .app (ih₁ hΓ) (ih₂ hΓ)

/-- Outside an `imm` closure, anything typed inside one has its type too. -/
theorem HasType.weaken_imm {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ io Γ e t) {io' : Bool} (hio : io' = true → io = true) :
    HasType fns σ io' Γ e t := by
  induction ht generalizing io' with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h hm => exact .loc h (fun h' => hm (hio h'))
  | new _ hd ih => exact .new (ih hio) hd
  | get _ ih => exact .get (ih hio)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hio) (ih₂ hio)
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ hio) (ih₂ hio)
  | call hf hl _ ih => exact .call hf hl (ih hio)
  | lam _ ih =>
    refine .lam (ih ?_)
    intro h
    simp only [Bool.or_eq_true] at h ⊢
    exact h.imp_left hio
  | app _ _ ih₁ ih₂ => exact .app (ih₁ hio) (ih₂ hio)

theorem muOf_cases (b : Bool) : muOf b = .mutable ∨ muOf b = .immutable := by
  cases b
  · exact .inr rfl
  · exact .inl rfl

theorem muOf_immutable {b : Bool} (h : muOf b = .immutable) : b = false := by
  cases b
  · rfl
  · cases h

/-- A value whose type is `imm` all the way down can be captured by an `imm`
closure. -/
theorem HasType.value_imm {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {v : Expr} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ io Γ v t) (hd : DeepImm t) : HasType fns σ true Γ v t := by
  cases hv with
  | unit => cases ht; exact .unit
  | int => cases ht; exact .int
  | loc => cases ht with
    | loc h _ => exact .loc h (fun _ => hd.1)
  | @lam b x body => cases ht with
    | lam hb =>
      have hb' : b = false := muOf_immutable hd
      subst hb'
      exact .lam (hb.weaken_imm (fun _ => by simp))

/-- A value of a cell type is a location of that type. -/
theorem canonical_cell {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {v : Expr} {m : Mu} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ io Γ v (.cell m t)) : ∃ l, v = .loc l ∧ σ[l]? = some (m, t) := by
  cases hv with
  | unit => cases ht
  | int => cases ht
  | loc => cases ht with
    | loc h _ => exact ⟨_, rfl, h⟩
  | lam => cases ht

/-- A value of a function type is a closure. -/
theorem canonical_fn {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {v : Expr} {m : Mu} {a r : Ty}
    (hv : v.IsValue) (ht : HasType fns σ io Γ v (.fn m a r)) : ∃ b x body, v = .lam b x body := by
  cases hv with
  | unit => cases ht
  | int => cases ht
  | loc => cases ht
  | lam => exact ⟨_, _, _, rfl⟩

/-- Substituting a value for `x`, where `x` has the value's type or isn't in
scope (in the body of an `imm` closure that couldn't capture it). -/
theorem HasType.subst {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ' : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ io Γ' e t) {Γ : Ctx} {x : Nat} {v : Expr} (hΓ : ∀ y, y ≠ x → Γ' y = Γ y)
    (hx : ∀ s, Γ' x = some s → HasType fns σ io Ctx.empty v s) (hv : v.IsValue) :
    HasType fns σ io Γ (e.subst x v) t := by
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
  | get _ ih => exact .get (ih hΓ hx)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ hx) (ih₂ hΓ hx)
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
  | call hf hl _ ih => exact .call hf hl (ih hΓ hx)
  | @lam io₀ Γ₀ b y body a r hbody ih =>
    simp only [Expr.subst]
    by_cases hyx : y = x
    · subst hyx
      simp only [ite_true]
      refine .lam (hbody.weaken_ctx ?_)
      intro z s' hz
      simp only [Ctx.update] at hz ⊢
      by_cases hzy : z = y
      · simpa [hzy] using hz
      · simp only [hzy, ite_false] at hz ⊢
        rw [← Ctx.capture_congr b (hΓ z hzy)]
        exact hz
    · simp only [hyx, ite_false]
      refine .lam (ih ?_ ?_)
      · intro z hzx
        simp only [Ctx.update]
        split
        · rfl
        · exact Ctx.capture_congr b (hΓ z hzx)
      · intro s' hs'
        simp only [Ctx.update, Ne.symm hyx, ite_false, Ctx.capture_apply] at hs'
        cases b
        · simp only [Bool.false_eq_true, ite_false, Option.filter_eq_some_iff,
            decide_eq_true_eq] at hs'
          obtain ⟨hs, hd⟩ := hs'
          exact (hx s' hs).value_imm hv hd |>.weaken_imm (fun _ => rfl)
        · simp only [ite_true] at hs'
          simpa using hx s' hs'
  | app _ _ ih₁ ih₂ => exact .app (ih₁ hΓ hx) (ih₂ hΓ hx)

/-- Substituting a closed value for a variable just put in scope. -/
theorem HasType.subst_update {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {x : Nat} {t₁ t : Ty}
    {e v : Expr} (ht : HasType fns σ io (Γ.update x t₁) e t) (hv : v.IsValue)
    (hvt : HasType fns σ io Ctx.empty v t₁) : HasType fns σ io Γ (e.subst x v) t := by
  refine ht.subst (fun y hyx => by simp [Ctx.update, hyx]) (fun s hs => ?_) hv
  simp only [Ctx.update, ite_true, Option.some.injEq] at hs
  exact hs ▸ hvt

/-- The types of the variables, with mutability arguments for the
parameters. -/
def Ctx.msubst (ms : List Mu) (Γ : Ctx) : Ctx := fun y => (Γ y).map (Ty.subst ms)

theorem Ctx.msubst_update (ms : List Mu) (Γ : Ctx) (x : Nat) (t : Ty) :
    (Γ.update x t).msubst ms = (Γ.msubst ms).update x (t.subst ms) := by
  funext y
  simp only [Ctx.msubst, Ctx.update]
  split <;> rfl

theorem Ctx.msubst_empty (ms : List Mu) : Ctx.empty.msubst ms = Ctx.empty := rfl

/-- An `imm` closure captures at least as much once the mutability
parameters are known. -/
theorem Ctx.msubst_capture (ms : List Mu) (b : Bool) (Γ : Ctx) :
    ((Γ.capture b).msubst ms).Sub ((Γ.msubst ms).capture b) := by
  intro y s hy
  simp only [Ctx.msubst, Ctx.capture_apply] at hy ⊢
  cases b
  · simp only [Bool.false_eq_true, ite_false] at hy ⊢
    rcases hΓ : Γ y with _ | t
    · simp [hΓ] at hy
    · simp only [hΓ, Option.filter, Option.map] at hy ⊢
      by_cases hd : DeepImm t
      · simp only [hd, decide_true] at hy
        cases hy
        simp [DeepImm.subst ms hd]
      · simp [hd] at hy
  · simpa using hy

/-- A body typed with its mutability parameters unknown has the type with
any mutabilities for them. This is where a generic function can't change
what it's given: `set` needs `mutable` itself, not a parameter that may
become `immutable`. -/
theorem HasType.msubst {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ io Γ e t) (hn : e.NoLocs) (ms : List Mu) :
    HasType fns σ io (Γ.msubst ms) (e.msubst ms) (t.subst ms) := by
  induction ht with
  | var h => exact .var (by simp [Ctx.msubst, h])
  | unit => exact .unit
  | int => exact .int
  | loc _ _ => exact hn.elim
  | @new _ _ _ s b _ hd ih =>
    simp only [Expr.msubst, Ty.subst, Mu.subst_muOf]
    exact .new (ih hn) (fun hb => (hd hb).subst ms)
  | get _ ih =>
    have := ih hn
    simp only [Ty.subst] at this
    exact .get this
  | set _ _ ih₁ ih₂ =>
    have h₁ := ih₁ hn.1
    simp only [Ty.subst] at h₁
    exact .set h₁ (ih₂ hn.2)
  | letE _ _ ih₁ ih₂ =>
    have h₂ := ih₂ hn.2
    rw [Ctx.msubst_update] at h₂
    exact .letE (ih₁ hn.1) h₂
  | @call _ _ f fn ms' _ hf hl _ ih =>
    simp only [Expr.msubst, Ty.subst_subst]
    have h := ih hn
    rw [Ty.subst_subst] at h
    exact .call hf (by simp [hl]) h
  | @lam _ Γ₀ b x _ _ _ _ ih =>
    simp only [Expr.msubst, Ty.subst, Mu.subst_muOf]
    have h := ih hn
    rw [Ctx.msubst_update] at h
    exact .lam (h.weaken_ctx ((Ctx.msubst_capture ms b Γ₀).update _ _))
  | app _ _ ih₁ ih₂ =>
    have h₁ := ih₁ hn.1
    simp only [Ty.subst] at h₁
    exact .app h₁ (ih₂ hn.2)

/-- Inside an `imm` closure, every location is `imm`. -/
theorem HasType.hasLoc_imm {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ io Γ e t) (hio : io = true) {l : Nat} (hl : e.HasLoc l) :
    ∃ t', σ[l]? = some (.immutable, t') := by
  induction ht with
  | var | unit | int => exact hl.elim
  | loc h hm =>
    cases hl
    exact ⟨_, hm hio ▸ h⟩
  | new _ _ ih | get _ ih | call _ _ _ ih => exact ih hio hl
  | set _ _ ih₁ ih₂ | letE _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ =>
    exact hl.elim (ih₁ hio) (ih₂ hio)
  | lam _ ih => exact ih (by simp [hio]) hl

end BoltMut
