import BoltMut.Principal

/-!
# Least solutions for linked variables

`Problem.principal` is about one type variable whose lower bounds are
whole types. A signature links variables: in `fn f[T](a: [T], b: [T])`,
the array's mutability `?m` and `T` are solved together, and not one by
one. Called with `mut [mut [1]]` and `mut [[1]]`, `T` must be above
`mut [i32]` and `[i32]`, so `ro [i32]`, and then the array can't stay
`mut`, since a `mut [mut [i32]]` doesn't fit a `mut [ro [i32]]`: `?m` must
be `ro` too.

Here a type variable is taken to have the shape of what it's given, with
a variable for each mutability in it, as the checker gives it: all that's
given for it has that shape. So `a: [T]` is the pattern `?m [?n [i32]]`,
and what's left to solve are mutabilities, `σ`.

- `LinkedProblem.principal`: if there's a solution, there's a least one:
  each variable is as given, or `ro`, and only `ro` when it must be.
- It follows from `LinkedProblem.Solves.meet`: below two solutions there's
  always another, which takes each variable from one that isn't `ro`.

With nothing assumed, and without function types. A variable that isn't
in what's given has no least choice, which the checker defaults.
-/

namespace BoltMut

/-! ## Patterns -/

/-- The mutability `m` with its parameters given by `σ`. -/
def Mu.substF (σ : Nat → Mu) : Mu → Mu
  | .param j => σ j
  | m => m

/-- The pattern `t` with its mutability parameters given by `σ`. -/
def Ty.instF (σ : Nat → Mu) : Ty → Ty
  | .unit => .unit
  | .int => .int
  | .cell m t => .cell (m.substF σ) (t.instF σ)
  | .fn m a r => .fn (m.substF σ) (a.instF σ) (r.instF σ)
  | .prod a b => .prod (a.instF σ) (b.instF σ)
  | .tparam i => .tparam i
  | .roParam i => .roParam i

/-- The mutability parameters in a pattern. -/
def Ty.muVars : Ty → List Nat
  | .cell (.param j) t => j :: t.muVars
  | .cell _ t => t.muVars
  | .prod a b => a.muVars ++ b.muVars
  | _ => []

/-- Of two choices, the one that isn't `ro`, and the first if neither is. -/
def Mu.meet (a b : Mu) : Mu := if a = .readonly then b else a

def meetF (σ₁ σ₂ : Nat → Mu) : Nat → Mu := fun j => (σ₁ j).meet (σ₂ j)

theorem Mu.substF_meet (σ₁ σ₂ : Nat → Mu) (m : Mu) :
    m.substF (meetF σ₁ σ₂) = (m.substF σ₁).meet (m.substF σ₂) := by
  cases m with
  | param j => rfl
  | _ => simp [Mu.substF, Mu.meet]

theorem Mu.meet_ne_ro {a b : Mu} (h : a ≠ .readonly ∨ b ≠ .readonly) : a.meet b ≠ .readonly := by
  unfold Mu.meet
  split <;> rename_i ha
  · rcases h with h | h
    · exact absurd ha h
    · exact h
  · exact ha

/-! ## Taking the meet of two solutions -/

theorem Le.none_iff {x y : Mu} : Le .none x y ↔ x = .immutable ∨ y = .mutable ∨ x = y := by
  constructor
  · exact Le.none_inv
  · rintro (rfl | rfl | rfl)
    · exact .bot _
    · exact .top _
    · exact .refl _

/-- Meets keep "at most". -/
theorem Le.none_meet {a b μ₁ μ₂ : Mu} (h₁ : Le .none a μ₁) (h₂ : Le .none b μ₂) :
    Le .none (a.meet b) (μ₁.meet μ₂) := by
  rw [Le.none_iff] at h₁ h₂ ⊢
  unfold Mu.meet
  by_cases ha : a = .readonly
  · subst ha
    simp only [↓reduceIte]
    rcases h₁ with h | h | h
    · cases h
    · subst h; simp
    · subst h; simpa using h₂
  · simp only [ha, ↓reduceIte]
    by_cases hμ : μ₁ = .readonly
    · subst hμ
      rcases h₁ with h | h | h
      · exact .inl h
      · cases h
      · exact absurd h ha
    · simp only [hμ, ↓reduceIte]
      exact h₁

theorem Ty.NoFn.cell {m : Mu} {t : Ty} (h : (Ty.cell m t).NoFn) : t.NoFn := h

/-- What a type is at most stays so in the meet of two choices. -/
theorem AtMost.none_meet {σ₁ σ₂ : Nat → Mu} :
    ∀ {s : Ty} {μ₁ μ₂ : Mu}, s.NoFn → AtMost .none μ₁ (s.instF σ₁) →
      AtMost .none μ₂ (s.instF σ₂) → AtMost .none (μ₁.meet μ₂) (s.instF (meetF σ₁ σ₂))
  | .unit, _, _, _, _, _ | .int, _, _, _, _, _ => trivial
  | .fn .., _, _, h, _, _ => h.elim
  | .cell m t, _, _, hn, ⟨l₁, h₁⟩, ⟨l₂, h₂⟩ => by
    simp only [Ty.instF, AtMost, Mu.substF_meet]
    exact ⟨Le.none_meet l₁ l₂, AtMost.none_meet (s := t) hn h₁ h₂⟩
  | .prod a b, _, _, hn, ⟨ha₁, hb₁⟩, ⟨ha₂, hb₂⟩ =>
    ⟨AtMost.none_meet hn.1 ha₁ ha₂, AtMost.none_meet hn.2 hb₁ hb₂⟩
  | .tparam i, _, _, _, ⟨μ, h₁, l₁⟩, ⟨μ', h₂, l₂⟩ => by
    simp only [Assume.none, List.not_mem_nil, or_false] at h₁ h₂
    subst h₁; subst h₂
    exact ⟨.mutable, .inl rfl, by simpa [Mu.meet] using Le.none_meet l₁ l₂⟩
  | .roParam i, _, _, _, l₁, l₂ => by
    simpa [Ty.instF, AtMost, Mu.meet] using Le.none_meet l₁ l₂

/-- Where one choice is a subtype of the other, the meet is the smaller. -/
theorem Ty.instF_meet_left {σ₁ σ₂ : Nat → Mu} :
    ∀ {s : Ty}, s.NoFn → Sub .none (s.instF σ₁) (s.instF σ₂) → s.instF (meetF σ₁ σ₂) = s.instF σ₁
  | .unit, _, _ | .int, _, _ | .tparam _, _, _ | .roParam _, _, _ => rfl
  | .fn .., h, _ => h.elim
  | .cell m t, hn, h => by
    obtain ⟨m', s', he, hs, hm⟩ := h.of_cell
    simp only [Ty.instF, Ty.cell.injEq] at he
    obtain ⟨rfl, rfl⟩ := he
    simp only [Ty.instF, Mu.substF_meet, Ty.instF_meet_left (s := t) hn hs]
    congr 1
    unfold Mu.meet
    split
    · rename_i h; rcases hm with hm | hm <;> rw [hm, h]
    · rfl
  | .prod a b, hn, h => by
    obtain ⟨a', b', he, ha, hb⟩ := h.of_prod
    simp only [Ty.instF, Ty.prod.injEq] at he
    obtain ⟨rfl, rfl⟩ := he
    simp only [Ty.instF, Ty.instF_meet_left hn.1 ha, Ty.instF_meet_left hn.2 hb]

theorem Ty.instF_meet_right {σ₁ σ₂ : Nat → Mu} :
    ∀ {s : Ty}, s.NoFn → Sub .none (s.instF σ₂) (s.instF σ₁) → s.instF (meetF σ₁ σ₂) = s.instF σ₂
  | .unit, _, _ | .int, _, _ | .tparam _, _, _ | .roParam _, _, _ => rfl
  | .fn .., h, _ => h.elim
  | .cell m t, hn, h => by
    obtain ⟨m', s', he, hs, hm⟩ := h.of_cell
    simp only [Ty.instF, Ty.cell.injEq] at he
    obtain ⟨rfl, rfl⟩ := he
    simp only [Ty.instF, Mu.substF_meet, Ty.instF_meet_right (s := t) hn hs]
    congr 1
    unfold Mu.meet
    split
    · rfl
    · rename_i h; rcases hm with hm | hm
      · exact hm
      · exact absurd hm h
  | .prod a b, hn, h => by
    obtain ⟨a', b', he, ha, hb⟩ := h.of_prod
    simp only [Ty.instF, Ty.prod.injEq] at he
    obtain ⟨rfl, rfl⟩ := he
    simp only [Ty.instF, Ty.instF_meet_right hn.1 ha, Ty.instF_meet_right hn.2 hb]

theorem Mu.meet_self (a : Mu) : a.meet a = a := by
  unfold Mu.meet; split <;> simp_all

/-- The ways a cell can fit another: as it is, seen as `ro`, or as an
`imm` cell holding a supertype. -/
theorem Sub.cell_cases {Δ : Assume} {k a : Mu} {t x : Ty} (h : Sub Δ (.cell k t) (.cell a x)) :
    (a = k ∧ t = x) ∨ (a = .readonly ∧ Sub Δ t x ∧ AtMost Δ .readonly x) ∨
      (k = .immutable ∧ a = .immutable ∧ Sub Δ t x) := by
  cases h with
  | refl => exact .inl ⟨rfl, rfl⟩
  | ro h ha => exact .inr (.inl ⟨rfl, h, ha⟩)
  | immCell h => exact .inr (.inr ⟨rfl, rfl, h⟩)

/-- **Below two solutions there's another**: what's given fits the meet
of two choices it fits. -/
theorem Sub.none_meet {σ₁ σ₂ : Nat → Mu} :
    ∀ {p τ : Ty}, p.NoFn → AtMost .none .mutable τ → Sub .none τ (p.instF σ₁) →
      Sub .none τ (p.instF σ₂) → Sub .none τ (p.instF (meetF σ₁ σ₂))
  | .unit, _, _, _, h, _ | .int, _, _, _, h, _ | .tparam _, _, _, _, h, _
  | .roParam _, _, _, _, h, _ => h
  | .fn .., _, h, _, _, _ => h.elim
  | .prod a b, τ, hn, hw, h₁, h₂ => by
    obtain ⟨a₀, b₀, rfl, ha₁, hb₁⟩ := h₁.to_prod
    obtain ⟨_, _, he, ha₂, hb₂⟩ := h₂.to_prod
    cases he
    exact .prod (Sub.none_meet hn.1 hw.1 ha₁ ha₂) (Sub.none_meet hn.2 hw.2 hb₁ hb₂)
  | .cell m s, τ, hn, hw, h₁, h₂ => by
    obtain ⟨k, t, rfl⟩ := h₁.to_cell
    have hwt : AtMost .none .mutable t := hw.2.mono (.top _)
    have ih : ∀ {t'}, AtMost .none .mutable t' → Sub .none t' (s.instF σ₁) →
        Sub .none t' (s.instF σ₂) → Sub .none t' (s.instF (meetF σ₁ σ₂)) :=
      fun hw' => Sub.none_meet (σ₁ := σ₁) (σ₂ := σ₂) (p := s) hn hw'
    have am : AtMost .none .readonly (s.instF σ₁) → AtMost .none .readonly (s.instF σ₂) →
        AtMost .none .readonly (s.instF (meetF σ₁ σ₂)) := fun a b => by
      simpa [Mu.meet] using AtMost.none_meet (σ₁ := σ₁) (σ₂ := σ₂) (s := s) hn a b
    simp only [Ty.instF, Mu.substF_meet] at h₁ h₂ ⊢
    generalize m.substF σ₁ = a at h₁ ⊢
    generalize m.substF σ₂ = b at h₂ ⊢
    rcases Sub.cell_cases h₁ with ⟨rfl, e₁⟩ | ⟨rfl, s₁, r₁⟩ | ⟨rfl, rfl, s₁⟩ <;>
      rcases Sub.cell_cases h₂ with ⟨hb, e₂⟩ | ⟨rfl, s₂, r₂⟩ | ⟨hk, rfl, s₂⟩
    -- Both as they are.
    · subst hb; subst e₁
      rw [Mu.meet_self, Ty.instF_meet_left (s := s) hn (by rw [← e₂]; exact .refl _)]
      exact .refl _
    -- As it is, and seen as `ro`.
    · subst e₁
      by_cases hk : a = .readonly
      · subst hk
        exact .ro (ih hwt (.refl _) s₂) (am hw.2 r₂)
      · simp only [Mu.meet, hk, ↓reduceIte]
        rw [Ty.instF_meet_left (s := s) hn s₂]
        exact .refl _
    -- As it is, and an `imm` cell holding more.
    · subst hk; subst e₁
      exact .immCell (ih hwt (.refl _) s₂)
    -- Seen as `ro`, and as it is.
    · subst e₂
      by_cases hk : b = .readonly
      · subst hk; subst hb
        exact .ro (ih hwt s₁ (.refl _)) (am r₁ hw.2)
      · subst hb
        simp only [Mu.meet, ↓reduceIte]
        rw [Ty.instF_meet_right (s := s) hn s₁]
        exact .refl _
    -- Both seen as `ro`.
    · exact .ro (ih hwt s₁ s₂) (am r₁ r₂)
    -- Seen as `ro`, and an `imm` cell.
    · subst hk
      exact .immCell (ih hwt s₁ s₂)
    -- An `imm` cell, and as it is.
    · subst hb; subst e₂
      exact .immCell (ih hwt s₁ (.refl _))
    -- An `imm` cell, and seen as `ro`.
    · exact .immCell (ih hwt s₁ s₂)
    -- Both `imm` cells.
    · exact .immCell (ih hwt s₁ s₂)

/-- Two choices that what's given fits agree on a variable in the pattern,
unless one of them makes it `ro`. -/
theorem Sub.none_agree {σ₁ σ₂ : Nat → Mu} {j : Nat} :
    ∀ {p τ : Ty}, p.NoFn → j ∈ p.muVars → Sub .none τ (p.instF σ₁) → Sub .none τ (p.instF σ₂) →
      σ₁ j = σ₂ j ∨ σ₁ j = .readonly ∨ σ₂ j = .readonly
  | .unit, _, _, h, _, _ | .int, _, _, h, _, _ | .tparam _, _, _, h, _, _
  | .roParam _, _, _, h, _, _ => by simp [Ty.muVars] at h
  | .fn .., _, h, _, _, _ => h.elim
  | .prod a b, _, hn, hj, h₁, h₂ => by
    obtain ⟨_, _, rfl, ha₁, hb₁⟩ := h₁.to_prod
    obtain ⟨_, _, he, ha₂, hb₂⟩ := h₂.to_prod
    cases he
    simp only [Ty.muVars, List.mem_append] at hj
    rcases hj with hj | hj
    · exact Sub.none_agree hn.1 hj ha₁ ha₂
    · exact Sub.none_agree hn.2 hj hb₁ hb₂
  | .cell m s, _, hn, hj, h₁, h₂ => by
    obtain ⟨_, _, rfl⟩ := h₁.to_cell
    obtain ⟨m₁, s₁, he₁, hs₁, hm₁⟩ := h₁.of_cell
    obtain ⟨m₂, s₂, he₂, hs₂, hm₂⟩ := h₂.of_cell
    simp only [Ty.instF, Ty.cell.injEq] at he₁ he₂
    obtain ⟨he₁, rfl⟩ := he₁
    obtain ⟨he₂, rfl⟩ := he₂
    have inner : j ∈ s.muVars → _ := fun hj => Sub.none_agree (p := s) hn hj hs₁ hs₂
    cases m with
    | param j' =>
      simp only [Ty.muVars, List.mem_cons] at hj
      rcases hj with rfl | hj
      · simp only [Mu.substF] at he₁ he₂
        subst he₁; subst he₂
        rcases hm₁ with h | h <;> rcases hm₂ with h' | h'
        · exact .inl (h.trans h'.symm)
        · exact .inr (.inr h')
        · exact .inr (.inl h)
        · exact .inr (.inl h)
      · exact inner hj
    | _ => exact inner (by simpa [Ty.muVars] using hj)

/-! ## The least solution -/

/-- What a call says about the mutabilities of its signature: each given
type fits its pattern, `τ ≤ p[σ]`, and each bounded pattern is at most its
mutability. -/
structure LinkedProblem where
  lower : List (Ty × Ty)
  bounds : List (Mu × Ty) := []

def LinkedProblem.Solves (P : LinkedProblem) (σ : Nat → Mu) : Prop :=
  (∀ c ∈ P.lower, Sub .none c.1 (c.2.instF σ)) ∧ (∀ b ∈ P.bounds, AtMost .none b.1 (b.2.instF σ))

/-- The variables that something is given for. -/
def LinkedProblem.vars (P : LinkedProblem) : List Nat := P.lower.flatMap (·.2.muVars)

/-- Patterns and bounds without function types, and what's given
well-formed. -/
def LinkedProblem.Ok (P : LinkedProblem) : Prop :=
  (∀ c ∈ P.lower, c.2.NoFn ∧ AtMost .none .mutable c.1) ∧ ∀ b ∈ P.bounds, b.2.NoFn

theorem LinkedProblem.Solves.meet {P : LinkedProblem} (hP : P.Ok) {σ₁ σ₂ : Nat → Mu}
    (h₁ : P.Solves σ₁) (h₂ : P.Solves σ₂) : P.Solves (meetF σ₁ σ₂) := by
  refine ⟨fun c hc => ?_, fun b hb => ?_⟩
  · exact Sub.none_meet (hP.1 c hc).1 (hP.1 c hc).2 (h₁.1 c hc) (h₂.1 c hc)
  · simpa [Mu.meet] using AtMost.none_meet (hP.2 b hb) (h₁.2 b hb) (h₂.2 b hb)

/-- A solution that isn't `ro` wherever any solution isn't, for the
variables in `L`. -/
theorem LinkedProblem.exists_low {P : LinkedProblem} (hP : P.Ok) {σ₀ : Nat → Mu}
    (h₀ : P.Solves σ₀) : ∀ L : List Nat, ∃ W, P.Solves W ∧
      ∀ j ∈ L, ∀ σ, P.Solves σ → σ j ≠ .readonly → W j ≠ .readonly
  | [] => ⟨σ₀, h₀, by simp⟩
  | j :: L => by
    obtain ⟨W, hW, hlow⟩ := LinkedProblem.exists_low hP h₀ L
    by_cases h : ∃ σ, P.Solves σ ∧ σ j ≠ .readonly
    · obtain ⟨σ, hσ, hj⟩ := h
      refine ⟨meetF σ W, hσ.meet hP hW, fun j' hj' σ' hσ' hne => ?_⟩
      simp only [meetF]
      rcases List.mem_cons.mp hj' with rfl | hj'
      · exact Mu.meet_ne_ro (.inl hj)
      · exact Mu.meet_ne_ro (.inr (hlow j' hj' σ' hσ' hne))
    · refine ⟨W, hW, fun j' hj' σ' hσ' hne => ?_⟩
      rcases List.mem_cons.mp hj' with rfl | hj'
      · exact absurd ⟨σ', hσ', hne⟩ h
      · exact hlow j' hj' σ' hσ' hne

/-- **Linked variables have a least solution** if they have one: below
every other on each variable that something is given for, as that or
`ro` when it must be. -/
theorem LinkedProblem.principal {P : LinkedProblem} (hP : P.Ok) {σ₀ : Nat → Mu}
    (h₀ : P.Solves σ₀) :
    ∃ σ, P.Solves σ ∧ ∀ σ', P.Solves σ' → ∀ j ∈ P.vars, σ j = σ' j ∨ σ' j = .readonly := by
  obtain ⟨W, hW, hlow⟩ := LinkedProblem.exists_low hP h₀ P.vars
  refine ⟨W, hW, fun σ' hσ' j hj => ?_⟩
  by_cases hr : σ' j = .readonly
  · exact .inr hr
  · have hWj := hlow j hj σ' hσ' hr
    obtain ⟨c, hc, hjc⟩ := List.mem_flatMap.mp hj
    rcases Sub.none_agree (hP.1 c hc).1 hjc (hW.1 c hc) (hσ'.1 c hc) with h | h | h
    · exact .inl h
    · exact absurd h hWj
    · exact absurd h hr

/-! ## Example -/

/-- `f[T](a: [T], b: [T])` called with `mut [mut [1]]` and `mut [[1]]`:
`a: [T]` is `?0 [?1 [i32]]`, as is `b`. -/
def twoArrays : LinkedProblem :=
  { lower := [(.cell .mutable (.cell .mutable .int), .cell (.param 0) (.cell (.param 1) .int)),
      (.cell .mutable (.cell .immutable .int), .cell (.param 0) (.cell (.param 1) .int))] }

/-- Both `ro`: `ro [ro [i32]]`. -/
example : twoArrays.Solves (fun _ => .readonly) := by
  refine ⟨fun c hc => ?_, by simp [twoArrays]⟩
  simp only [twoArrays, List.mem_cons, List.not_mem_nil, or_false] at hc
  rcases hc with rfl | rfl <;> simp only [Ty.instF, Mu.substF] <;>
    exact .ro (.ro (.refl _) trivial) ⟨.refl _, trivial⟩

/-- And the array can't be `mut`: what's in it would have to be both a
`mut [i32]` and an `imm` one. -/
example (σ : Nat → Mu) (h : twoArrays.Solves σ) : σ 0 = .readonly := by
  have h₁ := h.1 (.cell .mutable (.cell .mutable .int), .cell (.param 0) (.cell (.param 1) .int))
    (by simp [twoArrays])
  have h₂ := h.1 (.cell .mutable (.cell .immutable .int), .cell (.param 0) (.cell (.param 1) .int))
    (by simp [twoArrays])
  simp only [Ty.instF, Mu.substF] at h₁ h₂
  rcases Sub.cell_cases h₂ with ⟨_, e₂⟩ | ⟨h0, -⟩ | ⟨h, -⟩
  · rcases Sub.cell_cases h₁ with ⟨_, e₁⟩ | ⟨h0, -⟩ | ⟨h, -⟩
    · simp only [Ty.cell.injEq, and_true] at e₁ e₂
      exact absurd (e₁.trans e₂.symm) (by decide)
    · exact h0
    · cases h
  · exact h0
  · cases h

end BoltMut
