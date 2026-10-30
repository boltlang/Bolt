import BoltMut.Soundness

/-!
# `imm` values never change

The promise of Bolt's mutability rules: nothing reachable from an `imm`
value ever changes. Here, that's two facts about a well-typed program:

* `imm_never_changes`: however many steps it takes, a location whose type is
  `imm` holds the same value.
* `reaches_only_imm`: everything reachable from an `imm` location is `imm`,
  since `imm` is deep.

Together, in `reachable_from_imm_never_changes`: nothing reachable from an
`imm` location ever changes. And `safety`: a well-typed program never gets
stuck.
-/

namespace BoltMut

/-- One step doesn't change what an `imm` location holds: the only step that
writes, `set`, needs the location to be `mut`. -/
theorem step_keeps_imm {fns : List Fn} {h h' : Heap} {e e' : Expr} (hs : Step fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty} {l : Nat} {ty : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      σ[l]? = some (.immutable, ty) → h'[l]? = h[l]? := by
  induction hs with
  | newCong _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | new he _ => exact ih hh he hl
    | newR he _ => exact ih hh he hl
  | newVal _ =>
    intro σ t l ty hh _ hl
    exact List.getElem?_append_left (hh.lt hl)
  | getCong _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | get he => exact ih hh he hl
  | getLoc _ => intros; rfl
  | setCong₁ _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set he₁ _ => exact ih hh he₁ hl
    | setR he₁ _ => exact ih hh he₁ hl
  | setCong₂ _ _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set _ he₂ => exact ih hh he₂ hl
    | setR _ he₂ => exact ih hh he₂ hl
  | @setLoc l₀ v _ _ =>
    intro σ t l ty hh ht hl
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set he₁ _ =>
      cases he₁ with
      | loc hσ _ =>
        -- The location written is `mut`, so it isn't `l`.
        have hne : l₀ ≠ l := by
          intro heq
          subst heq
          rw [hσ] at hl
          cases hl
        exact List.getElem?_set_ne hne
    | setR he₁ _ =>
      cases he₁ with
      | loc hσ _ =>
        -- Or it's in a region, so it isn't `l` either.
        have hne : l₀ ≠ l := by
          intro heq
          subst heq
          rw [hσ] at hl
          cases hl
        exact List.getElem?_set_ne hne
  | letCong _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | letE he₁ _ => exact ih hh he₁ hl
  | letVal _ => intros; rfl
  | callCong _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | call _ _ he => exact ih hh he hl
  | callVal _ _ => intros; rfl
  | appCong₁ _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | app he₁ _ => exact ih hh he₁ hl
  | appCong₂ _ _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | app _ he₂ => exact ih hh he₂ hl
  | appVal _ => intros; rfl
  | pairCong₁ _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | pair he₁ _ => exact ih hh he₁ hl
  | pairCong₂ _ _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | pair _ he₂ => exact ih hh he₂ hl
  | fstCong _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | fst he => exact ih hh he hl
  | sndCong _ ih =>
    intro σ t l ty hh ht hl
    cases ht with
    | snd he => exact ih hh he hl
  | fstVal _ _ | sndVal _ _ => intros; rfl

/-- However many steps a well-typed program takes, an `imm` location holds
the same value. -/
theorem imm_never_changes {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty} {l : Nat} {ty : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      σ[l]? = some (.immutable, ty) → h'[l]? = h[l]? := by
  induction hs with
  | refl => intros; rfl
  | step s _ ih =>
    intro σ t l ty hh ht hl
    have h₁ := step_keeps_imm s hh ht hl
    obtain ⟨σ', hx, hh', ht'⟩ := preservation hf s hh ht
    rw [ih hh' ht' (hx _ _ hl), h₁]

/-- `l'` can be reached from `l` through the heap: it's `l`, or reachable
from a location in what `l` holds, which is the location itself or what a
closure captured. -/
inductive Reaches (h : Heap) : Nat → Nat → Prop where
  | refl {l : Nat} : Reaches h l l
  | step {l l₁ l₂ : Nat} {v : Expr} : h[l]? = some v → v.HasLoc l₁ → Reaches h l₁ l₂ → Reaches h l l₂

/-- `imm` is deep: everything reachable from an `imm` location is `imm`. -/
theorem reaches_only_imm {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h)
    {l l' : Nat} (hr : Reaches h l l') :
    ∀ {ty : Ty}, σ[l]? = some (.immutable, ty) → ∃ ty', σ[l']? = some (.immutable, ty') := by
  induction hr with
  | refl => intro ty hl; exact ⟨ty, hl⟩
  | step hv₁ hl₁ _ ih =>
    intro ty hl
    obtain ⟨_, hdeep, v, hv, hval, hvt⟩ := hh.2 _ _ _ hl
    rw [hv₁] at hv
    cases hv
    -- What an `imm` location holds has a type that's `imm` all the way down,
    -- so it can be typed as if inside an `imm` closure, where every location
    -- is `imm`: the location itself, or what an `imm` closure captured.
    obtain ⟨ty', hσ⟩ := (hvt.value_imm hval hdeep).hasLoc_imm rfl hl₁
    exact ih hσ

/-- The promise of `imm`: nothing reachable from an `imm` location ever
changes, however many steps a well-typed program takes. -/
theorem reachable_from_imm_never_changes {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy}
    {h h' : Heap} {e e' : Expr} {t ty : Ty} {l l' : Nat}
    (hh : HeapOk fns σ h) (ht : HasType fns σ .none false Ctx.empty e t) (hs : Steps fns h e h' e')
    (hl : σ[l]? = some (.immutable, ty)) (hr : Reaches h l l') : h'[l']? = h[l']? := by
  obtain ⟨ty', hl'⟩ := reaches_only_imm hh hr hl
  exact imm_never_changes hf hs hh ht hl'

/-- A well-typed program never gets stuck: after any number of steps, it's a
value or can take another step. -/
theorem safety {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      e'.IsValue ∨ ∃ h'' e'', Step fns h' e' h'' e'' := by
  induction hs with
  | refl => intro σ t hh ht; exact progress hh ht
  | step s _ ih =>
    intro σ t hh ht
    obtain ⟨σ', _, hh', ht'⟩ := preservation hf s hh ht
    exact ih hh' ht'

end BoltMut

namespace BoltMut

/-- A program starts with an empty heap, which matches the empty typing. -/
theorem HeapOk.empty (fns : List Fn) : HeapOk fns [] [] :=
  ⟨rfl, fun _ _ _ h => by simp at h⟩

/-- Preservation over any number of steps. -/
theorem steps_preservation {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
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

/-- For a whole program, run from an empty heap: at any point, there's a
heap typing for the heap it has reached, such that nothing reachable from
a location that's `imm` in it ever changes afterwards. -/
theorem program_imm_never_changes {fns : List Fn} (hf : FnsOk fns) {e e₁ : Expr} {t : Ty}
    {h₁ : Heap} (ht : HasType fns [] .none false Ctx.empty e t) (hs₁ : Steps fns [] e h₁ e₁) :
    ∃ σ, HeapOk fns σ h₁ ∧
      ∀ {l l' : Nat} {ty : Ty} {h₂ : Heap} {e₂ : Expr}, σ[l]? = some (.immutable, ty) →
        Reaches h₁ l l' → Steps fns h₁ e₁ h₂ e₂ → h₂[l']? = h₁[l']? := by
  obtain ⟨σ, _, hh, ht₁⟩ := steps_preservation hf hs₁ (HeapOk.empty fns) ht
  exact ⟨σ, hh, fun hl hr hs₂ => reachable_from_imm_never_changes hf hh ht₁ hs₂ hl hr⟩

end BoltMut
