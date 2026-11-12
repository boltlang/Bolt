import BoltMut.Unique

/-!
# Checking that `toImm`s are unique (#284)

`toImms_in_place_change_nothing` needs what a program makes `imm` to be unique
whenever it gets there. Here, a check of the program as written gives
that: a variable that holds a new `mut` object, `let x = mut [..] in b`,
can be made `imm` in place in `b` if `b` only reads and writes `x`, or
gives it to functions that borrow it, before its last use, the `toImm`,
and doesn't let anything else have it: not another name, an object, a
closure, or a function that keeps it.

A function borrows its argument if its body only reads and writes it, or
gives it to functions that borrow it too (`Borrows`). It can borrow all of
it, or a part of a tuple, like `xs` in `push((xs, 1))`, which its body
reads and writes through `fst` or `snd` (a `Path`). The functions that do
are a set `B` that the check is given, so they can call each other. `B`
also has the functions that give back a new `mut` object: called, their
body is `mut [e₀]` (`Returns`), so `let x = f(..) in b` owns what it gets,
like `let x = mut [..] in b`.

The check is about the program with each such `x` replaced with its
object's location, which is what running it does: `Con B l e` says that
`e` consumes `l` that way, `Bor B l e` that it only borrows it, and `Ok`
that every `toImm` is of such a variable.
-/

namespace BoltMut

/-- Where a function's argument has what it borrows: all of it, or a part of
a tuple, like `xs` in `push((xs, 1))`. -/
inductive Pos where
  | whole
  | fst
  | snd
  /-- Not an argument: the function gives back a new object (`Returns`). -/
  | ret

-- `B f p`: the function `f` borrows its argument at `p`, which it only
-- reads and writes, and doesn't keep.
variable {B : Nat → Pos → Prop}

/-! ## Free variables -/

/-- `x` is free in `e`. -/
def Expr.Free (x : Nat) : Expr → Prop
  | .var y => y = x
  | .unit | .int _ | .loc _ => False
  | .new _ e | .get e | .call _ _ _ e | .fst e | .snd e | .toImm _ e => e.Free x
  | .set a b | .app a b | .pair a b => a.Free x ∨ b.Free x
  | .letE y a b => a.Free x ∨ (y ≠ x ∧ b.Free x)
  | .lam _ y e => y ≠ x ∧ e.Free x

/-- `e` has no free variable. -/
def Expr.NoFree (e : Expr) : Prop := ∀ x, ¬ e.Free x

theorem Expr.subst_of_not_free {e : Expr} {x : Nat} (h : ¬ e.Free x) (v : Expr) : e.subst x v = e := by
  induction e with
  | var y =>
    simp only [Expr.Free] at h
    simp [Expr.subst, h]
  | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.subst, ih h]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.Free, not_or] at h
    simp only [Expr.subst, ih₁ h.1, ih₂ h.2]
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.Free, not_or, not_and] at h
    simp only [Expr.subst, ih₁ h.1]
    split
    · rfl
    · rename_i hyx
      rw [ih₂ (h.2 hyx)]
  | lam b y _ ih =>
    simp only [Expr.Free, not_and] at h
    simp only [Expr.subst]
    split
    · rfl
    · rename_i hyx
      rw [ih (h hyx)]

/-- Substitutions of different variables, by values without variables,
commute. -/
theorem Expr.subst_comm (e : Expr) {x y : Nat} {v w : Expr} (hxy : x ≠ y) (hv : v.NoFree) (hw : w.NoFree) :
    (e.subst x v).subst y w = (e.subst y w).subst x v := by
  induction e with
  | var z =>
    by_cases hzx : z = x
    · subst hzx
      simp [Expr.subst, hxy, Expr.subst_of_not_free (hv y)]
    · by_cases hzy : z = y
      · subst hzy
        simp [Expr.subst, hzx, Expr.subst_of_not_free (hw x)]
      · simp [Expr.subst, hzx, hzy]
  | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.subst, ih]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => simp only [Expr.subst, ih₁, ih₂]
  | letE z _ _ ih₁ ih₂ =>
    simp only [Expr.subst, ih₁]
    by_cases hzx : z = x <;> by_cases hzy : z = y <;> simp [hzx, hzy, ih₂] <;> omega
  | lam b z _ ih =>
    simp only [Expr.subst]
    by_cases hzx : z = x <;> by_cases hzy : z = y <;> simp [hzx, hzy, ih] <;> omega

/-- A variable that isn't in scope isn't free in what's typed. -/
theorem HasType.not_free {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Γ e t) : ∀ x, Γ x = none → ¬ e.Free x := by
  induction ht with
  | var h | varSub h _ =>
    intro x hx hf
    simp only [Expr.Free] at hf
    subst hf
    rw [hx] at h
    cases h
  | unit | int | loc | locSub => intro _ _ h; exact h
  | new _ _ ih | get _ ih | call _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih | sub _ _ ih => exact ih
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    intro x hx hf
    exact hf.elim (ih₁ x hx) (ih₂ x hx)
  | @letE _ _ y _ _ _ _ _ _ ih₁ ih₂ =>
    intro x hx hf
    rcases hf with hf | ⟨hyx, hf⟩
    · exact ih₁ x hx hf
    · exact ih₂ x (by simp [Ctx.update, Ne.symm hyx, hx]) hf
  | @lam _ _ Γc _ y _ _ _ hc _ _ ih =>
    intro x hx hf
    obtain ⟨hyx, hf⟩ := hf
    refine ih x ?_ hf
    simp only [Ctx.update, Ne.symm hyx, ite_false]
    cases h : Γc x with
    | none => rfl
    | some s => rw [hc x s h] at hx; cases hx

theorem HasType.noFree {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {e : Expr} {t : Ty}
    (ht : HasType fns σ Δ io Ctx.empty e t) : e.NoFree :=
  fun x => ht.not_free x rfl

theorem Expr.subst_inst (e : Expr) (x : Nat) (v : Expr) (ms : List Mu) (τs : List Ty) :
    (e.subst x v).inst ms τs = (e.inst ms τs).subst x (v.inst ms τs) := by
  induction e with
  | var y => simp only [Expr.subst]; split <;> simp [Expr.inst, Expr.subst, *]
  | unit | int | loc => rfl
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.subst, Expr.inst, ih]
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => simp only [Expr.subst, Expr.inst, ih₁, ih₂]
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst, Expr.inst, ih₁]
    split <;> simp [ih₂]
  | lam b y _ ih =>
    simp only [Expr.subst, Expr.inst]
    split <;> simp [ih]

/-- Substituting doesn't remove locations. -/
theorem Expr.hasLoc_subst_of {e : Expr} {x : Nat} {v : Expr} {l : Nat} (h : e.HasLoc l) :
    (e.subst x v).HasLoc l := by
  induction e with
  | var | unit | int => exact h.elim
  | loc => exact h
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact h.imp ih₁ ih₂
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst, Expr.HasLoc]
    rcases h with h | h
    · exact .inl (ih₁ h)
    · split
      · exact .inr h
      · exact .inr (ih₂ h)
  | lam b y _ ih =>
    simp only [Expr.subst, Expr.HasLoc]
    split
    · exact h
    · exact ih h

/-! ## The check -/

/-- `e` is `l`, or a projection of a tuple that holds it, `fst (l, w)`: what a
function that borrows part of its argument reads and writes. -/
inductive Path (l : Nat) : Expr → Prop where
  | loc : Path l (.loc l)
  | fst {w : Expr} : w.IsValue → ¬ w.HasLoc l → Path l (.fst (.pair (.loc l) w))
  | snd {w : Expr} : w.IsValue → ¬ w.HasLoc l → Path l (.snd (.pair w (.loc l)))

/-- `e` only borrows `l`: it reads it, `get a`, writes to it, `set a b`,
with `a` a `Path` to it, or gives it to a function that borrows it, whole
or as part of a tuple, and has it nowhere else, so what it computes doesn't
hold `l`. -/

inductive Bor (B : Nat → Pos → Prop) (l : Nat) : Expr → Prop where
  | none {e : Expr} : ¬ e.HasLoc l → Bor B l e
  | getP {a : Expr} : Path l a → Bor B l (.get a)
  | setP {a b : Expr} : Path l a → Bor B l b → Bor B l (.set a b)
  /-- The other part of a tuple that holds `l`. -/
  | fstD {w : Expr} : w.IsValue → ¬ w.HasLoc l → Bor B l (.fst (.pair w (.loc l)))
  | sndD {w : Expr} : w.IsValue → ¬ w.HasLoc l → Bor B l (.snd (.pair (.loc l) w))
  | new {b : Bool} {e : Expr} : Bor B l e → Bor B l (.new b e)
  | get {e : Expr} : Bor B l e → Bor B l (.get e)
  | call {f : Nat} {ms : List Mu} {τs : List Ty} {e : Expr} : Bor B l e → Bor B l (.call f ms τs e)
  | fst {e : Expr} : Bor B l e → Bor B l (.fst e)
  | snd {e : Expr} : Bor B l e → Bor B l (.snd e)
  | toImm {b : Bool} {e : Expr} : Bor B l e → Bor B l (.toImm b e)
  /-- A call of a function that borrows its argument, with `l`. -/
  | callL {f : Nat} {ms : List Mu} {τs : List Ty} {a : Expr} : B f .whole → Path l a → Bor B l (.call f ms τs a)
  /-- A call with `l` in a tuple, at the part the function borrows. -/
  | callT₁ {f : Nat} {ms : List Mu} {τs : List Ty} {w : Expr} : B f .fst → w.IsValue → ¬ w.HasLoc l →
      Bor B l (.call f ms τs (.pair (.loc l) w))
  | callT₂ {f : Nat} {ms : List Mu} {τs : List Ty} {w : Expr} : B f .snd → w.IsValue → ¬ w.HasLoc l →
      Bor B l (.call f ms τs (.pair w (.loc l)))
  | set {a b : Expr} : Bor B l a → Bor B l b → Bor B l (.set a b)
  | letE {x : Nat} {a b : Expr} : Bor B l a → Bor B l b → Bor B l (.letE x a b)
  | app {a b : Expr} : Bor B l a → Bor B l b → Bor B l (.app a b)
  | pair {a b : Expr} : Bor B l a → Bor B l b → Bor B l (.pair a b)

/-- `e` consumes `l`: it borrows it until it makes it `imm`, once, which is its
last use. A `let` can name it, `let x = loc l in b`, and then `b` consumes
it. -/
inductive Con (B : Nat → Pos → Prop) (l : Nat) : Expr → Prop where
  | toImm : Con B l (.toImm false (.loc l))
  | toImmC {b : Bool} {e : Expr} : Con B l e → Con B l (.toImm b e)
  | new {b : Bool} {e : Expr} : Con B l e → Con B l (.new b e)
  | get {e : Expr} : Con B l e → Con B l (.get e)
  | call {f : Nat} {ms : List Mu} {τs : List Ty} {e : Expr} : Con B l e → Con B l (.call f ms τs e)
  | fst {e : Expr} : Con B l e → Con B l (.fst e)
  | snd {e : Expr} : Con B l e → Con B l (.snd e)
  | set₁ {a b : Expr} : Con B l a → ¬ b.HasLoc l → Con B l (.set a b)
  | set₂ {a b : Expr} : Bor B l a → Con B l b → Con B l (.set a b)
  | let₁ {x : Nat} {a b : Expr} : Con B l a → ¬ b.HasLoc l → Con B l (.letE x a b)
  | let₂ {x : Nat} {a b : Expr} : Bor B l a → Con B l b → Con B l (.letE x a b)
  | bind {x : Nat} {b : Expr} : ¬ b.HasLoc l → Con B l (b.subst x (.loc l)) → Con B l (.letE x (.loc l) b)
  | app₁ {a b : Expr} : Con B l a → ¬ b.HasLoc l → Con B l (.app a b)
  | app₂ {a b : Expr} : Bor B l a → Con B l b → Con B l (.app a b)
  | pair₁ {a b : Expr} : Con B l a → ¬ b.HasLoc l → Con B l (.pair a b)
  | pair₂ {a b : Expr} : Bor B l a → Con B l b → Con B l (.pair a b)

/-- **Every `toImm` is unique.** Every `toImm` is of a location in `L`, or of
a variable that holds a new `mut` object, `let x = mut [..] in b`, where
`b` consumes it: for whatever location the object gets, at least `n`. A
closure has no `toImm`. -/
inductive Ok (B : Nat → Pos → Prop) : Nat → (Nat → Prop) → Expr → Prop where
  | var {n : Nat} {L : Nat → Prop} {x : Nat} : Ok B n L (.var x)
  | unit {n : Nat} {L : Nat → Prop} : Ok B n L .unit
  | int {n : Nat} {L : Nat → Prop} {i : Int} : Ok B n L (.int i)
  | loc {n : Nat} {L : Nat → Prop} {l : Nat} : Ok B n L (.loc l)
  | lam {n : Nat} {L : Nat → Prop} {b : Bool} {x : Nat} {e : Expr} : e.NoToImm → Ok B n L (.lam b x e)
  | new {n : Nat} {L : Nat → Prop} {b : Bool} {e : Expr} : Ok B n L e → Ok B n L (.new b e)
  | get {n : Nat} {L : Nat → Prop} {e : Expr} : Ok B n L e → Ok B n L (.get e)
  | call {n : Nat} {L : Nat → Prop} {f : Nat} {ms : List Mu} {τs : List Ty} {e : Expr} :
      Ok B n L e → Ok B n L (.call f ms τs e)
  | fst {n : Nat} {L : Nat → Prop} {e : Expr} : Ok B n L e → Ok B n L (.fst e)
  | snd {n : Nat} {L : Nat → Prop} {e : Expr} : Ok B n L e → Ok B n L (.snd e)
  | set {n : Nat} {L : Nat → Prop} {a b : Expr} : Ok B n L a → Ok B n L b → Ok B n L (.set a b)
  | letE {n : Nat} {L : Nat → Prop} {x : Nat} {a b : Expr} : Ok B n L a → Ok B n L b → Ok B n L (.letE x a b)
  | app {n : Nat} {L : Nat → Prop} {a b : Expr} : Ok B n L a → Ok B n L b → Ok B n L (.app a b)
  | pair {n : Nat} {L : Nat → Prop} {a b : Expr} : Ok B n L a → Ok B n L b → Ok B n L (.pair a b)
  | toImm {n : Nat} {L : Nat → Prop} {b : Bool} {l : Nat} : L l → Ok B n L (.toImm b (.loc l))
  /-- `let x = mut [e₀] in b`, where `b` consumes `x`'s object. -/
  | own {n : Nat} {L : Nat → Prop} {x : Nat} {e₀ b : Expr} : Ok B n L e₀ →
      (∀ l, n ≤ l → Con B l (b.subst x (.loc l))) →
      (∀ l, n ≤ l → Ok B (l + 1) (fun l' => l' = l ∨ L l') (b.subst x (.loc l))) →
      Ok B n L (.letE x (.new true e₀) b)
  /-- `let x = f(e) in b`, where `f` gives back a new `mut` object and `b`
  consumes it. -/
  | ownCall {n : Nat} {L : Nat → Prop} {x f : Nat} {ms : List Mu} {τs : List Ty} {e b : Expr} :
      B f .ret → Ok B n L e →
      (∀ l, n ≤ l → Con B l (b.subst x (.loc l))) →
      (∀ l, n ≤ l → Ok B (l + 1) (fun l' => l' = l ∨ L l') (b.subst x (.loc l))) →
      Ok B n L (.letE x (.call f ms τs e) b)
  /-- The same, once the object is made. -/
  | bound {n : Nat} {L : Nat → Prop} {x l : Nat} {b : Expr} : L l → Ok B n L (b.subst x (.loc l)) →
      Ok B n L (.letE x (.loc l) b)

/-! ## Paths and values -/

theorem Expr.IsValue.not_toImmOf {v : Expr} (hv : v.IsValue) {l : Nat} : ¬ v.ToImmOf l := by
  induction hv with
  | unit | int | loc | lam => exact id
  | pair _ _ ih₁ ih₂ => exact fun h => h.elim ih₁ fun h => ih₂ h.2

theorem Expr.IsValue.subst {v : Expr} (hv : v.IsValue) (x : Nat) (u : Expr) : (v.subst x u).IsValue := by
  induction hv with
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam => simp only [Expr.subst]; split <;> exact .lam
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂

theorem Expr.IsValue.inst {v : Expr} (hv : v.IsValue) (ms : List Mu) (τs : List Ty) : (v.inst ms τs).IsValue := by
  induction hv with
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam => exact .lam
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂

theorem Path.not_toImmOf {l : Nat} {a : Expr} (h : Path l a) {l' : Nat} : ¬ a.ToImmOf l' := by
  cases h with
  | loc => exact id
  | fst hw _ => exact fun hf => hf.elim id fun h => hw.not_toImmOf h.2
  | snd hw _ => exact fun hf => hf.elim hw.not_toImmOf fun h => h.2

/-! ## What the check says -/

theorem Expr.count_zero {e : Expr} {l : Nat} (h : ¬ e.HasLoc l) : e.count l = 0 := by
  induction e with
  | var | unit | int => rfl
  | loc l' =>
    simp only [Expr.HasLoc] at h
    simp [Expr.count, h]
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.HasLoc, not_or] at h
    simp only [Expr.count, ih₁ h.1, ih₂ h.2]

theorem Bor.not_loc {l : Nat} (h : Bor B l (.loc l)) : False := by
  cases h with
  | none h => exact h rfl

/-- What a borrowing value is doesn't hold `l`. -/
theorem Bor.value {l : Nat} {v : Expr} (hv : v.IsValue) (h : Bor B l v) : ¬ v.HasLoc l := by
  induction hv with
  | unit | int => exact id
  | loc => cases h with
    | none h => exact h
  | lam => cases h with
    | none h => exact h
  | pair _ _ ih₁ ih₂ => cases h with
    | none h => exact h
    | pair h₁ h₂ => exact fun h => h.elim (ih₁ h₁) (ih₂ h₂)

/-- A value consumes nothing: it has no `toImm` to do. -/
theorem Con.not_value {l : Nat} {v : Expr} (hv : v.IsValue) (h : Con B l v) : False := by
  induction hv with
  | unit | int | loc | lam => cases h
  | pair _ _ ih₁ ih₂ => cases h with
    | pair₁ h₁ _ => exact ih₁ h₁
    | pair₂ _ h₂ => exact ih₂ h₂

/-- What only borrows `l` isn't about to make it `imm`. -/
theorem Bor.not_toImmOf {l : Nat} {e : Expr} (h : Bor B l e) : ¬ e.ToImmOf l := by
  induction h with
  | none h => exact fun hf => h hf.hasLoc
  | callL _ hp | getP hp => exact hp.not_toImmOf
  | setP hp _ ih => exact fun hf => hf.elim hp.not_toImmOf fun h => ih h.2
  | fstD hw _ | callT₂ _ hw _ => exact fun hf => hf.elim hw.not_toImmOf fun h => h.2
  | sndD hw _ | callT₁ _ hw _ => exact fun hf => hf.elim id fun h => hw.not_toImmOf h.2
  | new _ ih | get _ ih | call _ ih | fst _ ih | snd _ ih => exact ih
  | @toImm _ e h ih =>
    intro hf
    rcases hf with rfl | hf
    · exact h.not_loc
    · exact ih hf
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact fun hf => hf.elim ih₁ fun h => ih₂ h.2
  | letE _ _ ih₁ _ => exact ih₁

/-- When a program that consumes `l` is about to make it `imm`, that's the only
`l` in it. -/
theorem Con.count {l : Nat} {e : Expr} (h : Con B l e) (hf : e.ToImmOf l) : e.count l = 1 := by
  induction h with
  | toImm => simp [Expr.count]
  | @toImmC b e h ih =>
    rcases hf with rfl | hf
    · cases h
    · exact ih hf
  | new _ ih | get _ ih | call _ ih | fst _ ih | snd _ ih => exact ih hf
  | set₁ ha hb ih | app₁ ha hb ih | pair₁ ha hb ih =>
    rcases hf with hf | ⟨hv, _⟩
    · simp only [Expr.count, ih hf, Expr.count_zero hb]
    · exact (ha.not_value hv).elim
  | set₂ ha hb ih | app₂ ha hb ih | pair₂ ha hb ih =>
    rcases hf with hf | ⟨hv, hf⟩
    · exact (ha.not_toImmOf hf).elim
    · simp only [Expr.count, ih hf, Expr.count_zero (ha.value hv)]
  | let₁ _ hb ih => simp only [Expr.count, ih hf, Expr.count_zero hb]
  | let₂ ha _ _ => exact (ha.not_toImmOf hf).elim
  | bind => exact hf.elim

theorem Expr.hasLoc_inst {e : Expr} {ms : List Mu} {τs : List Ty} {l : Nat} :
    (e.inst ms τs).HasLoc l ↔ e.HasLoc l := by
  induction e with
  | var | unit | int | loc => simp [Expr.inst, Expr.HasLoc]
  | new _ _ ih | get _ ih | call _ _ _ _ ih | lam _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => simp only [Expr.inst, Expr.HasLoc, ih]
  | set _ _ ih₁ ih₂ | letE _ _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ =>
    simp only [Expr.inst, Expr.HasLoc, ih₁, ih₂]

theorem Expr.noLoc_subst {e v : Expr} {x l : Nat} (he : ¬ e.HasLoc l) (hv : ¬ v.HasLoc l) :
    ¬ (e.subst x v).HasLoc l := fun h => (Expr.hasLoc_subst h).elim he hv

theorem Path.subst {l : Nat} {a : Expr} (h : Path l a) {u : Expr} (hu : ¬ u.HasLoc l) (x : Nat) :
    Path l (a.subst x u) := by
  cases h with
  | loc => exact .loc
  | fst hw hwl => exact .fst (hw.subst x u) (Expr.noLoc_subst hwl hu)
  | snd hw hwl => exact .snd (hw.subst x u) (Expr.noLoc_subst hwl hu)

theorem Path.inst {l : Nat} {a : Expr} (h : Path l a) (ms : List Mu) (τs : List Ty) : Path l (a.inst ms τs) := by
  cases h with
  | loc => exact .loc
  | fst hw hwl => exact .fst (hw.inst ms τs) fun h' => hwl (Expr.hasLoc_inst.mp h')
  | snd hw hwl => exact .snd (hw.inst ms τs) fun h' => hwl (Expr.hasLoc_inst.mp h')

/-- A path that's a value is the location. -/
theorem Path.value {l : Nat} {a : Expr} (h : Path l a) (hv : a.IsValue) : a = .loc l := by
  cases h with
  | loc => rfl
  | fst => cases hv
  | snd => cases hv

/-- A path steps to the location, and changes nothing. -/
theorem Path.step {fns : List Fn} {l : Nat} {a a' : Expr} {h h' : Heap} (hp : Path l a)
    (s : Step fns h a h' a') : h' = h ∧ Path l a' := by
  cases hp with
  | loc => exact (Expr.IsValue.loc.no_step s).elim
  | fst hw _ =>
    cases s with
    | fstCong s' => exact ((Expr.IsValue.pair .loc hw).no_step s').elim
    | fstVal _ _ => exact ⟨rfl, .loc⟩
  | snd hw _ =>
    cases s with
    | sndCong s' => exact ((Expr.IsValue.pair hw .loc).no_step s').elim
    | sndVal _ _ => exact ⟨rfl, .loc⟩

theorem Bor.subst {l : Nat} {e : Expr} (h : Bor B l e) {v : Expr} (hv : ¬ v.HasLoc l) (x : Nat) :
    Bor B l (e.subst x v) := by
  induction h with
  | none h => exact .none (Expr.noLoc_subst h hv)
  | callL hf hp => exact .callL hf (hp.subst hv x)
  | getP hp => exact .getP (hp.subst hv x)
  | setP hp _ ih => exact .setP (hp.subst hv x) ih
  | fstD hw hwl => exact .fstD (hw.subst x v) (Expr.noLoc_subst hwl hv)
  | sndD hw hwl => exact .sndD (hw.subst x v) (Expr.noLoc_subst hwl hv)
  | callT₁ hf hw hwl => exact .callT₁ hf (hw.subst x v) (Expr.noLoc_subst hwl hv)
  | callT₂ hf hw hwl => exact .callT₂ hf (hw.subst x v) (Expr.noLoc_subst hwl hv)
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | call _ ih => exact .call ih
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih
  | toImm _ ih => exact .toImm ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂
  | @letE y _ _ _ hb ih₁ ih₂ =>
    simp only [Expr.subst]
    split
    · exact .letE ih₁ hb
    · exact .letE ih₁ ih₂

theorem Expr.noFree_loc (l : Nat) : (Expr.loc l).NoFree := fun _ h => h

theorem Con.subst {l : Nat} {e : Expr} (h : Con B l e) {v : Expr} (hv : ¬ v.HasLoc l) (hvf : v.NoFree)
    (x : Nat) : Con B l (e.subst x v) := by
  induction h with
  | toImm => exact .toImm
  | toImmC _ ih => exact .toImmC ih
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | call _ ih => exact .call ih
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih
  | set₁ _ hb ih => exact .set₁ ih (Expr.noLoc_subst hb hv)
  | set₂ ha _ ih => exact .set₂ (ha.subst hv x) ih
  | app₁ _ hb ih => exact .app₁ ih (Expr.noLoc_subst hb hv)
  | app₂ ha _ ih => exact .app₂ (ha.subst hv x) ih
  | pair₁ _ hb ih => exact .pair₁ ih (Expr.noLoc_subst hb hv)
  | pair₂ ha _ ih => exact .pair₂ (ha.subst hv x) ih
  | @let₁ y _ _ _ hb ih =>
    simp only [Expr.subst]
    split
    · exact .let₁ ih hb
    · exact .let₁ ih (Expr.noLoc_subst hb hv)
  | @let₂ y _ _ ha hb ih =>
    simp only [Expr.subst]
    split
    · exact .let₂ (ha.subst hv x) hb
    · exact .let₂ (ha.subst hv x) ih
  | @bind y b hb hc ih =>
    simp only [Expr.subst]
    split
    · exact .bind hb hc
    · rename_i hyx
      refine .bind (Expr.noLoc_subst hb hv) ?_
      rw [Expr.subst_comm b (Ne.symm hyx) hvf (Expr.noFree_loc l)]
      exact ih

theorem Ok.of_noToImm {n : Nat} {L : Nat → Prop} {e : Expr} (h : e.NoToImm) : Ok B n L e := by
  induction e with
  | var => exact .var
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam => exact .lam h
  | new _ _ ih => exact .new (ih h)
  | get _ ih => exact .get (ih h)
  | call _ _ _ _ ih => exact .call (ih h)
  | fst _ ih => exact .fst (ih h)
  | snd _ ih => exact .snd (ih h)
  | set _ _ ih₁ ih₂ => exact .set (ih₁ h.1) (ih₂ h.2)
  | letE _ _ _ ih₁ ih₂ => exact .letE (ih₁ h.1) (ih₂ h.2)
  | app _ _ ih₁ ih₂ => exact .app (ih₁ h.1) (ih₂ h.2)
  | pair _ _ ih₁ ih₂ => exact .pair (ih₁ h.1) (ih₂ h.2)
  | toImm => exact h.elim

/-- A later bound, and more owned locations, are fine. -/
theorem Ok.mono {n : Nat} {L : Nat → Prop} {e : Expr} (h : Ok B n L e) :
    ∀ {n' : Nat} {L' : Nat → Prop}, n ≤ n' → (∀ l, L l → L' l) → Ok B n' L' e := by
  induction h with
  | var => exact fun _ _ => .var
  | unit => exact fun _ _ => .unit
  | int => exact fun _ _ => .int
  | loc => exact fun _ _ => .loc
  | lam h => exact fun _ _ => .lam h
  | new _ ih => exact fun hn hL => .new (ih hn hL)
  | get _ ih => exact fun hn hL => .get (ih hn hL)
  | call _ ih => exact fun hn hL => .call (ih hn hL)
  | fst _ ih => exact fun hn hL => .fst (ih hn hL)
  | snd _ ih => exact fun hn hL => .snd (ih hn hL)
  | set _ _ ih₁ ih₂ => exact fun hn hL => .set (ih₁ hn hL) (ih₂ hn hL)
  | letE _ _ ih₁ ih₂ => exact fun hn hL => .letE (ih₁ hn hL) (ih₂ hn hL)
  | app _ _ ih₁ ih₂ => exact fun hn hL => .app (ih₁ hn hL) (ih₂ hn hL)
  | pair _ _ ih₁ ih₂ => exact fun hn hL => .pair (ih₁ hn hL) (ih₂ hn hL)
  | toImm hl => exact fun _ hL => .toImm (hL _ hl)
  | own _ hc _ ih₀ ih =>
    intro n' L' hn hL
    exact .own (ih₀ hn hL) (fun l hl => hc l (by omega))
      (fun l hl => ih l (by omega) (Nat.le_refl _) fun l' h => h.imp_right (hL l'))
  | ownCall hfB _ hc _ ih₀ ih =>
    intro n' L' hn hL
    exact .ownCall hfB (ih₀ hn hL) (fun l hl => hc l (by omega))
      (fun l hl => ih l (by omega) (Nat.le_refl _) fun l' h => h.imp_right (hL l'))
  | bound hl _ ih => exact fun hn hL => .bound (hL _ hl) (ih hn hL)

theorem Ok.subst {n : Nat} {L : Nat → Prop} {e : Expr} (h : Ok B n L e) :
    ∀ (x : Nat) {v : Expr}, v.IsValue → v.NoToImm → v.NoFree → (∀ l, v.HasLoc l → l < n) →
      Ok B n L (e.subst x v) := by
  induction h with
  | @var _ _ y =>
    intro x v _ hvn _ _
    simp only [Expr.subst]
    split
    · exact .of_noToImm hvn
    · exact .var
  | unit => exact fun _ _ _ _ _ _ => .unit
  | int => exact fun _ _ _ _ _ _ => .int
  | loc => exact fun _ _ _ _ _ _ => .loc
  | @lam _ _ _ y body hb =>
    intro x v _ hvn _ _
    simp only [Expr.subst]
    split
    · exact .lam hb
    · exact .lam (hb.subst hvn x)
  | new _ ih => exact fun x _ a b c d => .new (ih x a b c d)
  | get _ ih => exact fun x _ a b c d => .get (ih x a b c d)
  | call _ ih => exact fun x _ a b c d => .call (ih x a b c d)
  | fst _ ih => exact fun x _ a b c d => .fst (ih x a b c d)
  | snd _ ih => exact fun x _ a b c d => .snd (ih x a b c d)
  | set _ _ ih₁ ih₂ => exact fun x _ a b c d => .set (ih₁ x a b c d) (ih₂ x a b c d)
  | app _ _ ih₁ ih₂ => exact fun x _ a b c d => .app (ih₁ x a b c d) (ih₂ x a b c d)
  | pair _ _ ih₁ ih₂ => exact fun x _ a b c d => .pair (ih₁ x a b c d) (ih₂ x a b c d)
  | @letE _ _ y _ _ _ hb ih₁ ih₂ =>
    intro x v a b c d
    simp only [Expr.subst]
    split
    · exact .letE (ih₁ x a b c d) hb
    · exact .letE (ih₁ x a b c d) (ih₂ x a b c d)
  | toImm hl => exact fun _ _ _ _ _ _ => .toImm hl
  | @own n _ y _ b _ hc ho ih₀ ih =>
    intro x v hv hvn hvf hvl
    simp only [Expr.subst]
    split
    · exact .own (ih₀ x hv hvn hvf hvl) hc ho
    · rename_i hyx
      have hnl : ∀ l, n ≤ l → ¬ v.HasLoc l := fun l hl h => by have := hvl l h; omega
      refine .own (ih₀ x hv hvn hvf hvl) (fun l hl => ?_) (fun l hl => ?_)
      · rw [Expr.subst_comm b (Ne.symm hyx) hvf (Expr.noFree_loc l)]
        exact (hc l hl).subst (hnl l hl) hvf x
      · rw [Expr.subst_comm b (Ne.symm hyx) hvf (Expr.noFree_loc l)]
        exact ih l hl x hv hvn hvf fun l' h => by have := hvl l' h; omega
  | @ownCall n _ y _ _ _ _ b hfB _ hc ho ih₀ ih =>
    intro x v hv hvn hvf hvl
    simp only [Expr.subst]
    split
    · exact .ownCall hfB (ih₀ x hv hvn hvf hvl) hc ho
    · rename_i hyx
      have hnl : ∀ l, n ≤ l → ¬ v.HasLoc l := fun l hl h => by have := hvl l h; omega
      refine .ownCall hfB (ih₀ x hv hvn hvf hvl) (fun l hl => ?_) (fun l hl => ?_)
      · rw [Expr.subst_comm b (Ne.symm hyx) hvf (Expr.noFree_loc l)]
        exact (hc l hl).subst (hnl l hl) hvf x
      · rw [Expr.subst_comm b (Ne.symm hyx) hvf (Expr.noFree_loc l)]
        exact ih l hl x hv hvn hvf fun l' h => by have := hvl l' h; omega
  | @bound _ _ y l b hl _ ih =>
    intro x v hv hvn hvf hvl
    simp only [Expr.subst]
    split
    · exact .bound hl (by assumption)
    · rename_i hyx
      refine .bound hl ?_
      rw [Expr.subst_comm b (Ne.symm hyx) hvf (Expr.noFree_loc l)]
      exact ih x hv hvn hvf hvl

/-- What `Ok` lets a program make `imm` next is owned. -/
theorem Ok.toImmOf {n : Nat} {L : Nat → Prop} {e : Expr} (h : Ok B n L e) {l : Nat} (hf : e.ToImmOf l) : L l := by
  induction h with
  | var | unit | int | loc | lam | bound => exact hf.elim
  | new _ ih | get _ ih | call _ ih | fst _ ih | snd _ ih => exact ih hf
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact hf.elim ih₁ fun h => ih₂ h.2
  | letE _ _ ih₁ _ => exact ih₁ hf
  | @toImm _ _ _ l' hl =>
    rcases hf with h | h
    · cases h; exact hl
    · exact h.elim
  | own _ _ _ ih₀ _ | ownCall _ _ _ _ ih₀ _ => exact ih₀ hf

theorem Expr.NoToImm.of_subst {e : Expr} {x : Nat} {v : Expr} (h : (e.subst x v).NoToImm) : e.NoToImm := by
  induction e with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst] at h
    refine ⟨ih₁ h.1, ?_⟩
    split at h
    · exact h.2
    · exact ih₂ h.2
  | lam b y _ ih =>
    simp only [Expr.subst] at h
    split at h
    · exact h
    · exact ih h
  | toImm => exact h.elim

theorem Expr.LamNoToImm.of_subst {e : Expr} {x : Nat} {v : Expr} (h : (e.subst x v).LamNoToImm) : e.LamNoToImm := by
  induction e with
  | var | unit | int | loc => trivial
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih | toImm _ _ ih => exact ih h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁ h.1, ih₂ h.2⟩
  | letE y _ _ ih₁ ih₂ =>
    simp only [Expr.subst] at h
    refine ⟨ih₁ h.1, ?_⟩
    split at h
    · exact h.2
    · exact ih₂ h.2
  | lam b y _ _ =>
    simp only [Expr.subst] at h
    split at h
    · exact h
    · exact Expr.NoToImm.of_subst h

/-- A checked program has no `toImm` in a closure. -/
theorem Ok.lamNoToImm {n : Nat} {L : Nat → Prop} {e : Expr} (h : Ok B n L e) : e.LamNoToImm := by
  induction h with
  | var | unit | int | loc | toImm => trivial
  | lam h => exact h
  | new _ ih | get _ ih | call _ ih | fst _ ih | snd _ ih => exact ih
  | set _ _ ih₁ ih₂ | letE _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact ⟨ih₁, ih₂⟩
  | @own n _ _ _ _ _ _ _ ih₀ ih => exact ⟨ih₀, (ih n (Nat.le_refl _)).of_subst⟩
  | @ownCall n _ _ _ _ _ _ _ _ _ _ _ ih₀ ih => exact ⟨ih₀, (ih n (Nat.le_refl _)).of_subst⟩
  | bound _ _ ih => exact ⟨trivial, ih.of_subst⟩

/-! ## The invariant -/

/-- **What the running program keeps**: it's checked, with the locations
it owns in `L`, and each of those it either consumes or no longer has,
no object holds it, and it's `mut`. -/
def Inv (B : Nat → Pos → Prop) (h : Heap) (e : Expr) : Prop :=
  ∃ L : Nat → Prop, Ok B h.length L e ∧
    ∀ l, L l → l < h.length ∧ (Con B l e ∨ ¬ e.HasLoc l) ∧
      (∀ (a : Nat) (b : Bool) (v : Expr), h[a]? = some (b, v) → ¬ v.HasLoc l) ∧ ∃ w, h[l]? = some (true, w)

/-- Making a location `imm` is making it the value. -/
theorem Expr.ToImmArg.toImmOf {e : Expr} {l : Nat} (h : e.ToImmArg (.loc l)) : e.ToImmOf l := by
  induction e with
  | var | unit | int | loc | lam => exact h.elim
  | new _ _ ih | get _ ih | call _ _ _ _ ih | fst _ ih | snd _ ih => exact ih h
  | letE _ _ _ ih₁ _ => exact ih₁ h
  | set _ _ ih₁ ih₂ | app _ _ ih₁ ih₂ | pair _ _ ih₁ ih₂ => exact h.imp ih₁ fun h => ⟨h.1, ih₂ h.2⟩
  | toImm _ e ih => exact h.imp id ih

/-- The region of an object that holds only `imm` objects is the object. -/
theorem Shallow.region {h : Heap} {v : Expr} (hs : Shallow h v) {x : Nat} (hx : Region h v x) :
    ∃ l, v = .loc l ∧ x = l := by
  obtain ⟨l, b, w, rfl, hw, hwi⟩ := hs
  obtain ⟨l₀, hl₀, hr⟩ := hx
  simp only [Expr.TopLoc] at hl₀
  subst hl₀
  cases hr with
  | refl => exact ⟨_, rfl, rfl⟩
  | step hw' ht hr' =>
    rw [hw] at hw'
    cases hw'
    -- What it holds is `imm`, so no path through `mut` objects goes on.
    obtain ⟨w₂, hw₂⟩ := hr'.of_start
    obtain ⟨w', hw'⟩ := hwi _ ht.hasLoc
    rw [hw'] at hw₂
    cases hw₂

/-- The invariant gives the premise of `toImms_in_place_change_nothing`,
when what's made `imm` has contents that are `imm` already. -/
theorem Inv.uniqToImm {h : Heap} {e : Expr} (hi : Inv B h e) (hs : ∀ v, e.ToImmArg v → Shallow h v) :
    e.UniqToImm h := by
  obtain ⟨L, hok, hL⟩ := hi
  intro v hv x hx
  obtain ⟨l, rfl, rfl⟩ := (hs v hv).region hx
  have hf := hv.toImmOf
  obtain ⟨_, hc, hno, _⟩ := hL _ (hok.toImmOf hf)
  rcases hc with hc | hc
  · exact ⟨by rw [hc.count hf]; simp [Expr.count], fun a b w ha hw => (hno a b w ha hw).elim⟩
  · exact (hc hf.hasLoc).elim

/-! ## Instantiating and renaming -/

theorem Bor.inst {l : Nat} {e : Expr} (h : Bor B l e) (ms : List Mu) (τs : List Ty) : Bor B l (e.inst ms τs) := by
  induction h with
  | none h => exact .none fun h' => h (Expr.hasLoc_inst.mp h')
  | callL hf hp => exact .callL hf (hp.inst ms τs)
  | getP hp => exact .getP (hp.inst ms τs)
  | setP hp _ ih => exact .setP (hp.inst ms τs) ih
  | fstD hw hwl => exact .fstD (hw.inst ms τs) fun h' => hwl (Expr.hasLoc_inst.mp h')
  | sndD hw hwl => exact .sndD (hw.inst ms τs) fun h' => hwl (Expr.hasLoc_inst.mp h')
  | callT₁ hf hw hwl => exact .callT₁ hf (hw.inst ms τs) fun h' => hwl (Expr.hasLoc_inst.mp h')
  | callT₂ hf hw hwl => exact .callT₂ hf (hw.inst ms τs) fun h' => hwl (Expr.hasLoc_inst.mp h')
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | call _ ih => exact .call ih
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih
  | toImm _ ih => exact .toImm ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂

theorem Con.inst {l : Nat} {e : Expr} (h : Con B l e) (ms : List Mu) (τs : List Ty) : Con B l (e.inst ms τs) := by
  have nl : ∀ {e : Expr}, ¬ e.HasLoc l → ¬ (e.inst ms τs).HasLoc l := fun h h' => h (Expr.hasLoc_inst.mp h')
  induction h with
  | toImm => exact .toImm
  | toImmC _ ih => exact .toImmC ih
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | call _ ih => exact .call ih
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih
  | set₁ _ hb ih => exact .set₁ ih (nl hb)
  | set₂ ha _ ih => exact .set₂ (ha.inst ms τs) ih
  | app₁ _ hb ih => exact .app₁ ih (nl hb)
  | app₂ ha _ ih => exact .app₂ (ha.inst ms τs) ih
  | pair₁ _ hb ih => exact .pair₁ ih (nl hb)
  | pair₂ ha _ ih => exact .pair₂ (ha.inst ms τs) ih
  | let₁ _ hb ih => exact .let₁ ih (nl hb)
  | let₂ ha _ ih => exact .let₂ (ha.inst ms τs) ih
  | @bind x b hb _ ih =>
    refine .bind (nl hb) ?_
    have := ih
    rw [Expr.subst_inst] at this
    exact this

theorem Ok.inst {n : Nat} {L : Nat → Prop} {e : Expr} (h : Ok B n L e) (ms : List Mu) (τs : List Ty) :
    Ok B n L (e.inst ms τs) := by
  induction h with
  | var => exact .var
  | unit => exact .unit
  | int => exact .int
  | loc => exact .loc
  | lam h => exact .lam (h.inst ms τs)
  | new _ ih => exact .new ih
  | get _ ih => exact .get ih
  | call _ ih => exact .call ih
  | fst _ ih => exact .fst ih
  | snd _ ih => exact .snd ih
  | set _ _ ih₁ ih₂ => exact .set ih₁ ih₂
  | letE _ _ ih₁ ih₂ => exact .letE ih₁ ih₂
  | app _ _ ih₁ ih₂ => exact .app ih₁ ih₂
  | pair _ _ ih₁ ih₂ => exact .pair ih₁ ih₂
  | toImm hl => exact .toImm hl
  | @ownCall _ _ x _ _ _ _ b hfB _ hc _ ih₀ ih =>
    refine .ownCall hfB ih₀ (fun l hl => ?_) (fun l hl => ?_)
    · have := (hc l hl).inst ms τs
      rw [Expr.subst_inst] at this
      exact this
    · have := ih l hl
      rw [Expr.subst_inst] at this
      exact this
  | @own _ _ x _ b _ hc _ ih₀ ih =>
    refine .own ih₀ (fun l hl => ?_) (fun l hl => ?_)
    · have := (hc l hl).inst ms τs
      rw [Expr.subst_inst] at this
      exact this
    · have := ih l hl
      rw [Expr.subst_inst] at this
      exact this
  | bound hl _ ih =>
    refine .bound hl ?_
    rw [Expr.subst_inst] at ih
    exact ih

/-! ## Locations nothing has -/

/-- No object in `h` holds `l`. -/
def HeapNoLoc (h : Heap) (l : Nat) : Prop := ∀ (a : Nat) (b : Bool) (v : Expr), h[a]? = some (b, v) → ¬ v.HasLoc l

theorem HeapNoLoc.append {h : Heap} {l : Nat} (hh : HeapNoLoc h l) {b : Bool} {v : Expr} (hv : ¬ v.HasLoc l) :
    HeapNoLoc (h ++ [(b, v)]) l := by
  intro a b' w ha
  by_cases hlt : a < h.length
  · rw [List.getElem?_append_left hlt] at ha; exact hh a b' w ha
  · have : a = h.length := by
      have := (List.getElem?_eq_some_iff.mp ha).1
      simp at this
      omega
    subst this
    rw [List.getElem?_concat_length] at ha
    cases ha
    exact hv

theorem HeapNoLoc.set {h : Heap} {l : Nat} (hh : HeapNoLoc h l) {b : Bool} {v : Expr} (hv : ¬ v.HasLoc l) (a : Nat) :
    HeapNoLoc (h.set a (b, v)) l := by
  intro a' b' w ha
  rw [List.getElem?_set] at ha
  split at ha
  · split at ha
    · cases ha; exact hv
    · cases ha
  · exact hh a' b' w ha

theorem HeapNoLoc.freezeAt {h : Heap} {l : Nat} (hh : HeapNoLoc h l) (m : List (Nat × Nat)) :
    HeapNoLoc (h.freezeAt m) l := by
  intro a b w ha
  rw [Heap.freezeAt_get] at ha
  cases ho : h[a]? with
  | none => rw [ho] at ha; cases ha
  | some o =>
    rw [ho] at ha
    obtain ⟨b₀, w₀⟩ := o
    simp only [Option.map_some] at ha
    split at ha <;> (cases ha; exact hh a _ _ ho)

theorem Heap.freezeAt_length (h : Heap) (m : List (Nat × Nat)) : (h.freezeAt m).length = h.length := by
  simp [Heap.freezeAt]

/-- A copy doesn't make anything hold `l`, if nothing it copies does. -/
theorem Freeze.noLoc {h h' : Heap} {m m' : List (Nat × Nat)} {v v' : Expr}
    (hc : Freeze h m v h' m' v') {l : Nat} : HeapNoLoc h l → ¬ v.HasLoc l → l < h.length →
      (∀ o c, m.lookup o = some c → c ≠ l) →
      HeapNoLoc h' l ∧ ¬ v'.HasLoc l ∧ ∀ o c, m'.lookup o = some c → c ≠ l := by
  induction hc with
  | unit | int | lam | imm => exact fun hh hv _ hm => ⟨hh, hv, hm⟩
  | seen _ hlk => exact fun hh _ _ hm => ⟨hh, fun h => hm _ _ hlk h, hm⟩
  | @cell h h' m m' o w w' hw _ hcw ih =>
    intro hh _ hl hm
    obtain ⟨hh', hw', hm'⟩ := ih hh (hh _ _ _ hw) hl hm
    have hlen : h.length ≤ h'.length := by
      obtain ⟨e, rfl⟩ := hcw.prefix
      simp
    refine ⟨hh'.append hw', fun h => by simp only [Expr.HasLoc] at h; omega, fun o' c hk => ?_⟩
    rw [List.lookup_cons_nat] at hk
    split at hk
    · cases hk; omega
    · exact hm' o' c hk
  | pair hc₁ _ ih₁ ih₂ =>
    intro hh hv hl hm
    obtain ⟨hh₁, hv₁, hm₁⟩ := ih₁ hh (fun h => hv (.inl h)) hl hm
    have hlen : _ := hc₁.prefix
    obtain ⟨hh₂, hv₂, hm₂⟩ := ih₂ hh₁ (fun h => hv (.inr h))
      (by obtain ⟨e, rfl⟩ := hlen; simp; omega) hm₁
    exact ⟨hh₂, fun h => h.elim hv₁ hv₂, hm₂⟩

/-- A step doesn't give a location to what doesn't have it, nor to an
object, if nothing has it to start with. -/
theorem step_noLoc {fns : List Fn} (hfn : ∀ fn ∈ fns, fn.body.NoLocs) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') {l : Nat} :
    ¬ e.HasLoc l → l < h.length → HeapNoLoc h l → ¬ e'.HasLoc l ∧ HeapNoLoc h' l ∧ l < h'.length := by
  induction hs with
  | newCong _ ih | getCong _ ih | fstCong _ ih | sndCong _ ih | callCong _ ih | toImmCong _ ih => exact ih
  | setCong₁ _ ih | letCong _ ih | appCong₁ _ ih | pairCong₁ _ ih =>
    intro he hl hh
    obtain ⟨he', hh', hl'⟩ := ih (fun h => he (.inl h)) hl hh
    exact ⟨fun h => h.elim he' fun h => he (.inr h), hh', hl'⟩
  | setCong₂ _ _ ih | appCong₂ _ _ ih | pairCong₂ _ _ ih =>
    intro he hl hh
    obtain ⟨he', hh', hl'⟩ := ih (fun h => he (.inr h)) hl hh
    exact ⟨fun h => h.elim (fun h => he (.inl h)) he', hh', hl'⟩
  | newVal _ =>
    intro he hl hh
    exact ⟨fun h => by simp only [Expr.HasLoc] at h; omega, hh.append he, by simp; omega⟩
  | getLoc hw => exact fun _ hl hh => ⟨hh _ _ _ hw, hh, hl⟩
  | setLoc _ _ =>
    intro he hl hh
    exact ⟨id, hh.set (fun h => he (.inr h)) _, by simp [hl]⟩
  | letVal _ =>
    intro he hl hh
    exact ⟨Expr.noLoc_subst (fun h => he (.inr h)) (fun h => he (.inl h)), hh, hl⟩
  | @callVal f fn ms τs v hf _ =>
    intro he hl hh
    have hb := ((hfn _ (List.mem_of_getElem? hf)).inst ms τs).not_hasLoc l
    exact ⟨Expr.noLoc_subst hb he, hh, hl⟩
  | appVal _ =>
    intro he hl hh
    exact ⟨Expr.noLoc_subst (fun h => he (.inl h)) (fun h => he (.inr h)), hh, hl⟩
  | fstVal _ _ => exact fun he hl hh => ⟨fun h => he (.inl h), hh, hl⟩
  | sndVal _ _ => exact fun he hl hh => ⟨fun h => he (.inr h), hh, hl⟩
  | toImmVal _ hc =>
    intro he hl hh
    obtain ⟨hh', hv', _⟩ := hc.noLoc hh he hl (fun _ _ h => by simp at h)
    exact ⟨hv', hh', by obtain ⟨e, rfl⟩ := hc.prefix; simp; omega⟩
  | toImmKeep _ hc =>
    intro he hl hh
    obtain ⟨hh', _, _⟩ := hc.noLoc hh he hl (fun _ _ h => by simp at h)
    exact ⟨he, hh'.freezeAt _, by rw [Heap.freezeAt_length]; obtain ⟨e, rfl⟩ := hc.prefix; simp; omega⟩

/-! ## A step keeps borrowing and consuming -/

/-- An argument with `l` at `p`, the rest of which doesn't have it. -/
def ArgAt (l : Nat) : Pos → Expr → Prop
  | .whole, v => v = .loc l
  | .fst, v => ∃ w, v = .pair (.loc l) w ∧ w.IsValue ∧ ¬ w.HasLoc l
  | .snd, v => ∃ w, v = .pair w (.loc l) ∧ w.IsValue ∧ ¬ w.HasLoc l
  | .ret, _ => False

/-- The functions in `B` borrow their argument: a body, with a location as
its argument, only borrows it. -/
def Borrows (B : Nat → Pos → Prop) (fns : List Fn) : Prop :=
  ∀ f fn p, B f p → fns[f]? = some fn → ∀ ms τs l v, ArgAt l p v →
    Bor B l ((fn.body.inst ms τs).subst fn.param v)

/-- **A step keeps borrowing**: what only borrows `l` still does, and no
object gets `l`. -/
theorem step_bor {fns : List Fn} (hfn : ∀ fn ∈ fns, fn.body.NoLocs) (hB : Borrows B fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') {l : Nat} :
    Bor B l e → l < h.length → HeapNoLoc h l → Bor B l e' ∧ HeapNoLoc h' l ∧ l < h'.length := by
  have nl : ∀ {h h' : Heap} {e e' : Expr}, Step fns h e h' e' → ¬ e.HasLoc l → l < h.length → HeapNoLoc h l →
      Bor B l e' ∧ HeapNoLoc h' l ∧ l < h'.length := fun s hn hl hh => by
    obtain ⟨a, b, c⟩ := step_noLoc hfn s hn hl hh
    exact ⟨.none a, b, c⟩
  induction hs with
  | newCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.newCong s) hn hl hh
    | new hb => obtain ⟨a, b, c⟩ := ih hb hl hh; exact ⟨.new a, b, c⟩
  | newVal hv =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.newVal hv) hn hl hh
    | new hb => exact ⟨.none fun h => by simp only [Expr.HasLoc] at h; omega, hh.append (hb.value hv), by simp; omega⟩
  | getCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.getCong s) hn hl hh
    | getP hp => obtain ⟨rfl, hp'⟩ := hp.step s; exact ⟨.getP hp', hh, hl⟩
    | get hb => obtain ⟨a, b, c⟩ := ih hb hl hh; exact ⟨.get a, b, c⟩
  | getLoc hw => exact fun _ hl hh => ⟨.none (hh _ _ _ hw), hh, hl⟩
  | setCong₁ s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.setCong₁ s) hn hl hh
    | setP hp hb₂ => obtain ⟨rfl, hp'⟩ := hp.step s; exact ⟨.setP hp' hb₂, hh, hl⟩
    | set hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₁ hl hh; exact ⟨.set a hb₂, b, c⟩
  | setCong₂ hv s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.setCong₂ hv s) hn hl hh
    | setP hp hb₂ => obtain ⟨a, b, c⟩ := ih hb₂ hl hh; exact ⟨.setP hp a, b, c⟩
    | set hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₂ hl hh; exact ⟨.set hb₁ a, b, c⟩
  | @setLoc a b w v hw hv =>
    intro hb hl hh
    have hvl : ¬ v.HasLoc l := by
      cases hb with
      | none hn => exact fun h => hn (.inr h)
      | setP _ hb₂ => exact hb₂.value hv
      | set _ hb₂ => exact hb₂.value hv
    exact ⟨.none id, hh.set hvl _, by simp [hl]⟩
  | letCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.letCong s) hn hl hh
    | letE hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₁ hl hh; exact ⟨.letE a hb₂, b, c⟩
  | @letVal x v b hv =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.letVal hv) hn hl hh
    | letE hb₁ hb₂ => exact ⟨hb₂.subst (hb₁.value hv) x, hh, hl⟩
  | callCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.callCong s) hn hl hh
    | call hb => obtain ⟨a, b, c⟩ := ih hb hl hh; exact ⟨.call a, b, c⟩
    | callL hf hp => obtain ⟨rfl, hp'⟩ := hp.step s; exact ⟨.callL hf hp', hh, hl⟩
    | callT₁ _ hw _ => exact ((Expr.IsValue.pair .loc hw).no_step s).elim
    | callT₂ _ hw _ => exact ((Expr.IsValue.pair hw .loc).no_step s).elim
  | @callVal f fn ms τs v hf hv =>
    intro hb hl hh
    have hbody := ((hfn _ (List.mem_of_getElem? hf)).inst ms τs).not_hasLoc l
    cases hb with
    | none hn => exact ⟨.none (Expr.noLoc_subst hbody hn), hh, hl⟩
    | call hb => exact ⟨.none (Expr.noLoc_subst hbody (hb.value hv)), hh, hl⟩
    -- The function borrows what it's given.
    | callL hfB hp =>
      obtain rfl := hp.value hv
      exact ⟨hB f fn .whole hfB hf ms τs l _ rfl, hh, hl⟩
    | callT₁ hfB hw hwl => exact ⟨hB f fn .fst hfB hf ms τs l _ ⟨_, rfl, hw, hwl⟩, hh, hl⟩
    | callT₂ hfB hw hwl => exact ⟨hB f fn .snd hfB hf ms τs l _ ⟨_, rfl, hw, hwl⟩, hh, hl⟩
  | appCong₁ s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.appCong₁ s) hn hl hh
    | app hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₁ hl hh; exact ⟨.app a hb₂, b, c⟩
  | appCong₂ hv s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.appCong₂ hv s) hn hl hh
    | app hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₂ hl hh; exact ⟨.app hb₁ a, b, c⟩
  | @appVal c x body v hv =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.appVal hv) hn hl hh
    | app hb₁ hb₂ =>
      have hbl' := Bor.value (v := .lam c x body) .lam hb₁
      have hbl : ¬ body.HasLoc l := hbl'
      exact ⟨.none (Expr.noLoc_subst hbl (hb₂.value hv)), hh, hl⟩
  | pairCong₁ s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.pairCong₁ s) hn hl hh
    | pair hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₁ hl hh; exact ⟨.pair a hb₂, b, c⟩
  | pairCong₂ hv s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.pairCong₂ hv s) hn hl hh
    | pair hb₁ hb₂ => obtain ⟨a, b, c⟩ := ih hb₂ hl hh; exact ⟨.pair hb₁ a, b, c⟩
  | fstCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.fstCong s) hn hl hh
    | fst hb => obtain ⟨a, b, c⟩ := ih hb hl hh; exact ⟨.fst a, b, c⟩
    | fstD hw _ => exact ((Expr.IsValue.pair hw .loc).no_step s).elim
  | sndCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.sndCong s) hn hl hh
    | snd hb => obtain ⟨a, b, c⟩ := ih hb hl hh; exact ⟨.snd a, b, c⟩
    | sndD hw _ => exact ((Expr.IsValue.pair .loc hw).no_step s).elim
  | fstVal hv₁ hv₂ =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.fstVal hv₁ hv₂) hn hl hh
    | fst hb => exact ⟨.none fun h => hb.value (.pair hv₁ hv₂) (.inl h), hh, hl⟩
    | fstD _ hwl => exact ⟨.none hwl, hh, hl⟩
  | sndVal hv₁ hv₂ =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.sndVal hv₁ hv₂) hn hl hh
    | snd hb => exact ⟨.none fun h => hb.value (.pair hv₁ hv₂) (.inr h), hh, hl⟩
    | sndD _ hwl => exact ⟨.none hwl, hh, hl⟩
  | toImmCong s ih =>
    intro hb hl hh
    cases hb with
    | none hn => exact nl (.toImmCong s) hn hl hh
    | toImm hb => obtain ⟨a, b, c⟩ := ih hb hl hh; exact ⟨.toImm a, b, c⟩
  | @toImmVal h₀ m₀ v v' hv hc =>
    intro hb hl hh
    -- What's copied doesn't have `l`, so neither has the copy.
    have hvl : ¬ v.HasLoc l := by
      cases hb with
      | none hn => exact hn
      | toImm hb => exact hb.value hv
    obtain ⟨hh', hv', _⟩ := hc.noLoc hh hvl hl (fun _ _ h => by simp at h)
    exact ⟨.none hv', hh', by obtain ⟨e, rfl⟩ := hc.prefix; simp; omega⟩
  | @toImmKeep h₀ m₀ v v' hv hc =>
    intro hb hl hh
    have hvl : ¬ v.HasLoc l := by
      cases hb with
      | none hn => exact hn
      | toImm hb => exact hb.value hv
    obtain ⟨hh', _, _⟩ := hc.noLoc hh hvl hl (fun _ _ h => by simp at h)
    exact ⟨.none hvl, hh'.freezeAt _, by rw [Heap.freezeAt_length]; obtain ⟨e, rfl⟩ := hc.prefix; simp; omega⟩

/-- **A step keeps consuming**: what consumes `l` still does, or doesn't
have it any more, once it froze it; and no object gets `l`. -/
theorem step_con {fns : List Fn} (hfn : ∀ fn ∈ fns, fn.body.NoLocs) (hB : Borrows B fns) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') {l : Nat} :
    Con B l e → e.NoFree → l < h.length → HeapNoLoc h l → (∃ w, h[l]? = some (true, w)) →
      (Con B l e' ∨ ¬ e'.HasLoc l) ∧ HeapNoLoc h' l ∧ l < h'.length := by
  induction hs with
  | newCong _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | new hc => obtain ⟨a, b, c⟩ := ih hc hf hl hh hm; exact ⟨a.imp .new id, b, c⟩
  | newVal hv => intro hc; cases hc with | new hc => exact (hc.not_value hv).elim
  | getCong _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | get hc => obtain ⟨a, b, c⟩ := ih hc hf hl hh hm; exact ⟨a.imp .get id, b, c⟩
  | getLoc _ => intro hc; cases hc with | get hc => cases hc
  | setCong₁ s ih =>
    intro hc hf hl hh hm
    cases hc with
    | set₁ hc hb =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inl h)) hl hh hm
      exact ⟨a.imp (.set₁ · hb) (fun h h' => h'.elim h hb), b, c⟩
    | set₂ hb hc =>
      obtain ⟨a, b, c⟩ := step_bor hfn hB s hb hl hh
      exact ⟨.inl (.set₂ a hc), b, c⟩
  | setCong₂ hv _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | set₁ hc _ => exact (hc.not_value hv).elim
    | set₂ hb hc =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inr h)) hl hh hm
      exact ⟨a.imp (.set₂ hb) (fun h h' => h'.elim (hb.value hv) h), b, c⟩
  | setLoc _ hv =>
    intro hc
    cases hc with
    | set₁ hc _ => cases hc
    | set₂ _ hc => exact (hc.not_value hv).elim
  | letCong s ih =>
    intro hc hf hl hh hm
    cases hc with
    | let₁ hc hb =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inl h)) hl hh hm
      exact ⟨a.imp (.let₁ · hb) (fun h h' => h'.elim h hb), b, c⟩
    | let₂ hb hc =>
      obtain ⟨a, b, c⟩ := step_bor hfn hB s hb hl hh
      exact ⟨.inl (.let₂ a hc), b, c⟩
    | bind _ _ => exact (Expr.IsValue.loc.no_step s).elim
  | @letVal x v b hv =>
    intro hc hf hl hh hm
    cases hc with
    | let₁ hc _ => exact (hc.not_value hv).elim
    | let₂ hb hc =>
      have hvf : v.NoFree := fun y h => hf y (.inl h)
      exact ⟨.inl (hc.subst (hb.value hv) hvf x), hh, hl⟩
    | bind _ hc => exact ⟨.inl hc, hh, hl⟩
  | callCong _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | call hc => obtain ⟨a, b, c⟩ := ih hc hf hl hh hm; exact ⟨a.imp .call id, b, c⟩
  | callVal _ hv => intro hc; cases hc with | call hc => exact (hc.not_value hv).elim
  | appCong₁ s ih =>
    intro hc hf hl hh hm
    cases hc with
    | app₁ hc hb =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inl h)) hl hh hm
      exact ⟨a.imp (.app₁ · hb) (fun h h' => h'.elim h hb), b, c⟩
    | app₂ hb hc =>
      obtain ⟨a, b, c⟩ := step_bor hfn hB s hb hl hh
      exact ⟨.inl (.app₂ a hc), b, c⟩
  | appCong₂ hv _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | app₁ hc _ => exact (hc.not_value hv).elim
    | app₂ hb hc =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inr h)) hl hh hm
      exact ⟨a.imp (.app₂ hb) (fun h h' => h'.elim (hb.value hv) h), b, c⟩
  | appVal hv =>
    intro hc
    cases hc with
    | app₁ hc _ => cases hc
    | app₂ _ hc => exact (hc.not_value hv).elim
  | pairCong₁ s ih =>
    intro hc hf hl hh hm
    cases hc with
    | pair₁ hc hb =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inl h)) hl hh hm
      exact ⟨a.imp (.pair₁ · hb) (fun h h' => h'.elim h hb), b, c⟩
    | pair₂ hb hc =>
      obtain ⟨a, b, c⟩ := step_bor hfn hB s hb hl hh
      exact ⟨.inl (.pair₂ a hc), b, c⟩
  | pairCong₂ hv _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | pair₁ hc _ => exact (hc.not_value hv).elim
    | pair₂ hb hc =>
      obtain ⟨a, b, c⟩ := ih hc (fun x h => hf x (.inr h)) hl hh hm
      exact ⟨a.imp (.pair₂ hb) (fun h h' => h'.elim (hb.value hv) h), b, c⟩
  | fstCong _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | fst hc => obtain ⟨a, b, c⟩ := ih hc hf hl hh hm; exact ⟨a.imp .fst id, b, c⟩
  | sndCong _ ih =>
    intro hc hf hl hh hm
    cases hc with
    | snd hc => obtain ⟨a, b, c⟩ := ih hc hf hl hh hm; exact ⟨a.imp .snd id, b, c⟩
  | fstVal hv₁ hv₂ => intro hc; cases hc with | fst hc => exact (hc.not_value (.pair hv₁ hv₂)).elim
  | sndVal hv₁ hv₂ => intro hc; cases hc with | snd hc => exact (hc.not_value (.pair hv₁ hv₂)).elim
  | toImmCong s ih =>
    intro hc hf hl hh hm
    cases hc with
    | toImm => exact (Expr.IsValue.loc.no_step s).elim
    | toImmC hc => obtain ⟨a, b, c⟩ := ih hc hf hl hh hm; exact ⟨a.imp .toImmC id, b, c⟩
  | toImmVal hv hcp =>
    intro hc _ hl hh hm
    cases hc with
    | toImm =>
      -- The `toImm` is the last use: the copy is what's left. `l` is `mut`,
      -- so it's copied.
      obtain ⟨w, hw⟩ := hm
      cases hcp with
      | imm hw' => rw [hw] at hw'; cases hw'
      | seen _ hlk => simp at hlk
      | cell hw' _ hcw =>
        rw [hw] at hw'
        cases hw'
        obtain ⟨hh', hw'', _⟩ := hcw.noLoc hh (hh _ _ _ hw) hl (fun _ _ h => by simp at h)
        obtain ⟨ext, rfl⟩ := hcw.prefix
        exact ⟨.inr fun he => by simp only [Expr.HasLoc, List.length_append] at he; omega, hh'.append hw'',
          by simp; omega⟩
    | toImmC hc => exact (hc.not_value hv).elim
  | toImmKeep hv _ => intro hc; cases hc with | toImmC hc => exact (hc.not_value hv).elim

/-! ## A step keeps the check -/

theorem step_len {fns : List Fn} {h h' : Heap} {e e' : Expr} (hs : Step fns h e h' e') :
    h.length ≤ h'.length := by
  induction hs with
  | newCong _ ih | getCong _ ih | setCong₁ _ ih | setCong₂ _ _ ih | letCong _ ih | callCong _ ih
    | appCong₁ _ ih | appCong₂ _ _ ih | pairCong₁ _ ih | pairCong₂ _ _ ih | fstCong _ ih | sndCong _ ih
    | toImmCong _ ih => exact ih
  | newVal _ => simp
  | toImmVal _ hc => obtain ⟨e, rfl⟩ := hc.prefix; simp
  | toImmKeep _ hc => rw [Heap.freezeAt_length]; obtain ⟨e, rfl⟩ := hc.prefix; simp
  | setLoc _ _ => simp
  | getLoc _ | letVal _ | callVal _ _ | appVal _ | fstVal _ _ | sndVal _ _ => exact Nat.le_refl _

/-- A checked value has no `toImm`. -/
theorem Ok.value_noToImm {n : Nat} {L : Nat → Prop} {v : Expr} (hv : v.IsValue) (h : Ok B n L v) : v.NoToImm := by
  induction hv with
  | unit | int | loc => trivial
  | lam => cases h with | lam h => exact h
  | pair _ _ ih₁ ih₂ => cases h with | pair h₁ h₂ => exact ⟨ih₁ h₁, ih₂ h₂⟩

theorem Ok.up {n m : Nat} {L : Nat → Prop} {e : Expr} (h : Ok B n L e) (hn : n ≤ m) (N : Nat → Prop) :
    Ok B m (fun l => L l ∨ N l) e :=
  h.mono hn fun _ h => .inl h

theorem okNone {m : Nat} {L : Nat → Prop} {h h' : Heap} {e : Expr} (hok : Ok B m (fun l => L l ∨ False) e) :
    ∃ N : Nat → Prop, (∀ l, N l → h.length ≤ l ∧ l < h'.length ∧ Con B l e ∧ HeapNoLoc h' l ∧
        ∃ w, h'[l]? = some (true, w)) ∧
      Ok B m (fun l => L l ∨ N l) e :=
  ⟨fun _ => False, fun _ h => h.elim, hok⟩

/-- The functions in `B` at `ret` give back a new `mut` object: called, their
body is `mut [e₀]`, with `e₀` checked. -/
def Returns (B : Nat → Pos → Prop) (fns : List Fn) : Prop :=
  ∀ f fn, B f .ret → fns[f]? = some fn → ∀ ms τs (v : Expr) (n : Nat) (L : Nat → Prop), v.IsValue → v.NoToImm →
    v.NoFree → (∀ l, v.HasLoc l → l < n) →
    ∃ e₀, (fn.body.inst ms τs).subst fn.param v = .new true e₀ ∧ Ok B n L e₀

/-- Something new isn't in what's old. -/
theorem noLoc_of_lt {e : Expr} {n l : Nat} (he : ∀ l, e.HasLoc l → l < n) (hl : n ≤ l) : ¬ e.HasLoc l :=
  fun h => by have := he _ h; omega

/-- **A step keeps the check.** Its bound grows with the heap; and a new
`mut` object that a `let` names, whose body consumes it, is owned from now
on, consumed, and held by no object. -/
theorem step_ok {fns : List Fn} (hR : Returns B fns)
    (hfo : ∀ fn ∈ fns, Ok B 0 (fun _ => False) fn.body) {h h' : Heap} {e e' : Expr}
    (hs : Step fns h e h' e') {L : Nat → Prop} :
    Ok B h.length L e → (∀ l, e.HasLoc l → l < h.length) →
      (∀ (a : Nat) (b : Bool) (v : Expr), h[a]? = some (b, v) → ∀ l, v.HasLoc l → l < h.length) → HeapNoToImm h → e.NoFree →
      ∃ N : Nat → Prop, (∀ l, N l → h.length ≤ l ∧ l < h'.length ∧ Con B l e' ∧ HeapNoLoc h' l ∧
        ∃ w, h'[l]? = some (true, w)) ∧
        Ok B h'.length (fun l => L l ∨ N l) e' := by
  induction hs with
  | newCong _ ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | new hok =>
      obtain ⟨N, hN, hok'⟩ := ih hok hl hhl hnh hf
      exact ⟨N, fun l h => let ⟨a, b, c, d⟩ := hN l h; ⟨a, b, .new c, d⟩, .new hok'⟩
  | newVal hv => intro _ _ _ _ _; exact okNone .loc
  | getCong _ ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | get hok =>
      obtain ⟨N, hN, hok'⟩ := ih hok hl hhl hnh hf
      exact ⟨N, fun l h => let ⟨a, b, c, d⟩ := hN l h; ⟨a, b, .get c, d⟩, .get hok'⟩
  | getLoc hw => intro _ _ _ hnh _; exact okNone (.of_noToImm (hnh _ _ _ hw))
  | @setCong₁ _ a a' b s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | set ha hb =>
      obtain ⟨N, hN, ha'⟩ := ih ha (fun l h => hl l (.inl h)) hhl hnh
        (fun x h => hf x (.inl h))
      refine ⟨N, fun l h => ?_, .set ha' (hb.up (step_len s) N)⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .set₁ h₃ (noLoc_of_lt (fun l h => hl l (.inr h)) h₁), h₄⟩
  | @setCong₂ _ a b b' hv s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | set ha hb =>
      obtain ⟨N, hN, hb'⟩ := ih hb (fun l h => hl l (.inr h)) hhl hnh
        (fun x h => hf x (.inr h))
      refine ⟨N, fun l h => ?_, .set (ha.up (step_len s) N) hb'⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .set₂ (.none (noLoc_of_lt (fun l h => hl l (.inl h)) h₁)) h₃, h₄⟩
  | setLoc _ _ => intro _ _ _ _ _; exact okNone .unit
  | @letCong _ x a a' b s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | letE ha hb =>
      obtain ⟨N, hN, ha'⟩ := ih ha (fun l h => hl l (.inl h)) hhl hnh
        (fun y h => hf y (.inl h))
      refine ⟨N, fun l h => ?_, .letE ha' (hb.up (step_len s) N)⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .let₁ h₃ (noLoc_of_lt (fun l h => hl l (.inr h)) h₁), h₄⟩
    | own h₀ hc ho =>
      have hbl : ∀ l, h.length ≤ l → ¬ b.HasLoc l := fun l hl' => noLoc_of_lt (fun l h => hl l (.inr h)) hl'
      cases s with
      | newCong s' =>
        -- The object isn't made yet.
        obtain ⟨N, hN, ha'⟩ := ih (.new h₀) (fun l h => hl l (.inl h)) hhl
          hnh (fun y h => hf y (.inl h))
        cases ha' with
        | new h₀' =>
          refine ⟨N, fun l h => ?_, .own h₀' (fun l hl' => hc l (by have := step_len s'; omega))
            (fun l hl' => (ho l (by have := step_len s'; omega)).mono (Nat.le_refl _)
              fun l' h => h.imp_right .inl)⟩
          obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
          exact ⟨h₁, h₂, .let₁ h₃ (hbl l h₁), h₄⟩
      | @newVal v _ hv =>
        -- It's made now, at the end of the heap: owned from now on.
        refine ⟨fun l => l = h.length, fun l hl' => ?_, ?_⟩
        · subst hl'
          refine ⟨Nat.le_refl _, by simp, .bind (hbl _ (Nat.le_refl _)) (hc _ (Nat.le_refl _)), ?_⟩
          refine ⟨HeapNoLoc.append (fun a b' w hw hw' => ?_) (noLoc_of_lt (fun l h => hl l (.inl h)) (Nat.le_refl _)),
            by simp⟩
          have := hhl a b' w hw _ hw'
          omega
        · simp only [List.length_append, List.length_cons, List.length_nil]
          exact .bound (.inr rfl) ((ho _ (Nat.le_refl _)).mono (Nat.le_refl _) fun l' h => h.elim .inr .inl)
    | ownCall hfB h₀ hc ho =>
      have hbl : ∀ l, h.length ≤ l → ¬ b.HasLoc l := fun l hl' => noLoc_of_lt (fun l h => hl l (.inr h)) hl'
      cases s with
      | callCong s' =>
        -- The argument isn't computed yet.
        obtain ⟨N, hN, ha'⟩ := ih (.call h₀) (fun l h => hl l (.inl h)) hhl
          hnh (fun y h => hf y (.inl h))
        cases ha' with
        | call h₀' =>
          refine ⟨N, fun l h => ?_, .ownCall hfB h₀' (fun l hl' => hc l (by have := step_len s'; omega))
            (fun l hl' => (ho l (by have := step_len s'; omega)).mono (Nat.le_refl _)
              fun l' h => h.imp_right .inl)⟩
          obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
          exact ⟨h₁, h₂, .let₁ h₃ (hbl l h₁), h₄⟩
      | callVal hfi hv =>
        -- The body makes the new object: the `let` will own it.
        obtain ⟨e₀, hbe, h₀'⟩ := hR _ _ hfB hfi _ _ _ h.length (fun l => L l ∨ False) hv
          (h₀.value_noToImm hv) (fun y h => hf y (.inl h)) (fun l h => hl l (.inl h))
        rw [hbe]
        exact okNone (.own h₀' hc fun l hl' => (ho l hl').mono (Nat.le_refl _) fun l' h => h.imp_right .inl)
    | bound _ _ => exact (Expr.IsValue.loc.no_step s).elim
  | @letVal x v b hv =>
    intro hok hl hhl hnh hf
    cases hok with
    | letE ha hb =>
      exact okNone ((hb.subst x hv (ha.value_noToImm hv) (fun y h => hf y (.inl h)) (fun l h => hl l (.inl h))).up
        (Nat.le_refl _) _)
    | own _ _ _ | ownCall _ _ _ _ => cases hv
    | bound _ hb => exact okNone (hb.up (Nat.le_refl _) _)
  | callCong _ ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | call hok =>
      obtain ⟨N, hN, hok'⟩ := ih hok hl hhl hnh hf
      exact ⟨N, fun l h => let ⟨a, b, c, d⟩ := hN l h; ⟨a, b, .call c, d⟩, .call hok'⟩
  | @callVal f fn ms τs v hfi hv =>
    intro hok hl _ _ hf
    cases hok with
    | call hok =>
      have hb := ((hfo fn (List.mem_of_getElem? hfi)).mono (Nat.zero_le h.length)
        (L' := fun l => L l ∨ False) fun _ h => h.elim).inst ms τs
      exact okNone (hb.subst fn.param hv (hok.value_noToImm hv) hf hl)
  | @appCong₁ _ a a' b s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | app ha hb =>
      obtain ⟨N, hN, ha'⟩ := ih ha (fun l h => hl l (.inl h)) hhl hnh
        (fun x h => hf x (.inl h))
      refine ⟨N, fun l h => ?_, .app ha' (hb.up (step_len s) N)⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .app₁ h₃ (noLoc_of_lt (fun l h => hl l (.inr h)) h₁), h₄⟩
  | @appCong₂ _ a b b' hv s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | app ha hb =>
      obtain ⟨N, hN, hb'⟩ := ih hb (fun l h => hl l (.inr h)) hhl hnh
        (fun x h => hf x (.inr h))
      refine ⟨N, fun l h => ?_, .app (ha.up (step_len s) N) hb'⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .app₂ (.none (noLoc_of_lt (fun l h => hl l (.inl h)) h₁)) h₃, h₄⟩
  | @appVal c x body v hv =>
    intro hok _ _ _ _
    cases hok with
    | app hl hv' =>
      cases hl with
      | lam hb => exact okNone (.of_noToImm (hb.subst (hv'.value_noToImm hv) x))
  | @pairCong₁ _ a a' b s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | pair ha hb =>
      obtain ⟨N, hN, ha'⟩ := ih ha (fun l h => hl l (.inl h)) hhl hnh
        (fun x h => hf x (.inl h))
      refine ⟨N, fun l h => ?_, .pair ha' (hb.up (step_len s) N)⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .pair₁ h₃ (noLoc_of_lt (fun l h => hl l (.inr h)) h₁), h₄⟩
  | @pairCong₂ _ a b b' hv s ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | pair ha hb =>
      obtain ⟨N, hN, hb'⟩ := ih hb (fun l h => hl l (.inr h)) hhl hnh
        (fun x h => hf x (.inr h))
      refine ⟨N, fun l h => ?_, .pair (ha.up (step_len s) N) hb'⟩
      obtain ⟨h₁, h₂, h₃, h₄⟩ := hN l h
      exact ⟨h₁, h₂, .pair₂ (.none (noLoc_of_lt (fun l h => hl l (.inl h)) h₁)) h₃, h₄⟩
  | fstCong _ ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | fst hok =>
      obtain ⟨N, hN, hok'⟩ := ih hok hl hhl hnh hf
      exact ⟨N, fun l h => let ⟨a, b, c, d⟩ := hN l h; ⟨a, b, .fst c, d⟩, .fst hok'⟩
  | sndCong _ ih =>
    intro hok hl hhl hnh hf
    cases hok with
    | snd hok =>
      obtain ⟨N, hN, hok'⟩ := ih hok hl hhl hnh hf
      exact ⟨N, fun l h => let ⟨a, b, c, d⟩ := hN l h; ⟨a, b, .snd c, d⟩, .snd hok'⟩
  | fstVal _ _ =>
    intro hok _ _ _ _
    cases hok with
    | fst hok => cases hok with | pair h₁ _ => exact okNone (h₁.up (Nat.le_refl _) _)
  | sndVal _ _ =>
    intro hok _ _ _ _
    cases hok with
    | snd hok => cases hok with | pair _ h₂ => exact okNone (h₂.up (Nat.le_refl _) _)
  | toImmCong s _ =>
    intro hok
    cases hok with
    | toImm _ => exact (Expr.IsValue.loc.no_step s).elim
  | toImmVal _ hc =>
    intro hok _ _ hnh _
    cases hok with
    | toImm _ => exact okNone (.of_noToImm (hc.noToImm hnh trivial).2)
  | toImmKeep _ _ =>
    intro hok _ _ _ _
    cases hok with
    | toImm _ => exact okNone .loc

/-! ## The whole run -/

/-- **A step keeps the invariant.** -/
theorem step_inv {fns : List Fn} (hf : FnsOk fns) (hfo : ∀ fn ∈ fns, Ok B 0 (fun _ => False) fn.body)
    (hB : Borrows B fns) (hR : Returns B fns)
    {h h' : Heap} {e e' : Expr} (hs : Step fns h e h' e') {σ : HeapTy} {t : Ty} (hh : HeapOk fns σ h)
    (ht : HasType fns σ .none false Ctx.empty e t) (hnh : HeapNoToImm h) (hi : Inv B h e) : Inv B h' e' := by
  obtain ⟨L, hok, hL⟩ := hi
  have hfn : ∀ fn ∈ fns, fn.body.NoLocs := fun fn h => (hf fn h).1
  have hl : ∀ l, e.HasLoc l → l < h.length := fun l h' => by rw [hh.1]; exact ht.hasLoc_lt h'
  have hhl : ∀ (a : Nat) (b : Bool) (v : Expr), h[a]? = some (b, v) → ∀ l, v.HasLoc l → l < h.length := by
    intro a b v hv l hl'
    obtain ⟨⟨m, s⟩, hp⟩ : ∃ p, σ[a]? = some p :=
      ⟨_, List.getElem?_eq_getElem (hh.1 ▸ (List.getElem?_eq_some_iff.mp hv).1)⟩
    obtain ⟨_, _, w, hw, _, hwt⟩ := hh.2 a m s hp
    rw [hv] at hw
    cases hw
    rw [hh.1]
    exact hwt.hasLoc_lt hl'
  obtain ⟨N, hN, hok'⟩ := step_ok hR hfo hs hok hl hhl hnh ht.noFree
  refine ⟨fun l => L l ∨ N l, hok', fun l hl' => ?_⟩
  rcases hl' with hl' | hl'
  · obtain ⟨hlt, hc, hno, hm⟩ := hL l hl'
    -- It stays `mut`.
    have hm' : ∃ w, h'[l]? = some (true, w) := let ⟨_, hw⟩ := hm; step_keeps_tag hs ht hw
    rcases hc with hc | hc
    · obtain ⟨a, b, c⟩ := step_con hfn hB hs hc ht.noFree hlt hno hm
      exact ⟨c, a, b, hm'⟩
    · obtain ⟨a, b, c⟩ := step_noLoc hfn hs hc hlt hno
      exact ⟨c, .inr a, b, hm'⟩
  · obtain ⟨_, a, b, c⟩ := hN l hl'
    exact ⟨a, .inl b, c⟩

/-- However many steps it takes, a checked program keeps the invariant. -/
theorem steps_inv {fns : List Fn} (hf : FnsOk fns) (hfo : ∀ fn ∈ fns, Ok B 0 (fun _ => False) fn.body)
    (hB : Borrows B fns) (hR : Returns B fns)
    {h h' : Heap} {e e' : Expr} (hs : Steps fns h e h' e') :
    ∀ {σ : HeapTy} {t : Ty}, HeapOk fns σ h → HasType fns σ .none false Ctx.empty e t → HeapNoToImm h →
      Inv B h e → Inv B h' e' := by
  induction hs with
  | refl => exact fun _ _ _ hi => hi
  | step s _ ih =>
    intro σ t hh ht hnh hi
    obtain ⟨σ', _, hh', ht'⟩ := preservation hf s hh ht
    have hok := hi.choose_spec.1
    obtain ⟨hnh', _⟩ := step_lamNoToImm (fun fn h => (hfo fn h).lamNoToImm) s hnh hok.lamNoToImm
    exact ih hh' ht' hnh' (step_inv hf hfo hB hR s hh ht hnh hi)

/-- **A checked program can make `imm` in place.** If every `toImm` in a program,
and in its functions, is of a variable that holds a new `mut` object and
that it only reads, writes, and gives to functions that borrow it
(`Borrows`) before making it `imm` last (`Ok`), or a variable that holds
what a function that gives back a new object (`Returns`) returned, then the
program with every `toImm` in place computes what it does: whatever result
the program that copies gets, the other gets one that corresponds, and
the same integer. What each `toImm` makes `imm` must hold only `imm`
objects already when the program gets there (`Shallow`, #335), so that
it copies just the one object. -/
theorem checked_toImms_in_place_change_nothing {fns : List Fn} (hf : FnsOk fns)
    (hfo : ∀ fn ∈ fns, Ok B 0 (fun _ => False) fn.body) (hB : Borrows B fns) (hR : Returns B fns)
    {σ : HeapTy} {h h₁ : Heap} {e e₁ : Expr} {T : Ty}
    (hh : HeapOk fns σ h) (ht : HasType fns σ .none false Ctx.empty e T) (hnh : HeapNoToImm h)
    (hok : Ok B h.length (fun _ => False) e)
    (hsh : ∀ h' e', Steps fns h e h' e' → ∀ v, e'.ToImmArg v → Shallow h' v)
    (hs : Steps fns h e h₁ e₁) :
    ∃ (ρ : Nat → Nat → Prop) (h₂ : Heap) (e₂ : Expr),
      Steps (fns.map Fn.inPlace) h e.inPlace h₂ e₂ ∧ Rel ρ e₁.inPlace e₂ ∧ ∀ n, e₁ = .int n → e₂ = .int n :=
  toImms_in_place_change_nothing hf (fun fn h => (hfo fn h).lamNoToImm) hh ht hnh hok.lamNoToImm
    (fun _ _ _ hs' _ _ =>
      (steps_inv hf hfo hB hR hs' hh ht hnh ⟨fun _ => False, hok, fun _ h => h.elim⟩).uniqToImm (hsh _ _ hs'))
    hs

/-! ## Examples -/

/-- `let xs = mut [0]; xs[0] = 1; array_freeze(xs)` is checked: `xs` is
written, then made `imm` last, so the `toImm` can be in place. -/
example : Ok B 0 (fun _ => False)
    (.letE 0 (.new true (.int 0)) (.letE 1 (.set (.var 0) (.int 1)) (.toImm false (.var 0)))) := by
  refine .own .int (fun l _ => ?_) (fun l _ => ?_)
  · show Con B l (.letE 1 (.set (.loc l) (.int 1)) (.toImm false (.loc l)))
    exact .let₂ (.setP .loc (.none id)) .toImm
  · show Ok B _ _ (.letE 1 (.set (.loc l) (.int 1)) (.toImm false (.loc l)))
    exact .letE (.set .loc .int) (.toImm (.inl rfl))

/-- A `toImm` of what isn't a new object that a `let` names isn't. -/
example (n : Nat) : ¬ Ok B n (fun _ => False) (.toImm false (.var 0)) := fun h => by cases h

/-- Nor is one that comes before a write: `array_freeze(xs); xs[0] = 1`
writes to `xs` after its last use. -/
example (l : Nat) : ¬ Con B l (.letE 1 (.toImm false (.loc l)) (.set (.loc l) (.int 1))) := by
  intro h
  cases h with
  | let₁ _ hb => exact hb (.inl rfl)
  | let₂ ha _ => exact ha.not_toImmOf (.inl rfl)

/-- `fn fill(xs: mut [i32]) { xs[0] = 1; }` borrows its whole argument. -/
def fill₁ : Fn :=
  { assume := {}, param := 0, argTy := .cell .mutable .int, retTy := .unit, body := .set (.var 0) (.int 1) }

theorem fill₁_borrows : Borrows (fun f p => f = 0 ∧ p = .whole) [fill₁] := by
  intro f fn p hp hfn ms τs l v hv
  obtain ⟨rfl, rfl⟩ := hp
  simp only [List.getElem?_cons_zero, Option.some.injEq] at hfn
  subst hfn
  cases hv
  exact .setP .loc (.none id)

/-- `let xs = mut [0]; fill(xs); array_freeze(xs)` is checked: `fill` only
borrows `xs`, so the `toImm` can be in place. -/
example : Ok (fun f p => f = 0 ∧ p = .whole) 0 (fun _ => False)
    (.letE 0 (.new true (.int 0)) (.letE 1 (.call 0 [] [] (.var 0)) (.toImm false (.var 0)))) := by
  refine .own .int (fun l _ => ?_) (fun l _ => ?_)
  · show Con _ l (.letE 1 (.call 0 [] [] (.loc l)) (.toImm false (.loc l)))
    exact .let₂ (.callL ⟨rfl, rfl⟩ .loc) .toImm
  · show Ok _ _ _ (.letE 1 (.call 0 [] [] (.loc l)) (.toImm false (.loc l)))
    exact .letE (.call .loc) (.toImm (.inl rfl))

/-- `fn push(p: (mut [i32], i32)) { p.0[0] = p.1; }` borrows the array in
its argument, with what's put in it. -/
def push₁ : Fn :=
  { assume := {}, param := 0, argTy := .prod (.cell .mutable .int) .int, retTy := .unit,
    body := .set (.fst (.var 0)) (.snd (.var 0)) }

theorem push₁_borrows : Borrows (fun f p => f = 0 ∧ p = .fst) [push₁] := by
  intro f fn p hp hfn ms τs l v hv
  obtain ⟨rfl, rfl⟩ := hp
  simp only [List.getElem?_cons_zero, Option.some.injEq] at hfn
  subst hfn
  obtain ⟨w, rfl, hw, hwl⟩ := hv
  exact .setP (.fst hw hwl) (.sndD hw hwl)

/-- `let xs = mut [0]; push(xs, 1); array_freeze(xs)` is checked: the call
borrows `xs` in a tuple, so the `toImm` can be in place. -/
example : Ok (fun f p => f = 0 ∧ p = .fst) 0 (fun _ => False)
    (.letE 0 (.new true (.int 0)) (.letE 1 (.call 0 [] [] (.pair (.var 0) (.int 1))) (.toImm false (.var 0)))) := by
  refine .own .int (fun l _ => ?_) (fun l _ => ?_)
  · show Con _ l (.letE 1 (.call 0 [] [] (.pair (.loc l) (.int 1))) (.toImm false (.loc l)))
    exact .let₂ (.callT₁ ⟨rfl, rfl⟩ .int id) .toImm
  · show Ok _ _ _ (.letE 1 (.call 0 [] [] (.pair (.loc l) (.int 1))) (.toImm false (.loc l)))
    exact .letE (.call (.pair .loc .int)) (.toImm (.inl rfl))

/-- `fn id(xs: mut [i32]) -> mut [i32] = xs` doesn't borrow its argument: it
gives it back, so the caller would have another name for it. -/
def id₁ : Fn :=
  { assume := {}, param := 0, argTy := .cell .mutable .int, retTy := .cell .mutable .int, body := .var 0 }

example : ¬ Borrows (fun f p => f = 0 ∧ p = .whole) [id₁] :=
  fun h => (h 0 id₁ .whole ⟨rfl, rfl⟩ rfl [] [] 0 _ rfl).not_loc

/-- `fn make() -> mut [i32] = mut [0]` gives back a new object. -/
def make₁ : Fn :=
  { assume := {}, param := 0, argTy := .unit, retTy := .cell .mutable .int, body := .new true (.int 0) }

theorem make₁_returns : Returns (fun f p => f = 0 ∧ p = .ret) [make₁] := by
  intro f fn hp hfn ms τs v n L _ _ _ _
  obtain ⟨rfl, -⟩ := hp
  simp only [List.getElem?_cons_zero, Option.some.injEq] at hfn
  subst hfn
  exact ⟨.int 0, rfl, .int⟩

/-- `let xs = make(); xs[0] = 1; array_freeze(xs)` is checked: `make` gives
back a new object, which the `let` owns, so the `toImm` can be in place. -/
example : Ok (fun f p => f = 0 ∧ p = .ret) 0 (fun _ => False)
    (.letE 0 (.call 0 [] [] .unit) (.letE 1 (.set (.var 0) (.int 1)) (.toImm false (.var 0)))) := by
  refine .ownCall ⟨rfl, rfl⟩ .unit (fun l _ => ?_) (fun l _ => ?_)
  · show Con _ l (.letE 1 (.set (.loc l) (.int 1)) (.toImm false (.loc l)))
    exact .let₂ (.setP .loc (.none id)) .toImm
  · show Ok _ _ _ (.letE 1 (.set (.loc l) (.int 1)) (.toImm false (.loc l)))
    exact .letE (.set .loc .int) (.toImm (.inl rfl))

end BoltMut
