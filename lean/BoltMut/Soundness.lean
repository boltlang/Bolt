import BoltMut.Lemmas

/-!
# Type safety

Progress and preservation, with a heap that matches its typing. The heap
invariant `HeapOk` is where deep `imm` lives at run time: an `imm` location
holds a value whose type is `imm` all the way down.
-/

namespace BoltMut

/-- Whether objects of mutability `m` are `mut` at run time. -/
def Mu.isMut : Mu → Bool
  | .mutable => true
  | _ => false

@[simp] theorem Mu.isMut_muOf (b : Bool) : (muOf b).isMut = b := by cases b <;> rfl

/-- The heap matches its typing: each location holds a value of its type,
has `mut` or `imm` as its mutability (never a parameter), which its object
knows, and holds a type that's at most its mutability: if it's `imm`, one
that's `imm` all the way down. -/
def HeapOk (fns : List Fn) (σ : HeapTy) (h : Heap) : Prop :=
  h.length = σ.length ∧
    ∀ (l : Nat) (m : Mu) (t : Ty), σ[l]? = some (m, t) →
      (m = .mutable ∨ m = .immutable) ∧ AtMost .none m t ∧
        ∃ v, h[l]? = some (m.isMut, v) ∧ v.IsValue ∧ HasType fns σ .none false Ctx.empty v t

theorem HeapOk.lt {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {l : Nat} {p : Mu × Ty}
    (hl : σ[l]? = some p) : l < h.length := by
  rw [hh.1]
  exact (List.getElem?_eq_some_iff.mp hl).1

/-- Adding a location that holds a value of its type keeps the heap
matching its typing. -/
theorem HeapOk.alloc {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {b : Bool} {s : Ty}
    {v : Expr} (hd : AtMost .none (muOf b) s)
    (hv : v.IsValue) (hvt : HasType fns σ .none false Ctx.empty v s) :
    HeapOk fns (σ ++ [(muOf b, s)]) (h ++ [(b, v)]) := by
  refine ⟨by simp [hh.1], ?_⟩
  intro l m' t hl
  by_cases hlt : l < σ.length
  · rw [List.getElem?_append_left hlt] at hl
    obtain ⟨hm', hd', w, hw, hval, hwt⟩ := hh.2 l m' t hl
    refine ⟨hm', hd', w, ?_, hval, hwt.weaken_heap (Extends.append σ _)⟩
    rw [List.getElem?_append_left (hh.1 ▸ hlt)]
    exact hw
  · have hlen : l = σ.length := by
      have := (List.getElem?_eq_some_iff.mp hl).1
      simp at this
      omega
    subst hlen
    rw [List.getElem?_concat_length] at hl
    cases hl
    refine ⟨muOf_cases b, hd, v, ?_, hv, hvt.weaken_heap (Extends.append σ _)⟩
    rw [← hh.1, List.getElem?_concat_length, Mu.isMut_muOf]

/-- Writing a value of its type to a location keeps the heap matching its
typing. -/
theorem HeapOk.write {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {l : Nat} {m : Mu} {t : Ty}
    {b : Bool} {w v : Expr} (hσ : σ[l]? = some (m, t)) (hl : h[l]? = some (b, w)) (hv : v.IsValue)
    (hvt : HasType fns σ .none false Ctx.empty v t) : HeapOk fns σ (h.set l (b, v)) := by
  refine ⟨by simp [hh.1], ?_⟩
  have hlt : l < h.length := (List.getElem?_eq_some_iff.mp hl).1
  intro l' m' t' hl'
  by_cases heq : l = l'
  · subst heq
    rw [hσ] at hl'
    cases hl'
    obtain ⟨hm, hd, w₀, hw₀, _⟩ := hh.2 _ _ _ hσ
    rw [hl] at hw₀
    cases hw₀
    exact ⟨hm, hd, v, List.getElem?_set_self hlt, hv, hvt⟩
  · obtain ⟨hm, hd, w, hw, hwv, hwt⟩ := hh.2 l' m' t' hl'
    exact ⟨hm, hd, w, by rw [List.getElem?_set_ne heq]; exact hw, hwv, hwt⟩

/-! ## Freezing (#335)

At run time nothing is assumed, so the type of `imm e` is a function of
`e`'s: `Ty.imm`, for a type that can be copied (`Ty.Freezable`). -/

/-- Each cell `imm`, with what it holds done the same way. A `ro T` is done
as the `T` it views: neither can be copied (`Ty.Freezable`). -/
def Ty.imm : Ty → Ty
  | .cell _ t => .cell .immutable t.imm
  | .prod a b => .prod a.imm b.imm
  | .roParam i => .tparam i
  | t => t

/-- What `imm e` can copy, with nothing assumed: each closure in it is
`imm`, and there's no type parameter. -/
def Ty.Freezable : Ty → Prop
  | .unit | .int => True
  | .cell _ t => t.Freezable
  | .fn m _ _ => Le .none m .immutable
  | .prod a b => a.Freezable ∧ b.Freezable
  | .tparam _ | .roParam _ => False

/-- What's `imm` all the way down stays as it is, and can be copied. -/
theorem AtMost.imm_eq {t : Ty} (h : AtMost .none .immutable t) : t.imm = t ∧ t.Freezable := by
  induction t with
  | unit | int => exact ⟨rfl, trivial⟩
  | cell m t ih =>
    have hm := Le.none_immutable h.1
    subst hm
    obtain ⟨h₁, h₂⟩ := ih h.2
    exact ⟨by simp [Ty.imm, h₁], h₂⟩
  | fn m a r => exact ⟨rfl, h.none_fn⟩
  | prod a b iha ihb =>
    obtain ⟨h₁, h₂⟩ := iha h.1
    obtain ⟨h₃, h₄⟩ := ihb h.2
    exact ⟨by simp [Ty.imm, h₁, h₃], h₂, h₄⟩
  | tparam i =>
    obtain ⟨μ, hμ, hle⟩ := h
    rcases hμ with rfl | hμ
    · exact nomatch Le.none_immutable hle
    · simp [Assume.none] at hμ
  | roParam i => exact nomatch Le.none_immutable h

/-- With nothing assumed, `ImmTy` is `Ty.imm`. -/
theorem ImmTy.none_eq {t t' : Ty} (h : ImmTy .none t t') : t' = t.imm ∧ t.Freezable := by
  induction h with
  | asIs h => exact ⟨(h.imm_eq).1.symm, (h.imm_eq).2⟩
  | cell _ ih => exact ⟨by simp [Ty.imm, ih.1], ih.2⟩
  | prod _ _ iha ihb => exact ⟨by simp [Ty.imm, iha.1, ihb.1], iha.2, ihb.2⟩

/-- A copy is `imm` all the way down. -/
theorem Ty.Freezable.atMost {t : Ty} (h : t.Freezable) : AtMost .none .immutable t.imm := by
  induction t with
  | unit | int => trivial
  | cell m t ih => exact ⟨.refl _, ih h⟩
  | fn m a r => exact .inl h
  | prod a b iha ihb => exact ⟨iha h.1, ihb h.2⟩
  | tparam | roParam => exact h.elim

/-- Seeing objects as `ro` changes neither what's copied nor the copy. -/
theorem Sub.imm {Δ : Assume} {s t : Ty} (h : Sub Δ s t) : s.imm = t.imm ∧ (t.Freezable → s.Freezable) := by
  induction h with
  | refl => exact ⟨rfl, id⟩
  | ro _ _ ih => exact ⟨by simp [Ty.imm, ih.1], ih.2⟩
  | prod _ _ iha ihb => exact ⟨by simp [Ty.imm, iha.1, ihb.1], fun h => ⟨iha.2 h.1, ihb.2 h.2⟩⟩
  | tparam => exact ⟨rfl, fun h => h.elim⟩

/-- A copy only adds to the heap. -/
theorem Freeze.prefix {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') : ∃ ext, h' = h ++ ext := by
  induction hc with
  | unit | int | lam | imm | seen => exact ⟨[], by simp⟩
  | @cell _ _ _ _ _ _ w' _ _ _ ih =>
    obtain ⟨ext, rfl⟩ := ih
    exact ⟨ext ++ [(false, w')], by simp⟩
  | pair _ _ ih₁ ih₂ =>
    obtain ⟨e₁, rfl⟩ := ih₁
    obtain ⟨e₂, rfl⟩ := ih₂
    exact ⟨e₁ ++ e₂, by simp⟩

/-- So what was in the heap stays as it was. -/
theorem Freeze.keeps {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') {l : Nat} (hl : l < h.length) : h'[l]? = h[l]? := by
  obtain ⟨ext, rfl⟩ := hc.prefix
  exact List.getElem?_append_left hl

/-- What a copy already copied, it copied to a location of the copy's
type. -/
def MemoOk (σ : HeapTy) (m : List (Nat × Nat)) : Prop :=
  ∀ l c, m.lookup l = some c → ∃ s, σ[l]? = some (.mutable, s) ∧ σ[c]? = some (.immutable, s.imm)

theorem MemoOk.nil (σ : HeapTy) : MemoOk σ [] := fun _ _ h => by simp at h

theorem MemoOk.extend {σ σ' : HeapTy} {m : List (Nat × Nat)} (hm : MemoOk σ m) (hx : Extends σ σ') :
    MemoOk σ' m := by
  intro l c hl
  obtain ⟨s, h₁, h₂⟩ := hm l c hl
  exact ⟨s, hx _ _ h₁, hx _ _ h₂⟩

/-- The type of a location, through a `ro` view or not, gives the copy's. -/
theorem loc_imm {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {l : Nat} {t : Ty}
    (ht : HasType fns σ .none io Γ (.loc l) t) :
    ∃ m s, σ[l]? = some (m, s) ∧ t.imm = Ty.imm (.cell m s) ∧ (t.Freezable → s.Freezable) := by
  obtain ⟨_, _, h, _, hs⟩ := ht.loc_inv
  exact ⟨_, _, h, hs.imm.1.symm, hs.imm.2⟩

/-- **A copy is well-typed.** It has the value's type with each cell
`imm`, in a heap typing that only grows. -/
theorem Freeze.typed {fns : List Fn} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') :
    ∀ {σ : HeapTy} {io : Bool} {Γ : Ctx} {t : Ty}, HeapOk fns σ h → v.IsValue → HasType fns σ .none io Γ v t →
      t.Freezable → MemoOk σ m →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ v'.IsValue ∧ HasType fns σ' .none io Γ v' t.imm ∧
        MemoOk σ' m' := by
  induction hc with
  | unit =>
    intro σ io Γ t hh _ ht _ hmo
    cases ht.unit_inv
    exact ⟨σ, Extends.refl σ, hh, .unit, .unit, hmo⟩
  | int =>
    intro σ io Γ t hh _ ht _ hmo
    cases ht.int_inv
    exact ⟨σ, Extends.refl σ, hh, .int, .int, hmo⟩
  | lam =>
    intro σ io Γ t hh _ ht _ hmo
    -- A closure isn't copied: its type is a function type, which `imm`
    -- leaves as it is.
    obtain ⟨_, _, _, _, _, _, hs⟩ := ht.lam_inv
    obtain ⟨_, _, rfl⟩ := hs.of_fn
    exact ⟨σ, Extends.refl σ, hh, .lam, ht, hmo⟩
  | @imm h m l w hw =>
    intro σ io Γ t hh _ ht _ hmo
    obtain ⟨μ, s, hσ, himm, _⟩ := loc_imm ht
    obtain ⟨hμ, hd, w₀, hw₀, _⟩ := hh.2 _ _ _ hσ
    rw [hw] at hw₀
    -- The object is `imm`, so its type is `imm` all the way down.
    have hμ' : μ = .immutable := by
      rcases hμ with rfl | rfl
      · cases hw₀
      · rfl
    subst hμ'
    have heq : t.imm = .cell .immutable s := by
      rw [himm]; simp [Ty.imm, (hd.imm_eq).1]
    refine ⟨σ, Extends.refl σ, hh, .loc, ?_, hmo⟩
    rw [heq]
    exact .loc hσ (fun _ => rfl)
  | @seen h m l c w hw hlk =>
    intro σ io Γ t hh _ ht _ hmo
    obtain ⟨μ, s, hσ, himm, _⟩ := loc_imm ht
    obtain ⟨s', hl, hc⟩ := hmo l c hlk
    rw [hσ] at hl
    cases hl
    refine ⟨σ, Extends.refl σ, hh, .loc, ?_, hmo⟩
    rw [himm]
    exact .loc hc (fun _ => rfl)
  | @cell h h' m m' l w w' hw hlk _ ih =>
    intro σ io Γ t hh _ ht hf hmo
    obtain ⟨μ, s, hσ, himm, hfs⟩ := loc_imm ht
    obtain ⟨hμ, _, w₀, hw₀, hwv, hwt⟩ := hh.2 _ _ _ hσ
    rw [hw] at hw₀
    have hμ' : μ = .mutable := by
      rcases hμ with rfl | rfl
      · rfl
      · cases hw₀
    subst hμ'
    cases hw₀
    have hfs' := hfs hf
    obtain ⟨σ₁, hx, hh₁, hv', ht', hmo₁⟩ := ih hh hwv hwt hfs' hmo
    refine ⟨σ₁ ++ [(muOf false, s.imm)], hx.trans (Extends.append _ _),
      hh₁.alloc hfs'.atMost hv' ht', .loc, ?_, ?_⟩
    · rw [himm]
      exact .loc (by rw [hh₁.1, List.getElem?_concat_length]; rfl) (fun _ => rfl)
    · intro k c hk
      rw [List.lookup_cons] at hk
      split at hk
      · rename_i heq
        cases hk
        have hkl : k = l := by simpa using heq
        subst hkl
        refine ⟨s, hx.trans (Extends.append _ _) _ _ hσ, ?_⟩
        rw [hh₁.1, List.getElem?_concat_length]
        rfl
      · exact (hmo₁.extend (Extends.append _ _)) k c hk
  | pair _ _ ih₁ ih₂ =>
    intro σ io Γ t hh hv ht hf hmo
    cases hv with
    | pair hv₁ hv₂ =>
      -- Copied at the types its parts have: seeing them as `ro` changes
      -- neither what's copied nor the copy.
      obtain ⟨a, b, ht₁, ht₂, hs⟩ := ht.pair_inv
      obtain ⟨heq, hfs⟩ := hs.imm
      have hf' := hfs hf
      obtain ⟨σ₁, hx₁, hh₁, hv₁', ht₁', hmo₁⟩ := ih₁ hh hv₁ ht₁ hf'.1 hmo
      obtain ⟨σ₂, hx₂, hh₂, hv₂', ht₂', hmo₂⟩ := ih₂ hh₁ hv₂ (ht₂.weaken_heap hx₁) hf'.2 hmo₁
      refine ⟨σ₂, hx₁.trans hx₂, hh₂, .pair hv₁' hv₂', ?_, hmo₂⟩
      rw [← heq]
      exact .pair (ht₁'.weaken_heap hx₂) ht₂'

/-- A well-typed value can always be copied, also when it's seen through
`ro`. -/
theorem Freeze.exists {fns : List Fn} (t : Ty) :
    ∀ {σ : HeapTy} {h : Heap} {m : List (Nat × Nat)} {io : Bool} {Γ : Ctx} {v : Expr} {t₀ : Ty}, HeapOk fns σ h →
      v.IsValue → HasType fns σ .none io Γ v t₀ → Sub .none t₀ t → t.Freezable → MemoOk σ m →
      ∃ h' m' v', Freeze h m v h' m' v' := by
  induction t with
  | cell μ s ih =>
    intro σ h m io Γ v t₀ hh hv ht hs₀ hf hmo
    obtain ⟨l, m₀, s₀, rfl, hσ, hsub⟩ := canonical_cell_sub hv (ht.value_sub hv hs₀)
    obtain ⟨hμ, _, w, hw, hwv, hwt⟩ := hh.2 _ _ _ hσ
    rcases hμ with rfl | rfl
    · cases hlk : m.lookup l with
      | some c => exact ⟨_, _, _, .seen hw hlk⟩
      | none =>
        -- What it holds has a type that `s` is, seen through `ro` or not.
        have hs : Sub .none s₀ s := by
          rcases hsub.cell_inv with h | ⟨s', h, hs, _⟩
          · cases h; exact .refl _
          · cases h; exact hs
        obtain ⟨h', m', w', hc⟩ := ih hh hwv hwt hs hf hmo
        exact ⟨_, _, _, .cell hw hlk hc⟩
    · exact ⟨_, _, _, .imm hw⟩
  | prod a b iha ihb =>
    intro σ h m io Γ v t₀ hh hv ht hs₀ hf hmo
    obtain ⟨a₀, b₀, rfl, ha, hb⟩ := hs₀.to_prod_inv
    obtain ⟨v₁, v₂, rfl, hv₁, hv₂⟩ := canonical_prod hv ht
    obtain ⟨a₁, b₁, ht₁, ht₂, hs₁⟩ := ht.pair_inv
    obtain ⟨_, _, heq, ha₁, hb₁⟩ := hs₁.to_prod_inv
    cases heq
    have ha' := ha₁.trans ha
    have hb' := hb₁.trans hb
    obtain ⟨h₁, m₁, v₁', hc₁⟩ := iha hh hv₁ ht₁ ha' hf.1 hmo
    obtain ⟨σ₁, hx₁, hh₁, _, _, hmo₁⟩ := hc₁.typed hh hv₁ ht₁ (ha'.imm.2 hf.1) hmo
    obtain ⟨h₂, m₂, v₂', hc₂⟩ := ihb hh₁ hv₂ (ht₂.weaken_heap hx₁) hb' hf.2 hmo₁
    exact ⟨_, _, _, .pair hc₁ hc₂⟩
  | unit | int =>
    intro σ h m io Γ v t₀ _ hv ht hs₀ _ _
    have ht' := ht.value_sub hv hs₀
    cases hv with
    | unit => exact ⟨_, _, _, .unit⟩
    | int => exact ⟨_, _, _, .int⟩
    | loc =>
      obtain ⟨_, _, _, _, hs⟩ := ht'.loc_inv
      rcases hs.cell_inv with h | ⟨_, h, _⟩ <;> cases h
    | lam =>
      obtain ⟨_, _, _, _, _, _, hs⟩ := ht'.lam_inv
      obtain ⟨_, _, h⟩ := hs.of_fn
      cases h
    | pair =>
      obtain ⟨_, _, _, _, hs⟩ := ht'.pair_inv
      obtain ⟨_, _, h, _, _⟩ := hs.prod_inv
      cases h
  | fn μ a r =>
    intro σ h m io Γ v t₀ _ hv ht hs₀ _ _
    obtain ⟨b, x, body, rfl⟩ := canonical_fn hv (ht.value_sub hv hs₀)
    exact ⟨_, _, _, .lam⟩
  | tparam | roParam => intro _ _ _ _ _ _ _ _ _ _ _ hf _; exact hf.elim

/-- A well-typed program that isn't a value can take a step. -/
theorem progress {fns : List Fn} {σ : HeapTy} {h : Heap} {io : Bool} {e : Expr} {t : Ty}
    (hh : HeapOk fns σ h) (ht : HasType fns σ .none io Ctx.empty e t) :
    e.IsValue ∨ ∃ h' e', Step fns h e h' e' := by
  generalize hΓ : Ctx.empty = Γ at ht
  induction ht with
  | var hx | varSub hx _ =>
    subst hΓ
    cases hx
  | unit => exact .inl .unit
  | int => exact .inl .int
  | loc _ _ | locSub _ _ _ => exact .inl .loc
  | lam _ => exact .inl .lam
  | sub _ _ ih => exact ih hΓ
  | app he₁ _ ih₁ ih₂ =>
    rcases ih₁ hΓ with hv₁ | ⟨h', e', hs⟩
    · rcases ih₂ hΓ with hv₂ | ⟨h', e', hs⟩
      · obtain ⟨b, x, body, rfl⟩ := canonical_fn hv₁ he₁
        exact .inr ⟨_, _, .appVal hv₂⟩
      · exact .inr ⟨_, _, .appCong₂ hv₁ hs⟩
    · exact .inr ⟨_, _, .appCong₁ hs⟩
  | pair _ _ ih₁ ih₂ =>
    rcases ih₁ hΓ with hv₁ | ⟨h', e', hs⟩
    · rcases ih₂ hΓ with hv₂ | ⟨h', e', hs⟩
      · exact .inl (.pair hv₁ hv₂)
      · exact .inr ⟨_, _, .pairCong₂ hv₁ hs⟩
    · exact .inr ⟨_, _, .pairCong₁ hs⟩
  | fst he ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨v₁, v₂, rfl, h₁, h₂⟩ := canonical_prod hv he
      exact .inr ⟨_, _, .fstVal h₁ h₂⟩
    · exact .inr ⟨_, _, .fstCong hs⟩
  | snd he ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨v₁, v₂, rfl, h₁, h₂⟩ := canonical_prod hv he
      exact .inr ⟨_, _, .sndVal h₁ h₂⟩
    · exact .inr ⟨_, _, .sndCong hs⟩
  | new _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .newVal hv⟩
    · exact .inr ⟨_, _, .newCong hs⟩
  | get he ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨l, _, _, rfl, hl, _⟩ := canonical_cell_sub hv he
      obtain ⟨_, _, v, hv, _⟩ := hh.2 _ _ _ hl
      exact .inr ⟨_, _, .getLoc hv⟩
    · exact .inr ⟨_, _, .getCong hs⟩
  | set he₁ _ ih₁ ih₂ =>
    rcases ih₁ hΓ with hv₁ | ⟨h', e', hs⟩
    · rcases ih₂ hΓ with hv₂ | ⟨h', e', hs⟩
      · obtain ⟨l, rfl, hl⟩ := canonical_cell hv₁ he₁ (by simp)
        obtain ⟨_, _, w, hw, _⟩ := hh.2 _ _ _ hl
        exact .inr ⟨_, _, .setLoc hw hv₂⟩
      · exact .inr ⟨_, _, .setCong₂ hv₁ hs⟩
    · exact .inr ⟨_, _, .setCong₁ hs⟩
  | letE _ _ ih₁ _ =>
    rcases ih₁ hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .letVal hv⟩
    · exact .inr ⟨_, _, .letCong hs⟩
  | toImm he hi ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨h', m', v', hc⟩ := Freeze.exists _ hh hv he (.refl _) (hi.none_eq).2 (MemoOk.nil σ)
      exact .inr ⟨_, _, .toImmVal hv hc⟩
    · exact .inr ⟨_, _, .toImmCong hs⟩
  | call hf _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .callVal hf hv⟩
    · exact .inr ⟨_, _, .callCong hs⟩

/-- Preservation: a step keeps the heap matching a typing that only grew,
and the program's type. Each case takes the typing apart up to `sub`
(`HasType.app_inv` and the others), and makes what it gives a subtype of
the program's type again. -/
theorem preservation {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ HasType fns σ' .none false Ctx.empty e' t := by
  induction hs with
  | newCong _ ih =>
    intro σ t hh ht
    obtain ⟨s, he, hd, hs⟩ := ht.new_inv
    obtain ⟨σ', hx, hh', he'⟩ := ih hh he
    exact ⟨σ', hx, hh', .sub (.new he' hd) hs⟩
  | @newVal e b hv =>
    intro σ t hh ht
    obtain ⟨s, he, hd, hs⟩ := ht.new_inv
    refine ⟨σ ++ [(muOf b, s)], Extends.append σ _, hh.alloc hd hv he, .sub (.loc ?_ (fun h => nomatch h)) hs⟩
    rw [hh.1, List.getElem?_concat_length]
  | getCong _ ih =>
    intro σ t hh ht
    obtain ⟨m, s, he, hs⟩ := ht.get_inv
    obtain ⟨σ', hx, hh', he'⟩ := ih hh he
    exact ⟨σ', hx, hh', .sub (.get he') hs⟩
  | getLoc hl =>
    intro σ t hh ht
    obtain ⟨m, s, he, hs⟩ := ht.get_inv
    obtain ⟨m₀, s₀, hσ, _, hsub⟩ := he.loc_inv
    obtain ⟨_, _, v, hv, hvv, hvt⟩ := hh.2 _ _ _ hσ
    rw [hl] at hv
    cases hv
    -- Read through a `ro` view, the value is seen as `ro` too.
    have hs₀ : Sub .none s₀ s := by
      rcases hsub.cell_inv with h | ⟨s', h, hs', _⟩
      · cases h; exact .refl _
      · cases h; exact hs'
    exact ⟨σ, Extends.refl σ, hh, .sub (.sub hvt hs₀) hs⟩
  | setCong₁ _ ih =>
    intro σ t hh ht
    obtain ⟨s, he₁, he₂, rfl⟩ := ht.set_inv
    obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
    exact ⟨σ', hx, hh', .set he₁' (he₂.weaken_heap hx)⟩
  | setCong₂ _ _ ih =>
    intro σ t hh ht
    obtain ⟨s, he₁, he₂, rfl⟩ := ht.set_inv
    obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
    exact ⟨σ', hx, hh', .set (he₁.weaken_heap hx) he₂'⟩
  | @setLoc l b w v hl hv =>
    intro σ t hh ht
    obtain ⟨s, he₁, he₂, rfl⟩ := ht.set_inv
    obtain ⟨_, hl', hσ⟩ := canonical_cell .loc he₁ (by simp)
    cases hl'
    exact ⟨σ, Extends.refl σ, hh.write hσ hl hv he₂, .unit⟩
  | letCong _ ih =>
    intro σ t hh ht
    obtain ⟨t₁, t₂, he₁, he₂, hs⟩ := ht.letE_inv
    obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
    exact ⟨σ', hx, hh', .sub (.letE he₁' (he₂.weaken_heap hx)) hs⟩
  | letVal hv =>
    intro σ t hh ht
    obtain ⟨t₁, t₂, he₁, he₂, hs⟩ := ht.letE_inv
    exact ⟨σ, Extends.refl σ, hh, .sub (he₂.subst_update hv he₁) hs⟩
  | callCong _ ih =>
    intro σ t hh ht
    obtain ⟨fn, hf', hl, he, hs⟩ := ht.call_inv
    obtain ⟨σ', hx, hh', he'⟩ := ih hh he
    exact ⟨σ', hx, hh', .sub (.call hf' hl he') hs⟩
  | @callVal f fn ms τs v hfn hv =>
    intro σ t hh ht
    obtain ⟨fn', hf', hsat, he, hs⟩ := ht.call_inv
    rw [hfn] at hf'
    cases hf'
    obtain ⟨hn, hbody⟩ := hf fn (List.mem_of_getElem? hfn)
    -- The arguments meet what the function assumes, so its body has the
    -- type with them.
    have hb := (hbody.weaken_heap (Extends.nil σ)).inst hn hsat
    rw [Ctx.inst_update, Ctx.inst_empty] at hb
    exact ⟨σ, Extends.refl σ, hh, .sub (hb.subst_update hv he) hs⟩
  | appCong₁ _ ih =>
    intro σ t hh ht
    obtain ⟨m, a, r, he₁, he₂, hr⟩ := ht.app_inv
    obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
    exact ⟨σ', hx, hh', .sub (.app he₁' (he₂.weaken_heap hx)) hr⟩
  | appCong₂ _ _ ih =>
    intro σ t hh ht
    obtain ⟨m, a, r, he₁, he₂, hr⟩ := ht.app_inv
    obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
    exact ⟨σ', hx, hh', .sub (.app (he₁.weaken_heap hx) he₂') hr⟩
  | appVal hv =>
    intro σ t hh ht
    obtain ⟨m, a, r, he₁, he₂, hr⟩ := ht.app_inv
    obtain ⟨Γc, a₀, r₀, hc, _, hbody, hs⟩ := he₁.lam_inv
    obtain ⟨_, rfl, rfl⟩ := hs.fn_inv
    -- The body of an `imm` closure is typed inside one; outside, it has
    -- the same type. A closed closure saw nothing but its parameter.
    have hb := (hbody.weaken_ctx (hc.update _ _)).weaken_imm (io' := false) (fun h => nomatch h)
    exact ⟨σ, Extends.refl σ, hh, .sub (hb.subst_update hv he₂) hr⟩
  | pairCong₁ _ ih =>
    intro σ t hh ht
    obtain ⟨a, b, he₁, he₂, hs⟩ := ht.pair_inv
    obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
    exact ⟨σ', hx, hh', .sub (.pair he₁' (he₂.weaken_heap hx)) hs⟩
  | pairCong₂ _ _ ih =>
    intro σ t hh ht
    obtain ⟨a, b, he₁, he₂, hs⟩ := ht.pair_inv
    obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
    exact ⟨σ', hx, hh', .sub (.pair (he₁.weaken_heap hx) he₂') hs⟩
  | fstCong _ ih =>
    intro σ t hh ht
    obtain ⟨a, b, he, hs⟩ := ht.fst_inv
    obtain ⟨σ', hx, hh', he'⟩ := ih hh he
    exact ⟨σ', hx, hh', .sub (.fst he') hs⟩
  | sndCong _ ih =>
    intro σ t hh ht
    obtain ⟨a, b, he, hs⟩ := ht.snd_inv
    obtain ⟨σ', hx, hh', he'⟩ := ih hh he
    exact ⟨σ', hx, hh', .sub (.snd he') hs⟩
  | fstVal _ _ =>
    intro σ t hh ht
    obtain ⟨a, b, he, hs⟩ := ht.fst_inv
    obtain ⟨a₁, b₁, h₁, _, hs₁⟩ := he.pair_inv
    obtain ⟨_, _, heq, ha, _⟩ := hs₁.to_prod_inv
    cases heq
    exact ⟨σ, Extends.refl σ, hh, .sub (.sub h₁ ha) hs⟩
  | sndVal _ _ =>
    intro σ t hh ht
    obtain ⟨a, b, he, hs⟩ := ht.snd_inv
    obtain ⟨a₁, b₁, _, h₂, hs₁⟩ := he.pair_inv
    obtain ⟨_, _, heq, _, hb⟩ := hs₁.to_prod_inv
    cases heq
    exact ⟨σ, Extends.refl σ, hh, .sub (.sub h₂ hb) hs⟩
  | toImmCong _ ih =>
    intro σ t hh ht
    obtain ⟨rfl, s, s', he, hd, hs⟩ := ht.toImm_inv
    obtain ⟨σ', hx, hh', he'⟩ := ih hh he
    exact ⟨σ', hx, hh', .sub (.toImm he' hd) hs⟩
  | toImmVal hv hc =>
    intro σ T hh ht
    obtain ⟨_, s, s', he, hi, hs⟩ := ht.toImm_inv
    -- The copy has the type with each cell `imm`, in a heap typing that
    -- only grew.
    obtain ⟨heq, hf⟩ := hi.none_eq
    subst heq
    obtain ⟨σ', hx, hh', _, ht', _⟩ := hc.typed hh hv he hf (MemoOk.nil σ)
    exact ⟨σ', hx, hh', .sub ht' hs⟩
  | toImmKeep _ _ => intro σ t hh ht; exact nomatch ht.toImm_inv.1

end BoltMut
