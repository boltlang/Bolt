import BoltMut.Typing
import BoltMut.Lemmas

/-!
# Literals whose mutability is inferred (#415)

A literal like `(a, b)` or `[x]` without `mut` is `imm`, so a tuple of
values that may change, like the `match (a, b)` of two `Option[T]`s, is an
error. Its mutability could be inferred instead: the least it can be,
`imm` when what it holds is `imm` all the way down, and `ro` otherwise, but
never `mut`. Nothing is `mut` unless it's asked for: a literal is `mut` only
if its mutability is written, as `mut [x]`, or as a parameter of the
function, like the `'n` of `'n [x]`, which a call can make `mut`.

In the model, a literal's mutability is one that's written, or a
mutability parameter in scope (`Mu.Lit`). That one without a written
mutability is only `imm` or `ro` is about what the checker infers
(`Problem.principal_lit_ir`).

This file proves what that needs of the rules:

- A literal can always be `ro`, with what it holds seen as `ro`
  (`HasType.lit_ro`), and `imm` when that's `imm` all the way down
  (`HasType.lit_imm`).
- Making it `imm` when it can be loses nothing: the `imm` literal fits
  wherever the `ro` one does (`HasType.lit_imm_fits`).
- An inferred literal whose mutability is a parameter at most `ro`, as in
  `fn mk['m ⊑ ro, T ⊑ 'm](x: T) -> 'm [T]`, is given `imm` or `ro` by a
  call, never `mut` (`mk` and its examples).
- One whose mutability is written as a parameter, `'n [x]`, can be given
  `mut` too (`mkAny`).
- A program that runs has no mutability parameters in scope, so it has no
  literal whose mutability is a parameter (`HasType.new_param_none`).

That the checker finds the least mutability isn't proven: like the rest of
the proof, this is about the typing rules, with what's inferred given.
-/

namespace BoltMut

section

variable {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx} {e : Expr} {t : Ty}

/-- A literal can always be `ro`: what it holds is seen as `ro`. -/
theorem HasType.lit_ro (he : HasType fns σ Δ io Γ e t) :
    HasType fns σ Δ io Γ (.new .readonly e) (.cell .readonly t.ro) :=
  .new (.sub he (Sub.to_ro Δ t)) (AtMost.of_ro Δ t)

/-- A literal can be `imm` when what it holds is `imm` all the way down. -/
theorem HasType.lit_imm (he : HasType fns σ Δ io Γ e t) (hd : AtMost Δ .immutable t) :
    HasType fns σ Δ io Γ (.new .immutable e) (.cell .immutable t) :=
  .new he hd

/-- An `imm` literal can be seen as the `ro` one. -/
theorem Sub.lit_imm_ro (Δ : Assume) (t : Ty) : Sub Δ (.cell .immutable t) (.cell .readonly t.ro) :=
  .ro (Sub.to_ro Δ t) (AtMost.of_ro Δ t)

/-- **The least mutability loses nothing**: where the `ro` literal fits, so
does the `imm` one, when it can be `imm`. -/
theorem HasType.lit_imm_fits {T : Ty} (he : HasType fns σ Δ io Γ e t) (hd : AtMost Δ .immutable t)
    (hT : Sub Δ (.cell .readonly t.ro) T) : HasType fns σ Δ io Γ (.new .immutable e) T :=
  .sub (.lit_imm he hd) ((Sub.lit_imm_ro Δ t).trans hT)

/-- A program that runs assumes nothing, so none of its literals has a
parameter as its mutability: what a function's literal gets is given by
the call. -/
theorem HasType.new_param_none {j : Nat} (h : HasType fns σ .none io Γ (.new (.param j) e) t) :
    False := by
  obtain ⟨_, _, _, hl, _⟩ := h.new_inv
  exact Nat.not_lt_zero j hl

end

/-! ## A function whose literal's mutability is a parameter -/

/-- What `fn mk[T](x: T) = [x]` assumes once its literal's mutability is
inferred: its own `'m`, `param 0`, at most `ro`, and `T ⊑ 'm`. -/
def mkAssume : Assume := { mus := [(.param 0, .readonly)], tys := [(0, .param 0)], nmu := 1 }

/-- `fn mk['m ⊑ ro, T ⊑ 'm](x: T) -> 'm [T] = [x]`. -/
def mk : Fn :=
  { assume := mkAssume, param := 0, argTy := .tparam 0, retTy := .cell (.param 0) (.tparam 0),
    body := .new (.param 0) (.var 0) }

theorem mk_ok : FnsOk [mk] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .new (.var rfl) ⟨.param 0, .inr (by simp [mk, mkAssume]), .refl _⟩
    (by simp [Mu.Lit, mk, mkAssume])⟩

/-- `mk(1)` at `'m = imm`: an `imm [i32]`. -/
example : HasType [mk] [] .none false Ctx.empty (.call 0 [.immutable] [.int] (.int 1))
    (.cell .immutable .int) := by
  show HasType _ _ _ _ _ _ (mk.retTy.inst [.immutable] [.int])
  refine .call rfl ⟨?_, ?_, ?_⟩ .int
  · intro p h; simp [mk, mkAssume] at h; subst h; exact .bot _
  · intro p h; simp [mk, mkAssume] at h; subst h; trivial
  · refine ⟨fun i => ?_, fun j _ => ?_⟩
    · cases i <;> trivial
    · cases j <;> trivial

/-- `let xs = mut [1]; mk(xs)` at `'m = ro`, with `xs` seen as `ro [i32]`:
a `ro [ro [i32]]`, through which `xs` can't be changed. -/
example : HasType [mk] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 1)) (.call 0 [.readonly] [.cell .readonly .int] (.var 0)))
    (.cell .readonly (.cell .readonly .int)) := by
  refine .letE (.new .int trivial) ?_
  show HasType _ _ _ _ _ _ (mk.retTy.inst [.readonly] [.cell .readonly .int])
  refine .call rfl ⟨?_, ?_, ?_⟩ (.sub (.var (t := .cell .mutable .int) (by simp [Ctx.update]))
    (.ro (.refl _) trivial))
  · intro p h; simp [mk, mkAssume] at h; subst h; exact .refl _
  · intro p h; simp [mk, mkAssume] at h; subst h; exact ⟨.refl _, trivial⟩
  · refine ⟨fun i => ?_, fun j _ => ?_⟩
    · cases i with
      | zero => exact ⟨.top _, trivial⟩
      | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial
    · cases j <;> trivial

/-- But not at `'m = mut`: an inferred literal is never `mut`. -/
example (e : Expr) (τ t : Ty) :
    ¬ HasType [mk] [] .none false Ctx.empty (.call 0 [.mutable] [τ] e) t := by
  intro h
  obtain ⟨_, hf, hs, _, _⟩ := h.call_inv
  simp at hf
  subst hf
  have := hs.1 (.param 0, .readonly) (by simp [mk, mkAssume])
  rcases (show Le .none .mutable .readonly from this).none_inv with h | h | h <;> cases h

/-- And `mk`'s body can't change what it makes: `'m` may be `imm`. -/
example (t : Ty) :
    ¬ HasType [] [] mkAssume false Ctx.empty (.set (.new (.param 0) (.int 1)) (.int 2)) t := by
  intro h
  obtain ⟨_, hn, _, _⟩ := h.set_inv
  obtain ⟨_, _, _, _, hs⟩ := hn.new_inv
  cases hs.to_mut_cell

/-! ## A literal with a written mutability parameter -/

/-- `fn mkAny['n, T: 'n](x: T) -> 'n [T] = 'n [x]`: the literal's
mutability is written, so it can be anything, `mut` too. -/
def mkAny : Fn :=
  { assume := { tys := [(0, .param 0)], nmu := 1 }, param := 0, argTy := .tparam 0,
    retTy := .cell (.param 0) (.tparam 0), body := .new (.param 0) (.var 0) }

theorem mkAny_ok : FnsOk [mkAny] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .new (.var rfl) ⟨.param 0, .inr (by simp [mkAny]), .refl _⟩
    (by simp [Mu.Lit, mkAny])⟩

/-- `mkAny(mut [1])` at `'n = mut`: a `mut [mut [i32]]`, which its caller
can change. -/
example : HasType [mkAny] [] .none false Ctx.empty
    (.set (.call 0 [.mutable] [.cell .mutable .int] (.new .mutable (.int 1))) (.new .mutable (.int 2)))
    .unit := by
  refine .set ?_ (.new .int trivial)
  show HasType _ _ _ _ _ _ (mkAny.retTy.inst [.mutable] [.cell .mutable .int])
  refine .call rfl ⟨?_, ?_, fun i => ?_, fun j _ => ?_⟩ (.new .int trivial)
  · intro p h; simp [mkAny] at h
  · intro p h; simp [mkAny] at h; subst h; exact ⟨.refl _, trivial⟩
  · cases i with
    | zero => exact ⟨.refl _, trivial⟩
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial
  · cases j <;> trivial

/-- `mkAny(1)` at `'n = imm`: an `imm [i32]`. -/
example : HasType [mkAny] [] .none false Ctx.empty (.call 0 [.immutable] [.int] (.int 1))
    (.cell .immutable .int) := by
  show HasType _ _ _ _ _ _ (mkAny.retTy.inst [.immutable] [.int])
  refine .call rfl ⟨?_, ?_, fun i => ?_, fun j _ => ?_⟩ .int
  · intro p h; simp [mkAny] at h
  · intro p h; simp [mkAny] at h; subst h; trivial
  · cases i <;> trivial
  · cases j <;> trivial

/-- A function can't make a literal of a mutability parameter that isn't
its own: `'n [1]` in a function without `'n`. -/
example (t : Ty) : ¬ HasType [] [] {} false Ctx.empty (.new (.param 0) (.int 1)) t := by
  intro h
  obtain ⟨_, _, _, hl, _⟩ := h.new_inv
  exact Nat.not_lt_zero 0 hl

end BoltMut
