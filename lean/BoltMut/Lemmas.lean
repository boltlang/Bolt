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
  | get _ ih => exact .get ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | call hf hs _ ih => exact .call hf hs ih
  | lam hc himm _ ih => exact .lam hc himm ih
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
  | get _ ih => exact .get (ih hΓ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hΓ) (ih₂ hΓ)
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ hΓ) (ih₂ (hΓ.update _ _))
  | call hf hs _ ih => exact .call hf hs (ih hΓ)
  | lam hc himm hb _ => exact .lam (hc.trans hΓ) himm hb
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
  | get _ ih => exact .get (ih hio)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ hio) (ih₂ hio)
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ hio) (ih₂ hio)
  | call hf hs _ ih => exact .call hf hs (ih hio)
  | lam hc himm _ ih =>
    refine .lam hc himm (ih ?_)
    intro h
    simp only [Bool.or_eq_true] at h ⊢
    exact h.imp_left hio
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
    | lam hc himm hb =>
      have hb' : b = false := muOf_immutable (Le.none_immutable hd)
      subst hb'
      exact .lam hc himm (hb.weaken_imm (fun _ => by simp))

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
  | call hf hs _ ih => exact .call hf hs (ih hΓ hx)
  | @lam io₀ Γ₀ Γc b y body a r hc himm hbody ih =>
    -- The closure sees what it saw, without `x`.
    have hsub := Ctx.drop_sub (x := x) hc hΓ
    have himm' : b = false → ∀ z s, (Γc.drop x) z = some s → AtMost .none .immutable s :=
      fun hb z s hz => himm hb z s (Ctx.drop_some hz)
    simp only [Expr.subst]
    by_cases hyx : y = x
    · subst hyx
      simp only [ite_true]
      refine .lam hsub himm' (hbody.weaken_ctx ?_)
      intro z s' hz
      simp only [Ctx.update, Ctx.drop] at hz ⊢
      by_cases hzy : z = y
      · simpa [hzy] using hz
      · simpa [hzy] using hz
    · simp only [hyx, ite_false]
      refine .lam hsub himm' (ih ?_ ?_)
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
          exact (hv'.value_imm hv (himm rfl x s' hs')).weaken_imm (fun _ => rfl)
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

/-- The key to #243: a body typed under its assumptions, with its
parameters unknown, has the type with any arguments that meet them. This
is where a generic function can't change what it's given (`set` needs
`mutable` itself, not a parameter that may become `immutable`), and can
only put a value of a type parameter in an `imm` object if it assumes that
the parameter is `imm`. -/
theorem HasType.inst {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) (hn : e.NoLocs) {Δ' : Assume} {ms : List Mu} {τs : List Ty}
    (hs : Sat Δ' Δ ms τs) : HasType fns σ Δ' io (Γ.inst ms τs) (e.inst ms τs) (t.inst ms τs) := by
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
  | get _ ih =>
    have := ih hn
    simp only [Ty.inst] at this
    exact .get this
  | set _ _ ih₁ ih₂ =>
    have h₁ := ih₁ hn.1
    simp only [Ty.inst, Mu.subst] at h₁
    exact .set h₁ (ih₂ hn.2)
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
  | @lam _ Γ₀ Γc b x _ a r hc himm _ ih =>
    simp only [Expr.inst, Ty.inst, Mu.subst_muOf]
    have h := ih hn
    rw [Ctx.inst_update] at h
    refine .lam (hc.inst ms τs) (fun hb y s hy => ?_) h
    simp only [Ctx.inst, Option.map_eq_some_iff] at hy
    obtain ⟨s₀, h₀, rfl⟩ := hy
    exact AtMost.inst hs (himm hb y s₀ h₀)
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
  | new _ _ ih | get _ ih | call _ _ _ ih | fst _ ih | snd _ ih => exact ih hio hl
  | set _ _ ih₁ ih₂ | letE _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact hl.elim (ih₁ hio) (ih₂ hio)
  | lam _ _ _ ih => exact ih (by simp [hio]) hl

end BoltMut
