import BoltMut.Immutability

/-!
# What a program can change

The invariant behind what a call can change (`Frame.lean`): `Clean`, every
location the program can reach is `imm`, was made since it started, or is
allowed (`A`). A well-typed step keeps it, and only writes to locations
that are new or allowed (`step_clean`). So however many steps it takes, a
program changes only what's new or allowed (`steps_clean`).
-/

namespace BoltMut

/-- Every location in `e` is new (at least `n`), allowed by `A`, or
`imm`. -/
def Clean (σ : HeapTy) (n : Nat) (A : Nat → Prop) (e : Expr) : Prop :=
  ∀ l, e.HasLoc l → n ≤ l ∨ A l ∨ ∃ t, σ[l]? = some (.immutable, t)

/-- What the new and the allowed locations hold is clean too. The others
needn't be: an `imm` one holds only `imm` locations anyway, and the rest
can't be reached. -/
def HeapClean (σ : HeapTy) (n : Nat) (A : Nat → Prop) (h : Heap) : Prop :=
  ∀ l b v, (n ≤ l ∨ A l) → h[l]? = some (b, v) → Clean σ n A v

theorem Clean.extend {σ σ' : HeapTy} {n : Nat} {A : Nat → Prop} {e : Expr}
    (hc : Clean σ n A e) (hx : Extends σ σ') : Clean σ' n A e := by
  intro l hl
  rcases hc l hl with h | h | ⟨t, ht⟩
  · exact .inl h
  · exact .inr (.inl h)
  · exact .inr (.inr ⟨t, hx _ _ ht⟩)

theorem HeapClean.extend {σ σ' : HeapTy} {n : Nat} {A : Nat → Prop} {h : Heap}
    (hc : HeapClean σ n A h) (hx : Extends σ σ') : HeapClean σ' n A h :=
  fun l b v hA hv => (hc l b v hA hv).extend hx

/-- A value whose type is `imm` all the way down holds only `imm`
locations. -/
theorem clean_of_deepImm {fns : List Fn} {σ : HeapTy} {n : Nat} {A : Nat → Prop} {io : Bool} {Γ : Ctx}
    {v : Expr} {t : Ty} (hv : v.IsValue) (ht : HasType fns σ .none io Γ v t) (hd : AtMost .none .immutable t) : Clean σ n A v :=
  fun _ hl => .inr (.inr ((ht.value_imm hv hd).hasLoc_imm rfl hl))

/-- A copy keeps them clean: what it makes is new, and what it shares is
in the value or in a clean location, which holds what's clean. What it
copies is `mut`, so it's new or allowed. -/
theorem Freeze.clean {fns : List Fn} {σ : HeapTy} {n : Nat} {A : Nat → Prop} {h₀ : Heap} (hh : HeapOk fns σ h₀)
    (hn : n ≤ h₀.length) {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') :
    (∃ ext, h = h₀ ++ ext) → (∀ l c, m.lookup l = some c → n ≤ c) → Clean σ n A v → HeapClean σ n A h →
      Clean σ n A v' ∧ HeapClean σ n A h' ∧ ∀ l c, m'.lookup l = some c → n ≤ c := by
  induction hc with
  | unit | int | lam | imm => intro _ hm hcv hhc; exact ⟨hcv, hhc, hm⟩
  | seen _ hlk => intro _ hm _ hhc; exact ⟨fun l' hl' => .inl (hl' ▸ hm _ _ hlk), hhc, hm⟩
  | @cell h h' m m' l w w' hw _ hc' ih =>
    intro hpre hm hcv hhc
    -- What the location holds is clean: it's `mut`, so not `imm`.
    have hcw : Clean σ n A w := by
      rcases hcv l rfl with h₁ | h₁ | ⟨t₀, ht₀⟩
      · exact hhc l true w (.inl h₁) hw
      · exact hhc l true w (.inr h₁) hw
      · obtain ⟨_, _, w₀, hw₀, _⟩ := hh.2 _ _ _ ht₀
        obtain ⟨ext, rfl⟩ := hpre
        rw [List.getElem?_append_left (hh.lt ht₀), hw₀] at hw
        cases hw
    obtain ⟨hcw', hhc', hm'⟩ := ih hpre hm hcw hhc
    have hlen : n ≤ h'.length := by
      obtain ⟨ext, rfl⟩ := hpre
      obtain ⟨ext', rfl⟩ := hc'.prefix
      simp; omega
    refine ⟨fun l' hl' => .inl (hl' ▸ hlen), ?_, ?_⟩
    · intro l' b u hA hu
      by_cases hlt : l' < h'.length
      · rw [List.getElem?_append_left hlt] at hu
        exact hhc' l' b u hA hu
      · have hl' : l' = h'.length := by
          have := (List.getElem?_eq_some_iff.mp hu).1
          simp at this
          omega
        subst hl'
        rw [List.getElem?_concat_length] at hu
        cases hu
        exact hcw'
    · intro k c hk
      rw [List.lookup_cons] at hk
      split at hk
      · cases hk; exact hlen
      · exact hm' k c hk
  | @pair _ h₁ _ _ _ _ _ _ _ _ hc₁ _ ih₁ ih₂ =>
    intro hpre hm hcv hhc
    obtain ⟨hc₁', hhc₁, hm₁⟩ := ih₁ hpre hm (fun l hl => hcv l (.inl hl)) hhc
    have hpre₁ : ∃ ext, h₁ = h₀ ++ ext := by
      obtain ⟨ext, rfl⟩ := hpre
      obtain ⟨ext', rfl⟩ := hc₁.prefix
      exact ⟨ext ++ ext', by simp⟩
    obtain ⟨hc₂', hhc₂, hm₂⟩ := ih₂ hpre₁ hm₁ (fun l hl => hcv l (.inr hl)) hhc₁
    exact ⟨fun l hl => hl.elim (hc₁' l) (hc₂' l), hhc₂, hm₂⟩

/-- A step keeps the program and the heap clean, and changes no location
that's old and not allowed. -/
theorem step_clean {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr} (hs : Step fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty} {n : Nat} {A : Nat → Prop},
      HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t → n ≤ h.length →
      Clean σ n A e → HeapClean σ n A h →
      (∀ l, l < n → ¬ A l → h'[l]? = h[l]?) ∧ n ≤ h'.length ∧ Clean σ n A e' ∧ HeapClean σ n A h' := by
  induction hs with
  | newCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, he, _, _, _⟩ := ht.new_inv
    exact ih hh he hn hc hhc
  | @newVal e m _ =>
    intro σ t n A hh _ hn hc hhc
    refine ⟨fun l hl _ => List.getElem?_append_left (by omega), by simp; omega,
      fun l hl => .inl (hl ▸ hn), ?_⟩
    intro l b' v hA hv
    by_cases hlt : l < h.length
    · rw [List.getElem?_append_left hlt] at hv
      exact hhc l b' v hA hv
    · have hlen : l = h.length := by
        have := (List.getElem?_eq_some_iff.mp hv).1
        simp at this
        omega
      subst hlen
      rw [List.getElem?_concat_length] at hv
      cases hv
      exact hc
  | getCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, he, _⟩ := ht.get_inv
    exact ih hh he hn hc hhc
  | @getLoc l b v hl =>
    intro σ t n A hh ht hn hc hhc
    refine ⟨fun _ _ _ => rfl, hn, ?_, hhc⟩
    rcases hc l rfl with h₁ | h₁ | ⟨t₀, ht₀⟩
    · exact hhc l b v (.inl h₁) hl
    · exact hhc l b v (.inr h₁) hl
    · -- An `imm` location holds a value that's `imm` all the way down.
      obtain ⟨_, hd, w, hw, hwv, hwt⟩ := hh.2 _ _ _ ht₀
      rw [hl] at hw
      cases hw
      exact clean_of_deepImm hwv hwt hd
  | setCong₁ _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, he₁, _, _⟩ := ht.set_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | setCong₂ _ _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, he₂, _⟩ := ht.set_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₂ hn (fun l hl => hc l (.inr hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (fun hl => hc l (.inl hl)) (hc' l), hhc'⟩
  | @setLoc l₀ b w₀ v hl _ =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, he₁, _, _⟩ := ht.set_inv
    obtain ⟨_, hl', hσ⟩ := canonical_mut_cell .loc he₁
    cases hl'
    · -- The location written is `mut`, so it's new or allowed.
      have hw : n ≤ l₀ ∨ A l₀ := by
        rcases hc l₀ (.inl rfl) with h₁ | h₁ | ⟨t₀, ht₀⟩
        · exact .inl h₁
        · exact .inr h₁
        · rw [hσ] at ht₀
          cases ht₀
      refine ⟨?_, by simp; omega, fun l hl => hl.elim, ?_⟩
      · intro l hl hA
        have hne : l₀ ≠ l := by
          rintro rfl
          rcases hw with h₁ | h₁
          · omega
          · exact hA h₁
        exact List.getElem?_set_ne hne
      · intro l b' w hA hw'
        by_cases heq : l₀ = l
        · subst heq
          rw [List.getElem?_set_self (List.getElem?_eq_some_iff.mp hl).1] at hw'
          cases hw'
          exact fun l' hl' => hc l' (.inr hl')
        · rw [List.getElem?_set_ne heq] at hw'
          exact hhc l b' w hA hw'
  | letCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, he₁, _, _⟩ := ht.letE_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | letVal _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn,
      fun l hl => (Expr.hasLoc_subst hl).elim (fun h => hc l (.inr h)) (fun h => hc l (.inl h)), hhc⟩
  | callCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, _, he, _⟩ := ht.call_inv
    exact ih hh he hn hc hhc
  | @callVal f fn ms τs v hfn _ =>
    intro σ t n A _ _ hn hc hhc
    -- A function's body has no locations: only its argument does.
    have hb := ((hf fn (List.mem_of_getElem? hfn)).1).inst ms τs
    exact ⟨fun _ _ _ => rfl, hn,
      fun l hl => (Expr.hasLoc_subst hl).elim (fun h => (hb.not_hasLoc l h).elim) (hc l), hhc⟩
  | appCong₁ _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, _, he₁, _, _⟩ := ht.app_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | appCong₂ _ _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, _, _, he₂, _⟩ := ht.app_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₂ hn (fun l hl => hc l (.inr hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (fun hl => hc l (.inl hl)) (hc' l), hhc'⟩
  | appVal _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn,
      fun l hl => (Expr.hasLoc_subst hl).elim (fun h => hc l (.inl h)) (fun h => hc l (.inr h)), hhc⟩
  | pairCong₁ _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, he₁, _, _⟩ := ht.pair_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | pairCong₂ _ _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, _, he₂, _⟩ := ht.pair_inv
    obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₂ hn (fun l hl => hc l (.inr hl)) hhc
    exact ⟨hu, hn', fun l hl => hl.elim (fun hl => hc l (.inl hl)) (hc' l), hhc'⟩
  | fstCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, he, _⟩ := ht.fst_inv
    exact ih hh he hn hc hhc
  | sndCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, he, _⟩ := ht.snd_inv
    exact ih hh he hn hc hhc
  | fstVal _ _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn, fun l hl => hc l (.inl hl), hhc⟩
  | sndVal _ _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn, fun l hl => hc l (.inr hl), hhc⟩
  | toImmCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    obtain ⟨_, _, _, he, _, _⟩ := ht.toImm_inv
    exact ih hh he hn hc hhc
  | toImmVal _ hc' =>
    intro σ t n A hh _ hn hc hhc
    obtain ⟨hcv, hhc', _⟩ := Freeze.clean hh hn hc' ⟨[], by simp⟩ (fun _ _ h => by simp at h) hc hhc
    refine ⟨fun l hl _ => hc'.keeps (by omega), ?_, hcv, hhc'⟩
    obtain ⟨ext, he⟩ := hc'.prefix
    rw [he]
    simp
    omega
  | toImmKeep _ _ => intro σ t n A hh ht; exact nomatch ht.toImm_inv.1

/-- However many steps a clean program takes, the locations that are old
and not allowed keep their values. -/
theorem steps_clean {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr} (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty} {n : Nat} {A : Nat → Prop},
      HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t → n ≤ h.length →
      Clean σ n A e → HeapClean σ n A h → ∀ l, l < n → ¬ A l → h'[l]? = h[l]? := by
  induction hs with
  | refl => intros; rfl
  | step s _ ih =>
    intro σ t n A hh ht hn hc hhc l hl hA
    obtain ⟨hu, hn', hc', hhc'⟩ := step_clean hf s hh ht hn hc hhc
    obtain ⟨σ', hx, hh', ht'⟩ := preservation hf s hh ht
    rw [ih hh' ht' hn' (hc'.extend hx) (hhc'.extend hx) l hl hA, hu l hl hA]

/-- Following a location in what `l` holds extends a path from `l₀` to
`l`. -/
theorem Reaches.tail {h : Heap} {l₀ l : Nat} (hr : Reaches h l₀ l) :
    ∀ {b : Bool} {v : Expr} {l₁ : Nat}, h[l]? = some (b, v) → v.HasLoc l₁ → Reaches h l₀ l₁ := by
  induction hr with
  | refl => intro b v l₁ hv hl; exact .step hv hl .refl
  | step hv' hl' _ ih => intro b v l₁ hv hl; exact .step hv' hl' (ih hv hl)

end BoltMut
