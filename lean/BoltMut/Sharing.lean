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
several, but a `mut` one corresponds to exactly one,
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
  | toImm {b : Bool} {e₁ e₂ : Expr} : Rel ρ e₁ e₂ → Rel ρ (.toImm b e₁) (.toImm b e₂)

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
  | toImm _ ih => exact .toImm ih

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
  | toImm _ ih => exact .toImm (ih hρ)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))
  | letE _ _ ih₁ ih₂ => exact .letE (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))
  | app _ _ ih₁ ih₂ => exact .app (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ fun a c h => hρ a c (.inl h)) (ih₂ fun a c h => hρ a c (.inr h))


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
  | .toImm _ e, h => .toImm (Rel.refl (e := e) h)
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
  | toImm _ ih => exact .toImm ih

/-- The locations in a well-typed expression are in its heap typing. -/
theorem HasType.hasLoc_lt {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) {l : Nat} (hl : e.HasLoc l) : l < σ.length := by
  induction ht with
  | var | varSub | unit | int => exact hl.elim
  | loc hσ _ | locSub hσ _ _ =>
    simp only [Expr.HasLoc] at hl
    subst hl
    exact (List.getElem?_eq_some_iff.mp hσ).1
  | new _ _ ih | get _ ih | call _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih hl
  | set _ _ ih₁ ih₂ | letE _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact hl.elim ih₁ ih₂

/-! ## Heaps that correspond -/

/-- The heaps `h₁` and `h₂` correspond by `ρ`: corresponding locations hold
objects that are both `mut` or both `imm`, with corresponding values, and a
location in `M`, which has the `mut` ones of `h₁`, corresponds to exactly
one, both ways. -/
structure Corr (M : Nat → Prop) (ρ : Nat → Nat → Prop) (h₁ h₂ : Heap) : Prop where
  heap : ∀ a c, ρ a c → ∃ b v₁ v₂, h₁[a]? = some (b, v₁) ∧ h₂[c]? = some (b, v₂) ∧ Rel ρ v₁ v₂
  inj : ∀ a b c, ρ a c → ρ b c → M a → a = b
  func : ∀ a c d, ρ a c → ρ a d → M a → c = d

theorem Corr.lt₁ {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂) {a c : Nat}
    (h : ρ a c) : a < h₁.length := by
  obtain ⟨_, _, _, hv, _, _⟩ := hc.heap a c h
  exact (List.getElem?_eq_some_iff.mp hv).1

theorem Corr.lt₂ {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂) {a c : Nat}
    (h : ρ a c) : c < h₂.length := by
  obtain ⟨_, _, _, _, hv, _⟩ := hc.heap a c h
  exact (List.getElem?_eq_some_iff.mp hv).1

/-- What's past the end of the first heap can be in `M` too: nothing
corresponds to it. -/
theorem Corr.widen {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂) :
    Corr (fun a => M a ∨ h₁.length ≤ a) ρ h₁ h₂ where
  heap := hc.heap
  inj a b c hac hbc hM := hM.elim (hc.inj a b c hac hbc) fun hle => absurd (hc.lt₁ hac) (by omega)
  func a c d hac had hM := hM.elim (hc.func a c d hac had) fun hle => absurd (hc.lt₁ hac) (by omega)

/-- Fewer locations that must correspond to exactly one is easier. -/
theorem Corr.weaken {M M' : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂)
    (hM : ∀ a, M' a → M a) : Corr M' ρ h₁ h₂ where
  heap := hc.heap
  inj a b c hac hbc hM' := hc.inj a b c hac hbc (hM a hM')
  func a c d hac had hM' := hc.func a c d hac had (hM a hM')

/-- Adding a location to each heap, holding values that correspond, keeps
them corresponding, with the new locations corresponding to each other. -/
theorem Corr.alloc {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} (hc : Corr M ρ h₁ h₂)
    {b : Bool} {v₁ v₂ : Expr} (hr : Rel ρ v₁ v₂) :
    Corr (fun a => M a ∨ h₁.length ≤ a) (fun a c => ρ a c ∨ (a = h₁.length ∧ c = h₂.length))
      (h₁ ++ [(b, v₁)]) (h₂ ++ [(b, v₂)]) := by
  have old : ∀ a c, ρ a c → ρ a c ∨ (a = h₁.length ∧ c = h₂.length) := fun _ _ h => .inl h
  refine ⟨?_, ?_, ?_⟩
  · intro a c hac
    rcases hac with hac | ⟨rfl, rfl⟩
    · obtain ⟨b', w₁, w₂, h₁w, h₂w, hrel⟩ := hc.heap a c hac
      refine ⟨b', w₁, w₂, ?_, ?_, hrel.mono old⟩
      · rw [List.getElem?_append_left (hc.lt₁ hac)]
        exact h₁w
      · rw [List.getElem?_append_left (hc.lt₂ hac)]
        exact h₂w
    · exact ⟨b, v₁, v₂, by simp, by simp, hr.mono old⟩
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

def MemoRel (M : Nat → Prop) (ρ : Nat → Nat → Prop) (m₁ m₂ : List (Nat × Nat)) : Prop :=
  (∀ l₁ l₂, ρ l₁ l₂ → M l₁ → (m₁.lookup l₁ = none ∧ m₂.lookup l₂ = none) ∨
    ∃ c₁ c₂, m₁.lookup l₁ = some c₁ ∧ m₂.lookup l₂ = some c₂ ∧ ρ c₁ c₂) ∧
  (∀ l₂ c₂, m₂.lookup l₂ = some c₂ → ∃ l₁ c₁, ρ l₁ l₂ ∧ M l₁ ∧ m₁.lookup l₁ = some c₁)

theorem MemoRel.congr {M M' : Nat → Prop} {ρ : Nat → Nat → Prop} {m₁ m₂ : List (Nat × Nat)}
    (h : MemoRel M ρ m₁ m₂) (hM : ∀ a, M a ↔ M' a) : MemoRel M' ρ m₁ m₂ := by
  refine ⟨fun l₁ l₂ hl hM' => h.1 l₁ l₂ hl ((hM l₁).mpr hM'), fun l₂ c₂ hl => ?_⟩
  obtain ⟨l₁, c₁, h₁, hM₁, h₂⟩ := h.2 l₂ c₂ hl
  exact ⟨l₁, c₁, h₁, (hM l₁).mp hM₁, h₂⟩

/-- Adding what's past the end of the first heap changes nothing. -/
theorem MemoRel.widen {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₁ h₂ : Heap} {m₁ m₂ : List (Nat × Nat)}
    (h : MemoRel M ρ m₁ m₂) (hc : Corr M ρ h₁ h₂) : MemoRel (fun a => M a ∨ h₁.length ≤ a) ρ m₁ m₂ := by
  refine ⟨fun l₁ l₂ hl hM' => h.1 l₁ l₂ hl ?_, fun l₂ c₂ hl => ?_⟩
  · rcases hM' with hM' | hM'
    · exact hM'
    · exact absurd (hc.lt₁ hl) (by omega)
  · obtain ⟨l₁, c₁, h₁, hM₁, h₂⟩ := h.2 l₂ c₂ hl
    exact ⟨l₁, c₁, h₁, .inl hM₁, h₂⟩

theorem List.lookup_cons_nat (k l c : Nat) (m : List (Nat × Nat)) :
    ((l, c) :: m).lookup k = if k = l then some c else m.lookup k := by
  simp only [List.lookup]
  split <;> rename_i h <;> simp_all

/-- A `mut` object is at a `mut` location. -/
theorem HeapOk.mut_of_tag {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {a : Nat} {w : Expr}
    (ha : h[a]? = some (true, w)) : ∃ t, σ[a]? = some (.mutable, t) := by
  have hlt : a < σ.length := hh.1 ▸ (List.getElem?_eq_some_iff.mp ha).1
  obtain ⟨⟨m, t⟩, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem hlt⟩
  obtain ⟨hm, _, w₀, hw₀, _⟩ := hh.2 a m t hp
  rw [ha] at hw₀
  rcases hm with rfl | rfl
  · exact ⟨t, hp⟩
  · cases hw₀

/-- A copy in the first run is one in the second, of what corresponds, and
the copies correspond: the copy only looks at whether objects are `mut`,
which corresponding ones agree on, at what they hold, and at what it copied
already. -/
theorem Freeze.sim {h₁ h₁' : Heap} {m₁ m₁' : List (Nat × Nat)} {v₁ v₁' : Expr}
    (hcp : Freeze h₁ m₁ v₁ h₁' m₁' v₁') :
    ∀ {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {m₂ : List (Nat × Nat)} {v₂ : Expr},
      (∀ a w, h₁[a]? = some (true, w) → M a) → Corr M ρ h₁ h₂ → Rel ρ v₁ v₂ → MemoRel M ρ m₁ m₂ →
      (∀ l c, m₁.lookup l = some c → l < h₁.length) →
      ∃ ρ' h₂' m₂' v₂', Freeze h₂ m₂ v₂ h₂' m₂' v₂' ∧ (∀ a c, ρ a c → ρ' a c) ∧
        Corr (fun a => M a ∨ h₁.length ≤ a) ρ' h₁' h₂' ∧ Rel ρ' v₁' v₂' ∧
        MemoRel (fun a => M a ∨ h₁.length ≤ a) ρ' m₁' m₂' ∧ (∀ l c, m₁'.lookup l = some c → l < h₁'.length) ∧
        (∀ a c, ρ' a c → ρ a c ∨ (h₁.length ≤ a ∧ h₂.length ≤ c)) := by
  induction hcp with
  | unit =>
    intro M ρ h₂ m₂ v₂ _ hc hr hmr hk
    cases hr
    exact ⟨ρ, h₂, m₂, _, .unit, fun _ _ h => h, hc.widen, .unit, hmr.widen hc, hk, fun _ _ h => .inl h⟩
  | int =>
    intro M ρ h₂ m₂ v₂ _ hc hr hmr hk
    cases hr
    exact ⟨ρ, h₂, m₂, _, .int, fun _ _ h => h, hc.widen, .int, hmr.widen hc, hk, fun _ _ h => .inl h⟩
  | lam =>
    intro M ρ h₂ m₂ v₂ _ hc hr hmr hk
    cases hr with
    | lam hb => exact ⟨ρ, h₂, m₂, _, .lam, fun _ _ h => h, hc.widen, .lam hb, hmr.widen hc, hk, fun _ _ h => .inl h⟩
  | @imm h m l w hw =>
    intro M ρ h₂ m₂ v₂ _ hc hr hmr hk
    cases hr with
    | @loc _ c hlc =>
      obtain ⟨b, w₁, w₂, hw₁, hw₂, _⟩ := hc.heap l c hlc
      rw [hw] at hw₁
      cases hw₁
      exact ⟨ρ, h₂, m₂, _, .imm hw₂, fun _ _ h => h, hc.widen, .loc hlc, hmr.widen hc, hk, fun _ _ h => .inl h⟩
  | @seen h m l c w hw hlk =>
    intro M ρ h₂ m₂ v₂ hMt hc hr hmr hk
    cases hr with
    | @loc _ c₂ hlc =>
      obtain ⟨b, w₁, w₂, hw₁, hw₂, _⟩ := hc.heap l c₂ hlc
      rw [hw] at hw₁
      cases hw₁
      rcases hmr.1 l c₂ hlc (hMt l w hw) with ⟨hn, _⟩ | ⟨c₁, c₂', h₁, h₂', hcc⟩
      · rw [hlk] at hn; cases hn
      · rw [hlk] at h₁
        cases h₁
        exact ⟨ρ, h₂, m₂, _, .seen hw₂ h₂', fun _ _ h => h, hc.widen, .loc hcc, hmr.widen hc, hk, fun _ _ h => .inl h⟩
  | @cell h h' m m' l w w' hw hnone hcw ih =>
    intro M ρ h₂ m₂ v₂ hMt hc hr hmr hk
    have hMl : M l := hMt l w hw
    cases hr with
    | @loc _ l₂ hlc =>
      obtain ⟨b, w₁, w₂, hw₁, hw₂, hrw⟩ := hc.heap l l₂ hlc
      rw [hw] at hw₁
      cases hw₁
      have hnone₂ : m₂.lookup l₂ = none := by
        rcases hmr.1 l l₂ hlc hMl with ⟨_, h⟩ | ⟨c₁, _, h₁, _, _⟩
        · exact h
        · rw [hnone] at h₁; cases h₁
      obtain ⟨ρ₁, h₂₁, m₂₁, w₂', hc₂, hρ₁, hcorr₁, hrw', hmr₁, hk₁, hnew⟩ := ih hMt hc hrw hmr hk
      have hlen : h.length ≤ h'.length := by
        obtain ⟨e, rfl⟩ := hcw.prefix
        simp
      have hlen₂ : h₂.length ≤ h₂₁.length := by
        obtain ⟨e, rfl⟩ := hc₂.prefix
        simp
      have hl₂ : l₂ < h₂.length := hc.lt₂ hlc
      have hlh : l < h.length := hc.lt₁ hlc
      let M₁ : Nat → Prop := fun a => M a ∨ h.length ≤ a
      have hMl₁ : M₁ l := .inl hMl
      -- Nothing past the end of a heap has been copied.
      have hnew₁ : m'.lookup h'.length = none := by
        cases e : m'.lookup h'.length with
        | none => rfl
        | some c =>
          have := hk₁ _ _ e
          omega
      have hnew₂ : m₂₁.lookup h₂₁.length = none := by
        cases e : m₂₁.lookup h₂₁.length with
        | none => rfl
        | some c =>
          obtain ⟨l₁, _, hl₁, _, _⟩ := hmr₁.2 _ _ e
          have := hcorr₁.lt₂ hl₁
          omega
      refine ⟨fun a c' => ρ₁ a c' ∨ (a = h'.length ∧ c' = h₂₁.length), h₂₁ ++ [(false, w₂')],
        (l₂, h₂₁.length) :: m₂₁, .loc h₂₁.length, .cell hw₂ hnone₂ hc₂, fun a c' h => .inl (hρ₁ a c' h),
        (hcorr₁.alloc hrw').weaken fun a ha => .inl ha, .loc (.inr ⟨rfl, rfl⟩), ⟨?_, ?_⟩, ?_, ?_⟩
      · -- The original and its counterpart are copied together; the rest
        -- is as it was.
        intro k₁ k₂ hk₁₂ hMk
        rw [List.lookup_cons_nat, List.lookup_cons_nat]
        rcases hk₁₂ with hk₁₂ | ⟨rfl, rfl⟩
        · by_cases hkl : k₁ = l
          · subst hkl
            have hk₂ : k₂ = l₂ := hcorr₁.func k₁ k₂ l₂ hk₁₂ (hρ₁ _ _ hlc) hMl₁
            subst hk₂
            exact .inr ⟨_, _, by simp, by simp, .inr ⟨rfl, rfl⟩⟩
          · have hk₂ : k₂ ≠ l₂ := by
              rintro rfl
              exact hkl (hcorr₁.inj k₁ l k₂ hk₁₂ (hρ₁ _ _ hlc) hMk)
            simp only [hkl, hk₂, ite_false]
            rcases hmr₁.1 k₁ k₂ hk₁₂ hMk with h | ⟨c₁, c₂, h₁, h₂, hcc⟩
            · exact .inl h
            · exact .inr ⟨c₁, c₂, h₁, h₂, .inl hcc⟩
        · -- A new copy is no original.
          have hne₁ : h'.length ≠ l := by omega
          have hne₂ : h₂₁.length ≠ l₂ := by omega
          simp only [hne₁, hne₂, ite_false]
          exact .inl ⟨hnew₁, hnew₂⟩
      · intro k₂ c₂ hk₂
        rw [List.lookup_cons_nat] at hk₂
        split at hk₂
        · rename_i he
          have he : k₂ = l₂ := by simpa using he
          subst he
          exact ⟨l, h'.length, .inl (hρ₁ _ _ hlc), hMl₁, by simp⟩
        · rename_i hne
          have hne : k₂ ≠ l₂ := by simpa using hne
          obtain ⟨k₁, c₁, hk₁₂, hMk, hk₁⟩ := hmr₁.2 k₂ c₂ hk₂
          have hkl : k₁ ≠ l := by
            rintro rfl
            exact hne (hcorr₁.func k₁ k₂ l₂ hk₁₂ (hρ₁ _ _ hlc) hMk)
          exact ⟨k₁, c₁, .inl hk₁₂, hMk, by simp [List.lookup_cons_nat, hkl, hk₁]⟩
      · intro k c hk'
        rw [List.lookup_cons_nat] at hk'
        split at hk'
        · rename_i he
          have he : k = l := by simpa using he
          subst he
          simp; omega
        · have := hk₁ k c hk'
          simp; omega
      · -- What's related now that wasn't is new on both sides.
        rintro a c' (h | ⟨rfl, rfl⟩)
        · exact hnew a c' h
        · exact .inr ⟨hlen, hlen₂⟩
  | @pair h h₁ h₂ m m₁ m₂ v₁ v₂ v₁' v₂' hc₁ hc₂ ih₁ ih₂ =>
    intro M ρ hh₂ mm₂ e₂ hMt hc hr hmr hk
    cases hr with
    | pair hr₁ hr₂ =>
      obtain ⟨ρ₁, k₁, n₁, u₁, hk₁, hρ₁, hcorr₁, hru₁, hmr₁, hkk₁, hnew₁⟩ := ih₁ hMt hc hr₁ hmr hk
      have hle : h.length ≤ h₁.length := by
        obtain ⟨e₂, rfl⟩ := hc₁.prefix
        simp
      -- What's `mut` after the first part was before, or is new.
      have hMt₁ : ∀ a w, h₁[a]? = some (true, w) → M a ∨ h.length ≤ a := by
        intro a w ha
        by_cases hlt : a < h.length
        · rw [hc₁.keeps hlt] at ha
          exact .inl (hMt a w ha)
        · exact .inr (by omega)
      obtain ⟨ρ₂, k₂, n₂, u₂, hk₂, hρ₂, hcorr₂, hru₂, hmr₂, hkk₂, hnew₂⟩ :=
        ih₂ hMt₁ hcorr₁ (hr₂.mono hρ₁) hmr₁ hkk₁
      have hiff : ∀ x, ((M x ∨ h.length ≤ x) ∨ h₁.length ≤ x) ↔ (M x ∨ h.length ≤ x) := by
        intro x
        constructor
        · rintro ((hx | hx) | hx)
          · exact .inl hx
          · exact .inr hx
          · exact .inr (by omega)
        · exact .inl
      refine ⟨ρ₂, k₂, n₂, _, .pair hk₁ hk₂, fun a c h => hρ₂ a c (hρ₁ a c h),
        hcorr₂.weaken fun x hx => .inl hx, .pair (hru₁.mono hρ₂) hru₂, hmr₂.congr hiff, hkk₂, fun a c h => ?_⟩
      have hle₂ : hh₂.length ≤ k₁.length := by obtain ⟨e, rfl⟩ := hk₁.prefix; simp
      rcases hnew₂ a c h with h | ⟨h₁', h₂'⟩
      · exact hnew₁ a c h
      · exact .inr ⟨by omega, by omega⟩

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
    | new he _ =>
      cases hr with
      | new hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .newCong s₂, hρ, hc', .new hr'⟩
  | @newVal e b hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | @new _ _ e' hr =>
      exact ⟨_, h₂ ++ [(b, e')], .loc h₂.length, .newVal (hr.isValue hv), fun _ _ h => .inl h, hc.alloc hr,
        .loc (.inr ⟨rfl, rfl⟩)⟩
  | getCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | get he =>
      cases hr with
      | get hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .getCong s₂, hρ, hc', .get hr'⟩
  | @getLoc l b v hl =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | get hr =>
      cases hr with
      | loc hρ =>
        obtain ⟨b', v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap _ _ hρ
        rw [hl] at h₁v
        cases h₁v
        exact ⟨ρ, h₂, v₂, .getLoc h₂v, fun _ _ h => h, hc.widen, hrel⟩
  | setCong₁ _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | set he₁ _ =>
      cases hr with
      | set hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁
        exact ⟨ρ', h₂', _, .setCong₁ s₂, hρ, hc', .set hr' (hr₂.mono hρ)⟩
  | setCong₂ hv _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | set _ he₂ =>
      cases hr with
      | set hr₁ hr₂ =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂
        exact ⟨ρ', h₂', _, .setCong₂ (hr₁.isValue hv) s₂, hρ, hc', .set (hr₁.mono hρ) hr'⟩
  | @setLoc l₀ b w v hl hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | set he₁ _ =>
      obtain ⟨_, hl', hσ⟩ := canonical_cell .loc he₁ (by simp)
      cases hl'
      · have hMl : M l₀ := hM _ _ _ hσ (by intro h; cases h)
        cases hr with
        | @set _ _ _ v' hr₁ hr₂ =>
          cases hr₁ with
          | @loc _ c hρ =>
            have hc₂ : c < h₂.length := hc.lt₂ hρ
            have hl₀ : l₀ < h₁.length := hc.lt₁ hρ
            -- Both objects are `mut`, or both `imm`.
            obtain ⟨b₀, w₁, w₂, h₁w, h₂w, _⟩ := hc.heap _ _ hρ
            rw [hl] at h₁w
            cases h₁w
            refine ⟨ρ, h₂.set c (b, v'), .unit, .setLoc h₂w (hr₂.isValue hv), fun _ _ h => h, ?_, .unit⟩
            have key : Corr M ρ (h₁.set l₀ (b, v)) (h₂.set c (b, v')) := by
              refine ⟨?_, hc.inj, hc.func⟩
              intro a d had
              obtain ⟨b', v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap a d had
              by_cases ha : a = l₀
              · subst ha
                have hd : d = c := hc.func _ _ _ had hρ hMl
                subst hd
                exact ⟨b, v, v', List.getElem?_set_self hl₀, List.getElem?_set_self hc₂, hr₂⟩
              · have hd : d ≠ c := fun hd => ha (hc.inj l₀ a c hρ (hd ▸ had) hMl).symm
                refine ⟨b', v₁, v₂, ?_, ?_, hrel⟩
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
  | toImmCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases ht with
    | toImm he _ =>
      cases hr with
      | toImm hr =>
        obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he hM hc hr
        exact ⟨ρ', h₂', _, .toImmCong s₂, hρ, hc', .toImm hr'⟩
  | toImmVal hv hcp =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr
    cases hr with
    | toImm hr =>
      -- Both copy what corresponds, the same way.
      obtain ⟨ρ', h₂', m₂', v₂', hf₂, hρ, hc', hr', _, _⟩ :=
        hcp.sim (m₂ := []) (fun a w ha => let ⟨_, hσ⟩ := hh.mut_of_tag ha; hM a _ _ hσ (by intro h; cases h)) hc hr
          ⟨fun _ _ _ _ => .inl ⟨rfl, rfl⟩, fun _ _ h => by simp at h⟩ (fun _ _ h => by simp at h)
      exact ⟨ρ', h₂', v₂', .toImmVal (hr.isValue hv) hf₂, hρ, hc', hr'⟩
  | toImmKeep _ _ => intro σ t M ρ h₂ e₂ hh ht _ _ _; cases ht

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
    (hρ : ∀ a, a < h.length → ρ a a) {a : Nat} {b : Bool} {w : Expr} (hw : h[a]? = some (b, w)) : Rel ρ w w := by
  have ha : a < σ.length := hh.1 ▸ (List.getElem?_eq_some_iff.mp hw).1
  obtain ⟨⟨m, t⟩, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem ha⟩
  obtain ⟨_, _, w', hw', _, hwt⟩ := hh.2 a m t hp
  rw [hw] at hw'
  cases hw'
  exact Rel.refl fun l hl => hρ l (by rw [hh.1]; exact hwt.hasLoc_lt hl)

theorem HeapOk.get {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {a : Nat} {p : Mu × Ty}
    (hp : σ[a]? = some p) : ∃ b w, h[a]? = some (b, w) :=
  ⟨_, _, List.getElem?_eq_getElem (hh.lt hp)⟩

/-- The correspondence of a copy of `l`, at the end of a heap of `n`
locations: each location to itself, and `l` to its copy too. -/
def copyCorr (n l : Nat) : Nat → Nat → Prop := fun a c => (a = c ∧ a < n) ∨ (a = l ∧ c = n)

/-- **A copy of an `imm` value can be used for it.** With `l` an `imm`
location of `h`, and `v` what it holds: put a copy of `v` at the end of
the heap. A program `e₂` that's `e₁` with the copy in place of `l` in some
places computes what `e₁` does, with a result that corresponds, and the
same integer. Both run with the copy in the heap; `e₁` just doesn't use
it. -/
theorem imm_copy_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {l : Nat}
    {t t₀ : Ty} {v e₁ e₂ v₁ : Expr} (hh : HeapOk fns σ h) (hl : σ[l]? = some (.immutable, t))
    (hv : h[l]? = some (false, v)) (ht : HasType fns σ .none false Ctx.empty e₁ t₀)
    (hr : Rel (copyCorr h.length l) e₁ e₂) (hs : Steps fns (h ++ [(false, v)]) e₁ h' v₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (v₂ : Expr),
      Steps fns (h ++ [(false, v)]) e₂ h₂ v₂ ∧ Rel ρ v₁ v₂ ∧ ∀ n, v₁ = .int n → v₂ = .int n := by
  obtain ⟨_, _, w₀, hw₀, hv₀, hvt⟩ := hh.2 _ _ _ hl
  rw [hv] at hw₀
  cases hw₀
  let σ' := σ ++ [(Mu.immutable, t)]
  have hh' : HeapOk fns σ' (h ++ [(false, v)]) := hh.alloc (b := false) (hh.2 _ _ _ hl).2.1 hv₀ hvt
  have hl' : σ'[l]? = some (.immutable, t) := Extends.append σ _ _ _ hl
  let M : Nat → Prop := fun a => ∃ m t', σ'[a]? = some (m, t') ∧ m ≠ .immutable
  have notM : ¬ M l := fun ⟨_, _, h', hm⟩ => by rw [hl'] at h'; cases h'; exact hm rfl
  have self : ∀ {a b w}, h[a]? = some (b, w) → Rel (copyCorr h.length l) w w :=
    fun hw => hh.rel_self (fun _ ha => .inl ⟨rfl, ha⟩) hw
  have hc : Corr M (copyCorr h.length l) (h ++ [(false, v)]) (h ++ [(false, v)]) := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c hac
      rcases hac with ⟨rfl, ha⟩ | ⟨rfl, rfl⟩
      · obtain ⟨⟨b, w⟩, hw⟩ : ∃ o, h[a]? = some o := ⟨_, List.getElem?_eq_getElem ha⟩
        exact ⟨b, w, w, by rw [List.getElem?_append_left ha]; exact hw,
          by rw [List.getElem?_append_left ha]; exact hw, self hw⟩
      · exact ⟨false, v, v, by rw [List.getElem?_append_left (hh.lt hl)]; exact hv, by simp, self hv⟩
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
  obtain ⟨_, ρ, h₂, v₂, ss, _, hr'⟩ := steps_sim hf hs hh' (ht.weaken_heap (Extends.append σ _))
    (fun _ m t' h hm => ⟨m, t', h, hm⟩) hc hr
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
  have self : ∀ {a b w}, h[a]? = some (b, w) → Rel (mergeCorr h.length l l') w w :=
    fun hw => hh.rel_self (fun _ ha => .inl ⟨rfl, ha⟩) hw
  have hc : Corr M (mergeCorr h.length l l') h h := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c hac
      rcases hac with ⟨rfl, ha⟩ | ⟨rfl, rfl⟩
      · obtain ⟨⟨b, w⟩, hw⟩ : ∃ o, h[a]? = some o := ⟨_, List.getElem?_eq_getElem ha⟩
        exact ⟨b, w, w, hw, hw, self hw⟩
      · obtain ⟨b, w, hw⟩ := hh.get hl'
        exact ⟨b, w, w, hw, heq ▸ hw, self hw⟩
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
