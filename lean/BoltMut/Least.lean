import BoltMut.Subtyping

/-!
# Least solutions

At a call, a type parameter is given what flows into it, the types of the
arguments, which must fit it (`Sub`), and what it must be at most, like
the `ro` of `fn pass[T: ro](x: T) -> T`. The checker should give it the
least type that all of them fit, and that's within its bound. This file
shows that type exists and how to find it.

`Ty.lub Δ b t₁ t₂` is the least type both `t₁` and `t₂` fit; with
`b = true`, the least such type that's also at most `ro`. It's `none` when
there's no such type, and then there's no solution at all:

- `Ty.lub_sound`: what it gives is above both, and at most `ro` with `b`.
- `Ty.lub_least`: it's below every other type that is, so it's the least
  one. If there is one at all, it finds it.
- `Ty.least_ro`: with `t₁ = t₂ = t`, the least type above `t` that's at
  most `ro`. It isn't `t`'s view, `t.ro`: what's already at most `ro`, like
  an `imm` array, is kept as it is, since the view is above it.

`lub_least` needs that every function type in `t₁` takes what the one at
the same place in `t₂` takes (`Ty.Agree`), which holds for `t₁ = t₂`. In
general, a function's parameters would need the greatest type below both,
which isn't here. Two functions of different mutabilities have no type
above both at all (`Sub.no_fn_join`).

It also needs that `ro ⊑ imm` isn't assumed: then what's above something
`imm` all the way down is too, which `ro` isn't.
-/

namespace BoltMut

/-! ## Inverting subtyping at the types it ends at -/

theorem Sub.of_tparam {Δ : Assume} {i : Nat} {u : Ty} (h : Sub Δ (.tparam i) u) :
    u = .tparam i ∨ u = .roParam i := by
  cases h
  · exact .inl rfl
  · exact .inr rfl

theorem Sub.of_roParam {Δ : Assume} {i : Nat} {u : Ty} (h : Sub Δ (.roParam i) u) :
    u = .roParam i := by
  cases h; rfl

theorem Sub.to_tparam {Δ : Assume} {i : Nat} {t : Ty} (h : Sub Δ t (.tparam i)) :
    t = .tparam i := by
  cases h; rfl

theorem Sub.to_roParam {Δ : Assume} {i : Nat} {t : Ty} (h : Sub Δ t (.roParam i)) :
    t = .tparam i ∨ t = .roParam i := by
  cases h
  · exact .inr rfl
  · exact .inl rfl

/-- What a cell can be seen as, with what each way needs. -/
theorem Sub.of_cell' {Δ : Assume} {m : Mu} {t u : Ty} (h : Sub Δ (.cell m t) u) :
    u = .cell m t ∨ (∃ s, u = .cell .readonly s ∧ Sub Δ t s ∧ AtMost Δ .readonly s) ∨
      (m = .immutable ∧ ∃ s, u = .cell .immutable s ∧ Sub Δ t s) := by
  cases h with
  | refl => exact .inl rfl
  | ro h ha => exact .inr (.inl ⟨_, rfl, h, ha⟩)
  | immCell h => exact .inr (.inr ⟨rfl, _, rfl, h⟩)

/-- What a type above two cells can be: one of them, when they're the
same, a `ro` cell, or an `imm` one, when both are. -/
theorem Sub.cell_ub {Δ : Assume} {m₁ m₂ : Mu} {s₁ s₂ u : Ty} (h₁ : Sub Δ (.cell m₁ s₁) u)
    (h₂ : Sub Δ (.cell m₂ s₂) u) :
    (m₁ = m₂ ∧ s₁ = s₂ ∧ u = .cell m₁ s₁) ∨
      (∃ s, u = .cell .readonly s ∧ Sub Δ s₁ s ∧ Sub Δ s₂ s ∧ AtMost Δ .readonly s) ∨
      (m₁ = .immutable ∧ m₂ = .immutable ∧ ∃ s, u = .cell .immutable s ∧ Sub Δ s₁ s ∧ Sub Δ s₂ s) := by
  rcases h₁.of_cell' with rfl | ⟨s, rfl, hs, ha⟩ | ⟨rfl, s, rfl, hs⟩ <;>
    rcases h₂.of_cell' with h | ⟨s', h, hs', ha'⟩ | ⟨rfl, s', h, hs'⟩
  · cases h; exact .inl ⟨rfl, rfl, rfl⟩
  · cases h; exact .inr (.inl ⟨_, rfl, .refl _, hs', ha'⟩)
  · cases h; exact .inr (.inr ⟨rfl, rfl, _, rfl, .refl _, hs'⟩)
  · cases h; exact .inr (.inl ⟨_, rfl, hs, .refl _, ha⟩)
  · cases h; exact .inr (.inl ⟨_, rfl, hs, hs', ha⟩)
  · cases h
  · cases h; exact .inr (.inr ⟨rfl, rfl, _, rfl, hs, .refl _⟩)
  · cases h
  · cases h; exact .inr (.inr ⟨rfl, rfl, _, rfl, hs, hs'⟩)

/-- Unless `ro ⊑ imm` is assumed, what's below something `imm` all the way
down is too. -/
theorem Sub.atMost_imm_of {Δ : Assume} (hΔ : ¬ Le Δ .readonly .immutable) {t t' : Ty}
    (h : Sub Δ t t') (hd : AtMost Δ .immutable t') : AtMost Δ .immutable t := by
  induction h with
  | refl => exact hd
  | ro _ _ => exact absurd hd.1 hΔ
  | immCell _ ih => exact ⟨hd.1, ih hd.2⟩
  | prod _ _ iha ihb => exact ⟨iha hd.1, ihb hd.2⟩
  | fn => exact hd
  | tparam => exact absurd hd hΔ

/-! ## The least type above two -/

/-- Every function type in the first takes what the one at the same place
in the second takes. -/
def Ty.Agree : Ty → Ty → Prop
  | .cell _ s, .cell _ s' => s.Agree s'
  | .prod a b, .prod a' b' => a.Agree a' ∧ b.Agree b'
  | .fn _ a r, .fn _ a' r' => a = a' ∧ r.Agree r'
  | _, _ => True

theorem Ty.agree_refl : ∀ t : Ty, t.Agree t
  | .unit | .int | .tparam _ | .roParam _ => trivial
  | .cell _ s => s.agree_refl
  | .prod a b => ⟨a.agree_refl, b.agree_refl⟩
  | .fn _ _ r => ⟨rfl, r.agree_refl⟩

open Classical in
/-- The least type both fit, and with `b` the least such type that's at
most `ro`. Two cells give one of them when they're the same, an `imm` one
when both are, and a `ro` one otherwise. -/
noncomputable def Ty.lub (Δ : Assume) : Bool → Ty → Ty → Option Ty
  | _, .unit, .unit => some .unit
  | _, .int, .int => some .int
  | b, .prod a₁ c₁, .prod a₂ c₂ =>
    (Ty.lub Δ b a₁ a₂).bind fun a => (Ty.lub Δ b c₁ c₂).map fun c => .prod a c
  | false, .tparam i, .tparam j => if i = j then some (.tparam i) else none
  | true, .tparam i, .tparam j =>
    if i = j then
      if AtMost Δ .readonly (.tparam i) then some (.tparam i) else some (.roParam i)
    else none
  | _, .tparam i, .roParam j | _, .roParam i, .tparam j | _, .roParam i, .roParam j =>
    if i = j then some (.roParam i) else none
  | _, .fn m₁ a₁ r₁, .fn m₂ a₂ r₂ =>
    if m₁ = m₂ ∧ a₁ = a₂ then (Ty.lub Δ false r₁ r₂).map (.fn m₁ a₁ ·) else none
  | b, .cell m₁ s₁, .cell m₂ s₂ =>
    if m₁ = .immutable ∧ m₂ = .immutable then
      match Ty.lub Δ false s₁ s₂ with
      | some j =>
        if b = false ∨ AtMost Δ .immutable j then some (.cell .immutable j)
        else (Ty.lub Δ true s₁ s₂).map (.cell .readonly)
      | none => (Ty.lub Δ true s₁ s₂).map (.cell .readonly)
    else if m₁ = m₂ ∧ s₁ = s₂ ∧ (b = false ∨ AtMost Δ .readonly (.cell m₁ s₁)) then
      some (.cell m₁ s₁)
    else (Ty.lub Δ true s₁ s₂).map (.cell .readonly)
  | _, _, _ => none

/-- What `lub` gives is above both, and at most `ro` when asked. -/
theorem Ty.lub_sound {Δ : Assume} : ∀ (t₁ t₂ j : Ty) (b : Bool), Ty.lub Δ b t₁ t₂ = some j →
    Sub Δ t₁ j ∧ Sub Δ t₂ j ∧ (b = true → AtMost Δ .readonly j) := by
  intro t₁
  induction t₁ with
  | unit =>
    intro t₂ j b h
    cases b <;> cases t₂ <;> simp [Ty.lub] at h <;> subst h <;> exact ⟨.refl _, .refl _, fun _ => trivial⟩
  | int =>
    intro t₂ j b h
    cases b <;> cases t₂ <;> simp [Ty.lub] at h <;> subst h <;> exact ⟨.refl _, .refl _, fun _ => trivial⟩
  | prod a c iha ihc =>
    intro t₂ j b h
    cases b <;> cases t₂ <;> simp only [Ty.lub, reduceCtorEq] at h <;>
      simp only [Option.bind_eq_some_iff, Option.map_eq_some_iff] at h <;>
    · obtain ⟨x, hx, y, hy, rfl⟩ := h
      obtain ⟨s1, s2, s3⟩ := iha _ _ _ hx
      obtain ⟨t1, t2, t3⟩ := ihc _ _ _ hy
      exact ⟨.prod s1 t1, .prod s2 t2, fun hb => ⟨s3 hb, t3 hb⟩⟩
  | tparam i =>
    intro t₂ j b h
    cases b <;> cases t₂ <;> simp [Ty.lub] at h
    · obtain ⟨rfl, rfl⟩ := h; exact ⟨.refl _, .refl _, nofun⟩
    · obtain ⟨rfl, rfl⟩ := h; exact ⟨.tparam _, .refl _, nofun⟩
    · obtain ⟨rfl, h⟩ := h
      by_cases ha : AtMost Δ .readonly (.tparam i)
      · simp [ha] at h; subst h; exact ⟨.refl _, .refl _, fun _ => ha⟩
      · simp [ha] at h; subst h; exact ⟨.tparam _, .tparam _, fun _ => .refl _⟩
    · obtain ⟨rfl, rfl⟩ := h; exact ⟨.tparam _, .refl _, fun _ => .refl _⟩
  | roParam i =>
    intro t₂ j b h
    cases b <;> cases t₂ <;> simp [Ty.lub] at h <;> obtain ⟨rfl, rfl⟩ := h
    · exact ⟨.refl _, .tparam _, nofun⟩
    · exact ⟨.refl _, .refl _, nofun⟩
    · exact ⟨.refl _, .tparam _, fun _ => .refl _⟩
    · exact ⟨.refl _, .refl _, fun _ => .refl _⟩
  | fn m a r _ ihr =>
    intro t₂ j b h
    cases b <;> cases t₂ <;> simp [Ty.lub] at h <;>
    · obtain ⟨⟨rfl, rfl⟩, x, hx, rfl⟩ := h
      obtain ⟨s1, s2, _⟩ := ihr _ _ _ hx
      exact ⟨.fn (.refl _) s1, .fn (.refl _) s2, fun _ => .inr (.refl _)⟩
  | cell m₁ s₁ ih =>
    intro t₂ j b h
    cases t₂ <;> simp only [Ty.lub] at h <;> try cases h
    rename_i m₂ s₂
    -- A `ro` cell of what's above both, at most `ro`.
    have ro : ∀ {x}, Ty.lub Δ true s₁ s₂ = some x →
        Sub Δ (.cell m₁ s₁) (.cell .readonly x) ∧ Sub Δ (.cell m₂ s₂) (.cell .readonly x) ∧
          (b = true → AtMost Δ .readonly (.cell .readonly x)) := fun hx => by
      obtain ⟨s1, s2, s3⟩ := ih _ _ _ hx
      exact ⟨.ro s1 (s3 rfl), .ro s2 (s3 rfl), fun _ => ⟨.refl _, s3 rfl⟩⟩
    split at h
    · rename_i hi
      obtain ⟨rfl, rfl⟩ := hi
      split at h
      · rename_i x hx
        split at h
        · rename_i hc
          cases h
          obtain ⟨s1, s2, _⟩ := ih _ _ _ hx
          refine ⟨.immCell s1, .immCell s2, fun hb => ⟨.bot _, ?_⟩⟩
          rcases hc with hc | hc
          · simp [hb] at hc
          · exact hc
        · obtain ⟨y, hy, rfl⟩ := Option.map_eq_some_iff.mp h; exact ro hy
      · obtain ⟨y, hy, rfl⟩ := Option.map_eq_some_iff.mp h; exact ro hy
    · split at h
      · rename_i hc
        obtain ⟨rfl, rfl, hc⟩ := hc
        cases h
        refine ⟨.refl _, .refl _, fun hb => ?_⟩
        rcases hc with hc | hc
        · simp [hb] at hc
        · exact hc
      · obtain ⟨y, hy, rfl⟩ := Option.map_eq_some_iff.mp h; exact ro hy

/-- **What `lub` gives is the least**: below every type above both (and at
most `ro`, when asked). So if there's any such type, `lub` finds one. -/
theorem Ty.lub_least {Δ : Assume} (hΔ : ¬ Le Δ .readonly .immutable) :
    ∀ (t₁ t₂ u : Ty) (b : Bool), t₁.Agree t₂ → Sub Δ t₁ u → Sub Δ t₂ u →
      (b = true → AtMost Δ .readonly u) → ∃ j, Ty.lub Δ b t₁ t₂ = some j ∧ Sub Δ j u := by
  intro t₁
  induction t₁ with
  | unit =>
    intro t₂ u b _ h₁ h₂ _
    cases h₁; cases h₂; exact ⟨.unit, by cases b <;> simp [Ty.lub], .refl _⟩
  | int =>
    intro t₂ u b _ h₁ h₂ _
    cases h₁; cases h₂; exact ⟨.int, by cases b <;> simp [Ty.lub], .refl _⟩
  | prod a c iha ihc =>
    intro t₂ u b hag h₁ h₂ hb
    obtain ⟨a', c', rfl, ha₁, hc₁⟩ := h₁.of_prod
    obtain ⟨a₂, c₂, rfl, ha₂, hc₂⟩ := h₂.to_prod
    simp only [Ty.Agree] at hag
    obtain ⟨x, hx, sx⟩ := iha a₂ a' b hag.1 ha₁ ha₂ (fun h => (hb h).1)
    obtain ⟨y, hy, sy⟩ := ihc c₂ c' b hag.2 hc₁ hc₂ (fun h => (hb h).2)
    exact ⟨.prod x y, by cases b <;> simp [Ty.lub, hx, hy], .prod sx sy⟩
  | tparam i =>
    intro t₂ u b _ h₁ h₂ hb
    rcases h₁.of_tparam with rfl | rfl
    · obtain rfl := h₂.to_tparam
      cases b
      · exact ⟨_, by simp [Ty.lub], .refl _⟩
      · exact ⟨_, by simp [Ty.lub, hb rfl], .refl _⟩
    · rcases h₂.to_roParam with rfl | rfl
      · cases b
        · exact ⟨_, by simp [Ty.lub], .tparam i⟩
        · by_cases h : AtMost Δ .readonly (.tparam i)
          · exact ⟨_, by simp [Ty.lub, h], .tparam i⟩
          · exact ⟨_, by simp [Ty.lub, h], .refl _⟩
      · exact ⟨_, by cases b <;> simp [Ty.lub], .refl _⟩
  | roParam i =>
    intro t₂ u b _ h₁ h₂ _
    obtain rfl := h₁.of_roParam
    rcases h₂.to_roParam with rfl | rfl <;> exact ⟨_, by cases b <;> simp [Ty.lub], .refl _⟩
  | fn m a r _ ihr =>
    intro t₂ u b hag h₁ h₂ _
    obtain ⟨a', r', rfl, sa, sr⟩ := h₁.of_fn
    obtain ⟨a₂, r₂, rfl⟩ := h₂.to_fn
    obtain ⟨_, _, he, _, sr₂⟩ := h₂.of_fn
    cases he
    simp only [Ty.Agree] at hag
    obtain ⟨rfl, hr⟩ := hag
    obtain ⟨x, hx, sx⟩ := ihr r₂ r' false hr sr sr₂ nofun
    exact ⟨.fn m a x, by cases b <;> simp [Ty.lub, hx], .fn sa sx⟩
  | cell m₁ s₁ ih =>
    intro t₂ u b hag h₁ h₂ hb
    obtain ⟨_, _, rfl, -, -⟩ := h₁.of_cell
    obtain ⟨m₂, s₂, rfl⟩ := h₂.to_cell
    simp only [Ty.Agree] at hag
    -- What's above both as a `ro` cell is above the `ro` cell `lub` gives.
    have viaRo : ∀ {s}, Sub Δ s₁ s → Sub Δ s₂ s → AtMost Δ .readonly s →
        ∃ j, (Ty.lub Δ true s₁ s₂).map (.cell .readonly) = some j ∧ Sub Δ j (.cell .readonly s) :=
      fun hs₁ hs₂ ha => by
        obtain ⟨x, hx, sx⟩ := ih s₂ _ true hag hs₁ hs₂ (fun _ => ha)
        exact ⟨.cell .readonly x, by simp [hx], .ro sx ha⟩
    have hu := Sub.cell_ub h₁ h₂
    by_cases hi : m₁ = .immutable ∧ m₂ = .immutable
    · obtain ⟨rfl, rfl⟩ := hi
      simp only [Ty.lub, and_self, ite_true]
      -- `u` is an `imm` or a `ro` cell above both.
      rcases hu with ⟨-, rfl, ⟨⟩⟩ | ⟨s, ⟨⟩, hs₁, hs₂, ha⟩ | ⟨-, -, s, ⟨⟩, hs₁, hs₂⟩
      · obtain ⟨x, hx, sx⟩ := ih s₁ s₁ false hag (.refl _) (.refl _) nofun
        have hd : b = false ∨ AtMost Δ .immutable x := by
          cases b
          · exact .inl rfl
          · exact .inr (sx.atMost_imm_of hΔ (hb rfl).2)
        exact ⟨.cell .immutable x, by simp [hx, hd], .immCell sx⟩
      · obtain ⟨x, hx, sx⟩ := ih s₂ _ false hag hs₁ hs₂ nofun
        rw [hx]
        by_cases hd : b = false ∨ AtMost Δ .immutable x
        · exact ⟨.cell .immutable x, by simp [hd], .ro sx ha⟩
        · simp only [hd, ite_false]; exact viaRo hs₁ hs₂ ha
      · obtain ⟨x, hx, sx⟩ := ih s₂ _ false hag hs₁ hs₂ nofun
        have hd : b = false ∨ AtMost Δ .immutable x := by
          cases b
          · exact .inl rfl
          · exact .inr (sx.atMost_imm_of hΔ (hb rfl).2)
        exact ⟨.cell .immutable x, by simp [hx, hd], .immCell sx⟩
    · simp only [Ty.lub, hi, ite_false]
      by_cases he : m₁ = m₂ ∧ s₁ = s₂ ∧ (b = false ∨ AtMost Δ .readonly (.cell m₁ s₁))
      · exact ⟨_, by split; rfl; contradiction, h₁⟩
      · simp only [he, ite_false]
        rcases hu with ⟨rfl, rfl, ⟨⟩⟩ | ⟨s, ⟨⟩, hs₁, hs₂, ha⟩ | ⟨h₁', h₂', -⟩
        · cases b
          · exact absurd ⟨rfl, rfl, .inl rfl⟩ he
          · exact absurd ⟨rfl, rfl, .inr (hb rfl)⟩ he
        · exact viaRo hs₁ hs₂ ha
        · exact absurd ⟨h₁', h₂'⟩ hi

/-- A program that runs assumes nothing, so not `ro ⊑ imm`. -/
theorem Le.none_not_ro_imm : ¬ Le .none .readonly .immutable := fun h =>
  nomatch Le.none_immutable h

/-- **The least type at most `ro` above `t`**: there's always one, and it's
below every other. It's what a parameter `T: ro` should be given for an
argument of type `t`. -/
theorem Ty.least_ro {Δ : Assume} (hΔ : ¬ Le Δ .readonly .immutable) (t : Ty) :
    ∃ j, Ty.lub Δ true t t = some j ∧ Sub Δ t j ∧ AtMost Δ .readonly j ∧
      ∀ u, Sub Δ t u → AtMost Δ .readonly u → Sub Δ j u := by
  obtain ⟨j, hj, -⟩ :=
    Ty.lub_least hΔ t t t.ro true t.agree_refl (Sub.to_ro Δ t) (Sub.to_ro Δ t) (fun _ => AtMost.of_ro Δ t)
  obtain ⟨hs, -, ha⟩ := Ty.lub_sound t t j true hj
  refine ⟨j, hj, hs, ha rfl, fun u hu hau => ?_⟩
  obtain ⟨j', hj', sj'⟩ := Ty.lub_least hΔ t t u true t.agree_refl hu hu (fun _ => hau)
  rw [hj] at hj'
  cases hj'
  exact sj'

/-- Two closures of different mutabilities have no type above both: an
`imm` and a `mut` one can't be given to the same `T`. -/
theorem Sub.no_fn_join {Δ : Assume} {m₁ m₂ : Mu} {a₁ a₂ r₁ r₂ u : Ty} (hm : m₁ ≠ m₂)
    (h₁ : Sub Δ (.fn m₁ a₁ r₁) u) (h₂ : Sub Δ (.fn m₂ a₂ r₂) u) : False := by
  obtain ⟨_, _, rfl, -, -⟩ := h₁.of_fn
  obtain ⟨_, _, he, -, -⟩ := h₂.of_fn
  cases he
  exact hm rfl

/-! ## Examples, with nothing assumed -/

/-- With nothing assumed, `mut` isn't at most `ro`. -/
theorem Le.none_not_mut_ro : ¬ Le .none .mutable .readonly := fun h => by
  rcases h.none_inv with h | h | h <;> cases h

/-- `pass(mut [1])` for `fn pass[T: ro](x: T) -> T`: `T` is `ro [i32]`. -/
example : Ty.lub .none true (.cell .mutable .int) (.cell .mutable .int) =
    some (.cell .readonly .int) := by
  simp [Ty.lub, AtMost, Le.none_not_mut_ro]

/-- `pass([1])`: `T` stays `imm [i32]`, not its view `ro [i32]`. -/
example : Ty.lub .none true (.cell .immutable .int) (.cell .immutable .int) =
    some (.cell .immutable .int) := by
  simp [Ty.lub, AtMost]

/-- Only what may change is seen as `ro`: `pass(mut ([1], mut [2]))` gives
`ro (imm [i32], ro [i32])`, where the view would be
`ro (ro [i32], ro [i32])`. -/
example : Ty.lub .none true
    (.cell .mutable (.prod (.cell .immutable .int) (.cell .mutable .int)))
    (.cell .mutable (.prod (.cell .immutable .int) (.cell .mutable .int))) =
    some (.cell .readonly (.prod (.cell .immutable .int) (.cell .readonly .int))) := by
  simp [Ty.lub, AtMost, Le.none_not_mut_ro]

/-- `both(mut [1], [2])` and `both([1], mut [2])` for
`fn both[T: ro](a: T, b: T) -> T`: `ro [i32]` either way (#448). -/
example : Ty.lub .none true (.cell .immutable .int) (.cell .mutable .int) =
    some (.cell .readonly .int) ∧
    Ty.lub .none true (.cell .mutable .int) (.cell .immutable .int) =
    some (.cell .readonly .int) := by
  simp [Ty.lub]

/-- With no bound, two `mut` arrays of different types have none above
both but a `ro` one: `ro [ro [i32]]` for `mut [mut [i32]]` and
`mut [[i32]]`. -/
example : Ty.lub .none false (.cell .mutable (.cell .mutable .int))
    (.cell .mutable (.cell .immutable .int)) =
    some (.cell .readonly (.cell .readonly .int)) := by
  simp [Ty.lub]

end BoltMut
