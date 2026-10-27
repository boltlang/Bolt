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

theorem HasType.weaken_heap {fns : List Fn} {σ σ' : HeapTy} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Γ e t) (hx : Extends σ σ') : HasType fns σ' Γ e t := by
  induction ht with
  | var h => exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h => exact .loc (hx _ _ h)
  | new _ hd ih => exact .new ih hd
  | get _ ih => exact .get ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | call hf hl _ ih => exact .call hf hl ih

/-- Typing only depends on the variables that are in scope. -/
theorem HasType.weaken_ctx {fns : List Fn} {σ : HeapTy} {Γ Γ' : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Γ e t) (hΓ : ∀ x s, Γ x = some s → Γ' x = some s) : HasType fns σ Γ' e t := by
  induction ht generalizing Γ' with
  | var h => exact .var (hΓ _ _ h)
  | unit => exact .unit
  | int => exact .int
  | loc h => exact .loc h
  | new _ hd ih => exact .new (ih hΓ) hd
  | get _ ih => exact .get (ih hΓ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ) (ih₂ hΓ)
  | @letE Γ₀ x e₁ e₂ t₁ t₂ _ _ ih₁ ih₂ =>
    refine .letE (ih₁ hΓ) (ih₂ ?_)
    intro y s hy
    simp only [Ctx.update] at hy ⊢
    by_cases hyx : y = x
    · simpa [hyx] using hy
    · simp only [hyx] at hy ⊢
      exact hΓ _ _ hy
  | call hf hl _ ih => exact .call hf hl (ih hΓ)

/-- A value has its type in any scope. -/
theorem HasType.value_any_ctx {fns : List Fn} {σ : HeapTy} {Γ Γ' : Ctx} {v : Expr} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ Γ v t) : HasType fns σ Γ' v t := by
  cases hv <;> cases ht
  · exact .unit
  · exact .int
  · exact .loc ‹_›

/-- A value of a cell type is a location of that type. -/
theorem canonical_cell {fns : List Fn} {σ : HeapTy} {Γ : Ctx} {v : Expr} {m : Mu} {t : Ty}
    (hv : v.IsValue) (ht : HasType fns σ Γ v (.cell m t)) : ∃ l, v = .loc l ∧ σ[l]? = some (m, t) := by
  cases hv with
  | unit => cases ht
  | int => cases ht
  | loc => cases ht with
    | loc h => exact ⟨_, rfl, h⟩

theorem Ctx.update_shadow (Γ : Ctx) (x : Nat) (t s : Ty) : (Γ.update x t).update x s = Γ.update x s := by
  funext y
  simp only [Ctx.update]
  split <;> rfl

theorem Ctx.update_comm (Γ : Ctx) {x y : Nat} (t s : Ty) (h : y ≠ x) :
    (Γ.update x t).update y s = (Γ.update y s).update x t := by
  funext z
  simp only [Ctx.update]
  by_cases hzy : z = y
  · subst hzy
    simp [h]
  · simp [hzy]

/-- Substituting a value of the variable's type keeps the type. -/
theorem HasType.subst {fns : List Fn} {σ : HeapTy} {Γ' : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Γ' e t) {Γ : Ctx} {x : Nat} {t₁ : Ty} {v : Expr} (hΓ : Γ' = Γ.update x t₁)
    (hv : v.IsValue) (hvt : HasType fns σ Ctx.empty v t₁) : HasType fns σ Γ (e.subst x v) t := by
  induction ht generalizing Γ with
  | @var Γ₀ y s h =>
    subst hΓ
    simp only [Expr.subst]
    by_cases hyx : y = x
    · simp only [hyx, Ctx.update, ite_true] at h ⊢
      cases h
      exact hvt.value_any_ctx hv
    · simp only [hyx, Ctx.update] at h ⊢
      exact .var h
  | unit => exact .unit
  | int => exact .int
  | loc h => exact .loc h
  | new _ hd ih => exact .new (ih hΓ) hd
  | get _ ih => exact .get (ih hΓ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ) (ih₂ hΓ)
  | @letE Γ₀ y e₁ e₂ s t₂ he₁ he₂ ih₁ ih₂ =>
    simp only [Expr.subst]
    subst hΓ
    by_cases hyx : y = x
    · subst hyx
      simp only [ite_true]
      refine .letE (ih₁ rfl) ?_
      rw [Ctx.update_shadow] at he₂
      exact he₂
    · simp only [hyx, ite_false]
      exact .letE (ih₁ rfl) (ih₂ (Ctx.update_comm Γ t₁ s hyx))
  | call hf hl _ ih => exact .call hf hl (ih hΓ)

/-- The types of the variables, with mutability arguments for the
parameters. -/
def Ctx.msubst (ms : List Mu) (Γ : Ctx) : Ctx := fun y => (Γ y).map (Ty.subst ms)

theorem Ctx.msubst_update (ms : List Mu) (Γ : Ctx) (x : Nat) (t : Ty) :
    (Γ.update x t).msubst ms = (Γ.msubst ms).update x (t.subst ms) := by
  funext y
  simp only [Ctx.msubst, Ctx.update]
  split <;> rfl

theorem Ctx.msubst_empty (ms : List Mu) : Ctx.empty.msubst ms = Ctx.empty := rfl

/-- A body typed with its mutability parameters unknown has the type with
any mutabilities for them. This is where a generic function can't change
what it's given: `set` needs `mutable` itself, not a parameter that may
become `immutable`. -/
theorem HasType.msubst {fns : List Fn} {σ : HeapTy} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Γ e t) (hn : e.NoLocs) (ms : List Mu) :
    HasType fns σ (Γ.msubst ms) (e.msubst ms) (t.subst ms) := by
  induction ht with
  | var h => exact .var (by simp [Ctx.msubst, h])
  | unit => exact .unit
  | int => exact .int
  | loc _ => exact hn.elim
  | @new Γ₀ e s b _ hd ih =>
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
  | @call Γ₀ f fn ms' e hf hl _ ih =>
    simp only [Expr.msubst, Ty.subst_subst]
    have h := ih hn
    rw [Ty.subst_subst] at h
    exact .call hf (by simp [hl]) h

end BoltMut
