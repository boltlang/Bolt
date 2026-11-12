import BoltMut.Least

/-!
# Principal solutions

At a call, the checker gets constraints about a type variable `?t`: what
flows into it (`τ ≤ ?t`), where its value goes (`?t ≤ σ`), what it must
be exactly, like the `T` of `mut [T]` given a `mut [i32]` (`?t = ε`), and
what it must be at most (`?t ⊑ ro`). With an inferred literal there's also
a mutability variable: the literal's `?m`, with `?t ⊑ ?m ⊑ ro`.

A solution is *principal* if it's below every other solution, so taking it
never rules out what another solution would allow. This file shows when
there is one:

- `Problem.principal`: constraints about one type variable have a least
  solution whenever they have a solution at all, if something flows into
  it or it's given exactly, and no function type flows into it.
- `Problem.principal_lit`: so do those of an inferred literal, `?t` and its
  `?m` together, with nothing assumed: `?m` is `imm` if the least `?t` is
  `imm` all the way down, and `ro` otherwise.
- `no_least_lit`: but not with what a function assumes. Mutabilities that
  a function relates by its assumptions needn't have a least one above
  them, so a literal holding them can have two solutions, neither below
  the other.

With nothing flowing in, there's no least type at all: nothing is below
every type. The checker leaves such a variable for the function's type to
be generic over, or gives it a default.
-/

namespace BoltMut

/-! ## The least type above many -/

/-- No function type in it. -/
def Ty.NoFn : Ty → Prop
  | .fn .. => False
  | .cell _ s => s.NoFn
  | .prod a b => a.NoFn ∧ b.NoFn
  | _ => True

theorem Ty.NoFn.agree : ∀ {t : Ty}, t.NoFn → ∀ t', t.Agree t'
  | .unit, _, t' | .int, _, t' | .tparam _, _, t' | .roParam _, _, t' => by
    cases t' <;> trivial
  | .cell _ s, h, t' => by
    cases t' <;> first | trivial | exact Ty.NoFn.agree (t := s) h _
  | .prod a b, h, t' => by
    cases t' <;> first | trivial | exact ⟨Ty.NoFn.agree h.1 _, Ty.NoFn.agree h.2 _⟩

/-- The least type above all of them, and with `b` the least such type
that's at most `ro`. -/
noncomputable def Ty.lubs (Δ : Assume) (b : Bool) : List Ty → Option Ty
  | [] => none
  | [t] => Ty.lub Δ b t t
  | t :: t' :: ts => (Ty.lubs Δ b (t' :: ts)).bind (Ty.lub Δ b t ·)

theorem Ty.lubs_sound {Δ : Assume} {b : Bool} :
    ∀ {ts : List Ty} {j : Ty}, Ty.lubs Δ b ts = some j →
      (∀ τ ∈ ts, Sub Δ τ j) ∧ (b = true → AtMost Δ .readonly j)
  | [], _, h => by simp [Ty.lubs] at h
  | [t], j, h => by
    obtain ⟨s1, -, s3⟩ := Ty.lub_sound t t j b h
    exact ⟨by simpa using s1, s3⟩
  | t :: t' :: ts, j, h => by
    simp only [Ty.lubs, Option.bind_eq_some_iff] at h
    obtain ⟨j', hj', hj⟩ := h
    obtain ⟨ih, -⟩ := Ty.lubs_sound hj'
    obtain ⟨s1, s2, s3⟩ := Ty.lub_sound t j' j b hj
    refine ⟨fun τ hτ => ?_, s3⟩
    rcases List.mem_cons.mp hτ with rfl | hτ
    · exact s1
    · exact (ih τ hτ).trans s2

theorem Ty.lubs_least {Δ : Assume} {b : Bool} (hΔ : ¬ Le Δ .readonly .immutable) :
    ∀ {ts : List Ty} {u : Ty}, ts ≠ [] → (∀ τ ∈ ts, τ.NoFn) → (∀ τ ∈ ts, Sub Δ τ u) →
      (b = true → AtMost Δ .readonly u) → ∃ j, Ty.lubs Δ b ts = some j ∧ Sub Δ j u
  | [], _, h, _, _, _ => absurd rfl h
  | [t], u, _, _, hs, hb =>
    Ty.lub_least hΔ t t u b t.agree_refl (hs t (by simp)) (hs t (by simp)) hb
  | t :: t' :: ts, u, _, hn, hs, hb => by
    obtain ⟨j', hj', sj'⟩ := Ty.lubs_least hΔ (List.cons_ne_nil t' ts)
      (fun τ h => hn τ (List.mem_cons_of_mem _ h)) (fun τ h => hs τ (List.mem_cons_of_mem _ h)) hb
    obtain ⟨j, hj, sj⟩ :=
      Ty.lub_least hΔ t j' u b ((hn t (by simp)).agree j') (hs t (by simp)) sj' hb
    exact ⟨j, by simp only [Ty.lubs, hj', Option.bind_some, hj], sj⟩

/-! ## One type variable -/

/-- What a type variable must be: above each of `lower`, below each of
`upper`, each of `eqs`, and at most `ro` with `ro`. -/
structure Problem where
  lower : List Ty
  upper : List Ty := []
  eqs : List Ty := []
  ro : Bool := false

def Problem.Solves (Δ : Assume) (P : Problem) (t : Ty) : Prop :=
  (∀ τ ∈ P.lower, Sub Δ τ t) ∧ (∀ σ ∈ P.upper, Sub Δ t σ) ∧ (∀ ε ∈ P.eqs, t = ε) ∧
    (P.ro = true → AtMost Δ .readonly t)

/-- **Constraints about one type variable have a least solution** if they
have any, when something flows into it or it's given exactly, and no
function type flows into it. It's the least type above what flows in. -/
theorem Problem.principal {Δ : Assume} (hΔ : ¬ Le Δ .readonly .immutable) (P : Problem)
    (hl : P.lower ≠ [] ∨ P.eqs ≠ []) (hn : ∀ τ ∈ P.lower, τ.NoFn) {t' : Ty}
    (ht' : P.Solves Δ t') : ∃ t, P.Solves Δ t ∧ ∀ u, P.Solves Δ u → Sub Δ t u := by
  cases he : P.eqs with
  | cons ε es =>
    -- It's given exactly, so there's one solution.
    refine ⟨t', ht', fun u hu => ?_⟩
    rw [ht'.2.2.1 ε (by simp [he]), hu.2.2.1 ε (by simp [he])]
    exact .refl _
  | nil =>
    have hne : P.lower ≠ [] := by rcases hl with h | h; exact h; exact absurd he h
    obtain ⟨j, hj, sj⟩ := Ty.lubs_least (b := P.ro) hΔ hne hn ht'.1 ht'.2.2.2
    obtain ⟨hlo, hro⟩ := Ty.lubs_sound hj
    refine ⟨j, ⟨hlo, fun σ hσ => sj.trans (ht'.2.1 σ hσ), by simp [he], hro⟩, fun u hu => ?_⟩
    obtain ⟨j', hj', sj'⟩ := Ty.lubs_least (b := P.ro) hΔ hne hn hu.1 hu.2.2.2
    rw [hj] at hj'
    cases hj'
    exact sj'

/-! ## An inferred literal -/

/-- What an inferred literal must be, with nothing assumed: a mutability
`m` at most `ro`, and a type `t` for what it holds, solving `P`, with
`t ⊑ m`. -/
def Problem.SolvesLit (P : Problem) (m : Mu) (t : Ty) : Prop :=
  Le .none m .readonly ∧ P.Solves .none t ∧ AtMost .none m t

/-- **An inferred literal has a least solution**, with nothing assumed,
when what it holds does: `?t` is the least type at most `ro`, and `?m` is
`imm` if that's `imm` all the way down, and `ro` otherwise. -/
theorem Problem.principal_lit (P : Problem) (hl : P.lower ≠ [] ∨ P.eqs ≠ [])
    (hn : ∀ τ ∈ P.lower, τ.NoFn) {m' : Mu} {t' : Ty} (h' : P.SolvesLit m' t') :
    ∃ m t, P.SolvesLit m t ∧ ∀ μ u, P.SolvesLit μ u → Le .none m μ ∧ Sub .none t u := by
  -- At most `m`, which is at most `ro`, is at most `ro`.
  let P' : Problem := { P with ro := true }
  have lift : ∀ {μ u}, P.SolvesLit μ u → P'.Solves .none u := fun ⟨hμ, hs, ha⟩ =>
    ⟨hs.1, hs.2.1, hs.2.2.1, fun _ => ha.mono hμ⟩
  obtain ⟨t, ht, hleast⟩ := Problem.principal Le.none_not_ro_imm P' hl hn (lift h')
  have hP : P.Solves .none t := ⟨ht.1, ht.2.1, ht.2.2.1, fun _ => ht.2.2.2 rfl⟩
  by_cases hi : AtMost .none .immutable t
  · refine ⟨.immutable, t, ⟨.bot _, hP, hi⟩, fun μ u hu => ⟨.bot _, hleast u (lift hu)⟩⟩
  · refine ⟨.readonly, t, ⟨.refl _, hP, ht.2.2.2 rfl⟩, fun μ u hu => ⟨?_, hleast u (lift hu)⟩⟩
    -- `μ` is `imm` or `ro`, and `imm` would make `t` `imm` all the way down.
    rcases hu.1.none_inv with rfl | h | rfl
    · exact absurd ((hleast u (lift hu)).atMost_imm_of Le.none_not_ro_imm hu.2.2) hi
    · cases h
    · exact .refl _

/-! ## With assumptions, there may be none -/

/-- What's above something in `S` is in `S`, if `S` holds `mut` but not
`imm`, and each assumption leads from `S` to `S`. -/
theorem Le.closed {Δ : Assume} (S : Mu → Prop) (hmut : S .mutable) (himm : ¬ S .immutable)
    (hΔ : ∀ p ∈ Δ.mus, S p.1 → S p.2) {x y : Mu} (h : Le Δ x y) : S x → S y := by
  induction h with
  | bot => exact fun h => absurd h himm
  | top => exact fun _ => hmut
  | refl => exact id
  | assume hp => exact hΔ _ hp
  | trans _ _ ih₁ ih₂ => exact ih₂ ∘ ih₁

/-- `fn f['a, 'b, 'c, 'd]` with `'a ⊑ 'c`, `'b ⊑ 'c`, `'a ⊑ 'd`, `'b ⊑ 'd`,
`'c ⊑ ro` and `'d ⊑ ro`: both `'c` and `'d` are above `'a` and `'b`, and
neither is below the other. -/
def twoAbove : Assume :=
  { mus := [(.param 0, .param 2), (.param 1, .param 2), (.param 0, .param 3),
      (.param 1, .param 3), (.param 2, .readonly), (.param 3, .readonly)] }

/-- What `(x, y)` holds for `x: 'a [i32]` and `y: 'b [i32]`. -/
def twoCells : Ty := .prod (.cell (.param 0) .int) (.cell (.param 1) .int)

/-- **With assumptions, an inferred literal can have no least
mutability**: `(x, y)` in `f` can be `'c` and can be `'d`, but nothing it
can be is below both. -/
theorem no_least_lit :
    (Le twoAbove (.param 2) .readonly ∧ AtMost twoAbove (.param 2) twoCells) ∧
    (Le twoAbove (.param 3) .readonly ∧ AtMost twoAbove (.param 3) twoCells) ∧
    ¬ ∃ m, AtMost twoAbove m twoCells ∧ Le twoAbove m (.param 2) ∧ Le twoAbove m (.param 3) := by
  have a : ∀ i j, (Mu.param i, Mu.param j) ∈ twoAbove.mus → Le twoAbove (.param i) (.param j) :=
    fun _ _ h => .assume h
  refine ⟨⟨.assume (by simp [twoAbove]), ⟨a 0 2 (by simp [twoAbove]), trivial⟩,
      a 1 2 (by simp [twoAbove]), trivial⟩,
    ⟨.assume (by simp [twoAbove]), ⟨a 0 3 (by simp [twoAbove]), trivial⟩,
      a 1 3 (by simp [twoAbove]), trivial⟩, ?_⟩
  rintro ⟨m, ⟨⟨h0, -⟩, h1, -⟩, h2, h3⟩
  -- Above `'a`: `'a`, `'c`, `'d`, `ro` or `mut`.
  have up0 := Le.closed (· ∈ [.param 0, .param 2, .param 3, .readonly, .mutable]) (by simp)
    (by simp) (by decide) h0 (by simp)
  -- Below `'c`: `imm`, `'a`, `'b` or `'c`; below `'d`: `imm`, `'a`, `'b` or `'d`.
  have dn2 := Le.closed (· ∉ [.immutable, .param 0, .param 1, .param 2]) (by simp) (by simp)
    (by decide) h2
  have dn3 := Le.closed (· ∉ [.immutable, .param 0, .param 1, .param 3]) (by simp) (by simp)
    (by decide) h3
  -- So `m` is `'a`, which isn't above `'b`.
  have up1 := Le.closed (· ∈ [.param 1, .param 2, .param 3, .readonly, .mutable]) (by simp)
    (by simp) (by decide) h1 (by simp)
  simp only [List.mem_cons, List.not_mem_nil, or_false] at up0 up1 dn2 dn3
  rcases up0 with rfl | rfl | rfl | rfl | rfl <;> simp_all

end BoltMut
