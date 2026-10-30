import BoltMut.Immutability

/-!
# What a call to an `imm` function can change

An `imm` closure only captures what's `imm` all the way down. So calling one
with arguments that are `imm` all the way down changes nothing that existed
before the call (`imm_fn_changes_nothing`): it can change only the objects
it makes itself, which no one else can see.

With algebraic effects, an operation that resumes right away, like `state`
and `read` of the `Reactive` effect in `site/docs/internals/reactive.mdx`,
compiles to a call of its handler, passed as a hidden argument. Then
`imm_fn_changes_only_through_handler`: calling an `imm` function with `imm`
arguments and a handler changes only what the handler can reach. That's
what makes tracking complete: while drawing, a component can only affect
the world through its effects.

Both follow from one invariant, `Clean`: every location the program can
reach is `imm`, was made since the call started, or is allowed (`A`, what
the handler can reach). A well-typed step keeps it, and only writes to
locations that are new or allowed (`step_clean`).
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
  ∀ l v, (n ≤ l ∨ A l) → h[l]? = some v → Clean σ n A v

theorem Clean.extend {σ σ' : HeapTy} {n : Nat} {A : Nat → Prop} {e : Expr}
    (hc : Clean σ n A e) (hx : Extends σ σ') : Clean σ' n A e := by
  intro l hl
  rcases hc l hl with h | h | ⟨t, ht⟩
  · exact .inl h
  · exact .inr (.inl h)
  · exact .inr (.inr ⟨t, hx _ _ ht⟩)

theorem HeapClean.extend {σ σ' : HeapTy} {n : Nat} {A : Nat → Prop} {h : Heap}
    (hc : HeapClean σ n A h) (hx : Extends σ σ') : HeapClean σ' n A h :=
  fun l v hA hv => (hc l v hA hv).extend hx

/-- A value whose type is `imm` all the way down holds only `imm`
locations. -/
theorem clean_of_deepImm {fns : List Fn} {σ : HeapTy} {n : Nat} {A : Nat → Prop} {io : Bool} {Γ : Ctx}
    {v : Expr} {t : Ty} (hv : v.IsValue) (ht : HasType fns σ .none io Γ v t) (hd : AtMost .none .immutable t) : Clean σ n A v :=
  fun _ hl => .inr (.inr ((ht.value_imm hv hd).hasLoc_imm rfl hl))

/-- A freeze's copy keeps them clean: what it makes is new, and what it
shares is in the value or in a clean location, which holds what's clean. -/
theorem Copy.clean {fns : List Fn} {σ : HeapTy} {n : Nat} {A : Nat → Prop} {h₀ : Heap} (hh : HeapOk fns σ h₀)
    (hn : n ≤ h₀.length) {r : Nat} {t : Ty} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Copy r t h m v h' m' v') :
    (∃ ext, h = h₀ ++ ext) → (∀ l c, m.lookup l = some c → n ≤ c) → Clean σ n A v → HeapClean σ n A h →
      Clean σ n A v' ∧ HeapClean σ n A h' ∧ ∀ l c, m'.lookup l = some c → n ≤ c := by
  induction hc with
  | keep => intro _ hm hcv hhc; exact ⟨hcv, hhc, hm⟩
  | seen _ hlk => intro _ hm _ hhc; exact ⟨fun l' hl' => .inl (hl' ▸ hm _ _ hlk), hhc, hm⟩
  | @cell μ s h h' m m' l w w' _ _ hw hc' ih =>
    intro hpre hm hcv hhc
    -- What the location holds is clean.
    have hcw : Clean σ n A w := by
      rcases hcv l rfl with h₁ | h₁ | ⟨t₀, ht₀⟩
      · exact hhc l w (.inl h₁) hw
      · exact hhc l w (.inr h₁) hw
      · obtain ⟨_, hd, w₀, hw₀, hwv, hwt⟩ := hh.2 _ _ _ ht₀
        obtain ⟨ext, rfl⟩ := hpre
        rw [List.getElem?_append_left (hh.lt ht₀), hw₀] at hw
        cases hw
        exact clean_of_deepImm hwv hwt hd
    obtain ⟨hcw', hhc', hm'⟩ := ih hpre hm hcw hhc
    have hlen : n ≤ h'.length := by
      obtain ⟨ext, rfl⟩ := hpre
      obtain ⟨ext', rfl⟩ := hc'.prefix
      simp; omega
    refine ⟨fun l' hl' => .inl (hl' ▸ hlen), ?_, ?_⟩
    · intro l' u hA hu
      by_cases hlt : l' < h'.length
      · rw [List.getElem?_append_left hlt] at hu
        exact hhc' l' u hA hu
      · have hl' : l' = h'.length := by
          have := (List.getElem?_eq_some_iff.mp hu).1
          simp at this
          omega
        subst hl'
        rw [List.getElem?_concat_length] at hu
        cases hu
        exact hcw'
    · intro k c hk
      simp only [List.lookup] at hk
      split at hk
      · cases hk; exact hlen
      · exact hm' k c hk
  | @pair _ _ _ h₁ _ _ _ _ _ _ _ _ _ hc₁ _ ih₁ ih₂ =>
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
    cases ht with
    | newP _ hi _ => simp [Assume.none] at hi
    | new he _ | newR he _ => exact ih hh he hn hc hhc
  | @newVal e b _ =>
    intro σ t n A hh _ hn hc hhc
    refine ⟨fun l hl _ => List.getElem?_append_left (by omega), by simp; omega,
      fun l hl => .inl (hl ▸ hn), ?_⟩
    intro l v hA hv
    by_cases hlt : l < h.length
    · rw [List.getElem?_append_left hlt] at hv
      exact hhc l v hA hv
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
    cases ht with
    | get he => exact ih hh he hn hc hhc
  | @getLoc l v hl =>
    intro σ t n A hh ht hn hc hhc
    refine ⟨fun _ _ _ => rfl, hn, ?_, hhc⟩
    rcases hc l rfl with h₁ | h₁ | ⟨t₀, ht₀⟩
    · exact hhc l v (.inl h₁) hl
    · exact hhc l v (.inr h₁) hl
    · -- An `imm` location holds a value that's `imm` all the way down.
      obtain ⟨_, hd, w, hw, hwv, hwt⟩ := hh.2 _ _ _ ht₀
      rw [hl] at hw
      cases hw
      exact clean_of_deepImm hwv hwt hd
  | setCong₁ _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set he₁ _ | setR he₁ _ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | setCong₂ _ _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set _ he₂ | setR _ he₂ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₂ hn (fun l hl => hc l (.inr hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (fun hl => hc l (.inl hl)) (hc' l), hhc'⟩
  | @setLoc l₀ v hl _ =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | setP _ hi _ => simp [Assume.none] at hi
    | set he₁ _ | setR he₁ _ =>
      cases he₁ with
      | loc hσ _ =>
        -- The location written is `mut` or in a region, so it's new or
        -- allowed.
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
        · intro l w hA hw'
          by_cases heq : l₀ = l
          · subst heq
            rw [List.getElem?_set_self hl] at hw'
            cases hw'
            exact fun l' hl' => hc l' (.inr hl')
          · rw [List.getElem?_set_ne heq] at hw'
            exact hhc l w hA hw'
  | letCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | letE he₁ _ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | letVal _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn,
      fun l hl => (Expr.hasLoc_subst hl).elim (fun h => hc l (.inr h)) (fun h => hc l (.inl h)), hhc⟩
  | callCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | call _ _ he => exact ih hh he hn hc hhc
  | @callVal f fn ms τs v hfn _ =>
    intro σ t n A _ _ hn hc hhc
    -- A function's body has no locations: only its argument does.
    have hb := ((hf fn (List.mem_of_getElem? hfn)).1).inst ms τs
    exact ⟨fun _ _ _ => rfl, hn,
      fun l hl => (Expr.hasLoc_subst hl).elim (fun h => (hb.not_hasLoc l h).elim) (hc l), hhc⟩
  | appCong₁ _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | app he₁ _ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | appCong₂ _ _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | app _ he₂ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₂ hn (fun l hl => hc l (.inr hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (fun hl => hc l (.inl hl)) (hc' l), hhc'⟩
  | appVal _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn,
      fun l hl => (Expr.hasLoc_subst hl).elim (fun h => hc l (.inl h)) (fun h => hc l (.inr h)), hhc⟩
  | pairCong₁ _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | pair he₁ _ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₁ hn (fun l hl => hc l (.inl hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (hc' l) (fun hl => hc l (.inr hl)), hhc'⟩
  | pairCong₂ _ _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | pair _ he₂ =>
      obtain ⟨hu, hn', hc', hhc'⟩ := ih hh he₂ hn (fun l hl => hc l (.inr hl)) hhc
      exact ⟨hu, hn', fun l hl => hl.elim (fun hl => hc l (.inl hl)) (hc' l), hhc'⟩
  | fstCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | fst he => exact ih hh he hn hc hhc
  | sndCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | snd he => exact ih hh he hn hc hhc
  | fstVal _ _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn, fun l hl => hc l (.inl hl), hhc⟩
  | sndVal _ _ =>
    intro σ t n A _ _ hn hc hhc
    exact ⟨fun _ _ _ => rfl, hn, fun l hl => hc l (.inr hl), hhc⟩
  | freezeCong _ ih =>
    intro σ t n A hh ht hn hc hhc
    cases ht with
    | freeze he _ _ _ _ => exact ih hh he hn hc hhc
  | freezeVal _ hc' =>
    intro σ t n A hh _ hn hc hhc
    obtain ⟨hcv, hhc', _⟩ := Copy.clean hh hn hc' ⟨[], by simp⟩ (fun _ _ h => by simp at h) hc hhc
    refine ⟨fun l hl _ => hc'.keeps (by omega), ?_, hcv, hhc'⟩
    obtain ⟨ext, he⟩ := hc'.prefix
    rw [he]
    simp
    omega
  | @blockEnter k t e =>
    -- The marker is new; the body's locations are the block's.
    intro σ t n A _ _ hn hc hhc
    refine ⟨fun l hl _ => List.getElem?_append_left (by omega), by simp; omega,
      fun l hl => hc l (Expr.hasLoc_ren (e := e) hl), fun l v hA hv => ?_⟩
    rw [List.getElem?_append] at hv
    split at hv
    · exact hhc l v hA hv
    · rcases hx : l - _ with _ | j <;> rw [hx] at hv <;> simp at hv
      rw [← hv]
      exact fun _ h => h.elim
  | blockKeep => intro σ t n A hh ht; cases ht
  | keepCong _ _ => intro σ t n A hh ht; cases ht
  | keepVal _ _ => intro σ t n A hh ht; cases ht

/-- However many steps a clean program takes, the locations that are old
and not allowed keep their values. -/
theorem steps_clean {fns : List Fn} (hf : FnsOk fns) {h h' : Heap} {e e' : Expr} (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty} {n : Nat} {A : Nat → Prop},
      HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t → HeapClosed h → e.Closed → n ≤ h.length →
      Clean σ n A e → HeapClean σ n A h → ∀ l, l < n → ¬ A l → h'[l]? = h[l]? := by
  induction hs with
  | refl => intros; rfl
  | step s _ ih =>
    intro σ t n A hh ht hcl hcl' hn hc hhc l hl hA
    obtain ⟨hu, hn', hc', hhc'⟩ := step_clean hf s hh ht hn hc hhc
    obtain ⟨σ', hx, hh', ht'⟩ := preservation hf s hh ht hcl'
    obtain ⟨hcl₁, hcl₁'⟩ := step_closed (fun fn h => (hf fn h).2.2.2) s hcl hcl'
    rw [ih hh' ht' hcl₁ hcl₁' hn' (hc'.extend hx) (hhc'.extend hx) l hl hA, hu l hl hA]

/-- Calling an `imm` function with an argument that's `imm` all the way
down changes nothing that existed before the call, however many steps it
takes. -/
theorem imm_fn_changes_nothing {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap}
    {f v e' : Expr} {a r : Ty} (hh : HeapOk fns σ h)
    (hfv : f.IsValue) (hft : HasType fns σ .none false Ctx.empty f (.fn .immutable a r))
    (hvv : v.IsValue) (hvt : HasType fns σ .none false Ctx.empty v a) (hd : AtMost .none .immutable a)
    (hcl : HeapClosed h) (hfc : f.Closed) (hvc : v.Closed)
    (hs : Steps fns h (.app f v) h' e') : ∀ l, l < h.length → h'[l]? = h[l]? := by
  intro l hl
  refine steps_clean hf hs (A := fun _ => False) hh (.app hft hvt) hcl ⟨hfc, hvc⟩ (Nat.le_refl _) ?_ ?_ l hl id
  · intro l' hl'
    rcases hl' with hl' | hl'
    · exact clean_of_deepImm (n := h.length) (A := fun _ => False) (t := .fn .immutable a r) hfv hft (.refl _) l' hl'
    · exact clean_of_deepImm (n := h.length) (A := fun _ => False) hvv hvt hd l' hl'
  · intro l' w hA hw
    rcases hA with hA | hA
    · have := (List.getElem?_eq_some_iff.mp hw).1
      omega
    · exact hA.elim

/-- Following a location in what `l` holds extends a path from `l₀` to
`l`. -/
theorem Reaches.tail {h : Heap} {l₀ l : Nat} (hr : Reaches h l₀ l) :
    ∀ {v : Expr} {l₁ : Nat}, h[l]? = some v → v.HasLoc l₁ → Reaches h l₀ l₁ := by
  induction hr with
  | refl => intro v l₁ hv hl; exact .step hv hl .refl
  | step hv' hl' _ ih => intro v l₁ hv hl; exact .step hv' hl' (ih hv hl)

/-- With effects: calling an `imm` function with an argument that's `imm`
all the way down and a handler `k`, as the hidden argument of an effect
whose operations resume right away, changes only what the handler can
reach. -/
theorem imm_fn_changes_only_through_handler {fns : List Fn} (hf : FnsOk fns) {σ : HeapTy} {h h' : Heap}
    {f v k e' : Expr} {a kt r : Ty} (hh : HeapOk fns σ h)
    (hfv : f.IsValue) (hft : HasType fns σ .none false Ctx.empty f (.fn .immutable (.prod a kt) r))
    (hvv : v.IsValue) (hvt : HasType fns σ .none false Ctx.empty v a) (hd : AtMost .none .immutable a)
    (hkt : HasType fns σ .none false Ctx.empty k kt)
    (hcl : HeapClosed h) (hfc : f.Closed) (hvc : v.Closed) (hkc : k.Closed)
    (hs : Steps fns h (.app f (.pair v k)) h' e') :
    ∀ l, l < h.length → (¬ ∃ l₀, k.HasLoc l₀ ∧ Reaches h l₀ l) → h'[l]? = h[l]? := by
  intro l hl hA
  let A : Nat → Prop := fun l => ∃ l₀, k.HasLoc l₀ ∧ Reaches h l₀ l
  refine steps_clean hf hs (A := A) hh (.app hft (.pair hvt hkt)) hcl ⟨hfc, hvc, hkc⟩ (Nat.le_refl _) ?_ ?_ l hl hA
  · intro l' hl'
    rcases hl' with hl' | hl' | hl'
    · exact clean_of_deepImm (n := h.length) (A := A) (t := .fn .immutable (.prod a kt) r) hfv hft (.refl _) l' hl'
    · exact clean_of_deepImm (n := h.length) (A := A) hvv hvt hd l' hl'
    · exact .inr (.inl ⟨l', hl', .refl⟩)
  · intro l' w hA' hw
    rcases hA' with hA' | ⟨l₀, hk, hr⟩
    · have := (List.getElem?_eq_some_iff.mp hw).1
      omega
    · exact fun l₁ hl₁ => .inr (.inl ⟨l₀, hk, hr.tail hw hl₁⟩)

end BoltMut
