import BoltMut.Syntax

/-!
# The solver's shortcuts

The checker doesn't search for derivations of `AtMost` and `Le`: its solver
(`crates/boltlang-hir-ty/src/solve.rs`) takes shortcuts. This file proves
that they're right, and when.

- **Splitting** (`Solver::solve_at_most`): "a type is at most `μ`" is split
  into the mutabilities it holds, each at most the one around it, and the
  type parameters in it. `atMost_iff_split`: that says exactly what
  `AtMost` does.
- **Deciding early**: an unknown mutability that must be at most `imm` is
  made `imm` right away, and one that something `mut` must be below is made
  `mut`. `Le.eq_immutable` and `Le.eq_mutable`: that loses no solution,
  unless an assumption ends at `imm`, like `'v: imm`, or starts at `mut`.
  Then a parameter is a solution too, as the examples show.
- **Following assumptions** (`Solver::mutability_le`): `m ⊑ μ` holds by the
  order, or through a chain of assumptions. `leSearch` follows them, from
  `m` and from `mut`, which everything is below. `le_iff_leSearch`: it finds
  every chain once it can take as many steps as there are assumptions,
  since a chain never needs one twice. `leSearchOld`, which follows them
  from `m` only, at most 8 steps, misses some.
-/

namespace BoltMut

/-! ## Splitting a type into what it holds -/

/-- What "at most" is split into: a mutability at most another, or a type
parameter at most a mutability. -/
inductive Atom where
  | le (m μ : Mu)
  | tparam (i : Nat) (μ : Mu)
  deriving DecidableEq, Repr

/-- What an atom says, as `AtMost` does. -/
def Atom.Holds (Δ : Assume) : Atom → Prop
  | .le m μ => Le Δ m μ
  | .tparam i μ => ∃ μ', (μ' = .mutable ∨ (i, μ') ∈ Δ.tys) ∧ Le Δ μ' μ

/-- What the solver splits "`t` is at most `μ`" into: a cell is at most `μ`,
and what it holds at most the cell's own mutability; a closure is at most
`μ`; a tuple is split into its parts, like an enum; a type parameter stays. -/
def split (μ : Mu) : Ty → List Atom
  | .unit | .int => []
  | .cell m t => .le m μ :: split m t
  | .fn m _ _ => [.le m μ]
  | .prod a b => split μ a ++ split μ b
  | .tparam i => [.tparam i μ]

/-- Splitting says exactly what `AtMost` does. -/
theorem atMost_iff_split (Δ : Assume) (μ : Mu) (t : Ty) :
    AtMost Δ μ t ↔ ∀ a ∈ split μ t, a.Holds Δ := by
  induction t generalizing μ with
  | unit => simp [AtMost, split]
  | int => simp [AtMost, split]
  | cell m t ih => simp [AtMost, split, Atom.Holds, ih]
  | fn m a r => simp [AtMost, split, Atom.Holds]
  | prod a b iha ihb => simp [AtMost, split, iha, ihb, or_imp, forall_and]
  | tparam i => simp [AtMost, split, Atom.Holds]

/-! ## Deciding early -/

theorem Le.eq_immutable_of {Δ : Assume} (hΔ : ∀ p ∈ Δ.mus, p.2 ≠ .immutable) {m μ : Mu}
    (h : Le Δ m μ) : μ = .immutable → m = .immutable := by
  induction h with
  | bot => intro; rfl
  | top => intro h; cases h
  | refl => exact id
  | assume hp => intro h; subst h; exact (hΔ _ hp rfl).elim
  | trans _ _ ih₁ ih₂ => intro h; exact ih₁ (ih₂ h)

/-- Only `imm` is at most `imm`, unless an assumption ends at `imm`. So the
solver can make an unknown `m` with `m ⊑ imm` into `imm` right away. -/
theorem Le.eq_immutable {Δ : Assume} (hΔ : ∀ p ∈ Δ.mus, p.2 ≠ .immutable) {m : Mu}
    (h : Le Δ m .immutable) : m = .immutable :=
  Le.eq_immutable_of hΔ h rfl

theorem Le.eq_mutable_of {Δ : Assume} (hΔ : ∀ p ∈ Δ.mus, p.1 ≠ .mutable) {m μ : Mu}
    (h : Le Δ m μ) : m = .mutable → μ = .mutable := by
  induction h with
  | bot => intro h; cases h
  | top => intro; rfl
  | refl => exact id
  | assume hp => intro h; subst h; exact (hΔ _ hp rfl).elim
  | trans _ _ ih₁ ih₂ => intro h; exact ih₂ (ih₁ h)

/-- Only `mut` is above `mut`, unless an assumption starts at `mut`. So the
solver can make an unknown `μ` with `mut ⊑ μ` into `mut` right away. -/
theorem Le.eq_mutable {Δ : Assume} (hΔ : ∀ p ∈ Δ.mus, p.1 ≠ .mutable) {μ : Mu}
    (h : Le Δ .mutable μ) : μ = .mutable :=
  Le.eq_mutable_of hΔ h rfl

/-- With `'v: imm` assumed, `'v` is at most `imm` too, so an unknown below
`imm` can't be decided right away: it could be `'v`. -/
example : Le { mus := [(.param 0, .immutable)] } (.param 0) .immutable := .assume (by simp)

/-- With `mut: 'v` assumed, `mut` is below `'v` too. -/
example : Le { mus := [(.mutable, .param 0)] } .mutable (.param 0) := .assume (by simp)

/-! ## Following assumptions -/

/-- `m ⊑ μ` by the order itself. -/
def baseLe (m μ : Mu) : Bool :=
  m == μ || m == .immutable || μ == .mutable

/-- Whether `m ⊑ μ`, following the assumptions at most `fuel` steps: by the
order itself, or through an assumption `a ⊑ b` that starts at `m`, or at
`mut`, which everything is below, with `b ⊑ μ`. -/
def leSearch (Δ : Assume) : Nat → Mu → Mu → Bool
  | 0, m, μ => baseLe m μ
  | n + 1, m, μ => baseLe m μ || Δ.mus.any fun p => (p.1 == m || p.1 == .mutable) && leSearch Δ n p.2 μ

theorem Le.of_baseLe {Δ : Assume} {m μ : Mu} (h : baseLe m μ = true) : Le Δ m μ := by
  simp only [baseLe, Bool.or_eq_true, beq_iff_eq] at h
  rcases h with (h | h) | h
  · subst h; exact .refl _
  · subst h; exact .bot _
  · subst h; exact .top _

/-- What the search finds holds. -/
theorem leSearch_sound {Δ : Assume} : ∀ {n : Nat} {m μ : Mu}, leSearch Δ n m μ = true → Le Δ m μ
  | 0, _, _, h => Le.of_baseLe h
  | n + 1, m, μ, h => by
    simp only [leSearch, Bool.or_eq_true, List.any_eq_true, Bool.and_eq_true, beq_iff_eq] at h
    rcases h with h | ⟨⟨a, b⟩, hp, hm, hs⟩
    · exact Le.of_baseLe h
    · have hb : Le Δ b μ := leSearch_sound hs
      simp only at hm
      rcases hm with hm | hm
      · subst hm; exact .trans (.assume hp) hb
      · subst hm; exact .trans (.top m) (.trans (.assume hp) hb)

/-- A chain of assumptions from `m` to `μ`, as the search follows them: each
starts where the one before ended, or at `mut`, and the last ends below
`μ` by the order. -/
inductive Chain (Δ : Assume) : Mu → Mu → List (Mu × Mu) → Prop where
  | base {m μ : Mu} : baseLe m μ = true → Chain Δ m μ []
  | step {m μ a b : Mu} {ps : List (Mu × Mu)} :
      (a, b) ∈ Δ.mus → (a = m ∨ a = .mutable) → Chain Δ b μ ps → Chain Δ m μ ((a, b) :: ps)

theorem leSearch_of_baseLe {Δ : Assume} {n : Nat} {m μ : Mu} (h : baseLe m μ = true) :
    leSearch Δ n m μ = true := by
  cases n <;> simp [leSearch, h]

/-- The search finds a chain if it can take as many steps as the chain has. -/
theorem Chain.search {Δ : Assume} {m μ : Mu} {ps : List (Mu × Mu)} (h : Chain Δ m μ ps) :
    ∀ n, ps.length ≤ n → leSearch Δ n m μ = true := by
  induction h with
  | base hb => intro n _; exact leSearch_of_baseLe hb
  | step hp hm _ ih =>
    intro n hn
    cases n with
    | zero => simp at hn
    | succ n =>
      simp only [leSearch, Bool.or_eq_true, List.any_eq_true, Bool.and_eq_true, beq_iff_eq]
      exact .inr ⟨_, hp, hm, ih n (by simp at hn; omega)⟩

theorem Chain.subset {Δ : Assume} {m μ : Mu} {ps : List (Mu × Mu)} (h : Chain Δ m μ ps) : ps ⊆ Δ.mus := by
  induction h with
  | base => intro _ h; simp at h
  | step hp _ _ ih =>
    intro x hx
    rcases List.mem_cons.mp hx with rfl | hx
    · exact hp
    · exact ih hx

/-- Chains compose, as `Le.trans` does. A chain to `mut` is followed by
one from `mut`, which a chain can start at from anywhere. -/
theorem Chain.trans {Δ : Assume} {m x ν : Mu} {ps qs : List (Mu × Mu)} (h₁ : Chain Δ m x ps) :
    Chain Δ x ν qs → ∃ rs, Chain Δ m ν rs := by
  induction h₁ with
  | @base m x hb =>
    intro h₂
    simp only [baseLe, Bool.or_eq_true, beq_iff_eq] at hb
    rcases hb with (hb | hb) | hb
    · subst hb; exact ⟨_, h₂⟩
    · subst hb; exact ⟨[], .base (by simp [baseLe])⟩
    · subst hb
      cases h₂ with
      | base hb' =>
        simp only [baseLe, Bool.or_eq_true, beq_iff_eq] at hb'
        refine ⟨[], .base ?_⟩
        rcases hb' with (h | h) | h
        · subst h; simp [baseLe]
        · cases h
        · subst h; simp [baseLe]
      | step hp hm hc => exact ⟨_, .step hp (by rcases hm with h | h <;> exact .inr h) hc⟩
  | step hp hm _ ih =>
    intro h₂
    obtain ⟨rs, hr⟩ := ih h₂
    exact ⟨_, .step hp hm hr⟩

/-- Every `m ⊑ μ` has a chain. -/
theorem Le.chain {Δ : Assume} {m μ : Mu} (h : Le Δ m μ) : ∃ ps, Chain Δ m μ ps := by
  induction h with
  | bot => exact ⟨[], .base (by simp [baseLe])⟩
  | top => exact ⟨[], .base (by simp [baseLe])⟩
  | refl => exact ⟨[], .base (by simp [baseLe])⟩
  | assume hp => exact ⟨_, .step hp (.inl rfl) (.base (by simp [baseLe]))⟩
  | trans _ _ ih₁ ih₂ =>
    obtain ⟨_, h₁⟩ := ih₁
    obtain ⟨_, h₂⟩ := ih₂
    exact h₁.trans h₂

/-- After an assumption, a chain goes on from where it ends, whatever came
before it. -/
theorem Chain.drop {Δ : Assume} {ν : Mu} {p : Mu × Mu} {bs : List (Mu × Mu)} :
    ∀ {as : List (Mu × Mu)} {x : Mu}, Chain Δ x ν (as ++ p :: bs) → Chain Δ p.2 ν bs := by
  intro as
  induction as with
  | nil => intro x h; simp only [List.nil_append] at h; cases h with | step _ _ h => exact h
  | cons c as ih => intro x h; simp only [List.cons_append] at h; cases h with | step _ _ h => exact ih h

/-- A chain never needs an assumption twice: from the second time on, it
goes on as it did from the first. -/
theorem Chain.nodup {Δ : Assume} {m μ : Mu} {ps : List (Mu × Mu)} (h : Chain Δ m μ ps) :
    ∃ qs, Chain Δ m μ qs ∧ qs.Nodup := by
  induction h with
  | base hb => exact ⟨[], .base hb, .nil⟩
  | @step m μ a b ps hp hm _ ih =>
    obtain ⟨qs, hq, hnd⟩ := ih
    by_cases hin : (a, b) ∈ qs
    · obtain ⟨s, t, rfl⟩ := List.append_of_mem hin
      exact ⟨(a, b) :: t, .step hp hm hq.drop, hnd.sublist (List.sublist_append_right s _)⟩
    · exact ⟨(a, b) :: qs, .step hp hm hq, List.nodup_cons.mpr ⟨hin, hnd⟩⟩

/-- The search finds every `m ⊑ μ` once it can take as many steps as there
are assumptions. -/
theorem leSearch_complete {Δ : Assume} {m μ : Mu} (h : Le Δ m μ) : leSearch Δ Δ.mus.length m μ = true := by
  obtain ⟨ps, hc⟩ := h.chain
  obtain ⟨qs, hq, hnd⟩ := hc.nodup
  exact hq.search _ (hnd.length_le_of_subset hq.subset)

theorem le_iff_leSearch (Δ : Assume) (m μ : Mu) : Le Δ m μ ↔ leSearch Δ Δ.mus.length m μ = true :=
  ⟨leSearch_complete, leSearch_sound⟩

/-! ## What the solver did before

`Solver::mutability_le` followed assumptions from `m` only, at most 8
steps. It misses chains of more than 8 assumptions, and those through
`mut`. -/

/-- The search of `Solver::mutability_le` before, with `fuel` 8. -/
def leSearchOld (Δ : Assume) : Nat → Mu → Mu → Bool
  | 0, m, μ => baseLe m μ
  | n + 1, m, μ => baseLe m μ || Δ.mus.any fun p => p.1 == m && leSearchOld Δ n p.2 μ

/-- `'a: 'b, 'b: 'c, ...`: nine assumptions, from `'0` to `'9`. -/
def nineSteps : Assume := { mus := (List.range 9).map fun i => (.param i, .param (i + 1)) }

example : Le nineSteps (.param 0) (.param 9) := leSearch_sound (n := 9) (by decide)
example : leSearchOld nineSteps 8 (.param 0) (.param 9) = false := by decide

/-- `mut: 'v`, so every mutability is at most `'v`. -/
def throughMut : Assume := { mus := [(.mutable, .param 1)] }

example : Le throughMut (.param 0) (.param 1) := .trans (.top _) (.assume (by simp [throughMut]))
example : leSearchOld throughMut 8 (.param 0) (.param 1) = false := by decide

end BoltMut
