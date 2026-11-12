import BoltMut.Region

/-!
# `toImm` in place (#284, #335)

`imm v` copies the region of `v`, the `mut` objects it reaches, into new
`imm` objects. If nothing outside the region has those objects, nobody
could tell if the copy were left out, and the region made `imm` where it
is instead. Uniqueness is tracked separately from typing: the program that
copies is the one that's typed, and the one that freezes in place is
shown to compute the same.
-/

namespace BoltMut

/-! ## Every `toImm` in place

The program that leaves copies out is the same program with every `toImm` in
place (`Expr.inPlace`, `Fn.inPlace`), which makes the region of what it
freezes `imm` where it is (`Region.lean`). The premise is that whenever
the program that copies reaches a `toImm` of a value, the value's region
is its own: the program has the region's objects only in the value, and no
object outside the region holds one (`Expr.UniqToImm`). Then each copy can
be the original (`region_freeze_step`). A closure has no `toImm` (`Expr.LamNoToImm`),
so values, and the heap, have none: a closure that makes an `imm` copy uses
`new false (get xs)` instead. -/

/-- `e` with every `toImm` in place. -/
def Expr.inPlace : Expr → Expr
  | .var x => .var x
  | .unit => .unit
  | .int n => .int n
  | .loc l => .loc l
  | .new b e => .new b e.inPlace
  | .get e => .get e.inPlace
  | .set a b => .set a.inPlace b.inPlace
  | .letE x a b => .letE x a.inPlace b.inPlace
  | .call f ms τs e => .call f ms τs e.inPlace
  | .lam b x e => .lam b x e.inPlace
  | .app a b => .app a.inPlace b.inPlace
  | .pair a b => .pair a.inPlace b.inPlace
  | .fst e => .fst e.inPlace
  | .snd e => .snd e.inPlace
  | .toImm _ e => .toImm true e.inPlace

/-- A function whose `toImm`s are in place. -/
def Fn.inPlace (fn : Fn) : Fn := { fn with body := fn.body.inPlace }

/-- `e` has no `toImm`. -/
def Expr.NoToImm : Expr → Prop
  | .var _ | .unit | .int _ | .loc _ => True
  | .new _ e | .get e | .call _ _ _ e | .lam _ _ e | .fst e | .snd e => e.NoToImm
  | .set a b | .letE _ a b | .app a b | .pair a b => a.NoToImm ∧ b.NoToImm
  | .toImm _ _ => False

/-- No closure in `e` has a `toImm`. -/
def Expr.LamNoToImm : Expr → Prop
  | .var _ | .unit | .int _ | .loc _ => True
  | .lam _ _ e => e.NoToImm
  | .new _ e | .get e | .call _ _ _ e | .fst e | .snd e | .toImm _ e => e.LamNoToImm
  | .set a b | .letE _ a b | .app a b | .pair a b => a.LamNoToImm ∧ b.LamNoToImm

/-- How many times `l` is in `e`. -/
def Expr.count (l : Nat) : Expr → Nat
  | .var _ | .unit | .int _ => 0
  | .loc l' => if l' = l then 1 else 0
  | .new _ e | .get e | .call _ _ _ e | .lam _ _ e | .fst e | .snd e | .toImm _ e => e.count l
  | .set a b | .letE _ a b | .app a b | .pair a b => a.count l + b.count l

/-- `e` is about to make `l` `imm`: `toImm _ (loc l)` is where `e` runs next, or
on the way there, past what's run already. -/
def Expr.ToImmOf (l : Nat) : Expr → Prop
  | .var _ | .unit | .int _ | .loc _ | .lam _ _ _ => False
  | .new _ e | .get e | .call _ _ _ e | .fst e | .snd e => e.ToImmOf l
  | .letE _ a _ => a.ToImmOf l
  | .set a b | .app a b | .pair a b => a.ToImmOf l ∨ a.IsValue ∧ b.ToImmOf l
  | .toImm _ e => e = .loc l ∨ e.ToImmOf l

/-- `e` is about to make the value `v` `imm`: `toImm _ v` is where `e` runs
next. -/
def Expr.ToImmArg : Expr → Expr → Prop
  | .var _, _ | .unit, _ | .int _, _ | .loc _, _ | .lam _ _ _, _ => False
  | .new _ e, v | .get e, v | .call _ _ _ e, v | .fst e, v | .snd e, v => e.ToImmArg v
  | .letE _ a _, v => a.ToImmArg v
  | .set a b, v | .app a b, v | .pair a b, v => a.ToImmArg v ∨ a.IsValue ∧ b.ToImmArg v
  | .toImm _ e, v => e = v ∨ e.ToImmArg v

/-- What `toImm` is given is an object whose contents are `imm` already,
so that it copies just the object (#335). -/
def Shallow (h : Heap) (v : Expr) : Prop :=
  ∃ l b w, v = .loc l ∧ h[l]? = some (b, w) ∧ ∀ l', w.HasLoc l' → ∃ w', h[l']? = some (false, w')

/-- **What a program makes `imm` is its own** when it gets there: the
program has the objects of its region only in what it makes `imm`, and no
object outside the region holds one. -/
def Expr.UniqToImm (h : Heap) (e : Expr) : Prop :=
  ∀ v, e.ToImmArg v → ∀ x, Region h v x →
    e.count x = v.count x ∧ ∀ (a : Nat) (b : Bool) (w : Expr), h[a]? = some (b, w) → w.HasLoc x → Region h v a

/-- `x` is an object of the region `e` is about to make `imm`. -/
def Expr.Frozen (h : Heap) (e : Expr) (x : Nat) : Prop := ∃ v, e.ToImmArg v ∧ Region h v x

theorem Expr.Frozen.of {h : Heap} {a e : Expr} {x : Nat} (hx : a.Frozen h x)
    (hg : ∀ v, a.ToImmArg v → e.ToImmArg v) : e.Frozen h x :=
  let ⟨v, hv, hr⟩ := hx; ⟨v, hg v hv, hr⟩

/-- Every value in the heap has no `toImm`. -/
def HeapNoToImm (h : Heap) : Prop := ∀ (l : Nat) (b : Bool) (v : Expr), h[l]? = some (b, v) → v.NoToImm

theorem Expr.NoToImm.inPlace {e : Expr} (h : e.NoToImm) : e.inPlace = e := by
  induction e with
  | var | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih => simp only [Expr.inPlace, ih h]
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.inPlace, ih₁ h.1, ih₂ h.2]
  | toImm => exact h.elim

theorem Expr.NoToImm.lamNoToImm {e : Expr} (h : e.NoToImm) : e.LamNoToImm := by
  induction e with
  | var | unit | int | loc => trivial
  | lam => exact h
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | toImm => exact h.elim

/-- A value with no `toImm` in its closures has none. -/
theorem Expr.IsValue.noToImm {v : Expr} (hv : v.IsValue) (h : v.LamNoToImm) : v.NoToImm := by
  induction hv with
  | unit | int | loc => trivial
  | lam => exact h
  | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩

theorem Expr.IsValue.inPlace {v : Expr} (hv : v.IsValue) : v.inPlace.IsValue := by
  induction hv with
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam => exact .lam
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂

theorem Expr.inPlace_subst (e : Expr) (x : Nat) (v : Expr) :
    (e.subst x v).inPlace = e.inPlace.subst x v.inPlace := by
  induction e with
  | var y => simp only [Expr.subst, Expr.inPlace]; split <;> simp [Expr.inPlace]
  | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.subst, Expr.inPlace, ih]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => simp only [Expr.subst, Expr.inPlace, ih₁, ih₂]
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst, Expr.inPlace, ih₁]
    split <;> simp [ih₂]
  | lam b y _ ih =>
    simp only [Expr.subst, Expr.inPlace]
    split <;> simp [ih]

theorem Expr.inPlace_inst (e : Expr) (ms : List Mu) (τs : List Ty) :
    (e.inst ms τs).inPlace = e.inPlace.inst ms τs := by
  induction e with
  | var | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.inst, Expr.inPlace, ih]
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.inst, Expr.inPlace, ih₁, ih₂]

theorem Expr.hasLoc_inPlace (e : Expr) (l : Nat) : e.inPlace.HasLoc l ↔ e.HasLoc l := by
  induction e with
  | var | unit | int | loc => simp [Expr.inPlace, Expr.HasLoc]
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.inPlace, Expr.HasLoc, ih]
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.inPlace, Expr.HasLoc, ih₁, ih₂]

theorem Expr.NoLocs.inPlace {e : Expr} (h : e.NoLocs) : e.inPlace.NoLocs := by
  induction e with
  | var | unit | int => trivial
  | loc => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩

/-- A location in `e` is counted. -/
theorem Expr.count_pos {e : Expr} {l : Nat} (h : e.HasLoc l) : 0 < e.count l := by
  induction e with
  | var | unit | int => exact h.elim
  | loc l' => simp only [Expr.HasLoc] at h; simp [Expr.count, h]
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.count]
    rcases h with h | h
    · have := ih₁ h; omega
    · have := ih₂ h; omega

/-- What `e` makes `imm` is in it. -/
theorem Expr.ToImmOf.hasLoc {e : Expr} {l : Nat} (h : e.ToImmOf l) : e.HasLoc l := by
  induction e with
  | var | unit | int | loc | lam => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | letE _ _ _ ih₁ _ => exact .inl (ih₁ h)
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact h.imp ih₁ fun h => ih₂ h.2
  | toImm _ e ih =>
    rcases h with rfl | h
    · rfl
    · exact ih h

/-- A part of a program whose `toImm`s are unique has unique ones too, and
what it makes `imm` isn't in the rest. -/
theorem Expr.ToImmArg.count_le {e v : Expr} (h : e.ToImmArg v) (x : Nat) : v.count x ≤ e.count x := by
  induction e with
  | var | unit | int | loc | lam => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | letE _ _ _ ih₁ _ => have := ih₁ h; simp only [Expr.count]; omega
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    rcases h with h | ⟨_, h⟩
    · have := ih₁ h; simp only [Expr.count]; omega
    · have := ih₂ h; simp only [Expr.count]; omega
  | toImm _ e ih =>
    rcases h with rfl | h
    · exact Nat.le_refl _
    · exact ih h

/-- A part of a program whose `toImm`s are unique has unique ones too, and
what it makes `imm` isn't in the rest. -/
theorem Expr.UniqToImm.part {h : Heap} {e a b : Expr} (hu : e.UniqToImm h)
    (hc : ∀ l, e.count l = a.count l + b.count l) (hg : ∀ v, a.ToImmArg v → e.ToImmArg v) :
    a.UniqToImm h ∧ ∀ x, a.Frozen h x → ¬ b.HasLoc x := by
  have key : ∀ v, a.ToImmArg v → ∀ x, Region h v x → a.count x = v.count x ∧ b.count x = 0 := by
    intro v hv x hx
    have h₁ := (hu v (hg v hv) x hx).1
    have h₂ := hv.count_le x
    rw [hc x] at h₁
    omega
  refine ⟨fun v hv x hx => ⟨(key v hv x hx).1, (hu v (hg v hv) x hx).2⟩, fun x ⟨v, hv, hx⟩ hb => ?_⟩
  have := Expr.count_pos hb
  have := (key v hv x hx).2
  omega

theorem Expr.NoToImm.subst {e v : Expr} (h : e.NoToImm) (hv : v.NoToImm) (x : Nat) : (e.subst x v).NoToImm := by
  induction e with
  | var y => simp only [Expr.subst]; split; exact hv; trivial
  | unit | int | loc => trivial
  | new _ _ ih | get _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst]
    refine ⟨ih₁ h.1, ?_⟩
    split; exact h.2; exact ih₂ h.2
  | lam _ y _ ih =>
    simp only [Expr.subst]
    split; exact h; exact ih h
  | toImm => exact h.elim

theorem Expr.LamNoToImm.subst {e v : Expr} (h : e.LamNoToImm) (hv : v.NoToImm) (x : Nat) :
    (e.subst x v).LamNoToImm := by
  induction e with
  | var y => simp only [Expr.subst]; split; exact hv.lamNoToImm; trivial
  | unit | int | loc => trivial
  | new _ _ ih | get _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst]
    refine ⟨ih₁ h.1, ?_⟩
    split; exact h.2; exact ih₂ h.2
  | lam _ y _ _ =>
    simp only [Expr.subst]
    split; exact h; exact Expr.NoToImm.subst h hv x

theorem Expr.NoToImm.inst {e : Expr} (h : e.NoToImm) (ms : List Mu) (τs : List Ty) : (e.inst ms τs).NoToImm := by
  induction e with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | toImm => exact h.elim

theorem Expr.LamNoToImm.inst {e : Expr} (h : e.LamNoToImm) (ms : List Mu) (τs : List Ty) :
    (e.inst ms τs).LamNoToImm := by
  induction e with
  | var | unit | int | loc => trivial
  | lam => exact Expr.NoToImm.inst h ms τs
  | new _ _ ih | get _ ih | fst _ ih | snd _ ih | call _ _ _ _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩

theorem HeapNoToImm.append {h : Heap} (hh : HeapNoToImm h) {b : Bool} {v : Expr} (hv : v.NoToImm) :
    HeapNoToImm (h ++ [(b, v)]) := by
  intro l b' w hl
  by_cases hlt : l < h.length
  · rw [List.getElem?_append_left hlt] at hl; exact hh l b' w hl
  · have : l = h.length := by
      have := (List.getElem?_eq_some_iff.mp hl).1
      simp at this
      omega
    subst this
    rw [List.getElem?_concat_length] at hl
    cases hl
    exact hv

theorem HeapNoToImm.set {h : Heap} (hh : HeapNoToImm h) {b : Bool} {v : Expr} (hv : v.NoToImm) (l : Nat) :
    HeapNoToImm (h.set l (b, v)) := by
  intro l' b' w hl
  rw [List.getElem?_set] at hl
  split at hl
  · split at hl
    · cases hl; exact hv
    · cases hl
  · exact hh l' b' w hl

theorem HeapNoToImm.freezeAt {h : Heap} (hh : HeapNoToImm h) (m : List (Nat × Nat)) :
    HeapNoToImm (h.freezeAt m) := by
  intro l b w hl
  rw [Heap.freezeAt_get] at hl
  cases ho : h[l]? with
  | none => rw [ho] at hl; cases hl
  | some o =>
    rw [ho] at hl
    obtain ⟨b₀, w₀⟩ := o
    simp only [Option.map_some] at hl
    split at hl <;> (cases hl; exact hh l _ _ ho)

/-- A copy has no `toImm`, nor what it adds to the heap. -/
theorem Freeze.noToImm {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') : HeapNoToImm h → v.NoToImm → HeapNoToImm h' ∧ v'.NoToImm := by
  induction hc with
  | unit | int | lam | imm | seen => exact fun hh hv => ⟨hh, by first | exact hv | trivial⟩
  | cell hw _ _ ih =>
    intro hh _
    obtain ⟨hh', hw'⟩ := ih hh (hh _ _ _ hw)
    exact ⟨hh'.append hw', trivial⟩
  | pair _ _ ih₁ ih₂ =>
    intro hh hv
    obtain ⟨hh₁, hv₁⟩ := ih₁ hh hv.1
    obtain ⟨hh₂, hv₂⟩ := ih₂ hh₁ hv.2
    exact ⟨hh₂, hv₁, hv₂⟩

/-- A step keeps closures without `toImm`, and the heap without any. -/
theorem step_lamNoToImm {fns : List Fn} (hfn : ∀ fn ∈ fns, fn.body.LamNoToImm) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') : HeapNoToImm h → e.LamNoToImm → HeapNoToImm h' ∧ e'.LamNoToImm := by
  induction hs with
  | newCong _ ih | getCong _ ih | fstCong _ ih | sndCong _ ih | callCong _ ih | toImmCong _ ih => exact ih
  | setCong₁ _ ih | letCong _ ih | appCong₁ _ ih | pairCong₁ _ ih =>
    intro hh he
    obtain ⟨hh', he'⟩ := ih hh he.1
    exact ⟨hh', he', he.2⟩
  | setCong₂ _ _ ih | appCong₂ _ _ ih | pairCong₂ _ _ ih =>
    intro hh he
    obtain ⟨hh', he'⟩ := ih hh he.2
    exact ⟨hh', he.1, he'⟩
  | newVal hv => exact fun hh he => ⟨hh.append (hv.noToImm he), trivial⟩
  | getLoc hl => exact fun hh _ => ⟨hh, (hh _ _ _ hl).lamNoToImm⟩
  | setLoc _ hv => exact fun hh he => ⟨hh.set (hv.noToImm he.2) _, trivial⟩
  | @letVal x v e₂ hv => exact fun hh he => ⟨hh, he.2.subst (hv.noToImm he.1) x⟩
  | @callVal f fn ms τs v hf hv =>
    exact fun hh he => ⟨hh, ((hfn fn (List.mem_of_getElem? hf)).inst ms τs).subst (hv.noToImm he) fn.param⟩
  | @appVal b x body v hv =>
    exact fun hh he => ⟨hh, (Expr.NoToImm.subst (e := body) he.1 (hv.noToImm he.2) x).lamNoToImm⟩
  | fstVal _ _ => exact fun hh he => ⟨hh, he.1⟩
  | sndVal _ _ => exact fun hh he => ⟨hh, he.2⟩
  | toImmVal hv hc =>
    intro hh he
    obtain ⟨hh', hv'⟩ := hc.noToImm hh (hv.noToImm he)
    exact ⟨hh', hv'.lamNoToImm⟩
  | toImmKeep hv hc => exact fun hh he => ⟨(hc.noToImm hh (hv.noToImm he)).1.freezeAt _, he⟩

theorem Expr.UniqToImm.sub {h : Heap} {e a : Expr} (hu : e.UniqToImm h) (hc : ∀ l, e.count l = a.count l)
    (hg : ∀ v, a.ToImmArg v → e.ToImmArg v) : a.UniqToImm h :=
  (hu.part (b := .unit) (fun l => by simp [hc l, Expr.count]) hg).1

/-- What's beside the part that steps keeps corresponding, if the part's
`toImm`s aren't in it. -/
theorem Rel.beside {h : Heap} {ρ ρ' : Nat → Nat → Prop} {a b₁ b₂ : Expr} (hr : Rel ρ b₁.inPlace b₂)
    (hρ : ∀ x c, ρ x c → ¬ a.Frozen h x → ρ' x c) (hd : ∀ x, a.Frozen h x → ¬ b₁.HasLoc x) :
    Rel ρ' b₁.inPlace b₂ :=
  hr.mono_on fun x c hx hxc => hρ x c hxc fun hfx => hd x hfx ((Expr.hasLoc_inPlace b₁ x).mp hx)

theorem Heap.freezeAt_none {h : Heap} {m : List (Nat × Nat)} {i : Nat} (hi : m.lookup i = none) :
    (h.freezeAt m)[i]? = h[i]? := by
  rw [Heap.freezeAt_get, hi]
  cases h[i]? <;> simp

theorem Heap.freezeAt_some {h : Heap} {m : List (Nat × Nat)} {i c : Nat} (hi : m.lookup i = some c)
    {b : Bool} {w : Expr} (hw : h[i]? = some (b, w)) : (h.freezeAt m)[i]? = some (false, w) := by
  rw [Heap.freezeAt_get, hi, hw]
  simp

/-- **Freezing a region in place**: the step of `toImm` itself. The other
program makes the region's objects `imm` where they are; each copy
corresponds to its original there, and the originals to nothing, since
nothing outside the region has them. -/
theorem region_freeze_step {fns : List Fn} {h₁ h₁' : Heap} {m₁ : List (Nat × Nat)} {v v' : Expr}
    (hv : v.IsValue) (hcp : Freeze h₁ [] v h₁' m₁ v')
    {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {e₂ : Expr}
    (hh : HeapOk fns σ h₁) (ht : HasType fns σ .none false Ctx.empty (.toImm false v) t)
    (hM : ∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) (hc : Corr M ρ h₁ h₂)
    (hr : Rel ρ (Expr.toImm false v).inPlace e₂) (hu : (Expr.toImm false v).UniqToImm h₁)
    (hnh : HeapNoToImm h₁) (hln : (Expr.toImm false v).LamNoToImm) :
    ∃ ρ' h₂' e₂', Step (fns.map Fn.inPlace) h₂ e₂ h₂' e₂' ∧
      (∀ a c, ρ a c → ¬ (Expr.toImm false v).Frozen h₁ a → ρ' a c) ∧
      Corr (fun a => M a ∨ h₁.length ≤ a) ρ' h₁' h₂' ∧ Rel ρ' v'.inPlace e₂' := by
    have hvn := hv.noToImm hln
    simp only [Expr.inPlace, hvn.inPlace] at hr
    obtain ⟨_, _, _, he, hi, _⟩ := ht.toImm_inv
    cases hr with
    | @toImm _ _ v₂ hr =>
    have hf := hi.none_eq.2
    -- What's `mut` is in `M`.
    have hMt : ∀ a w, h₁[a]? = some (true, w) → M a := fun a w ha =>
      let ⟨_, hσ⟩ := hh.mut_of_tag ha; hM a _ _ hσ (by intro h; cases h)
    -- The other copies what corresponds, the same way, but goes on with the
    -- value, and makes what it copied `imm`.
    obtain ⟨ρ₁, h₂', m₂, v₂', hf₂, hρ₁, _, _, hmr, _, hnew⟩ :=
      hcp.sim (m₂ := []) hMt hc hr ⟨fun _ _ _ _ => .inl ⟨rfl, rfl⟩, fun _ _ h => by simp at h⟩
        (fun _ _ h => by simp at h)
    obtain ⟨hrv, hshape⟩ := hcp.shape hh hv he hf (MemoOk.nil σ) (MemoShape.nil _)
    have hfresh := hcp.fresh (MemoFresh.nil _)
    -- What's frozen is the value's region, which nothing else has.
    have hreg := hu v (.inl rfl)
    have hkey : ∀ {a c}, m₁.lookup a = some c → Region h₁ v a ∧ h₁.length ≤ c := fun ha =>
      match hcp.keys ha with
      | .inl h => by simp at h
      | .inr ⟨h₁, h₂, _⟩ => ⟨h₁, h₂⟩
    have hold₁ : ∀ {a}, a < h₁.length → h₁'[a]? = h₁[a]? := fun ha => hcp.keeps ha
    have hold₂ : ∀ {d}, d < h₂.length → h₂'[d]? = h₂[d]? := fun hd => hf₂.keeps hd
    let H₂ := h₂'.freezeAt m₂
    -- A copy corresponds to what its original did; the original, to nothing.
    let ρ' : Nat → Nat → Prop := fun a d => (ρ a d ∧ ¬ Region h₁ v a) ∨ ∃ a₀, m₁.lookup a₀ = some a ∧ ρ a₀ d
    have toρ' : ∀ {a c d}, CopyCorr h₁' m₁ a c → ρ a d → ρ' c d := by
      intro a c d hac had
      rcases hac with ⟨⟨w, hw⟩, rfl⟩ | hac
      · refine .inl ⟨had, fun hr => ?_⟩
        obtain ⟨w', hw'⟩ := hr.mut
        rw [hold₁ (hc.lt₁ had), hw'] at hw
        cases hw
      · exact .inr ⟨a, hac, had⟩
    have hrel : ∀ {x x' y}, Rel (CopyCorr h₁' m₁) x x' → Rel ρ x y → Rel ρ' x' y := fun h₁ h₂ =>
      (h₁.comp h₂).mono fun _ _ ⟨_, hac, had⟩ => toρ' hac had
    -- What the other copies corresponds to what this copies.
    have hfrozen : ∀ {a₀ c d}, m₁.lookup a₀ = some c → ρ a₀ d → ∃ c₂, m₂.lookup d = some c₂ := by
      intro a₀ c d hc₀ had
      obtain ⟨w, hw⟩ := (hkey hc₀).1.mut
      rcases hmr.1 a₀ d (hρ₁ _ _ had) (.inl (hMt _ _ hw)) with ⟨h, _⟩ | ⟨_, c₂, _, h, _⟩
      · rw [hc₀] at h; cases h
      · exact ⟨c₂, h⟩
    have hnotfrozen : ∀ {a d}, ρ a d → ¬ Region h₁ v a → m₂.lookup d = none := by
      intro a d had hna
      cases hd : m₂.lookup d with
      | none => rfl
      | some c₂ =>
        obtain ⟨l₁, _, hl₁, hM₁, hk₁⟩ := hmr.2 d c₂ hd
        have hl₁' : ρ l₁ d := by
          rcases hnew l₁ d hl₁ with h | ⟨_, h⟩
          · exact h
          · exact absurd (hc.lt₂ had) (by omega)
        rcases hM₁ with hM₁ | hM₁
        · exact absurd ((hc.inj l₁ a d hl₁' had hM₁) ▸ (hkey hk₁).1) hna
        · exact absurd (hc.lt₁ hl₁') (by omega)
    refine ⟨ρ', H₂, v₂, .toImmKeep (hr.isValue hv) hf₂,
      fun a c h hn => .inl ⟨h, fun hx => hn ⟨v, .inl rfl, hx⟩⟩, ⟨?_, ?_, ?_⟩, ?_⟩
    · intro a d had
      rcases had with ⟨had, hna⟩ | ⟨a₀, hc₀, had⟩
      · -- Outside the region, as it was.
        obtain ⟨b, x, y, h₁x, h₂y, hxy⟩ := hc.heap a d had
        refine ⟨b, x, y, by rw [hold₁ (hc.lt₁ had)]; exact h₁x, ?_, ?_⟩
        · show (h₂'.freezeAt m₂)[d]? = _
          rw [Heap.freezeAt_none (hnotfrozen had hna), hold₂ (hc.lt₂ had)]
          exact h₂y
        · refine hxy.mono_on fun z e hz hze => .inl ⟨hze, fun hrz => hna ?_⟩
          exact (hreg z hrz).2 a b x h₁x hz
      · -- A copy, against the original made `imm`.
        obtain ⟨w, w', hw, hw', hww⟩ := hshape a₀ a hc₀
        obtain ⟨b₀, w₀, w₂, h₁w, h₂w, hw₀₂⟩ := hc.heap a₀ d had
        rw [← hold₁ (hc.lt₁ had), hw] at h₁w
        cases h₁w
        obtain ⟨c₂, hc₂⟩ := hfrozen hc₀ had
        refine ⟨false, w', w₂, hw', ?_, hrel hww hw₀₂⟩
        exact Heap.freezeAt_some hc₂ (by rw [hold₂ (hc.lt₂ had)]; exact h₂w)
    · intro a b d had hbd hMa
      rcases had with ⟨had, hna⟩ | ⟨a₀, ha₀, had⟩ <;> rcases hbd with ⟨hbd, hnb⟩ | ⟨b₀, hb₀, hbd⟩
      · exact hMa.elim (hc.inj a b d had hbd) fun hle => absurd (hc.lt₁ had) (by omega)
      · obtain ⟨w, hw⟩ := (hkey hb₀).1.mut
        exact absurd ((hc.inj b₀ a d hbd had (hMt _ _ hw)) ▸ (hkey hb₀).1) hna
      · obtain ⟨w, hw⟩ := (hkey ha₀).1.mut
        exact absurd ((hc.inj a₀ b d had hbd (hMt _ _ hw)) ▸ (hkey ha₀).1) hnb
      · obtain ⟨w, hw⟩ := (hkey ha₀).1.mut
        have := hc.inj a₀ b₀ d had hbd (hMt _ _ hw)
        subst this
        rw [ha₀] at hb₀
        cases hb₀
        rfl
    · intro a d d' had had' hMa
      rcases had with ⟨had, _⟩ | ⟨a₀, ha₀, had⟩ <;> rcases had' with ⟨had', _⟩ | ⟨a₀', ha₀', had'⟩
      · exact hMa.elim (hc.func a d d' had had') fun hle => absurd (hc.lt₁ had) (by omega)
      · exact absurd (hkey ha₀').2 (by have := hc.lt₁ had; omega)
      · exact absurd (hkey ha₀).2 (by have := hc.lt₁ had'; omega)
      · have := hfresh.2 a₀ a₀' a ha₀ ha₀'
        subst this
        obtain ⟨w, hw⟩ := (hkey ha₀).1.mut
        exact hc.func a₀ d d' had had' (hMt _ _ hw)
    · rw [(hcp.noToImm hnh hvn).2.inPlace]
      exact hrel hrv hr

/-- One step of the program that copies is one of the one whose `toImm`s are in
place, and they still correspond, but for what the step froze: the copy
corresponds to what the original did, and the original, which nothing
has any more, to nothing. -/
theorem step_unique {fns : List Fn} (hf : FnsOk fns) {h₁ h₁' : Heap} {e₁ e₁' : Expr}
    (hs : Step fns h₁ e₁ h₁' e₁') :
    ∀ {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {e₂ : Expr},
      HeapOk fns σ h₁ → HasType fns σ .none false Ctx.empty e₁ t →
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) → Corr M ρ h₁ h₂ → Rel ρ e₁.inPlace e₂ →
      e₁.UniqToImm h₁ → HeapNoToImm h₁ → e₁.LamNoToImm →
      ∃ ρ' h₂' e₂', Step (fns.map Fn.inPlace) h₂ e₂ h₂' e₂' ∧ (∀ a c, ρ a c → ¬ e₁.Frozen h₁ a → ρ' a c) ∧
        Corr (fun a => M a ∨ h₁.length ≤ a) ρ' h₁' h₂' ∧ Rel ρ' e₁'.inPlace e₂' := by
  induction hs with
  | newCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    obtain ⟨_, he, _, _⟩ := ht.new_inv
    cases hr with
    | new hr =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ :=
        ih hh he hM hc hr (hu.sub (fun _ => rfl) (fun _ h => h)) hnh hln
      exact ⟨ρ', h₂', _, .newCong s₂, hρ, hc', .new hr'⟩
  | @newVal e b hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace, (hv.noToImm hln).inPlace] at hr
    cases hr with
    | @new _ _ e' hr =>
      refine ⟨_, h₂ ++ [(b, e')], .loc h₂.length, .newVal (hr.isValue hv), fun _ _ h _ => .inl h, hc.alloc hr,
        .loc (.inr ⟨rfl, rfl⟩)⟩
  | getCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    obtain ⟨_, _, he, _⟩ := ht.get_inv
    cases hr with
    | get hr =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ :=
        ih hh he hM hc hr (hu.sub (fun _ => rfl) (fun _ h => h)) hnh hln
      exact ⟨ρ', h₂', _, .getCong s₂, hρ, hc', .get hr'⟩
  | @getLoc l b v hl =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    cases hr with
    | get hr =>
      cases hr with
      | loc hρ =>
        obtain ⟨b', v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap _ _ hρ
        rw [hl] at h₁v
        cases h₁v
        rw [(hnh _ _ _ hl).inPlace]
        exact ⟨ρ, h₂, v₂, .getLoc h₂v, fun _ _ h _ => h, hc.widen, hrel⟩
  | @setCong₁ _ a a' b s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hua, hd⟩ := hu.part (a := a) (b := b) (fun _ => rfl) (fun _ h => .inl h)
    obtain ⟨_, he₁, _, _⟩ := ht.set_inv
    cases hr with
    | set hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁ hua hnh hln.1
      exact ⟨ρ', h₂', _, .setCong₁ s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inl h), hc',
        .set hr' (hr₂.beside hρ hd)⟩
  | @setCong₂ _ a b b' hv s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hub, hd⟩ := hu.part (a := b) (b := a) (fun _ => by simp only [Expr.count]; omega) (fun _ h => .inr ⟨hv, h⟩)
    obtain ⟨_, _, he₂, _⟩ := ht.set_inv
    cases hr with
    | set hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂ hub hnh hln.2
      exact ⟨ρ', h₂', _, .setCong₂ (hr₁.isValue hv.inPlace) s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inr ⟨hv, h⟩),
        hc', .set (hr₁.beside hρ hd) hr'⟩
  | @setLoc l₀ b w v hl hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace, (hv.noToImm hln.2).inPlace] at hr
    obtain ⟨_, he₁, _, _⟩ := ht.set_inv
    obtain ⟨_, hl', hσ⟩ := canonical_mut_cell .loc he₁
    cases hl'
    · have hMl : M l₀ := hM _ _ _ hσ (by intro h; cases h)
      cases hr with
      | @set _ _ _ v' hr₁ hr₂ =>
        cases hr₁ with
        | @loc _ c hρ =>
          have hc₂ : c < h₂.length := hc.lt₂ hρ
          have hl₀ : l₀ < h₁.length := hc.lt₁ hρ
          obtain ⟨b₀, w₁, w₂, h₁w, h₂w, _⟩ := hc.heap _ _ hρ
          rw [hl] at h₁w
          cases h₁w
          refine ⟨ρ, h₂.set c (b, v'), .unit, .setLoc h₂w (hr₂.isValue hv), fun _ _ h _ => h, ?_, .unit⟩
          have key : Corr M ρ (h₁.set l₀ (b, v)) (h₂.set c (b, v')) := by
            refine ⟨?_, hc.inj, hc.func⟩
            intro a d had
            obtain ⟨b', v₁, v₂, h₁v, h₂v, hrel⟩ := hc.heap a d had
            by_cases ha : a = l₀
            · subst ha
              have hd : d = c := hc.func _ _ _ had hρ hMl
              subst hd
              exact ⟨b, v, v', List.getElem?_set_self hl₀, List.getElem?_set_self hc₂, hr₂⟩
            · have hd : d ≠ c := fun hd => ha (hc.inj l₀ a c hρ (hd ▸ had) hMl).symm
              refine ⟨b', v₁, v₂, ?_, ?_, hrel⟩
              · rw [List.getElem?_set_ne (Ne.symm ha)]
                exact h₁v
              · rw [List.getElem?_set_ne (Ne.symm hd)]
                exact h₂v
          have := key.widen
          simp only [List.length_set] at this
          exact this
  | @letCong _ x a a' b s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hua, hd⟩ := hu.part (a := a) (b := b) (fun _ => rfl) (fun _ h => h)
    obtain ⟨_, _, he₁, _, _⟩ := ht.letE_inv
    cases hr with
    | letE hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁ hua hnh hln.1
      exact ⟨ρ', h₂', _, .letCong s₂, fun x c h hn => hρ x c h hn, hc',
        .letE hr' (hr₂.beside hρ hd)⟩
  | @letVal x v e₂₀ hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    cases hr with
    | letE hr₁ hr₂ =>
      refine ⟨ρ, h₂, _, .letVal (hr₁.isValue hv.inPlace), fun _ _ h _ => h, hc.widen, ?_⟩
      rw [Expr.inPlace_subst]
      exact hr₂.subst hr₁ x
  | callCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    obtain ⟨_, _, _, he, _⟩ := ht.call_inv
    cases hr with
    | call hr =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ :=
        ih hh he hM hc hr (hu.sub (fun _ => rfl) (fun _ h => h)) hnh hln
      exact ⟨ρ', h₂', _, .callCong s₂, hρ, hc', .call hr'⟩
  | @callVal f fn ms τs v hfn hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    cases hr with
    | call hr =>
      -- The other run calls the function with its `toImm`s in place.
      have hfn' : (fns.map Fn.inPlace)[f]? = some fn.inPlace := by simp [hfn]
      have hb := (((hf fn (List.mem_of_getElem? hfn)).1).inst ms τs).inPlace
      have hbody : Rel ρ (fn.body.inst ms τs).inPlace (fn.body.inst ms τs).inPlace :=
        Rel.refl fun l hl => (hb.not_hasLoc l hl).elim
      refine ⟨ρ, h₂, _, .callVal hfn' (hr.isValue hv.inPlace), fun _ _ h _ => h, hc.widen, ?_⟩
      rw [Expr.inPlace_subst]
      have := hbody.subst hr fn.param
      rw [Expr.inPlace_inst] at this ⊢
      exact this
  | @appCong₁ _ a a' b s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hua, hd⟩ := hu.part (a := a) (b := b) (fun _ => rfl) (fun _ h => .inl h)
    obtain ⟨_, _, _, he₁, _, _⟩ := ht.app_inv
    cases hr with
    | app hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁ hua hnh hln.1
      exact ⟨ρ', h₂', _, .appCong₁ s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inl h), hc',
        .app hr' (hr₂.beside hρ hd)⟩
  | @appCong₂ _ a b b' hv s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hub, hd⟩ := hu.part (a := b) (b := a) (fun _ => by simp only [Expr.count]; omega) (fun _ h => .inr ⟨hv, h⟩)
    obtain ⟨_, _, _, _, he₂, _⟩ := ht.app_inv
    cases hr with
    | app hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂ hub hnh hln.2
      exact ⟨ρ', h₂', _, .appCong₂ (hr₁.isValue hv.inPlace) s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inr ⟨hv, h⟩),
        hc', .app (hr₁.beside hρ hd) hr'⟩
  | @appVal b x body v hv =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    cases hr with
    | app hr₁ hr₂ =>
      cases hr₁ with
      | lam hbody =>
        refine ⟨ρ, h₂, _, .appVal (hr₂.isValue hv.inPlace), fun _ _ h _ => h, hc.widen, ?_⟩
        rw [Expr.inPlace_subst]
        exact hbody.subst hr₂ x
  | @pairCong₁ _ a a' b s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hua, hd⟩ := hu.part (a := a) (b := b) (fun _ => rfl) (fun _ h => .inl h)
    obtain ⟨_, _, he₁, _, _⟩ := ht.pair_inv
    cases hr with
    | pair hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₁ hM hc hr₁ hua hnh hln.1
      exact ⟨ρ', h₂', _, .pairCong₁ s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inl h), hc',
        .pair hr' (hr₂.beside hρ hd)⟩
  | @pairCong₂ _ a b b' hv s ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    have ⟨hub, hd⟩ := hu.part (a := b) (b := a) (fun _ => by simp only [Expr.count]; omega) (fun _ h => .inr ⟨hv, h⟩)
    obtain ⟨_, _, _, he₂, _⟩ := ht.pair_inv
    cases hr with
    | pair hr₁ hr₂ =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ := ih hh he₂ hM hc hr₂ hub hnh hln.2
      exact ⟨ρ', h₂', _, .pairCong₂ (hr₁.isValue hv.inPlace) s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inr ⟨hv, h⟩),
        hc', .pair (hr₁.beside hρ hd) hr'⟩
  | fstCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    obtain ⟨_, _, he, _⟩ := ht.fst_inv
    cases hr with
    | fst hr =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ :=
        ih hh he hM hc hr (hu.sub (fun _ => rfl) (fun _ h => h)) hnh hln
      exact ⟨ρ', h₂', _, .fstCong s₂, hρ, hc', .fst hr'⟩
  | sndCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    obtain ⟨_, _, he, _⟩ := ht.snd_inv
    cases hr with
    | snd hr =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ :=
        ih hh he hM hc hr (hu.sub (fun _ => rfl) (fun _ h => h)) hnh hln
      exact ⟨ρ', h₂', _, .sndCong s₂, hρ, hc', .snd hr'⟩
  | fstVal hv₁ hv₂ =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    cases hr with
    | fst hr =>
      cases hr with
      | pair hr₁ hr₂ =>
        exact ⟨ρ, h₂, _, .fstVal (hr₁.isValue hv₁.inPlace) (hr₂.isValue hv₂.inPlace), fun _ _ h _ => h,
          hc.widen, hr₁⟩
  | sndVal hv₁ hv₂ =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    cases hr with
    | snd hr =>
      cases hr with
      | pair hr₁ hr₂ =>
        exact ⟨ρ, h₂, _, .sndVal (hr₁.isValue hv₁.inPlace) (hr₂.isValue hv₂.inPlace), fun _ _ h _ => h,
          hc.widen, hr₂⟩
  | toImmCong _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    simp only [Expr.inPlace] at hr
    obtain ⟨_, _, _, he, _, _⟩ := ht.toImm_inv
    cases hr with
    | toImm hr =>
      obtain ⟨ρ', h₂', e₂', s₂, hρ, hc', hr'⟩ :=
        ih hh he hM hc hr (hu.sub (fun _ => rfl) (fun _ h => .inr h)) hnh hln
      exact ⟨ρ', h₂', _, .toImmCong s₂, fun x c h hn => hρ x c h fun h' => hn (h'.of fun _ h => .inr h), hc', .toImm hr'⟩
  | toImmVal hv hcp =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    exact region_freeze_step hv hcp hh ht hM hc hr hu hnh hln
  | toImmKeep _ _ => intro σ t M ρ h₂ e₂ hh ht; exact nomatch ht.toImm_inv.1

/-- However many steps the program that copies takes, the one whose `toImm`s are
in place takes them too, and they still correspond, if what's frozen is
unique whenever the program gets there. -/
theorem steps_unique {fns : List Fn} (hf : FnsOk fns) (hfn : ∀ fn ∈ fns, fn.body.LamNoToImm) {h₁ h₁' : Heap}
    {e₁ e₁' : Expr} (hs : Steps fns h₁ e₁ h₁' e₁') :
    ∀ {σ : HeapTy} {t : Ty} {M : Nat → Prop} {ρ : Nat → Nat → Prop} {h₂ : Heap} {e₂ : Expr},
      HeapOk fns σ h₁ → HasType fns σ .none false Ctx.empty e₁ t →
      (∀ a m t', σ[a]? = some (m, t') → m ≠ .immutable → M a) → Corr M ρ h₁ h₂ → Rel ρ e₁.inPlace e₂ →
      (∀ σ' h e, Steps fns h₁ e₁ h e → HeapOk fns σ' h → HasType fns σ' .none false Ctx.empty e t →
        e.UniqToImm h) →
      HeapNoToImm h₁ → e₁.LamNoToImm →
      ∃ (M' : Nat → Prop) (ρ' : Nat → Nat → Prop) (h₂' : Heap) (e₂' : Expr),
        Steps (fns.map Fn.inPlace) h₂ e₂ h₂' e₂' ∧ Corr M' ρ' h₁' h₂' ∧ Rel ρ' e₁'.inPlace e₂' := by
  induction hs with
  | refl =>
    intro σ t M ρ h₂ e₂ _ _ _ hc hr _ _ _
    exact ⟨M, ρ, h₂, e₂, .refl, hc, hr⟩
  | @step h₀ _ _ _ _ _ s _ ih =>
    intro σ t M ρ h₂ e₂ hh ht hM hc hr hu hnh hln
    obtain ⟨ρ', h₂', e₂', s₂, _, hc', hr'⟩ :=
      step_unique hf s hh ht hM hc hr (hu σ _ _ .refl hh ht) hnh hln
    obtain ⟨σ', hx, hh', ht'⟩ := preservation hf s hh ht
    obtain ⟨hnh', hln'⟩ := step_lamNoToImm hfn s hnh hln
    -- What isn't `imm` after the step wasn't before, or is new.
    have hM' : ∀ a m t', σ'[a]? = some (m, t') → m ≠ .immutable → M a ∨ h₀.length ≤ a := by
      intro a m t' ha hm
      by_cases hlt : a < h₀.length
      · have hσ : a < σ.length := hh.1 ▸ hlt
        have hp : σ[a]? = some σ[a] := List.getElem?_eq_getElem hσ
        rw [hx a _ hp] at ha
        exact .inl (hM a m t' (hp.trans ha) hm)
      · exact .inr (by omega)
    obtain ⟨M'', ρ'', h₂'', e₂'', ss, hc'', hr''⟩ :=
      ih hh' ht' hM' hc' hr' (fun σ'' h e hs' hh'' ht'' => hu σ'' h e (.step s hs') hh'' ht'') hnh' hln'
    exact ⟨M'', ρ'', h₂'', e₂'', .step s₂ ss, hc'', hr''⟩

/-- **Every unique `toImm` can be in place.** The program, with every `toImm`
in place in it and in its functions, computes what it computes: whatever
result the program that copies gets, the other gets one that corresponds,
and the same integer. The premise is that whenever the program reaches a
`toImm`, the region of what it makes `imm` is its own (`Expr.UniqToImm`):
the program has its objects only there, and no object outside it holds
one. No closure has a `toImm`. -/
theorem toImms_in_place_change_nothing {fns : List Fn} (hf : FnsOk fns) (hfn : ∀ fn ∈ fns, fn.body.LamNoToImm)
    {σ : HeapTy} {h h₁ : Heap} {e e₁ : Expr} {T : Ty} (hh : HeapOk fns σ h)
    (ht : HasType fns σ .none false Ctx.empty e T) (hnh : HeapNoToImm h) (hln : e.LamNoToImm)
    (hu : ∀ σ' h' e', Steps fns h e h' e' → HeapOk fns σ' h' → HasType fns σ' .none false Ctx.empty e' T →
      e'.UniqToImm h')
    (hs : Steps fns h e h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps (fns.map Fn.inPlace) h e.inPlace h₂ e₂ ∧ Rel ρ e₁.inPlace e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n := by
  -- At first, each location corresponds to itself.
  let ρ : Nat → Nat → Prop := fun a c => a = c ∧ a < h.length
  have hself : ∀ a, a < h.length → ρ a a := fun a ha => ⟨rfl, ha⟩
  have hc : Corr (fun a => ∃ m t', σ[a]? = some (m, t') ∧ m ≠ .immutable) ρ h h := by
    refine ⟨?_, ?_, ?_⟩
    · intro a c ⟨hac, ha⟩
      subst hac
      obtain ⟨⟨b, w⟩, hw⟩ : ∃ o, h[a]? = some o := ⟨_, List.getElem?_eq_getElem ha⟩
      exact ⟨b, w, w, hw, hw, hh.rel_self hself hw⟩
    · intro a b c ⟨h₁, _⟩ ⟨h₂, _⟩ _
      rw [h₁, h₂]
    · intro a c d ⟨h₁, _⟩ ⟨h₂, _⟩ _
      rw [← h₁, ← h₂]
  have hr : Rel ρ e.inPlace e.inPlace :=
    Rel.refl fun l hl => ⟨rfl, by rw [hh.1]; exact ht.hasLoc_lt ((Expr.hasLoc_inPlace e l).mp hl)⟩
  obtain ⟨_, ρ', h₂, e₂, ss, _, hr'⟩ :=
    steps_unique hf hfn hs hh ht (fun a m t' h hm => ⟨m, t', h, hm⟩) hc hr hu hnh hln
  refine ⟨ρ', h₂, e₂, ss, hr', fun n hn => ?_⟩
  subst hn
  exact hr'.int_eq

end BoltMut
