import BoltMut.Sharing

/-!
# Regions (#335)

`imm v` copies the `mut` objects that `v` reaches, following the
locations it holds but not what closures captured: its *region*
(`Region`). Freezing in place makes those objects `imm` where they are
instead. That can't be noticed when nothing outside the region has them:
not the rest of the program, and no object outside it.

This file proves what the copy does to the region: it copies exactly
objects of it (`Freeze.keys`), each to a new location of its own
(`Freeze.memo_inj`), and each copy holds what the original does, with the
region's objects replaced by their copies (`Freeze.shape`).
-/

namespace BoltMut

/-- The locations a value holds, but not in what closures captured: what
`imm` follows. -/
def Expr.TopLoc : Expr → Nat → Prop
  | .loc l', l => l' = l
  | .pair a b, l => a.TopLoc l ∨ b.TopLoc l
  | _, _ => False

theorem Expr.TopLoc.hasLoc {v : Expr} {l : Nat} (h : v.TopLoc l) : v.HasLoc l := by
  induction v with
  | loc => exact h
  | pair _ _ ih₁ ih₂ => exact h.imp ih₁ ih₂
  | _ => exact h.elim

/-- `a` can be reached from `l` through `mut` objects only: both are `mut`,
and each holds the next. -/
inductive MutReach (h : Heap) : Nat → Nat → Prop where
  | refl {l : Nat} {w : Expr} : h[l]? = some (true, w) → MutReach h l l
  | step {l l₁ a : Nat} {w : Expr} : h[l]? = some (true, w) → w.TopLoc l₁ → MutReach h l₁ a → MutReach h l a

theorem MutReach.mut {h : Heap} {l a : Nat} (hr : MutReach h l a) : ∃ w, h[a]? = some (true, w) := by
  induction hr with
  | refl hw => exact ⟨_, hw⟩
  | step _ _ _ ih => exact ih

/-- Where a path through `mut` objects starts is `mut`. -/
theorem MutReach.of_start {h : Heap} {l a : Nat} (hr : MutReach h l a) : ∃ w, h[l]? = some (true, w) := by
  cases hr with
  | refl hw => exact ⟨_, hw⟩
  | step hw _ _ => exact ⟨_, hw⟩

/-- **The region of `v`**: the `mut` objects it reaches. -/
def Region (h : Heap) (v : Expr) (a : Nat) : Prop := ∃ l, v.TopLoc l ∧ MutReach h l a

theorem Region.mut {h : Heap} {v : Expr} {a : Nat} (hr : Region h v a) : ∃ w, h[a]? = some (true, w) :=
  let ⟨_, _, hm⟩ := hr; hm.mut

/-! ## What a copy adds to the heap -/

/-- A copy only adds `imm` objects. -/
theorem Freeze.ext_imm {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') : ∀ a, h.length ≤ a → ∀ b w, h'[a]? = some (b, w) → b = false := by
  induction hc with
  | unit | int | lam | imm | seen =>
    intro a ha b w hw
    have := (List.getElem?_eq_some_iff.mp hw).1
    omega
  | @cell h h' m m' l w w' _ _ hcw ih =>
    intro a ha b u hu
    have hle : h.length ≤ h'.length := by obtain ⟨e, rfl⟩ := hcw.prefix; simp
    by_cases hlt : a < h'.length
    · rw [List.getElem?_append_left hlt] at hu
      exact ih a ha b u hu
    · have : a = h'.length := by
        have := (List.getElem?_eq_some_iff.mp hu).1
        simp at this
        omega
      subst this
      rw [List.getElem?_concat_length] at hu
      cases hu
      rfl
  | @pair h h₁ h₂ _ _ _ _ _ _ _ hc₁ hc₂ ih₁ ih₂ =>
    intro a ha b w hw
    have hle : h.length ≤ h₁.length := by obtain ⟨e, rfl⟩ := hc₁.prefix; simp
    by_cases hlt : a < h₁.length
    · obtain ⟨e₂, rfl⟩ := hc₂.prefix
      rw [List.getElem?_append_left hlt] at hw
      exact ih₁ a ha b w hw
    · exact ih₂ a (by omega) b w hw

/-- So what's reached through `mut` objects after a copy was reached
before. -/
theorem MutReach.of_ext {h ext : Heap} (hext : ∀ a, h.length ≤ a → ∀ b w, (h ++ ext)[a]? = some (b, w) → b = false)
    {l a : Nat} (hr : MutReach (h ++ ext) l a) : MutReach h l a := by
  have old : ∀ {x : Nat} {w : Expr}, (h ++ ext)[x]? = some (true, w) → h[x]? = some (true, w) := by
    intro x w hx
    by_cases hlt : x < h.length
    · rwa [List.getElem?_append_left hlt] at hx
    · exact absurd (hext x (by omega) true w hx) (by simp)
  induction hr with
  | refl hw => exact .refl (old hw)
  | step hw hl _ ih => exact .step (old hw) hl ih

/-! ## What a copy copies -/

/-- A copy keeps what was copied before. -/
theorem Freeze.lookup_keep {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') {a c : Nat} (ha : m.lookup a = some c) : m'.lookup a = some c := by
  induction hc with
  | unit | int | lam | imm | seen => exact ha
  | @cell _ _ _ _ l _ _ _ hnone _ ih =>
    have hal : a ≠ l := by rintro rfl; rw [hnone] at ha; cases ha
    simp only [List.lookup_cons_nat, hal, ite_false]
    exact ih ha
  | pair _ _ ih₁ ih₂ => exact ih₂ (ih₁ ha)

/-- **A copy copies only the region.** What it copies that wasn't copied
before is an object of the value's region, copied to a new location. -/
theorem Freeze.keys {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') {a c : Nat} (ha : m'.lookup a = some c) :
    m.lookup a = some c ∨ (Region h v a ∧ h.length ≤ c ∧ c < h'.length) := by
  induction hc with
  | unit | int | lam | imm | seen => exact .inl ha
  | @cell h h' m m' l w w' hw hnone hcw ih =>
    have hle : h.length ≤ h'.length := by obtain ⟨e, rfl⟩ := hcw.prefix; simp
    rw [List.lookup_cons_nat] at ha
    split at ha
    · rename_i hal
      subst hal
      cases ha
      exact .inr ⟨⟨a, rfl, .refl hw⟩, hle, by simp⟩
    · rcases ih ha with h₁ | ⟨⟨l₁, ht, hr⟩, h₂, h₃⟩
      · exact .inl h₁
      · exact .inr ⟨⟨l, rfl, .step hw ht hr⟩, h₂, by simp; omega⟩
  | @pair h h₁ h₂ _ _ _ _ _ _ _ hc₁ hc₂ ih₁ ih₂ =>
    have hle₁ : h.length ≤ h₁.length := by obtain ⟨e, rfl⟩ := hc₁.prefix; simp
    have hle₂ : h₁.length ≤ h₂.length := by obtain ⟨e, rfl⟩ := hc₂.prefix; simp
    rcases ih₂ ha with h₁' | ⟨⟨l₁, ht, hr⟩, h₂', h₃'⟩
    · rcases ih₁ h₁' with h₁'' | ⟨⟨l₁, ht, hr⟩, h₂'', h₃''⟩
      · exact .inl h₁''
      · exact .inr ⟨⟨l₁, .inl ht, hr⟩, h₂'', by omega⟩
    · obtain ⟨ext, rfl⟩ := hc₁.prefix
      exact .inr ⟨⟨l₁, .inr ht, hr.of_ext hc₁.ext_imm⟩, by omega, h₃'⟩

/-- What's copied so far was copied to locations below `n`, each its own. -/
def MemoFresh (n : Nat) (m : List (Nat × Nat)) : Prop :=
  (∀ a c, m.lookup a = some c → c < n) ∧ ∀ a a' c, m.lookup a = some c → m.lookup a' = some c → a = a'

theorem MemoFresh.nil (n : Nat) : MemoFresh n [] := ⟨fun _ _ h => by simp at h, fun _ _ _ h => by simp at h⟩

/-- **Each object is copied to a location of its own.** -/
theorem Freeze.fresh {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') : MemoFresh h.length m → MemoFresh h'.length m' := by
  induction hc with
  | unit | int | lam | imm | seen => exact id
  | @cell h h' m m' l w w' _ _ hcw ih =>
    intro hf
    obtain ⟨hlt, hinj⟩ := ih hf
    refine ⟨fun a c ha => ?_, fun a a' c ha ha' => ?_⟩
    · rw [List.lookup_cons_nat] at ha
      split at ha
      · cases ha; simp
      · have := hlt a c ha; simp; omega
    · rw [List.lookup_cons_nat] at ha ha'
      split at ha <;> split at ha'
      · rename_i h₁ h₂; rw [h₁, h₂]
      · cases ha; exact absurd (hlt a' _ ha') (by omega)
      · cases ha'; exact absurd (hlt a _ ha) (by omega)
      · exact hinj a a' c ha ha'
  | pair _ _ ih₁ ih₂ => exact fun hf => ih₂ (ih₁ hf)

/-! ## What the copies hold -/

/-- How a copy relates to what it copied: an `imm` object is itself, and a
copied one is its copy. -/
def CopyCorr (h : Heap) (m : List (Nat × Nat)) (a c : Nat) : Prop :=
  ((∃ w, h[a]? = some (false, w)) ∧ c = a) ∨ m.lookup a = some c

theorem CopyCorr.mono {h h' : Heap} {m m' : List (Nat × Nat)} (hh : ∃ ext, h' = h ++ ext)
    (hm : ∀ a c, m.lookup a = some c → m'.lookup a = some c) {a c : Nat} (hc : CopyCorr h m a c) :
    CopyCorr h' m' a c := by
  rcases hc with ⟨⟨w, hw⟩, rfl⟩ | hc
  · obtain ⟨ext, rfl⟩ := hh
    exact .inl ⟨⟨w, by rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp hw).1]; exact hw⟩, rfl⟩
  · exact .inr (hm _ _ hc)

/-- Each object copied so far is `mut`, and its copy is `imm` and holds
what it does, with the copied objects replaced. -/
def MemoShape (h : Heap) (m : List (Nat × Nat)) : Prop :=
  ∀ a c, m.lookup a = some c →
    ∃ w w', h[a]? = some (true, w) ∧ h[c]? = some (false, w') ∧ Rel (CopyCorr h m) w w'

theorem MemoShape.nil (h : Heap) : MemoShape h [] := fun _ _ h => by simp at h

theorem MemoShape.mono {h h' : Heap} {m m' : List (Nat × Nat)} (hs : MemoShape h m) (hh : ∃ ext, h' = h ++ ext)
    (hm : ∀ a c, m.lookup a = some c → m'.lookup a = some c) : ∀ a c, m.lookup a = some c →
      ∃ w w', h'[a]? = some (true, w) ∧ h'[c]? = some (false, w') ∧ Rel (CopyCorr h' m') w w' := by
  intro a c ha
  obtain ⟨w, w', hw, hw', hr⟩ := hs a c ha
  obtain ⟨ext, rfl⟩ := hh
  exact ⟨w, w', by rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp hw).1]; exact hw,
    by rw [List.getElem?_append_left (List.getElem?_eq_some_iff.mp hw').1]; exact hw',
    hr.mono fun _ _ h => h.mono ⟨ext, rfl⟩ hm⟩

/-- An `imm` closure captures only `imm` objects. -/
theorem lam_imm {fns : List Fn} {σ : HeapTy} {h : Heap} (hh : HeapOk fns σ h) {io : Bool} {Γ : Ctx} {b : Bool}
    {x : Nat} {e : Expr} {t : Ty} (ht : HasType fns σ .none io Γ (.lam b x e) t) (hf : t.Freezable) :
    ∀ l, e.HasLoc l → ∃ w, h[l]? = some (false, w) := by
  obtain ⟨_, _, _, _, _, hb, hs⟩ := ht.lam_inv
  obtain ⟨_, _, rfl⟩ := hs.of_fn
  intro l hl
  cases b with
  | true => exact (nomatch (show Le .none .mutable .immutable from hf).none_immutable)
  | false =>
    obtain ⟨s, hσ⟩ := hb.hasLoc_imm (by simp) hl
    obtain ⟨_, _, w, hw, _⟩ := hh.2 _ _ _ hσ
    exact ⟨w, hw⟩

/-- A type's size: what an object holding a `t` is bigger than. -/
def Ty.size : Ty → Nat
  | .cell _ t => t.size + 1
  | .prod a b => a.size + b.size + 1
  | _ => 1

theorem Sub.size {Δ : Assume} {s t : Ty} (h : Sub Δ s t) : s.size = t.size := by
  induction h with
  | refl => rfl
  | ro _ _ ih => simp [Ty.size, ih]
  | prod _ _ iha ihb => simp [Ty.size, iha, ihb]
  | tparam => rfl

/-- **What a copy copies is smaller than the value.** So an object isn't
reached again while what it holds is copied. -/
theorem Freeze.key_size {fns : List Fn} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') :
    ∀ {σ : HeapTy} {io : Bool} {Γ : Ctx} {t : Ty}, HeapOk fns σ h → v.IsValue → HasType fns σ .none io Γ v t →
      t.Freezable → MemoOk σ m → ∀ a c, m'.lookup a = some c → m.lookup a = none →
      ∃ s, σ[a]? = some (.mutable, s) ∧ s.size + 1 ≤ t.size := by
  induction hc with
  | unit | int | lam | imm | seen =>
    intro σ io Γ t _ _ _ _ _ a c ha hn
    rw [hn] at ha; cases ha
  | @cell h h' m m' l w w' hw hnone hcw ih =>
    intro σ io Γ t hh _ ht hf hmo a c ha hn
    obtain ⟨μ, s, hσ, _, hfs⟩ := loc_imm ht
    obtain ⟨hμ, _, w₀, hw₀, hwv, hwt⟩ := hh.2 _ _ _ hσ
    rw [hw] at hw₀
    have hμ' : μ = .mutable := by
      rcases hμ with rfl | rfl
      · rfl
      · cases hw₀
    subst hμ'
    cases hw₀
    -- `t` is `l`'s type, seen through `ro` or not.
    have hts : t.size = s.size + 1 := by
      obtain ⟨_, _, h₁, _, hsub⟩ := ht.loc_inv
      rw [hσ] at h₁; cases h₁; rw [← hsub.size]; rfl
    rw [List.lookup_cons_nat] at ha
    split at ha
    · rename_i hal
      subst hal
      exact ⟨s, hσ, by omega⟩
    · obtain ⟨s', h₁, h₂⟩ := ih hh hwv hwt (hfs hf) hmo a c ha hn
      exact ⟨s', h₁, by omega⟩
  | @pair h h₁ h₂ m m₁ m₂ v₁ v₂ v₁' v₂' hc₁ hc₂ ih₁ ih₂ =>
    intro σ io Γ t hh hv ht hf hmo a c ha hn
    cases hv with
    | pair hv₁ hv₂ =>
      -- At the types of its parts, whose sizes seeing them as `ro` keeps.
      obtain ⟨ta, tb, ht₁, ht₂, hsp⟩ := ht.pair_inv
      obtain ⟨_, _, rfl, hsa', hsb'⟩ := hsp.prod_inv
      have hf₁ := hsa'.imm.2 hf.1
      have hf₂ := hsb'.imm.2 hf.2
      have hsa := hsa'.size
      have hsb := hsb'.size
      cases h₁a : m₁.lookup a with
      | none =>
        obtain ⟨σ₁, hx₁, hh₁, _, _, hmo₁⟩ := hc₁.typed hh hv₁ ht₁ hf₁ hmo
        obtain ⟨s', h₁', h₂'⟩ := ih₂ hh₁ hv₂ (ht₂.weaken_heap hx₁) hf₂ hmo₁ a c ha h₁a
        -- It's an old location, since it was copied.
        have hlt : a < σ.length := by
          rcases hc₂.keys ha with h₃ | ⟨hr, _, _⟩
          · rw [h₁a] at h₃; cases h₃
          · obtain ⟨w, hw⟩ := hr.mut
            by_cases hge : a < h.length
            · rw [hh.1] at hge; exact hge
            · cases hc₁.ext_imm a (by omega) true w hw
        obtain ⟨p, hp⟩ : ∃ p, σ[a]? = some p := ⟨_, List.getElem?_eq_getElem hlt⟩
        rw [hx₁ _ _ hp] at h₁'
        exact ⟨s', hp.trans h₁', by simp [Ty.size]; omega⟩
      | some c₁ =>
        obtain ⟨s', h₁', h₂'⟩ := ih₁ hh hv₁ ht₁ hf₁ hmo a c₁ h₁a hn
        exact ⟨s', h₁', by simp [Ty.size]; omega⟩

/-- **What a copy is.** The value's copy is the value with each copied
object replaced by its copy, and each copy holds what its original does,
done the same way. -/
theorem Freeze.shape {fns : List Fn} {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') :
    ∀ {σ : HeapTy} {io : Bool} {Γ : Ctx} {t : Ty}, HeapOk fns σ h → v.IsValue → HasType fns σ .none io Γ v t →
      t.Freezable → MemoOk σ m → MemoShape h m →
      Rel (CopyCorr h' m') v v' ∧ MemoShape h' m' := by
  induction hc with
  | unit => intro σ io Γ t _ _ _ _ _ hs; exact ⟨.unit, hs⟩
  | int => intro σ io Γ t _ _ _ _ _ hs; exact ⟨.int, hs⟩
  | lam =>
    intro σ io Γ t hh _ ht hf _ hs
    exact ⟨Rel.refl fun l hl => .inl ⟨lam_imm hh ht hf l hl, rfl⟩, hs⟩
  | imm hw => intro σ io Γ t _ _ _ _ _ hs; exact ⟨.loc (.inl ⟨⟨_, hw⟩, rfl⟩), hs⟩
  | seen _ hlk => intro σ io Γ t _ _ _ _ _ hs; exact ⟨.loc (.inr hlk), hs⟩
  | @cell h h' m m' l w w' hw hnone hcw ih =>
    intro σ io Γ t hh _ ht hf hmo hs
    obtain ⟨μ, s, hσ, _, hfs⟩ := loc_imm ht
    obtain ⟨hμ, _, w₀, hw₀, hwv, hwt⟩ := hh.2 _ _ _ hσ
    rw [hw] at hw₀
    have hμ' : μ = .mutable := by
      rcases hμ with rfl | rfl
      · rfl
      · cases hw₀
    subst hμ'
    cases hw₀
    obtain ⟨hr, hs'⟩ := ih hh hwv hwt (hfs hf) hmo hs
    let m'' := (l, h'.length) :: m'
    have hm : ∀ a c, m'.lookup a = some c → m''.lookup a = some c := by
      intro a c ha
      have hal : a ≠ l := by
        rintro rfl
        cases hn : m.lookup a with
        | some c' => rw [hnone] at hn; cases hn
        | none =>
          -- What `l` holds is smaller than `l`.
          obtain ⟨s', h₁, h₂⟩ := hcw.key_size hh hwv hwt (hfs hf) hmo a c ha hn
          rw [hσ] at h₁
          cases h₁
          omega
      simp only [m'', List.lookup_cons_nat, hal, ite_false]
      exact ha
    have hext : ∃ ext, h' ++ [(false, w')] = h' ++ ext := ⟨_, rfl⟩
    refine ⟨.loc (.inr (by simp)), fun a c ha => ?_⟩
    simp only [List.lookup_cons_nat] at ha
    split at ha
    · rename_i hal
      subst hal
      cases ha
      have hpre : ∃ ext, h' = h ++ ext := hcw.prefix
      refine ⟨w, w', ?_, by simp, hr.mono fun _ _ h => h.mono hext hm⟩
      obtain ⟨ext, rfl⟩ := hpre
      rw [List.getElem?_append_left (by simp; have := (List.getElem?_eq_some_iff.mp hw).1; omega),
        List.getElem?_append_left (List.getElem?_eq_some_iff.mp hw).1]
      exact hw
    · exact hs'.mono hext hm a c ha
  | @pair h h₁ h₂ m m₁ m₂ v₁ v₂ v₁' v₂' hc₁ hc₂ ih₁ ih₂ =>
    intro σ io Γ t hh hv ht hf hmo hs
    cases hv with
    | pair hv₁ hv₂ =>
      obtain ⟨_, _, ht₁, ht₂, hsp⟩ := ht.pair_inv
      obtain ⟨_, _, rfl, ha, hb⟩ := hsp.prod_inv
      have hf₁ := ha.imm.2 hf.1
      have hf₂ := hb.imm.2 hf.2
      obtain ⟨hr₁, hs₁⟩ := ih₁ hh hv₁ ht₁ hf₁ hmo hs
      obtain ⟨σ₁, hx₁, hh₁, _, _, hmo₁⟩ := hc₁.typed hh hv₁ ht₁ hf₁ hmo
      obtain ⟨hr₂, hs₂⟩ := ih₂ hh₁ hv₂ (ht₂.weaken_heap hx₁) hf₂ hmo₁ hs₁
      exact ⟨.pair (hr₁.mono fun _ _ h => h.mono hc₂.prefix fun _ _ h => hc₂.lookup_keep h) hr₂, hs₂⟩

/-- Two expressions related to the same one are related to each other,
through it. -/
theorem Rel.comp {σ ρ : Nat → Nat → Prop} {x y z : Expr} (h₁ : Rel σ x y) (h₂ : Rel ρ x z) :
    Rel (fun c d => ∃ a, σ a c ∧ ρ a d) y z := by
  induction h₁ generalizing z with
  | var => cases h₂; exact .var
  | unit => cases h₂; exact .unit
  | int => cases h₂; exact .int
  | loc h => cases h₂ with | loc h' => exact .loc ⟨_, h, h'⟩
  | new _ ih => cases h₂ with | new h₂ => exact .new (ih h₂)
  | get _ ih => cases h₂ with | get h₂ => exact .get (ih h₂)
  | call _ ih => cases h₂ with | call h₂ => exact .call (ih h₂)
  | lam _ ih => cases h₂ with | lam h₂ => exact .lam (ih h₂)
  | fst _ ih => cases h₂ with | fst h₂ => exact .fst (ih h₂)
  | snd _ ih => cases h₂ with | snd h₂ => exact .snd (ih h₂)
  | toImm _ ih => cases h₂ with | toImm h₂ => exact .toImm (ih h₂)
  | set _ _ ih₁ ih₂ => cases h₂ with | set h₂ h₃ => exact .set (ih₁ h₂) (ih₂ h₃)
  | letE _ _ ih₁ ih₂ => cases h₂ with | letE h₂ h₃ => exact .letE (ih₁ h₂) (ih₂ h₃)
  | app _ _ ih₁ ih₂ => cases h₂ with | app h₂ h₃ => exact .app (ih₁ h₂) (ih₂ h₃)
  | pair _ _ ih₁ ih₂ => cases h₂ with | pair h₂ h₃ => exact .pair (ih₁ h₂) (ih₂ h₃)

end BoltMut
