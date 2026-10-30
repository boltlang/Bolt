import BoltMut.Lemmas

/-!
# Type safety

Progress and preservation, with a heap that matches its typing. The heap
invariant `HeapOk` is where deep `imm` lives at run time: an `imm` location
holds a value whose type is `imm` all the way down.
-/

namespace BoltMut

/-- The heap matches its typing: each location holds a value of its type,
has `mut`, `imm` or a region as its mutability (never a parameter), and holds a type
that's at most its mutability: if it's `imm`, one that's `imm` all the way
down. -/
def HeapOk (fns : List Fn) (σ : HeapTy) (h : Heap) : Prop :=
  h.length = σ.length ∧
    ∀ (l : Nat) (m : Mu) (t : Ty), σ[l]? = some (m, t) →
      (m = .mutable ∨ m = .immutable ∨ ∃ r, m = .region r) ∧ AtMost .none m t ∧
        ∃ v, h[l]? = some v ∧ v.IsValue ∧ HasType fns σ .none false Ctx.empty v t

theorem HeapOk.lt {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {l : Nat} {p : Mu × Ty}
    (hl : σ[l]? = some p) : l < h.length := by
  rw [hh.1]
  exact (List.getElem?_eq_some_iff.mp hl).1

/-- Adding a location that holds a value of its type keeps the heap
matching its typing. -/
theorem HeapOk.alloc {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {m : Mu} {s : Ty} {v : Expr}
    (hm : m = .mutable ∨ m = .immutable ∨ ∃ r, m = .region r) (hd : AtMost .none m s)
    (hv : v.IsValue) (hvt : HasType fns σ .none false Ctx.empty v s) :
    HeapOk fns (σ ++ [(m, s)]) (h ++ [v]) := by
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
    refine ⟨hm, hd, v, ?_, hv, hvt.weaken_heap (Extends.append σ _)⟩
    rw [← hh.1, List.getElem?_concat_length]

/-- Writing a value of its type to a location keeps the heap matching its
typing. -/
theorem HeapOk.write {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {l : Nat} {m : Mu} {t : Ty}
    {v : Expr} (hσ : σ[l]? = some (m, t)) (hl : l < h.length) (hv : v.IsValue)
    (hvt : HasType fns σ .none false Ctx.empty v t) : HeapOk fns σ (h.set l v) := by
  refine ⟨by simp [hh.1], ?_⟩
  intro l' m' t' hl'
  by_cases heq : l = l'
  · subst heq
    rw [hσ] at hl'
    cases hl'
    obtain ⟨hm, hd, _⟩ := hh.2 _ _ _ hσ
    exact ⟨hm, hd, v, List.getElem?_set_self hl, hv, hvt⟩
  · obtain ⟨hm, hd, w, hw, hwv, hwt⟩ := hh.2 l' m' t' hl'
    exact ⟨hm, hd, w, by rw [List.getElem?_set_ne heq]; exact hw, hwv, hwt⟩

/-! ## Allocation order (#292)

A location's type only reaches regions made before it: what a cell holds,
and a closure's own mutability, mention only older regions. A closure's
parameter and result don't matter, since it can't keep what it's given
but through what it captured (rule 5). So what's older than a block can't
reach what it makes. -/

/-- A value of type `t` can reach an object of the region `r`: a cell or a
closure in it has the region as its mutability, or a cell holds one. -/
def Ty.reaches (r : Nat) : Ty → Bool
  | .unit | .int | .tparam _ => false
  | .cell m t => m.mentions r || t.reaches r
  | .fn m _ _ => m.mentions r
  | .prod a b => a.reaches r || b.reaches r

/-- What a value of type `t` can reach is of regions that exist. -/
def Ty.ReachBelow (n : Nat) (t : Ty) : Prop := ∀ r, t.reaches r = true → Made n r

/-- **Allocation order.** Each location's type reaches only regions below
it, which existed when it was made. -/
def AllocOk (σ : HeapTy) : Prop := ∀ (l : Nat) (m : Mu) (t : Ty), σ[l]? = some (m, t) → (Ty.cell m t).ReachBelow l

theorem Ty.ReachBelow.mono {n n' : Nat} {t : Ty} (h : t.ReachBelow n) (hn : n ≤ n') : t.ReachBelow n' :=
  fun r hr => (h r hr).mono hn

/-- Freeing a region to what mentions none reaches no more. -/
theorem Ty.reaches_free {t : Ty} {r r' : Nat} (h : (t.free r .immutable).reaches r' = true) : t.reaches r' = true := by
  induction t with
  | unit | int | tparam => cases h
  | cell m t ih =>
    simp only [Ty.free, Ty.reaches, Bool.or_eq_true] at h ⊢
    rcases h with h | h
    · left
      cases m with
      | region r'' =>
        simp only [Mu.free] at h
        split at h
        · cases h
        · exact h
      | _ => exact h
    · exact .inr (ih h)
  | fn m a b =>
    simp only [Ty.free, Ty.reaches] at h ⊢
    cases m with
    | region r'' =>
      simp only [Mu.free] at h
      split at h
      · cases h
      · exact h
    | _ => exact h
  | prod a b iha ihb =>
    simp only [Ty.free, Ty.reaches, Bool.or_eq_true] at h ⊢
    exact h.imp iha ihb

/-- A value's type only reaches regions that exist. -/
theorem Expr.IsValue.reachBelow {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {v : Expr} {s : Ty}
    (hv : v.IsValue) (ha : AllocOk σ) (ht : HasType fns σ Δ io Γ v s) : s.ReachBelow σ.length := by
  induction hv generalizing s with
  | unit => cases ht; intro r h; cases h
  | int => cases ht; intro r h; cases h
  | loc => cases ht with
    | loc h _ => exact (ha _ _ _ h).mono (Nat.le_of_lt (List.getElem?_eq_some_iff.mp h).1)
  | lam => cases ht with
    | lam _ _ _ _ _ _ hw => exact fun r h => hw r h
  | pair _ _ ih₁ ih₂ => cases ht with
    | pair h₁ h₂ =>
      intro r h
      simp only [Ty.reaches, Bool.or_eq_true] at h
      exact h.elim (ih₁ h₁ r) (ih₂ h₂ r)

theorem AllocOk.append {σ : HeapTy} (ha : AllocOk σ) {m : Mu} {t : Ty} (hn : (Ty.cell m t).ReachBelow σ.length) :
    AllocOk (σ ++ [(m, t)]) := by
  intro l m' t' hl
  by_cases hlt : l < σ.length
  · rw [List.getElem?_append_left hlt] at hl
    exact ha l m' t' hl
  · have : l = σ.length := by
      have := (List.getElem?_eq_some_iff.mp hl).1
      simp at this
      omega
    subst this
    rw [List.getElem?_concat_length] at hl
    cases hl
    exact hn

/-! ## The copy of a freeze (#292) -/

/-- A copy only adds to the heap. -/
theorem Copy.prefix {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') : ∃ ext, h' = h ++ ext := by
  induction hc with
  | keep | seen => exact ⟨[], by simp⟩
  | @cell _ _ _ _ _ _ _ _ w' _ _ _ _ ih =>
    obtain ⟨ext, rfl⟩ := ih
    exact ⟨ext ++ [w'], by simp⟩
  | pair _ _ _ ih₁ ih₂ =>
    obtain ⟨e₁, rfl⟩ := ih₁
    obtain ⟨e₂, rfl⟩ := ih₂
    exact ⟨e₁ ++ e₂, by simp⟩

/-- So what was in the heap stays as it was. -/
theorem Copy.keeps {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') {l : Nat} (hl : l < h.length) : h'[l]? = h[l]? := by
  obtain ⟨ext, rfl⟩ := hc.prefix
  exact List.getElem?_append_left hl

/-- What a copy already copied, it copied to a location of the freed type. -/
def MemoOk (σ : HeapTy) (r : Nat) (m : List (Nat × Nat)) : Prop :=
  ∀ l c, m.lookup l = some c → ∃ p : Mu × Ty, σ[l]? = some p ∧ σ[c]? = some (p.1.free r .immutable, p.2.free r .immutable)

theorem MemoOk.nil (σ : HeapTy) (r : Nat) : MemoOk σ r [] := fun _ _ h => by simp at h

theorem MemoOk.extend {σ σ' : HeapTy} {r : Nat} {m : List (Nat × Nat)} (hm : MemoOk σ r m) (hx : Extends σ σ') :
    MemoOk σ' r m := by
  intro l c hl
  obtain ⟨p, h₁, h₂⟩ := hm l c hl
  exact ⟨p, hx _ _ h₁, hx _ _ h₂⟩

/-- An object whose type mentions `r` and that a freeze of `r` can copy is
of `r`. -/
theorem Ty.Freezable.cell_region {r : Nat} {μ : Mu} {s : Ty} (hf : (Ty.cell μ s).Freezable r)
    (hmen : (Ty.cell μ s).mentions r = true) : μ = .region r := by
  simp only [Ty.mentions, Bool.or_eq_true] at hmen
  rcases hmen with hm | hm
  · cases μ with
    | region r' => simp only [Mu.mentions, beq_iff_eq] at hm; rw [hm]
    | _ => cases hm
  · exact hf.1 hm

/-- **A freeze's copy is well-typed.** It has the value's type with the
region replaced with `imm`, in a heap typing that only grows: a copied
object gets its own type freed, and what it holds is at most that, as
the original's is. Every object it adds is `imm`, since only objects of
the region are copied. -/
theorem Copy.typed {fns : List Fn} {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') :
    ∀ {σ : HeapTy} {io : Bool} {Γ : Ctx}, HeapOk fns σ h → v.IsValue → HasType fns σ .none io Γ v t →
      t.Freezable r → MemoOk σ r m →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ v'.IsValue ∧ HasType fns σ' .none io Γ v' (t.free r .immutable) ∧
        (∀ (l : Nat) (p : Mu × Ty), σ.length ≤ l → σ'[l]? = some p → p.1 = .immutable) ∧ MemoOk σ' r m' ∧
        (AllocOk σ → AllocOk σ') := by
  induction hc with
  | keep hm =>
    intro σ io Γ hh hv ht _ hmo
    rw [Ty.free_not_mentions hm]
    refine ⟨σ, Extends.refl σ, hh, hv, ht, fun l p hl hp => ?_, hmo, id⟩
    have := (List.getElem?_eq_some_iff.mp hp).1
    omega
  | @seen μ s h m l c hmen hlk =>
    intro σ io Γ hh _ ht hf hmo
    cases ht with
    | loc hσ _ =>
      have hreg := hf.cell_region hmen
      subst hreg
      obtain ⟨p, hp, hc⟩ := hmo l c hlk
      rw [hσ] at hp
      cases hp
      refine ⟨σ, Extends.refl σ, hh, .loc, .loc hc (fun _ => by simp), fun l' p' hl' hp' => ?_, hmo, id⟩
      have := (List.getElem?_eq_some_iff.mp hp').1
      omega
  | @cell μ s h h' m m' l w w' hmen _ hw _ ih =>
    intro σ io Γ hh _ ht hf hmo
    cases ht with
    | loc hσ _ =>
      obtain ⟨_, hd, w₀, hw₀, hwv, hwt⟩ := hh.2 l μ s hσ
      rw [hw] at hw₀
      cases hw₀
      obtain ⟨σ₁, hx, hh₁, hv', ht', hnew, hmo₁, hal₁⟩ := ih hh hwv hwt hf.2 hmo
      -- Only an object of the region is copied, so the copy is `imm`.
      have hreg := hf.cell_region hmen
      subst hreg
      refine ⟨σ₁ ++ [(.immutable, s.free r .immutable)], hx.trans (Extends.append _ _),
        hh₁.alloc (.inr (.inl rfl)) (by simpa using hd.free r .immutable) hv' ht', .loc, ?_, ?_, ?_, ?_⟩
      · have : (Ty.cell (Mu.region r) s).free r .immutable = .cell .immutable (s.free r .immutable) := by
          simp [Ty.free]
        rw [this]
        exact .loc (by rw [hh₁.1, List.getElem?_concat_length]) (fun _ => rfl)
      · intro l' p hl' hp
        by_cases hlt : l' < σ₁.length
        · rw [List.getElem?_append_left hlt] at hp
          exact hnew l' p hl' hp
        · have : l' = σ₁.length := by
            have := (List.getElem?_eq_some_iff.mp hp).1
            simp at this
            omega
          subst this
          rw [List.getElem?_concat_length] at hp
          cases hp
          rfl
      · intro k c hk
        simp only [List.lookup] at hk
        split at hk
        · rename_i heq
          cases hk
          have hkl : k = l := by simpa using heq
          subst hkl
          refine ⟨(.region r, s), hx.trans (Extends.append _ _) _ _ hσ, ?_⟩
          rw [hh₁.1, List.getElem?_concat_length]
          simp
        · exact (hmo₁.extend (Extends.append _ _)) k c hk
      · -- The copy reaches what the original does, which is older.
        intro ha
        refine (hal₁ ha).append fun r' hr' => ?_
        simp only [Ty.reaches, Mu.mentions, Bool.false_or] at hr'
        have h₁ := ha l _ s hσ r' (by simp [Ty.reaches, Ty.reaches_free hr'])
        have h₂ : l < σ.length := (List.getElem?_eq_some_iff.mp hσ).1
        exact h₁.mono (by have := hx.length_le; omega)
  | pair _ _ _ ih₁ ih₂ =>
    intro σ io Γ hh hv ht hf hmo
    cases hv with
    | pair hv₁ hv₂ =>
      cases ht with
      | pair ht₁ ht₂ =>
        obtain ⟨σ₁, hx₁, hh₁, hv₁', ht₁', hn₁, hmo₁, hal₁⟩ := ih₁ hh hv₁ ht₁ hf.1 hmo
        obtain ⟨σ₂, hx₂, hh₂, hv₂', ht₂', hn₂, hmo₂, hal₂⟩ := ih₂ hh₁ hv₂ (ht₂.weaken_heap hx₁) hf.2 hmo₁
        refine ⟨σ₂, hx₁.trans hx₂, hh₂, .pair hv₁' hv₂', .pair (ht₁'.weaken_heap hx₂) ht₂', fun l p hl hp => ?_, hmo₂,
          fun ha => hal₂ (hal₁ ha)⟩
        by_cases hlt : l < σ₁.length
        · obtain ⟨q, hq⟩ : ∃ q, σ₁[l]? = some q := ⟨_, List.getElem?_eq_getElem hlt⟩
          rw [hx₂ l q hq] at hp
          exact Option.some.inj hp ▸ hn₁ l q hl hq
        · exact hn₂ l p (by omega) hp

/-- A freeze can always copy a well-typed value. -/
theorem Copy.exists {fns : List Fn} {r : Nat} (t : Ty) :
    ∀ {σ : HeapTy} {h : Heap} {m : List (Nat × Nat)} {io : Bool} {Γ : Ctx} {v : Expr}, HeapOk fns σ h → v.IsValue →
      HasType fns σ .none io Γ v t → t.Freezable r → MemoOk σ r m → ∃ h' m' v', Copy r t h m v h' m' v' := by
  induction t with
  | unit | int | tparam => intro σ h m io Γ v _ _ _ _ _; exact ⟨_, _, _, .keep rfl⟩
  | cell μ s ih =>
    intro σ h m io Γ v hh hv ht hf hmo
    cases hmen : (Ty.cell μ s).mentions r with
    | false => exact ⟨_, _, _, .keep hmen⟩
    | true =>
      obtain ⟨l, rfl, hσ⟩ := canonical_cell hv ht
      cases hlk : m.lookup l with
      | some c => exact ⟨_, _, _, .seen hmen hlk⟩
      | none =>
        obtain ⟨_, _, w, hw, hwv, hwt⟩ := hh.2 l μ s hσ
        obtain ⟨h', m', w', hc⟩ := ih hh hwv hwt hf.2 hmo
        exact ⟨_, _, _, .cell hmen hlk hw hc⟩
  | fn μ a b => intro σ h m io Γ v _ _ _ hf _; exact ⟨_, _, _, .keep hf⟩
  | prod a b iha ihb =>
    intro σ h m io Γ v hh hv ht hf hmo
    cases hmen : (Ty.prod a b).mentions r with
    | false => exact ⟨_, _, _, .keep hmen⟩
    | true =>
      obtain ⟨v₁, v₂, rfl, hv₁, hv₂⟩ := canonical_prod hv ht
      cases ht with
      | pair ht₁ ht₂ =>
        obtain ⟨h₁, m₁, v₁', hc₁⟩ := iha hh hv₁ ht₁ hf.1 hmo
        obtain ⟨σ₁, hx₁, hh₁, _, _, _, hmo₁, _⟩ := hc₁.typed hh hv₁ ht₁ hf.1 hmo
        obtain ⟨h₂, m₂, v₂', hc₂⟩ := ihb hh₁ hv₂ (ht₂.weaken_heap hx₁) hf.2 hmo₁
        exact ⟨_, _, _, .pair hmen hc₁ hc₂⟩

/-- A well-typed program that isn't a value can take a step. -/
theorem progress {fns : List Fn} {σ : HeapTy} {h : Heap} {io : Bool} {e : Expr} {t : Ty}
    (hh : HeapOk fns σ h) (ht : HasType fns σ .none io Ctx.empty e t) :
    e.IsValue ∨ ∃ h' e', Step fns h e h' e' := by
  generalize hΓ : Ctx.empty = Γ at ht
  induction ht with
  | var hx =>
    subst hΓ
    cases hx
  | unit => exact .inl .unit
  | int => exact .inl .int
  | loc _ _ => exact .inl .loc
  | lam _ => exact .inl .lam
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
  | newR _ _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .newVal hv⟩
    · exact .inr ⟨_, _, .newCong hs⟩
  | setP _ hi _ _ _ => simp [Assume.none] at hi
  | freeze he hf _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨h', m', v', hc⟩ := Copy.exists _ hh hv he hf (MemoOk.nil σ _)
      exact .inr ⟨_, _, .freezeVal hv hc⟩
    · exact .inr ⟨_, _, .freezeCong hs⟩
  | newP _ hi _ _ => simp [Assume.none] at hi
  | setR he₁ _ ih₁ ih₂ =>
    rcases ih₁ hΓ with hv₁ | ⟨h', e', hs⟩
    · rcases ih₂ hΓ with hv₂ | ⟨h', e', hs⟩
      · obtain ⟨l, rfl, hl⟩ := canonical_cell hv₁ he₁
        exact .inr ⟨_, _, .setLoc (hh.lt hl) hv₂⟩
      · exact .inr ⟨_, _, .setCong₂ hv₁ hs⟩
    · exact .inr ⟨_, _, .setCong₁ hs⟩
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
  | call hf _ _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .callVal hf hv⟩
    · exact .inr ⟨_, _, .callCong hs⟩

/-- A step keeps the program's type, and the heap matching a typing that
only grows. -/
theorem preservation_alloc {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ HasType fns σ' .none false Ctx.empty e' t ∧
        (AllocOk σ → AllocOk σ') := by
  induction hs with
  | newCong _ ih =>
    intro σ t hh ht
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | new he hd =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .new he' hd, ha⟩
    | newR he hd hr =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .newR he' hd (hr.mono hx.length_le), ha⟩
  | @newVal e b hv =>
    intro σ t hh ht
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | @new _ _ _ s _ he hd =>
      refine ⟨σ ++ [(muOf b, s)], Extends.append σ _, hh.alloc ((muOf_cases b).imp_right .inl) hd hv he,
        .loc ?_ (fun h => nomatch h), fun ha => ha.append fun r hr => ?_⟩
      · rw [hh.1, List.getElem?_concat_length]
      · -- What's made reaches only what its value does, which exists.
        simp only [Ty.reaches, Bool.or_eq_true] at hr
        rcases hr with hr | hr
        · cases b <;> simp [muOf, Mu.mentions] at hr
        · exact hv.reachBelow ha he r hr
    | @newR _ _ _ s r he hd hr₀ =>
      refine ⟨σ ++ [(.region r, s)], Extends.append σ _, hh.alloc (.inr (.inr ⟨r, rfl⟩)) hd hv he,
        .loc ?_ (fun h => nomatch h), fun ha => ha.append fun r' hr => ?_⟩
      · rw [hh.1, List.getElem?_concat_length]
      · -- The region exists already.
        simp only [Ty.reaches, Bool.or_eq_true] at hr
        rcases hr with hr | hr
        · simp only [Mu.mentions, beq_iff_eq] at hr
          subst hr
          exact hr₀
        · exact hv.reachBelow ha he r' hr
  | getCong _ ih =>
    intro σ t hh ht
    cases ht with
    | get he =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .get he', ha⟩
  | getLoc hl =>
    intro σ t hh ht
    cases ht with
    | get he =>
      cases he with
      | loc hσ _ =>
        obtain ⟨_, _, v, hv, _, hvt⟩ := hh.2 _ _ _ hσ
        rw [hl] at hv
        cases hv
        exact ⟨σ, Extends.refl σ, hh, hvt, id⟩
  | setCong₁ _ ih =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁', ha⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .set he₁' (he₂.weaken_heap hx), ha⟩
    | setR he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁', ha⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .setR he₁' (he₂.weaken_heap hx), ha⟩
    | setP _ hi _ => simp [Assume.none] at hi
  | setCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂', ha⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .set (he₁.weaken_heap hx) he₂', ha⟩
    | setR he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂', ha⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .setR (he₁.weaken_heap hx) he₂', ha⟩
    | setP _ hi _ => simp [Assume.none] at hi
  | @setLoc l v hl hv =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      cases he₁ with
      | loc hσ _ => exact ⟨σ, Extends.refl σ, hh.write hσ hl hv he₂, .unit, id⟩
    | setR he₁ he₂ =>
      cases he₁ with
      | loc hσ _ => exact ⟨σ, Extends.refl σ, hh.write hσ hl hv he₂, .unit, id⟩
    | setP _ hi _ => simp [Assume.none] at hi
  | letCong _ ih =>
    intro σ t hh ht
    cases ht with
    | letE he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁', ha⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .letE he₁' (he₂.weaken_heap hx), ha⟩
  | letVal hv =>
    intro σ t hh ht
    cases ht with
    | letE he₁ he₂ =>
      exact ⟨σ, Extends.refl σ, hh, he₂.subst_update hv he₁, id⟩
  | callCong _ ih =>
    intro σ t hh ht
    cases ht with
    | call hf' hl he hb =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .call hf' hl he' (fun m hm => (hb m hm).mono hx.length_le), ha⟩
  | @callVal f fn ms τs v hfn hv =>
    intro σ t hh ht
    cases ht with
    | call hf' hs he hbw =>
      rw [hfn] at hf'
      cases hf'
      obtain ⟨hn, hbody⟩ := hf fn (List.mem_of_getElem? hfn)
      -- The arguments meet what the function assumes, so its body has the
      -- type with them.
      have hb := (hbody.weaken_heap (Extends.nil σ)).inst hn hs hbw
      rw [Ctx.inst_update, Ctx.inst_empty] at hb
      exact ⟨σ, Extends.refl σ, hh, hb.subst_update hv he, id⟩
  | appCong₁ _ ih =>
    intro σ t hh ht
    cases ht with
    | app he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁', ha⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .app he₁' (he₂.weaken_heap hx), ha⟩
  | appCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | app he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂', ha⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .app (he₁.weaken_heap hx) he₂', ha⟩
  | appVal hv =>
    intro σ t hh ht
    cases ht with
    | app he₁ he₂ =>
      cases he₁ with
      | lam _ hc _ _ _ hbody =>
        -- The body of an `imm` closure is typed inside one; outside, it
        -- has the same type. A closed closure saw nothing but its
        -- parameter.
        have hb := (hbody.weaken_ctx (hc.update _ _)).weaken_imm (io' := false) (fun h => nomatch h)
        exact ⟨σ, Extends.refl σ, hh, hb.subst_update hv he₂, id⟩
  | pairCong₁ _ ih =>
    intro σ t hh ht
    cases ht with
    | pair he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁', ha⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .pair he₁' (he₂.weaken_heap hx), ha⟩
  | pairCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | pair he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂', ha⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .pair (he₁.weaken_heap hx) he₂', ha⟩
  | fstCong _ ih =>
    intro σ t hh ht
    cases ht with
    | fst he =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .fst he', ha⟩
  | sndCong _ ih =>
    intro σ t hh ht
    cases ht with
    | snd he =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .snd he', ha⟩
  | fstVal _ _ =>
    intro σ t hh ht
    cases ht with
    | fst he => cases he with
      | pair h₁ _ => exact ⟨σ, Extends.refl σ, hh, h₁, id⟩
  | sndVal _ _ =>
    intro σ t hh ht
    cases ht with
    | snd he => cases he with
      | pair _ h₂ => exact ⟨σ, Extends.refl σ, hh, h₂, id⟩
  | freezeCong _ ih =>
    intro σ t hh ht
    cases ht with
    | freeze he hf hp =>
      obtain ⟨σ', hx, hh', he', ha⟩ := ih hh he
      exact ⟨σ', hx, hh', .freeze he' hf hp, ha⟩
  | freezeVal hv hc =>
    intro σ t hh ht
    cases ht with
    | freeze he hf _ =>
      -- The copy has the type with the region `imm`, in a heap typing that
      -- only grew.
      obtain ⟨σ', hx, hh', _, ht', _, _, hal⟩ := hc.typed hh hv he hf (MemoOk.nil σ _)
      exact ⟨σ', hx, hh', ht', hal⟩
  | blockEnter => intro σ t hh ht; cases ht
  | keepCong _ _ => intro σ t hh ht; cases ht
  | keepVal _ _ => intro σ t hh ht; cases ht

/-- Preservation: a step keeps the heap matching a typing that only grew,
and the program's type. -/
theorem preservation {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') {σ : HeapTy} {t : Ty} (hh : HeapOk fns σ h)
    (ht : HasType fns σ .none false Ctx.empty e t) :
    ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ HasType fns σ' .none false Ctx.empty e' t :=
  let ⟨σ', hx, hh', ht', _⟩ := preservation_alloc hf hs hh ht
  ⟨σ', hx, hh', ht'⟩

end BoltMut
