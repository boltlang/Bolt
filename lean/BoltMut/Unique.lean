import BoltMut.Sharing

/-!
# Freezing a unique value in place (#284)

Making an `imm` value from a `mut` one copies it, as `array_freeze` does in
Bolt: `new false (get xs)` is an `imm` object holding what `xs` holds,
which is `imm` all the way down already. The copy is shallow, so only `xs`
itself is copied.

If nothing else has `xs`, and it isn't used again, nobody could tell if the
copy were left out. Uniqueness is tracked separately from typing: the
program that copies is the one that's typed, and the one that uses `xs`
itself is shown to compute the same, as for regions (`Elision.lean`).

`unique_freeze_changes_nothing`: once the copy is made, a program that uses
`xs` in place of the copy, and doesn't use `xs` otherwise, computes what the
one with the copy does, if no object holds `xs`.
-/

namespace BoltMut

/-- The correspondence of using `l` for its copy, at the end of a heap of
`n` locations: each other location to itself, and the copy to `l`. `l`
itself corresponds to nothing, since only the copy is used. -/
def freezeCorr (n l : Nat) : Nat → Nat → Prop := fun a c => (a = c ∧ a < n ∧ a ≠ l) ∨ (a = n ∧ c = l)

/-- **A unique `mut` object can be frozen in place.** With `l` a `mut`
location of `h` that holds `w`, which is `imm` all the way down, and that
no object holds: put the copy, `w`, at the end of the heap, as
`new false (get (loc l))` does. A program `e₁` that has the copy but not
`l` computes what `e₂` does, which is `e₁` with `l` in place of the copy:
with a result that corresponds, and the same integer. Both run with the
copy in the heap; `e₂` just doesn't use it. -/
theorem unique_freeze_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap} {l : Nat}
    {t t₀ : Ty} {w e₁ e₂ v₁ : Expr} (hh : HeapOk fns σ h) (hl : σ[l]? = some (.mutable, t))
    (hd : AtMost .none .immutable t) (hw : h[l]? = some w) (hno : ∀ (a : Nat) (v : Expr), h[a]? = some v → ¬ v.HasLoc l)
    (ht : HasType fns (σ ++ [(.immutable, t)]) .none false Ctx.empty e₁ t₀)
    (hr : Rel (freezeCorr h.length l) e₁ e₂) (hcl : HeapClosed h) (hc₁ : e₁.Closed)
    (hs : Steps fns (h ++ [w]) e₁ h' v₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (v₂ : Expr),
      Steps fns (h ++ [w]) e₂ h₂ v₂ ∧ Rel ρ v₁ v₂ ∧ ∀ n, v₁ = .int n → v₂ = .int n := by
  obtain ⟨_, _, w₀, hw₀, hwv, hwt⟩ := hh.2 _ _ _ hl
  rw [hw] at hw₀
  cases hw₀
  let σ' := σ ++ [(Mu.immutable, t)]
  have hh' : HeapOk fns σ' (h ++ [w]) := hh.alloc (.inr (.inl rfl)) hd hwv hwt
  let M : Nat → Prop := fun a => ∃ m t', σ'[a]? = some (m, t') ∧ m ≠ .immutable
  -- The copy is `imm`.
  have notM : ¬ M h.length := fun ⟨_, _, h', hm⟩ => by
    rw [show σ'[h.length]? = some (.immutable, t) by
      simp only [σ']; rw [hh.1, List.getElem?_concat_length]] at h'
    cases h'
    exact hm rfl
  -- What an old object holds corresponds to itself: it doesn't hold `l`.
  have self : ∀ {a : Nat} {v : Expr}, h[a]? = some v → Rel (freezeCorr h.length l) v v := by
    intro a v hv
    have ha : a < σ.length := hh.1 ▸ (List.getElem?_eq_some_iff.mp hv).1
    obtain ⟨⟨m, s⟩, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem ha⟩
    obtain ⟨_, _, v', hv', _, hvt⟩ := hh.2 a m s hp
    rw [hv] at hv'
    cases hv'
    refine Rel.refl fun l' hl' => .inl ⟨rfl, by rw [hh.1]; exact hvt.hasLoc_lt hl', ?_⟩
    rintro rfl
    exact hno a v hv hl'
  have hc : Corr M (freezeCorr h.length l) (h ++ [w]) (h ++ [w]) := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c hac
      rcases hac with ⟨rfl, ha, _⟩ | ⟨rfl, rfl⟩
      · obtain ⟨v, hv⟩ : ∃ v, h[a]? = some v := ⟨_, List.getElem?_eq_getElem ha⟩
        exact ⟨v, v, by rw [List.getElem?_append_left ha]; exact hv,
          by rw [List.getElem?_append_left ha]; exact hv, self hv⟩
      · exact ⟨w, w, by simp, by rw [List.getElem?_append_left (hh.lt hl)]; exact hw, self hw⟩
    · intro a b c hac hbc hMa
      rcases hac with ⟨rfl, _, hal⟩ | ⟨rfl, _⟩
      · rcases hbc with ⟨rfl, _⟩ | ⟨_, rfl⟩
        · rfl
        · exact (hal rfl).elim
      · exact (notM hMa).elim
    · intro a c d hac had hMa
      rcases hac with ⟨rfl, ha, _⟩ | ⟨rfl, _⟩
      · rcases had with ⟨rfl, _⟩ | ⟨rfl, _⟩
        · rfl
        · omega
      · exact (notM hMa).elim
  obtain ⟨_, ρ, h₂, v₂, ss, _, hr'⟩ := steps_sim hf hs hh' ht
    (fun _ m t' h hm => ⟨m, t', h, hm⟩) hc hr (hcl.append (hcl _ _ hw)) hc₁ rfl
  exact ⟨ρ, h₂, v₂, ss, hr', fun n hn => (hn ▸ hr').int_eq⟩

end BoltMut
