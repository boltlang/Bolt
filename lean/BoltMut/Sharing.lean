import BoltMut.Immutability

/-!
# Copying and sharing `imm` values (#254)

An `imm` value never changes (`imm_never_changes`), so a copy of it can't be
told apart from it: the code generator can store small `imm` structs by
value, and make `imm` literals constants that are shared. This proves it.

Two runs are compared: one on a heap, and one on a heap where `imm`
locations are laid out differently, like with a copy of one, or two equal
ones merged. A *correspondence* `ρ` says which location of the first is
which of the second (`Rel`, `Corr`). An `imm` location can correspond to
several, but a `mut` one, or one of a region, corresponds to exactly one,
both ways: writing to it must be seen through every name for it, on both
sides.

`steps_sim`: whatever the first run does, the second does too, step for
step, with results that correspond. An integer is the same in both
(`Rel.int`). So:

- `imm_copy_changes_nothing`: using a fresh copy of an `imm` location for
  some of its uses computes the same;
- `imm_merge_changes_nothing`: using one of two `imm` locations that hold
  the same value for both computes the same.
-/

namespace BoltMut

/-- `e₁` and `e₂` are the same, but for their locations: a location `a` in
`e₁` is one `c` that `ρ` relates it to in `e₂`. -/
inductive Rel (ρ : Nat → Nat → Prop) : Expr → Expr → Prop where
  | var {x : Nat} : Rel ρ (.var x) (.var x)
  | unit : Rel ρ .unit .unit
  | int {n : Int} : Rel ρ (.int n) (.int n)
  | loc {a c : Nat} : ρ a c → Rel ρ (.loc a) (.loc c)
  | new {b : Bool} {e₁ e₂ : Expr} : Rel ρ e₁ e₂ → Rel ρ (.new b e₁) (.new b e₂)
  | get {e₁ e₂ : Expr} : Rel ρ e₁ e₂ → Rel ρ (.get e₁) (.get e₂)
  | set {a₁ a₂ b₁ b₂ : Expr} : Rel ρ a₁ a₂ → Rel ρ b₁ b₂ → Rel ρ (.set a₁ b₁) (.set a₂ b₂)
  | letE {x : Nat} {a₁ a₂ b₁ b₂ : Expr} : Rel ρ a₁ a₂ → Rel ρ b₁ b₂ → Rel ρ (.letE x a₁ b₁) (.letE x a₂ b₂)
  | call {f : Nat} {ms : List Mu} {τs : List Ty} {e₁ e₂ : Expr} :
      Rel ρ e₁ e₂ → Rel ρ (.call f ms τs e₁) (.call f ms τs e₂)
  | lam {b : Bool} {x : Nat} {e₁ e₂ : Expr} : Rel ρ e₁ e₂ → Rel ρ (.lam b x e₁) (.lam b x e₂)
  | app {a₁ a₂ b₁ b₂ : Expr} : Rel ρ a₁ a₂ → Rel ρ b₁ b₂ → Rel ρ (.app a₁ b₁) (.app a₂ b₂)
  | pair {a₁ a₂ b₁ b₂ : Expr} : Rel ρ a₁ a₂ → Rel ρ b₁ b₂ → Rel ρ (.pair a₁ b₁) (.pair a₂ b₂)
  | fst {e₁ e₂ : Expr} : Rel ρ e₁ e₂ → Rel ρ (.fst e₁) (.fst e₂)
  | snd {e₁ e₂ : Expr} : Rel ρ e₁ e₂ → Rel ρ (.snd e₁) (.snd e₂)

theorem Rel.mono {ρ ρ' : Nat → Nat → Prop} (hρ : ∀ a c, ρ a c → ρ' a c) {e₁ e₂ : Expr} (h : Rel ρ e₁ e₂) :
    Rel ρ' e₁ e₂ := by
  induction h with
  | var => exact .var
  | unit => exact .unit
  | int => exact .int
  | loc h => exact .loc (hρ _ _ h)
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | call _ ih => exact .call ih
  | lam _ ih => exact .lam ih
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih

/-- What corresponds to a value is a value. -/
theorem Rel.isValue {ρ : Nat → Nat → Prop} {e₁ e₂ : Expr} (h : Rel ρ e₁ e₂) (hv : e₁.IsValue) : e₂.IsValue := by
  induction h with
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam => exact .lam
  | pair _ _ ih₁ ih₂ =>
    cases hv with
    | pair h₁ h₂ => exact .pair (ih₁ h₁) (ih₂ h₂)
  | _ => cases hv

/-- What corresponds to an integer is the same integer. -/
theorem Rel.int_eq {ρ : Nat → Nat → Prop} {n : Int} {e : Expr} (h : Rel ρ (.int n) e) : e = .int n := by
  cases h
  rfl

/-- An expression corresponds to itself when its locations do. -/
theorem Rel.refl {ρ : Nat → Nat → Prop} : ∀ {e : Expr}, (∀ l, e.HasLoc l → ρ l l) → Rel ρ e e
  | .var _, _ => .var
  | .unit, _ => .unit
  | .int _, _ => .int
  | .loc l, h => .loc (h l rfl)
  | .new _ e, h => .new (Rel.refl (e := e) h)
  | .get e, h => .get (Rel.refl (e := e) h)
  | .call _ _ _ e, h => .call (Rel.refl (e := e) h)
  | .lam _ _ e, h => .lam (Rel.refl (e := e) h)
  | .fst e, h => .fst (Rel.refl (e := e) h)
  | .snd e, h => .snd (Rel.refl (e := e) h)
  | .set _ _, h => .set (Rel.refl fun l hl => h l (.inl hl)) (Rel.refl fun l hl => h l (.inr hl))
  | .letE _ _ _, h => .letE (Rel.refl fun l hl => h l (.inl hl)) (Rel.refl fun l hl => h l (.inr hl))
  | .app _ _, h => .app (Rel.refl fun l hl => h l (.inl hl)) (Rel.refl fun l hl => h l (.inr hl))
  | .pair _ _, h => .pair (Rel.refl fun l hl => h l (.inl hl)) (Rel.refl fun l hl => h l (.inr hl))

/-- Substituting corresponding values keeps expressions corresponding. -/
theorem Rel.subst {ρ : Nat → Nat → Prop} {e₁ e₂ v₁ v₂ : Expr} (h : Rel ρ e₁ e₂) (hv : Rel ρ v₁ v₂) (x : Nat) :
    Rel ρ (e₁.subst x v₁) (e₂.subst x v₂) := by
  induction h with
  | @var y =>
    simp only [Expr.subst]
    split
    · exact hv
    · exact .var
  | unit => exact .unit
  | int => exact .int
  | loc h => exact .loc h
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | @letE y _ _ _ _ h₁ h₂ ih₁ ih₂ =>
    simp only [Expr.subst]
    split
    · exact .letE ih₁ h₂
    · exact .letE ih₁ ih₂
  | call _ ih => exact .call ih
  | @lam _ y _ _ h ih =>
    simp only [Expr.subst]
    split
    · exact .lam h
    · exact .lam ih
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih

/-- The locations in a well-typed expression are in its heap typing. -/
theorem HasType.hasLoc_lt {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) {l : Nat} (hl : e.HasLoc l) : l < σ.length := by
  induction ht with
  | var | unit | int => exact hl.elim
  | loc hσ _ =>
    simp only [Expr.HasLoc] at hl
    subst hl
    exact (List.getElem?_eq_some_iff.mp hσ).1
  | new _ _ ih | newR _ _ ih | get _ ih | call _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih => exact ih hl
  | set _ _ ih₁ ih₂ | setR _ _ ih₁ ih₂ | letE _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact hl.elim ih₁ ih₂

/-! ## Heaps that correspond -/

/-- The heaps `h₁` and `h₂` correspond by `ρ`: corresponding locations hold
corresponding values, and a location in `M`, which has the `mut` ones of
`h₁`, corresponds to exactly one, both ways. -/
structure Corr (M : Nat → Prop) (ρ : Nat → Nat → Prop) (h₁ h₂ : Heap) : Prop where
  heap : ∀ a c, ρ a c → ∃ v₁ v₂, h₁[a]? = some v₁ ∧ h₂[c]? = some v₂ ∧ Rel ρ v₁ v₂
  inj : ∀ a b c, ρ a c → ρ b c → M a → a = b
  func : ∀ a c d, ρ a c → ρ a d → M a → c = d

theorem Corr.lt₁ {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂) {a c : Nat}
    (h : ρ a c) : a < h₁.length := by
  obtain ⟨_, _, hv, _, _⟩ := hc.heap a c h
  exact (List.getElem?_eq_some_iff.mp hv).1

theorem Corr.lt₂ {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂) {a c : Nat}
    (h : ρ a c) : c < h₂.length := by
  obtain ⟨_, _, _, hv, _⟩ := hc.heap a c h
  exact (List.getElem?_eq_some_iff.mp hv).1

/-- What's past the end of the first heap can be in `M` too: nothing
corresponds to it. -/
theorem Corr.widen {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂) :
    Corr (fun a => M a ∨ h₁.length ≤ a) ρ h₁ h₂ where
  heap := hc.heap
  inj a b c hac hbc hM := hM.elim (hc.inj a b c hac hbc) fun hle => absurd (hc.lt₁ hac) (by omega)
  func a c d hac had hM := hM.elim (hc.func a c d hac had) fun hle => absurd (hc.lt₁ hac) (by omega)

/-- One step of the first run is one of the second, and they still
correspond. -/
theorem step_sim {fns : List Fn} (hf : FnsOk fns) {h₁ h₁' : Heap} {e₁ e₁' : Expr} (hs : Step fns h₁ e₁ h₁' e₁') :
    ∀ {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {e₂ : Expr},
      HeapOk fns σ h₁ → HasType fns σ .none false Ctx.empty e₁ t →
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) → Corr M ρ h₁ h₂ → Rel ρ e₁ e₂ →
      ∃ ρ' h₂' e₂', Step fns h₂ e₂ h₂' e₂' ∧ (∀ a c, ρ a c → ρ' a c) ∧
        Corr (fun a => M a ∨ h₁.length ≤ a) ρ' h₁' h₂' ∧ Rel ρ' e₁' e₂' := by
  induction hs with
  | newCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | new he _ | newR he _ =>
      cases hr with
      | new hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .newCong s₂, hρ, hc', .new hr'⟩
  | @newVal e b hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | @new _ _ e' hr =>
      let ρ' : Nat → Nat → Prop := fun a c => ρ a c ∨ (a = h₁.length ∧ c = h₂.length)
      have old : ∀ a c, ρ a c → ρ' a c := fun _ _ h => .inl h
      refine ⟨ρ', h₂ ++ [e'], .loc h₂.length, .newVal (hr.isValue hv), old, ?_, .loc (.inr ⟨rfl, rfl⟩)⟩
      refine ⟨?_, ?_, ?_⟩
      · intro a c hac
        rcases hac with hac | ⟨rfl, rfl⟩
        · obtain ⟨v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap a c hac
          refine ⟨v₁, v₂, ?_, ?_, hrel.mono old⟩
          · rw [List.getElem?_append_left (hc.lt₁ hac)]
            exact h₁v
          · rw [List.getElem?_append_left (hc.lt₂ hac)]
            exact h₂v
        · exact ⟨e, e', by simp, by simp, hr.mono old⟩
      · intro a b c hac hbc hMa
        rcases hac with hac | ⟨ha, hca⟩ <;> rcases hbc with hbc | ⟨hb, hcb⟩
        · exact hMa.elim (hc.inj a b c hac hbc) fun hle => absurd (hc.lt₁ hac) (by omega)
        · exact absurd (hc.lt₂ hac) (by omega)
        · exact absurd (hc.lt₂ hbc) (by omega)
        · omega
      · intro a c d hac had hMa
        rcases hac with hac | ⟨ha, hca⟩ <;> rcases had with had | ⟨ha', hda⟩
        · exact hMa.elim (hc.func a c d hac had) fun hle => absurd (hc.lt₁ hac) (by omega)
        · exact absurd (hc.lt₁ hac) (by omega)
        · exact absurd (hc.lt₁ had) (by omega)
        · omega
  | getCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | get he =>
      cases hr with
      | get hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .getCong s₂, hρ, hc', .get hr'⟩
  | @getLoc l v hl =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | get hr =>
      cases hr with
      | loc hρ =>
        obtain ⟨v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap _ _ hρ
        rw [hl] at h₁v
        cases h₁v
        exact ⟨ρ, h₂, v₂, .getLoc h₂v, fun _ _ h => h, hc.widen, hrel⟩
  | setCong₁ _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | set he₁ _ | setR he₁ _ =>
      cases hr with
      | set hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁
        exact ⟨ρ', h₂', _, .setCong₁ s₂, hρ, hc', .set hr' (hr₂.mono hρ)⟩
  | setCong₂ hv _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | set _ he₂ | setR _ he₂ =>
      cases hr with
      | set hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂
        exact ⟨ρ', h₂', _, .setCong₂ (hr₁.isValue hv) s₂, hρ, hc', .set (hr₁.mono hρ) hr'⟩
  | @setLoc l₀ v hl hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | set he₁ _ | setR he₁ _ =>
      cases he₁ with
      | loc hσ _ =>
        have hMl : M l₀ := hM _ _ _ hσ (by intro h; cases h)
        cases hr with
        | @set _ _ _ v' hr₁ hr₂ =>
          cases hr₁ with
          | @loc _ c hρ =>
            have hc₂ : c < h₂.length := hc.lt₂ hρ
            refine ⟨ρ, h₂.set c v', .unit, .setLoc hc₂ (hr₂.isValue hv), fun _ _ h => h, ?_, .unit⟩
            have key : Corr M ρ (h₁.set l₀ v) (h₂.set c v') := by
              refine ⟨?_, hc.inj, hc.func⟩
              intro a d had
              obtain ⟨v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap a d had
              by_cases ha : a = l₀
              · subst ha
                have hd : d = c := hc.func _ _ _ had hρ hMl
                subst hd
                exact ⟨v, v', List.getElem?_set_self hl, List.getElem?_set_self hc₂, hr₂⟩
              · have hd : d ≠ c := fun hd => ha (hc.inj l₀ a c hρ (hd ▸ had) hMl).symm
                refine ⟨v₁, v₂, ?_, ?_, hrel⟩
                · rw [List.getElem?_set_ne (Ne.symm ha)]
                  exact h₁v
                · rw [List.getElem?_set_ne (Ne.symm hd)]
                  exact h₂v
            have := key.widen
            simp only [List.length_set] at this
            exact this
  | letCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | letE he₁ _ =>
      cases hr with
      | letE hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁
        exact ⟨ρ', h₂', _, .letCong s₂, hρ, hc', .letE hr' (hr₂.mono hρ)⟩
  | @letVal x v _ hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | letE hr₁ hr₂ =>
      exact ⟨ρ, h₂, _, .letVal (hr₁.isValue hv), fun _ _ h => h, hc.widen, hr₂.subst hr₁ x⟩
  | callCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | call _ _ he =>
      cases hr with
      | call hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .callCong s₂, hρ, hc', .call hr'⟩
  | @callVal f fn ms τs v hfn hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | call hr =>
      -- A function's body has no locations, so it corresponds to itself.
      have hb := ((hf fn (List.mem_of_getElem? hfn)).1).inst ms τs
      have hbody : Rel ρ (fn.body.inst ms τs) (fn.body.inst ms τs) :=
        Rel.refl fun l hl => (hb.not_hasLoc l hl).elim
      exact ⟨ρ, h₂, _, .callVal hfn (hr.isValue hv), fun _ _ h => h, hc.widen, hbody.subst hr fn.param⟩
  | appCong₁ _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | app he₁ _ =>
      cases hr with
      | app hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁
        exact ⟨ρ', h₂', _, .appCong₁ s₂, hρ, hc', .app hr' (hr₂.mono hρ)⟩
  | appCong₂ hv _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | app _ he₂ =>
      cases hr with
      | app hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂
        exact ⟨ρ', h₂', _, .appCong₂ (hr₁.isValue hv) s₂, hρ, hc', .app (hr₁.mono hρ) hr'⟩
  | @appVal b x body v hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | app hr₁ hr₂ =>
      cases hr₁ with
      | lam hbody =>
        exact ⟨ρ, h₂, _, .appVal (hr₂.isValue hv), fun _ _ h => h, hc.widen, hbody.subst hr₂ x⟩
  | pairCong₁ _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | pair he₁ _ =>
      cases hr with
      | pair hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁
        exact ⟨ρ', h₂', _, .pairCong₁ s₂, hρ, hc', .pair hr' (hr₂.mono hρ)⟩
  | pairCong₂ hv _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | pair _ he₂ =>
      cases hr with
      | pair hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂
        exact ⟨ρ', h₂', _, .pairCong₂ (hr₁.isValue hv) s₂, hρ, hc', .pair (hr₁.mono hρ) hr'⟩
  | fstCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | fst he =>
      cases hr with
      | fst hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .fstCong s₂, hρ, hc', .fst hr'⟩
  | sndCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | snd he =>
      cases hr with
      | snd hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .sndCong s₂, hρ, hc', .snd hr'⟩
  | fstVal hv₁ hv₂ =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | fst hr =>
      cases hr with
      | pair hr₁ hr₂ =>
        exact ⟨ρ, h₂, _, .fstVal (hr₁.isValue hv₁) (hr₂.isValue hv₂), fun _ _ h => h, hc.widen, hr₁⟩
  | sndVal hv₁ hv₂ =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | snd hr =>
      cases hr with
      | pair hr₁ hr₂ =>
        exact ⟨ρ, h₂, _, .sndVal (hr₁.isValue hv₁) (hr₂.isValue hv₂), fun _ _ h => h, hc.widen, hr₂⟩

/-- However many steps the first run takes, the second takes them too, and
they still correspond. -/
theorem steps_sim {fns : List Fn} (hf : FnsOk fns) {h₁ h₁' : Heap} {e₁ e₁' : Expr} (hs : Steps fns h₁ e₁ h₁' e₁') :
    ∀ {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {e₂ : Expr},
      HeapOk fns σ h₁ → HasType fns σ .none false Ctx.empty e₁ t →
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) → Corr M ρ h₁ h₂ → Rel ρ e₁ e₂ →
      ∃ (M' : Nat → Prop) (ρ' : Nat → Nat → Prop) (h₂' : Heap) (e₂' : Expr),
        Steps fns h₂ e₂ h₂' e₂' ∧ Corr M' ρ' h₁' h₂' ∧ Rel ρ' e₁' e₂' := by
  induction hs with
  | refl =>
    intro σ t M ρ h₂ e₂ _ _ _ hc hr
    exact ⟨M, ρ, h₂, e₂, .refl, hc, hr⟩
  | @step h₀ _ _ _ _ _ s _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    obtain ⟨ρ', h₂', e₂', s₂, _, hc', hr'⟩ := step_sim hf s hh ht hM hc hr
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
    obtain ⟨M'', ρ'', h₂'', e₂'', ss, hc'', hr''⟩ := ih hh' ht' hM' hc' hr'
    exact ⟨M'', ρ'', h₂'', e₂'', .step s₂ ss, hc'', hr''⟩

/-! ## Copying and merging -/

/-- What a location of a well-typed heap holds corresponds to itself, when
each location of the heap does. -/
theorem HeapOk.rel_self {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {ρ : Nat → Nat → Prop}
    (hρ : ∀ a, a < h.length → ρ a a) {a : Nat} {w : Expr} (hw : h[a]? = some w) : Rel ρ w w := by
  have ha : a < σ.length := hh.1 ▸ (List.getElem?_eq_some_iff.mp hw).1
  obtain ⟨⟨m, t⟩, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem ha⟩
  obtain ⟨_, _, w', hw', _, hwt⟩ := hh.2 a m t hp
  rw [hw] at hw'
  cases hw'
  exact Rel.refl fun l hl => hρ l (by rw [hh.1]; exact hwt.hasLoc_lt hl)

theorem HeapOk.get {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {a : Nat} {p : Mu × Ty}
    (hp : σ[a]? = some p) : ∃ w, h[a]? = some w :=
  ⟨_, List.getElem?_eq_getElem (hh.lt hp)⟩

/-- The correspondence of a copy of `l`, at the end of a heap of `n`
locations: each location to itself, and `l` to its copy too. -/
def copyCorr (n l : Nat) : Nat → Nat → Prop := fun a c => (a = c ∧ a < n) ∨ (a = l ∧ c = n)

/-- **A copy of an `imm` value can be used for it.** With `l` an `imm`
location of `h`, and `v` what it holds: a program `e₂` that's `e₁` with
the copy of `v` at the end of the heap in place of `l` in some places
computes what `e₁` does, with a result that corresponds, and the same
integer. -/
theorem imm_copy_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {l : Nat}
    {t t₀ : Ty} {v e₁ e₂ v₁ : Expr} (hh : HeapOk fns σ h) (hl : σ[l]? = some (.immutable, t))
    (hv : h[l]? = some v) (ht : HasType fns σ .none false Ctx.empty e₁ t₀) (hr : Rel (copyCorr h.length l) e₁ e₂)
    (hs : Steps fns h e₁ h' v₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (v₂ : Expr),
      Steps fns (h ++ [v]) e₂ h₂ v₂ ∧ Rel ρ v₁ v₂ ∧ ∀ n, v₁ = .int n → v₂ = .int n := by
  let M : Nat → Prop := fun a => ∃ m t', σ[a]? = some (m, t') ∧ m ≠ .immutable
  have notM : ¬ M l := fun ⟨_, _, h', hm⟩ => by rw [hl] at h'; cases h'; exact hm rfl
  have self : ∀ {a w}, h[a]? = some w → Rel (copyCorr h.length l) w w :=
    fun hw => hh.rel_self (fun _ ha => .inl ⟨rfl, ha⟩) hw
  have hc : Corr M (copyCorr h.length l) h (h ++ [v]) := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c hac
      rcases hac with ⟨rfl, ha⟩ | ⟨rfl, rfl⟩
      · obtain ⟨w, hw⟩ : ∃ w, h[a]? = some w := ⟨_, List.getElem?_eq_getElem ha⟩
        exact ⟨w, w, hw, by rw [List.getElem?_append_left ha]; exact hw, self hw⟩
      · exact ⟨v, v, hv, by simp, self hv⟩
    · intro a b c hac hbc hMa
      rcases hac with ⟨hac, ha⟩ | ⟨rfl, _⟩
      · rcases hbc with ⟨hbc, _⟩ | ⟨_, hcb⟩
        · omega
        · omega
      · exact (notM hMa).elim
    · intro a c d hac had hMa
      rcases hac with ⟨hac, _⟩ | ⟨rfl, _⟩
      · rcases had with ⟨had, _⟩ | ⟨rfl, _⟩
        · omega
        · exact (notM hMa).elim
      · exact (notM hMa).elim
  obtain ⟨_, ρ, h₂, v₂, ss, _, hr'⟩ := steps_sim hf hs hh ht (fun _ m t' h hm => ⟨m, t', h, hm⟩) hc hr
  exact ⟨ρ, h₂, v₂, ss, hr', fun n hn => (hn ▸ hr').int_eq⟩

/-- The correspondence of merging `l'` into `l`, in a heap of `n`
locations: each location to itself, and `l'` to `l` too. -/
def mergeCorr (n l l' : Nat) : Nat → Nat → Prop := fun a c => (a = c ∧ a < n) ∨ (a = l' ∧ c = l)

/-- **Equal `imm` values can be shared.** With `l` and `l'` `imm` locations
of `h` that hold the same value: a program `e₂` that's `e₁` with `l` in
place of `l'` in some places computes what `e₁` does, with a result that
corresponds, and the same integer. -/
theorem imm_merge_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {l l' : Nat}
    {t t' t₀ : Ty} {e₁ e₂ v₁ : Expr} (hh : HeapOk fns σ h) (hl : σ[l]? = some (.immutable, t))
    (hl' : σ[l']? = some (.immutable, t')) (heq : h[l']? = h[l]?)
    (ht : HasType fns σ .none false Ctx.empty e₁ t₀) (hr : Rel (mergeCorr h.length l l') e₁ e₂)
    (hs : Steps fns h e₁ h' v₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (v₂ : Expr),
      Steps fns h e₂ h₂ v₂ ∧ Rel ρ v₁ v₂ ∧ ∀ n, v₁ = .int n → v₂ = .int n := by
  let M : Nat → Prop := fun a => ∃ m t', σ[a]? = some (m, t') ∧ m ≠ .immutable
  have notM : ∀ {a t}, σ[a]? = some (.immutable, t) → ¬ M a :=
    fun ha ⟨_, _, h', hm⟩ => by rw [ha] at h'; cases h'; exact hm rfl
  have self : ∀ {a w}, h[a]? = some w → Rel (mergeCorr h.length l l') w w :=
    fun hw => hh.rel_self (fun _ ha => .inl ⟨rfl, ha⟩) hw
  have hc : Corr M (mergeCorr h.length l l') h h := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c hac
      rcases hac with ⟨rfl, ha⟩ | ⟨rfl, rfl⟩
      · obtain ⟨w, hw⟩ : ∃ w, h[a]? = some w := ⟨_, List.getElem?_eq_getElem ha⟩
        exact ⟨w, w, hw, hw, self hw⟩
      · obtain ⟨w, hw⟩ := hh.get hl'
        exact ⟨w, w, hw, heq ▸ hw, self hw⟩
    · intro a b c hac hbc hMa
      rcases hac with ⟨hac, _⟩ | ⟨rfl, _⟩
      · rcases hbc with ⟨hbc, _⟩ | ⟨_, hcb⟩
        · omega
        · -- `a` is `l`, which isn't `mut`.
          exact (notM hl (hcb ▸ hac ▸ hMa)).elim
      · exact (notM hl' hMa).elim
    · intro a c d hac had hMa
      rcases hac with ⟨hac, _⟩ | ⟨rfl, _⟩
      · rcases had with ⟨had, _⟩ | ⟨rfl, _⟩
        · omega
        · exact (notM hl' hMa).elim
      · exact (notM hl' hMa).elim
  obtain ⟨_, ρ, h₂, v₂, ss, _, hr'⟩ := steps_sim hf hs hh ht (fun _ m t' h hm => ⟨m, t', h, hm⟩) hc hr
  exact ⟨ρ, h₂, v₂, ss, hr', fun n hn => (hn ▸ hr').int_eq⟩

end BoltMut
