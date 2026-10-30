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

/-! ## The copy of a freeze (#292) -/

/-- A copy only adds to the heap. -/
theorem Copy.prefix {r : Nat} {t : Ty} {h h' : Heap} {v v' : Expr} (hc : Copy r t h v h' v') :
    ∃ ext, h' = h ++ ext := by
  induction hc with
  | keep => exact ⟨[], by simp⟩
  | @cell _ _ _ _ _ _ w' _ _ _ ih =>
    obtain ⟨ext, rfl⟩ := ih
    exact ⟨ext ++ [w'], by simp⟩
  | pair _ _ _ ih₁ ih₂ =>
    obtain ⟨e₁, rfl⟩ := ih₁
    obtain ⟨e₂, rfl⟩ := ih₂
    exact ⟨e₁ ++ e₂, by simp⟩

/-- **A freeze's copy is well-typed.** It has the value's type with the
region replaced with `imm`, in a heap typing that only grows: a copied
object gets its own type freed, and what it holds is at most that, as
the original's is. Every object it adds is `imm`, since only objects of
the region are copied. -/
theorem Copy.typed {fns : List Fn} {r : Nat} {t : Ty} {h h' : Heap} {v v' : Expr} (hc : Copy r t h v h' v') :
    ∀ {σ : HeapTy} {io : Bool} {Γ : Ctx}, HeapOk fns σ h → v.IsValue → HasType fns σ .none io Γ v t → t.Freezable r →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ v'.IsValue ∧ HasType fns σ' .none io Γ v' (t.free r .immutable) ∧
        ∀ (l : Nat) (p : Mu × Ty), σ.length ≤ l → σ'[l]? = some p → p.1 = .immutable := by
  induction hc with
  | keep hm =>
    intro σ io Γ hh hv ht _
    rw [Ty.free_not_mentions hm]
    refine ⟨σ, Extends.refl σ, hh, hv, ht, fun l p hl hp => ?_⟩
    have := (List.getElem?_eq_some_iff.mp hp).1
    omega
  | @cell m s h h' l w w' hmen hw _ ih =>
    intro σ io Γ hh _ ht hf
    cases ht with
    | loc hσ hio =>
      obtain ⟨_, hd, w₀, hw₀, hwv, hwt⟩ := hh.2 l m s hσ
      rw [hw] at hw₀
      cases hw₀
      obtain ⟨σ₁, hx, hh₁, hv', ht', hnew⟩ := ih hh hwv hwt hf.2
      -- Only an object of the region is copied, so the copy is `imm`.
      have hreg : m = .region r := by
        simp only [Ty.mentions, Bool.or_eq_true] at hmen
        rcases hmen with hm | hm
        · cases m with
          | region r' => simp only [Mu.mentions, beq_iff_eq] at hm; rw [hm]
          | _ => cases hm
        · exact hf.1 hm
      subst hreg
      refine ⟨σ₁ ++ [(.immutable, s.free r .immutable)], hx.trans (Extends.append _ _),
        hh₁.alloc (.inr (.inl rfl)) (by simpa using hd.free r .immutable) hv' ht', .loc, ?_, ?_⟩
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
  | pair _ _ _ ih₁ ih₂ =>
    intro σ io Γ hh hv ht hf
    cases hv with
    | pair hv₁ hv₂ =>
      cases ht with
      | pair ht₁ ht₂ =>
        obtain ⟨σ₁, hx₁, hh₁, hv₁', ht₁', hn₁⟩ := ih₁ hh hv₁ ht₁ hf.1
        obtain ⟨σ₂, hx₂, hh₂, hv₂', ht₂', hn₂⟩ := ih₂ hh₁ hv₂ (ht₂.weaken_heap hx₁) hf.2
        refine ⟨σ₂, hx₁.trans hx₂, hh₂, .pair hv₁' hv₂', .pair (ht₁'.weaken_heap hx₂) ht₂', fun l p hl hp => ?_⟩
        by_cases hlt : l < σ₁.length
        · obtain ⟨q, hq⟩ : ∃ q, σ₁[l]? = some q := ⟨_, List.getElem?_eq_getElem hlt⟩
          rw [hx₂ l q hq] at hp
          exact Option.some.inj hp ▸ hn₁ l q hl hq
        · exact hn₂ l p (by omega) hp

/-- So what was in the heap stays as it was. -/
theorem Copy.keeps {r : Nat} {t : Ty} {h h' : Heap} {v v' : Expr} (hc : Copy r t h v h' v') {l : Nat}
    (hl : l < h.length) : h'[l]? = h[l]? := by
  obtain ⟨ext, rfl⟩ := hc.prefix
  exact List.getElem?_append_left hl

/-- A freeze can always copy a well-typed value. -/
theorem Copy.exists {fns : List Fn} {r : Nat} (t : Ty) :
    ∀ {σ : HeapTy} {h : Heap} {io : Bool} {Γ : Ctx} {v : Expr}, HeapOk fns σ h → v.IsValue →
      HasType fns σ .none io Γ v t → t.Freezable r → ∃ h' v', Copy r t h v h' v' := by
  induction t with
  | unit | int | tparam => intro σ h io Γ v _ _ _ _; exact ⟨_, _, .keep rfl⟩
  | cell m s ih =>
    intro σ h io Γ v hh hv ht hf
    cases hm : (Ty.cell m s).mentions r with
    | false => exact ⟨_, _, .keep hm⟩
    | true =>
      obtain ⟨l, rfl, hσ⟩ := canonical_cell hv ht
      obtain ⟨_, _, w, hw, hwv, hwt⟩ := hh.2 l m s hσ
      obtain ⟨h', w', hc⟩ := ih hh hwv hwt hf.2
      exact ⟨_, _, .cell hm hw hc⟩
  | fn m a b => intro σ h io Γ v _ _ _ hf; exact ⟨_, _, .keep hf⟩
  | prod a b iha ihb =>
    intro σ h io Γ v hh hv ht hf
    cases hm : (Ty.prod a b).mentions r with
    | false => exact ⟨_, _, .keep hm⟩
    | true =>
      obtain ⟨v₁, v₂, rfl, hv₁, hv₂⟩ := canonical_prod hv ht
      cases ht with
      | pair ht₁ ht₂ =>
        obtain ⟨h₁, v₁', hc₁⟩ := iha hh hv₁ ht₁ hf.1
        obtain ⟨σ₁, hx₁, hh₁, _, _, _⟩ := hc₁.typed hh hv₁ ht₁ hf.1
        obtain ⟨h₂, v₂', hc₂⟩ := ihb hh₁ hv₂ (ht₂.weaken_heap hx₁) hf.2
        exact ⟨_, _, .pair hm hc₁ hc₂⟩

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
  | newR _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .newVal hv⟩
    · exact .inr ⟨_, _, .newCong hs⟩
  | setP _ hi _ _ _ => simp [Assume.none] at hi
  | freeze he hf _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · obtain ⟨h', v', hc⟩ := Copy.exists _ hh hv he hf
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
  | call hf _ _ ih =>
    rcases ih hΓ with hv | ⟨h', e', hs⟩
    · exact .inr ⟨_, _, .callVal hf hv⟩
    · exact .inr ⟨_, _, .callCong hs⟩

/-- A step keeps the program's type, and the heap matching a typing that
only grows. -/
theorem preservation {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t →
      ∃ σ', Extends σ σ' ∧ HeapOk fns σ' h' ∧ HasType fns σ' .none false Ctx.empty e' t := by
  induction hs with
  | newCong _ ih =>
    intro σ t hh ht
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | new he hd =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .new he' hd⟩
    | newR he hd =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .newR he' hd⟩
  | @newVal e b hv =>
    intro σ t hh ht
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | @new _ _ _ s _ he hd =>
      refine ⟨σ ++ [(muOf b, s)], Extends.append σ _, hh.alloc ((muOf_cases b).imp_right .inl) hd hv he,
        .loc ?_ (fun h => nomatch h)⟩
      rw [hh.1, List.getElem?_concat_length]
    | @newR _ _ _ s r he hd =>
      refine ⟨σ ++ [(.region r, s)], Extends.append σ _, hh.alloc (.inr (.inr ⟨r, rfl⟩)) hd hv he,
        .loc ?_ (fun h => nomatch h)⟩
      rw [hh.1, List.getElem?_concat_length]
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
      | loc hσ _ =>
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
    | setR he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .setR he₁' (he₂.weaken_heap hx)⟩
    | setP _ hi _ => simp [Assume.none] at hi
  | setCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .set (he₁.weaken_heap hx) he₂'⟩
    | setR he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .setR (he₁.weaken_heap hx) he₂'⟩
    | setP _ hi _ => simp [Assume.none] at hi
  | @setLoc l v hl hv =>
    intro σ t hh ht
    cases ht with
    | set he₁ he₂ =>
      cases he₁ with
      | loc hσ _ => exact ⟨σ, Extends.refl σ, hh.write hσ hl hv he₂, .unit⟩
    | setR he₁ he₂ =>
      cases he₁ with
      | loc hσ _ => exact ⟨σ, Extends.refl σ, hh.write hσ hl hv he₂, .unit⟩
    | setP _ hi _ => simp [Assume.none] at hi
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
      exact ⟨σ, Extends.refl σ, hh, he₂.subst_update hv he₁⟩
  | callCong _ ih =>
    intro σ t hh ht
    cases ht with
    | call hf' hl he =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .call hf' hl he'⟩
  | @callVal f fn ms τs v hfn hv =>
    intro σ t hh ht
    cases ht with
    | call hf' hs he =>
      rw [hfn] at hf'
      cases hf'
      obtain ⟨hn, hbody⟩ := hf fn (List.mem_of_getElem? hfn)
      -- The arguments meet what the function assumes, so its body has the
      -- type with them.
      have hb := (hbody.weaken_heap (Extends.nil σ)).inst hn hs
      rw [Ctx.inst_update, Ctx.inst_empty] at hb
      exact ⟨σ, Extends.refl σ, hh, hb.subst_update hv he⟩
  | appCong₁ _ ih =>
    intro σ t hh ht
    cases ht with
    | app he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .app he₁' (he₂.weaken_heap hx)⟩
  | appCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | app he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .app (he₁.weaken_heap hx) he₂'⟩
  | appVal hv =>
    intro σ t hh ht
    cases ht with
    | app he₁ he₂ =>
      cases he₁ with
      | lam hc _ hbody =>
        -- The body of an `imm` closure is typed inside one; outside, it
        -- has the same type. A closed closure saw nothing but its
        -- parameter.
        have hb := (hbody.weaken_ctx (hc.update _ _)).weaken_imm (io' := false) (fun h => nomatch h)
        exact ⟨σ, Extends.refl σ, hh, hb.subst_update hv he₂⟩
  | pairCong₁ _ ih =>
    intro σ t hh ht
    cases ht with
    | pair he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₁'⟩ := ih hh he₁
      exact ⟨σ', hx, hh', .pair he₁' (he₂.weaken_heap hx)⟩
  | pairCong₂ _ _ ih =>
    intro σ t hh ht
    cases ht with
    | pair he₁ he₂ =>
      obtain ⟨σ', hx, hh', he₂'⟩ := ih hh he₂
      exact ⟨σ', hx, hh', .pair (he₁.weaken_heap hx) he₂'⟩
  | fstCong _ ih =>
    intro σ t hh ht
    cases ht with
    | fst he =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .fst he'⟩
  | sndCong _ ih =>
    intro σ t hh ht
    cases ht with
    | snd he =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .snd he'⟩
  | fstVal _ _ =>
    intro σ t hh ht
    cases ht with
    | fst he => cases he with
      | pair h₁ _ => exact ⟨σ, Extends.refl σ, hh, h₁⟩
  | sndVal _ _ =>
    intro σ t hh ht
    cases ht with
    | snd he => cases he with
      | pair _ h₂ => exact ⟨σ, Extends.refl σ, hh, h₂⟩
  | freezeCong _ ih =>
    intro σ t hh ht
    cases ht with
    | freeze he hf hp =>
      obtain ⟨σ', hx, hh', he'⟩ := ih hh he
      exact ⟨σ', hx, hh', .freeze he' hf hp⟩
  | freezeVal hv hc =>
    intro σ t hh ht
    cases ht with
    | freeze he hf _ =>
      -- The copy has the type with the region `imm`, in a heap typing that
      -- only grew.
      obtain ⟨σ', hx, hh', _, ht', _⟩ := hc.typed hh hv he hf
      exact ⟨σ', hx, hh', ht'⟩

end BoltMut
