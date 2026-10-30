import BoltMut.ElideAll

/-!
# Every freeze of a program can be in place

`freezes_in_place_change_nothing` needs that, whenever the program gets to
a freeze, what's around it can't reach the region's objects (`Expr.Sep`).
Here, that's so for a program written without freezes, whose blocks make
them (#292): `blocks_freeze_in_place`.

A running program keeps an order (`Expr.Run`): freezes are only where it's
running, each of a region `2 * n` made by a block, and what's beside one
has only locations older than its marker, `n`. With the allocation order
(`AllocOk`), such a location's type can't reach the region.
-/

namespace BoltMut

/-- `e` has no freeze, even in a closure or a block: a program as it's
written. -/
def Expr.NoFreeze : Expr → Prop
  | .var _ | .unit | .int _ | .loc _ => True
  | .new _ e | .get e | .call _ _ _ e | .lam _ _ e | .fst e | .snd e => e.NoFreeze
  | .block c _ _ e => c = false ∧ e.NoFreeze
  | .set a b | .letE _ a b | .app a b | .pair a b => a.NoFreeze ∧ b.NoFreeze
  | .freeze _ _ _ | .keep _ _ _ => False

/-- `b`'s locations are older than the markers of the regions that `a`
freezes. -/
def Expr.Before (a b : Expr) : Prop := ∀ r, a.Freezes r → ∀ l, b.HasLoc l → l < r / 2

/-- The order a running program keeps. -/
def Expr.Run : Expr → Prop
  | .var _ | .unit | .int _ | .loc _ => True
  | .lam _ _ e => e.NoFreeze
  | .block c _ _ e => c = false ∧ e.NoFreeze
  | .new _ e | .get e | .call _ _ _ e | .fst e | .snd e => e.Run
  | .freeze r _ e => r % 2 = 0 ∧ e.Run
  | .keep _ _ _ => False
  | .set a b | .app a b | .pair a b =>
    a.Run ∧ b.Run ∧ (a.IsValue ∨ b.NoFreeze) ∧ Expr.Before a b ∧ Expr.Before b a
  | .letE _ a b => a.Run ∧ b.NoFreeze ∧ Expr.Before a b

/-- Every value in the heap has no freeze. -/
def HeapNoFreeze (h : Heap) : Prop := ∀ (l : Nat) (v : Expr), h[l]? = some v → v.NoFreeze

theorem Expr.NoFreeze.not_freezes {e : Expr} (h : e.NoFreeze) (r : Nat) : ¬ e.Freezes r := by
  induction e with
  | var | unit | int | loc => exact id
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih =>
    exact ih h
  | block _ _ _ _ ih => exact ih h.2
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact fun hf => hf.elim (ih₁ h.1) (ih₂ h.2)
  | freeze | keep => exact h.elim

theorem Expr.Before.of_noFreeze {a b : Expr} (h : a.NoFreeze) : Expr.Before a b :=
  fun r hr => (h.not_freezes r hr).elim

theorem Expr.NoFreeze.run {e : Expr} (h : e.NoFreeze) : e.Run := by
  induction e with
  | var | unit | int | loc => trivial
  | lam => exact h
  | block => exact h
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact ⟨ih₁ h.1, ih₂ h.2, .inr h.2, .of_noFreeze h.1, .of_noFreeze h.2⟩
  | letE _ _ _ ih₁ _ => exact ⟨ih₁ h.1, h.2, .of_noFreeze h.1⟩
  | freeze | keep => exact h.elim

theorem Expr.NoFreeze.subst {e v : Expr} (h : e.NoFreeze) (hv : v.NoFreeze) (x : Nat) :
    (e.subst x v).NoFreeze := by
  induction e with
  | var y =>
    simp only [Expr.subst]
    split
    · exact hv
    · trivial
  | unit | int | loc => trivial
  | new _ _ ih | get _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih => exact ih h
  | block _ _ _ _ ih => exact ⟨h.1, ih h.2⟩
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
  | freeze | keep => exact h.elim

theorem Expr.NoFreeze.inst {e : Expr} (h : e.NoFreeze) (ms : List Mu) (τs : List Ty) :
    (e.inst ms τs).NoFreeze := by
  induction e with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih => exact ih h
  | block _ _ _ _ ih => exact ⟨h.1, ih h.2⟩
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | freeze | keep => exact h.elim

theorem Expr.NoFreeze.ren {e : Expr} (h : e.NoFreeze) (a b : Nat) : (e.ren a b).NoFreeze := by
  induction e with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih => exact ih h
  | block _ _ _ _ ih =>
    simp only [Expr.ren]
    split
    · exact h
    · exact ⟨h.1, ih h.2⟩
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | freeze | keep => exact h.elim

/-- A running program only freezes regions that blocks made. -/
theorem Expr.Run.freezes_even {e : Expr} (h : e.Run) {r : Nat} (hr : e.Freezes r) : r % 2 = 0 := by
  induction e with
  | var | unit | int | loc => exact hr.elim
  | lam => exact (Expr.NoFreeze.not_freezes (e := .lam _ _ _) h r hr).elim
  | block => exact (Expr.NoFreeze.not_freezes (e := .block _ _ _ _) h r hr).elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h hr
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact hr.elim (ih₁ h.1) (ih₂ h.2.1)
  | letE _ _ _ ih₁ _ => exact hr.elim (ih₁ h.1) fun hb => (h.2.1.not_freezes r hb).elim
  | freeze r' _ _ ih =>
    rcases hr with rfl | hr
    · exact h.1
    · exact ih h.2 hr
  | keep => exact h.elim

theorem Expr.IsValue.noFreeze {v : Expr} (hv : v.IsValue) (h : v.Run) : v.NoFreeze := by
  induction hv with
  | unit | int | loc => trivial
  | lam => exact h
  | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2.1⟩

theorem Expr.IsValue.no_step {fns : List Fn} {v : Expr} (hv : v.IsValue) {h h' : Heap} {e' : Expr}
    (hs : Step fns h v h' e') : False := by
  induction hv generalizing h' e' with
  | unit | int | loc | lam => cases hs
  | pair _ _ ih₁ ih₂ =>
    cases hs with
    | pairCong₁ s => exact ih₁ s
    | pairCong₂ _ s => exact ih₂ s

theorem HeapNoFreeze.append {h : Heap} (hh : HeapNoFreeze h) {v : Expr} (hv : v.NoFreeze) :
    HeapNoFreeze (h ++ [v]) := by
  intro l w hl
  by_cases hlt : l < h.length
  · rw [List.getElem?_append_left hlt] at hl; exact hh l w hl
  · rw [List.getElem?_append_right (by omega)] at hl
    rcases hx : l - h.length with _ | n
    · rw [hx] at hl; simp at hl; rw [← hl]; exact hv
    · rw [hx] at hl; simp at hl

theorem HeapNoFreeze.set {h : Heap} (hh : HeapNoFreeze h) {v : Expr} (hv : v.NoFreeze) (l : Nat) :
    HeapNoFreeze (h.set l v) := by
  intro l' w hl
  rw [List.getElem?_set] at hl
  split at hl
  · split at hl
    · cases hl; exact hv
    · cases hl
  · exact hh l' w hl

theorem Copy.noFreeze {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') : HeapNoFreeze h → v.NoFreeze → HeapNoFreeze h' ∧ v'.NoFreeze := by
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

/-- A step that makes a freeze makes one of the region of a block that
starts now. -/
theorem Expr.Before.step {a a' b : Expr} {n : Nat} (hb : Expr.Before a b)
    (hf : ∀ r, a'.Freezes r → a.Freezes r ∨ r = 2 * n) (hl : ∀ l, b.HasLoc l → l < n) : Expr.Before a' b :=
  fun r hr l hlb => (hf r hr).elim (fun h => hb r h l hlb) fun h => by subst h; have := hl l hlb; omega

/-- **A step keeps the order.** What it freezes it froze before, or is the
region of a block that starts now, whose marker is past every location. -/
theorem step_run {fns : List Fn} (hfn : ∀ fn ∈ fns, fn.body.NoFreeze) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') :
    HeapNoFreeze h → e.Run → (∀ l, e.HasLoc l → l < h.length) →
      HeapNoFreeze h' ∧ e'.Run ∧ ∀ r, e'.Freezes r → e.Freezes r ∨ r = 2 * h.length := by
  induction hs with
  | newCong _ ih | getCong _ ih | fstCong _ ih | sndCong _ ih | callCong _ ih => exact ih
  | freezeCong _ ih =>
    intro hh he hl
    obtain ⟨hh', he', hf⟩ := ih hh he.2 hl
    refine ⟨hh', ⟨he.1, he'⟩, fun r hr => ?_⟩
    rcases hr with rfl | hr
    · exact .inl (.inl rfl)
    · exact (hf r hr).imp_left .inr
  | @setCong₁ _ a a' b s ih | @appCong₁ _ a a' b s ih | @pairCong₁ _ a a' b s ih =>
    intro hh he hl
    obtain ⟨hh', ha', hf⟩ := ih hh he.1 fun l h => hl l (.inl h)
    have hb : b.NoFreeze := he.2.2.1.resolve_left fun hv => hv.no_step s
    refine ⟨hh', ⟨ha', he.2.1, .inr hb, he.2.2.2.1.step hf fun l h => hl l (.inr h), .of_noFreeze hb⟩,
      fun r hr => ?_⟩
    rcases hr with hr | hr
    · exact (hf r hr).imp_left .inl
    · exact .inl (.inr hr)
  | @setCong₂ _ a b b' hv s ih | @appCong₂ _ a b b' hv s ih | @pairCong₂ _ a b b' hv s ih =>
    intro hh he hl
    obtain ⟨hh', hb', hf⟩ := ih hh he.2.1 fun l h => hl l (.inr h)
    refine ⟨hh', ⟨he.1, hb', .inl hv, .of_noFreeze (hv.noFreeze he.1),
      he.2.2.2.2.step hf fun l h => hl l (.inl h)⟩, fun r hr => ?_⟩
    rcases hr with hr | hr
    · exact .inl (.inl hr)
    · exact (hf r hr).imp_left .inr
  | letCong s ih =>
    intro hh he hl
    obtain ⟨hh', ha', hf⟩ := ih hh he.1 fun l h => hl l (.inl h)
    refine ⟨hh', ⟨ha', he.2.1, he.2.2.step hf fun l h => hl l (.inr h)⟩, fun r hr => ?_⟩
    rcases hr with hr | hr
    · exact (hf r hr).imp_left .inl
    · exact .inl (.inr hr)
  | newVal hv =>
    exact fun hh he _ => ⟨hh.append (hv.noFreeze he), trivial, fun _ hr => hr.elim⟩
  | getLoc hl =>
    exact fun hh _ _ => ⟨hh, (hh _ _ hl).run, fun r hr => ((hh _ _ hl).not_freezes r hr).elim⟩
  | setLoc _ hv =>
    exact fun hh he _ => ⟨hh.set (hv.noFreeze he.2.1) _, trivial, fun _ hr => hr.elim⟩
  | @letVal x v e₂ hv =>
    intro hh he _
    have h' := he.2.1.subst (hv.noFreeze he.1) x
    exact ⟨hh, h'.run, fun r hr => (h'.not_freezes r hr).elim⟩
  | @callVal f fn ms τs v hf' hv =>
    intro hh he _
    have h' := ((hfn fn (List.mem_of_getElem? hf')).inst ms τs).subst (hv.noFreeze he) fn.param
    exact ⟨hh, h'.run, fun r hr => (h'.not_freezes r hr).elim⟩
  | @appVal b x body v hv =>
    intro hh he _
    have h' := Expr.NoFreeze.subst (e := body) he.1 (hv.noFreeze he.2.1) x
    exact ⟨hh, h'.run, fun r hr => (h'.not_freezes r hr).elim⟩
  | fstVal hv₁ _ =>
    intro hh he _
    exact ⟨hh, he.1, fun r hr => .inl (.inl hr)⟩
  | sndVal _ hv₂ =>
    intro hh he _
    exact ⟨hh, he.2.1, fun r hr => .inl (.inr hr)⟩
  | freezeVal hv hc =>
    intro hh he _
    obtain ⟨hh', hv'⟩ := hc.noFreeze hh (hv.noFreeze he.2)
    exact ⟨hh', hv'.run, fun r hr => (hv'.not_freezes r hr).elim⟩
  | @blockEnter k t e =>
    intro hh he _
    have h' := he.2.ren (2 * k + 1) (2 * h.length)
    refine ⟨hh.append trivial, ⟨by omega, h'.run⟩, fun r hr => ?_⟩
    rcases hr with rfl | hr
    · exact .inr rfl
    · exact (h'.not_freezes r hr).elim
  | blockKeep => intro _ he; cases he.1
  | keepCong _ _ | keepVal _ _ => intro _ he; exact he.elim

/-- **A running program's order gives the premise:** what's beside a
freeze has only locations older than its region, whose types can't reach
it. -/
theorem Expr.Run.sep {σ : HeapTy} (ha : AllocOk σ) {e : Expr} (h : e.Run)
    (hl : ∀ l, e.HasLoc l → l < σ.length) : e.Sep σ := by
  have disj : ∀ {a b : Expr}, a.Run → Expr.Before a b → (∀ l, b.HasLoc l → l < σ.length) → Expr.Disj σ a b := by
    intro a b ha' hb hlb l hl' r hr
    obtain ⟨⟨m, t⟩, hσ⟩ : ∃ p, σ[l]? = some p := ⟨_, List.getElem?_eq_getElem (hlb l hl')⟩
    refine ⟨m, t, hσ, ?_⟩
    cases e : (Ty.cell m t).reaches r with
    | false => rfl
    | true =>
      have h₁ := ha l m t hσ r e (ha'.freezes_even hr)
      have h₂ := hb r hr l hl'
      omega
  induction e with
  | var | unit | int | loc | lam => trivial
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h hl
  | freeze _ _ _ ih => exact ih h.2 hl
  | keep => exact h.elim
  | block _ _ _ _ ih => exact ih h.2.run hl
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    exact ⟨disj h.1 h.2.2.2.1 fun l h => hl l (.inr h), disj h.2.1 h.2.2.2.2 fun l h => hl l (.inl h),
      ih₁ h.1 fun l h => hl l (.inl h), ih₂ h.2.1 fun l h => hl l (.inr h)⟩
  | letE _ _ _ ih₁ ih₂ =>
    exact ⟨disj h.1 h.2.2 fun l h => hl l (.inr h), ih₁ h.1 fun l h => hl l (.inl h),
      ih₂ h.2.1.run fun l h => hl l (.inr h)⟩

/-- The order holds however many steps a well-typed program takes. -/
theorem steps_run {fns : List Fn} (hf : FnsOk fns) (hfn : ∀ fn ∈ fns, fn.body.NoFreeze) {h h' : Heap}
    {e e' : Expr} (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t → HeapClosed h → e.Closed →
      HeapNoFreeze h → e.Run → HeapNoFreeze h' ∧ e'.Run := by
  induction hs with
  | refl => exact fun _ _ _ _ hn hr => ⟨hn, hr⟩
  | step s _ ih =>
    intro σ t hh ht hcl hc hn hr
    obtain ⟨hn', hr', _⟩ := step_run hfn s hn hr fun l hl => hh.1 ▸ ht.hasLoc_lt hl
    obtain ⟨σ', _, hh', ht'⟩ := preservation hf s hh ht hc
    obtain ⟨hcl', hc'⟩ := step_closed (fun fn h => (hf fn h).2.2.2) s hcl hc
    exact ih hh' ht' hcl' hc' hn' hr'

/-- **Every block's freeze can be in place.** A well-typed program that
keeps the order, like one written without freezes, computes what it does
with every freeze erased, in it and in its functions: whatever result the
program that copies gets, the other gets one that corresponds, and the
same integer. -/
theorem blocks_freeze_in_place {fns : List Fn} (hf : FnsOk fns) (hfn : ∀ fn ∈ fns, fn.body.NoFreeze)
    {σ : HeapTy} {h h₁ : Heap} {e e₁ : Expr} {T : Ty} (hh : HeapOk fns σ h) (ha : AllocOk σ)
    (ht : HasType fns σ .none false Ctx.empty e T) (hcl : HeapClosed h) (hec : e.Closed) (hn : HeapNoFreeze h)
    (hr : e.Run) (hs : Steps fns h e h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps (fns.map Fn.erase) (h.map Expr.erase) e.erase h₂ e₂ ∧ Rel ρ e₁.erase e₂ ∧
        ∀ n, e₁ = .int n → e₂ = .int n :=
  freezes_in_place_change_nothing hf hh ht ha
    (fun _ _ _ hs' _ ht' ha' => (steps_run hf hfn hs' hh ht hcl hec hn hr).2.sep ha' fun _ hl => ht'.hasLoc_lt hl)
    hcl hec hs

/-- For a whole program, written without freezes and run from an empty
heap. -/
theorem program_blocks_freeze_in_place {fns : List Fn} (hf : FnsOk fns) (hfn : ∀ fn ∈ fns, fn.body.NoFreeze)
    {e e₁ : Expr} {T : Ty} {h₁ : Heap} (ht : HasType fns [] .none false Ctx.empty e T) (hec : e.Closed)
    (hnf : e.NoFreeze) (hs : Steps fns [] e h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps (fns.map Fn.erase) [] e.erase h₂ e₂ ∧ Rel ρ e₁.erase e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n :=
  blocks_freeze_in_place hf hfn (HeapOk.empty fns) (fun _ _ _ h => by simp at h) ht
    (fun _ _ h => by simp at h) hec (fun _ _ h => by simp at h) hnf.run hs

end BoltMut
