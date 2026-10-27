import BoltMut.Lemmas

/-!
# Type safety

Progress and preservation, with a heap that matches its typing. The heap
invariant `HeapOk` is where deep `imm` lives at run time: an `imm` location
holds a value whose type is `imm` all the way down.
-/

namespace BoltMut

/-- The heap matches its typing: each location holds a value of its type,
has `mut` or `imm` as its mutability (never a parameter), and if it's
`imm`, holds a type that's `imm` all the way down. -/
def HeapOk (fns : List Fn) (σ : HeapTy) (h : Heap) : Prop :=
  h.length = σ.length ∧
    ∀ (l : Nat) (m : Mu) (t : Ty), σ[l]? = some (m, t) →
      (m = .mutable ∨ m = .immutable) ∧ (m = .immutable → DeepImm t) ∧
        ∃ v, h[l]? = some v ∧ v.IsValue ∧ HasType fns σ Ctx.empty v t

theorem HeapOk.lt {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {l : Nat} {p : Mu × Ty}
    (hl : σ[l]? = some p) : l < h.length := by
  rw [hh.1]
  exact (List.getElem?_eq_some_iff.mp hl).1

theorem muOf_cases (b : Bool) : muOf b = .mutable ∨ muOf b = .immutable := by
  cases b
  · exact .inr rfl
  · exact .inl rfl

theorem muOf_immutable {b : Bool} (h : muOf b = .immutable) : b = false := by
  cases b
  · rfl
  · cases h

/-- A well-typed program that isn't a value can take a step. -/
theorem progress {fns : List Fn} {σ : HeapTy} {h : Heap} {e : Expr} {t : Ty}
    (hh : HeapOk fns σ h) (ht : HasType fns σ Ctx.empty e t) :
    e.IsValue ∨ ∃ h' e', Step fns h e h' e' := by
  generalize hΓ : Ctx.empty = Γ at ht
  induction ht with
  | var hx =>
    subst hΓ
    cases hx
  | unit => exact .inl .unit
  | int => exact .inl .int
  | loc _ => exact .inl .loc
  | new _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .newVal hv⟩
    · exact .inr ⟨_, _, .newCong hs⟩
  | get he ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨l, rfl, hl⟩ := canonical_cell hv he
      obtain ⟨_, _, v, hv, _⟩ := hh.2 _ _ _ hl
      exact .inr ⟨_, _, .getLoc hv⟩
    · exact .inr ⟨_, _, .getCong hs⟩
  | set he₁ _ ih₁ ih₂ =>
    rcases ih₁ hΓ with hv₁ | ⟨h', e', hs⟩
    · rcases ih₂ hΓ with hv₂ | ⟨h', e', hs⟩
      · obtain ⟨l, rfl, hl⟩ := canonical_cell hv₁ he₁
        exact .inr ⟨_, _, .setLoc (hh.lt hl) hv₂⟩
      · exact .inr ⟨_, _, .setCong₂ hv₁ hs⟩
    · exact .inr ⟨_, _, .setCong₁ hs⟩
  | letE _ _ ih₁ _ =>
    rcases ih₁ hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .letVal hv⟩
    · exact .inr ⟨_, _, .letCong hs⟩
  | call hf _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .callVal hf hv⟩
    · exact .inr ⟨_, _, .callCong hs⟩

/-- A step keeps the program's type, and the heap matching a typing that
only grows. -/
theorem preservation {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ Ctx.empty e t →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ HasType fns σ' Ctx.empty e' t := by
  induction hs with
  | newCong _ ih =>
    intro σ t hh ht
    cases ht with
    | new he hd =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .new he' hd⟩
  | @newVal e b hv =>
    intro σ t hh ht
    cases ht with
    | @new _ _ s _ he hd =>
      refine ⟨σ ++ [(muOf b, s)], Extends.append σ _, ⟨?_, ?_⟩, .loc ?_⟩
      · simp [hh.1]
      · intro l m t hl
        by_cases hlt : l < σ.length
        · rw [List.getElem?_append_left hlt] at hl
          obtain ⟨hm, hd', v, hv', hval, hvt⟩ := hh.2 l m t hl
          refine ⟨hm, hd', v, ?_, hval, hvt.weaken_heap (Extends.append σ _)⟩
          rw [List.getElem?_append_left (hh.1 ▸ hlt)]
          exact hv'
        · have hlen : l = σ.length := by
            have := (List.getElem?_eq_some_iff.mp hl).1
            simp at this
            omega
          subst hlen
          rw [List.getElem?_concat_length] at hl
          cases hl
          refine ⟨muOf_cases b, fun hm => hd (muOf_immutable hm), e, ?_, hv,
            he.weaken_heap (Extends.append σ _)⟩
          rw [← hh.1, List.getElem?_concat_length]
      · rw [hh.1, List.getElem?_concat_length]
  | getCong _ ih =>
    intro σ t hh ht
    cases ht with
    | get he =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .get he'⟩
  | getLoc hl =>
    intro σ t hh ht
    cases ht with
    | get he =>
      cases he with
      | loc hσ =>
        obtain ⟨_, _, v, hv, _, hvt⟩ := hh.2 _ _ _ hσ
        rw [hl] at hv
        cases hv
        exact ⟨σ, Extends.refl σ, hh, hvt⟩
  | setCong₁ _ ih =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .set he₁' (he₂.weaken_heap hx)⟩
  | setCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .set (he₁.weaken_heap hx) he₂'⟩
  | @setLoc l v hl hv =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      cases he₁ with
      | loc hσ =>
        refine ⟨σ, Extends.refl σ, ⟨by simp [hh.1], ?_⟩, .unit⟩
        intro l' m t' hl'
        by_cases heq : l = l'
        · subst heq
          rw [hσ] at hl'
          cases hl'
          refine ⟨.inl rfl, (fun hm => nomatch hm), v, List.getElem?_set_self hl, hv, ?_⟩
          exact he₂.value_any_ctx hv
        · obtain ⟨hm, hd, w, hw, hwv, hwt⟩ := hh.2 l' m t' hl'
          exact ⟨hm, hd, w, by rw [List.getElem?_set_ne heq]; exact hw, hwv, hwt⟩
  | letCong _ ih =>
    intro σ t hh ht
    cases ht with
    | letE he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .letE he₁' (he₂.weaken_heap hx)⟩
  | letVal hv =>
    intro σ t hh ht
    cases ht with
    | letE he₁ he₂ =>
      exact ⟨σ, Extends.refl σ, hh, he₂.subst rfl hv he₁⟩
  | callCong _ ih =>
    intro σ t hh ht
    cases ht with
    | call hf' hl he =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .call hf' hl he'⟩
  | @callVal f fn ms v hfn hv =>
    intro σ t hh ht
    cases ht with
    | call hf' hl he =>
      rw [hfn] at hf'
      cases hf'
      obtain ⟨hn, hbody⟩ := hf fn (List.mem_of_getElem? hfn)
      have hb := (hbody.weaken_heap (Extends.nil σ)).msubst hn ms
      rw [Ctx.msubst_update, Ctx.msubst_empty] at hb
      exact ⟨σ, Extends.refl σ, hh, hb.subst rfl hv he⟩

end BoltMut
