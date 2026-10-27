import BoltMut.Immutability

/-!
# Examples

What the rules accept and reject, as in Bolt.
-/

namespace BoltMut

/-- `let xs = mut [1]; xs[0] = 2`: a `mut` object can be changed. -/
example : HasType [] [] Ctx.empty (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.new .int (fun h => nomatch h)) (.set (.var rfl) .int)

/-- `let xs = [1]; xs[0] = 2`: an `imm` one can't. -/
example (t : Ty) :
    ¬ HasType [] [] Ctx.empty (.letE 0 (.new false (.int 1)) (.set (.var 0) (.int 2))) t := by
  intro h
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | new _ _ =>
      cases h₂ with
      | set hx _ =>
        cases hx with
        | var hx => simp [Ctx.update, muOf] at hx

/-- `[mut [1]]`: an `imm` object can't hold a `mut` one, since `imm` is
deep. -/
example (t : Ty) : ¬ HasType [] [] Ctx.empty (.new false (.new true (.int 1))) t := by
  intro h
  cases h with
  | new hin hd =>
    cases hin with
    | new _ _ =>
      have := hd rfl
      simp [DeepImm, muOf] at this

/-- A generic function that reads what it's given:
`fn first(xs: 'm [i32]) -> i32 = xs[0]`, with the mutability `'m` as its
parameter `0`. -/
def first : Fn :=
  { arity := 1, param := 0, argTy := .cell (.param 0) .int, retTy := .int, body := .get (.var 0) }

theorem first_ok : FnsOk [first] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .get (.var rfl)⟩

/-- It can be called with a `mut` object and with an `imm` one. -/
example : HasType [first] [] Ctx.empty (.call 0 [.mutable] (.new true (.int 1))) .int := by
  show HasType _ _ _ _ (first.retTy.subst [.mutable])
  exact .call rfl rfl (.new .int (fun h => nomatch h))

example : HasType [first] [] Ctx.empty (.call 0 [.immutable] (.new false (.int 1))) .int := by
  show HasType _ _ _ _ (first.retTy.subst [.immutable])
  exact .call rfl rfl (.new .int (fun _ => trivial))

/-- But a generic function can't change what it's given:
`fn poke(xs: 'm [i32]) { xs[0] = 1; }` is rejected, since `'m` may be `imm`. -/
example (t : Ty) :
    ¬ HasType [] [] (Ctx.empty.update 0 (.cell (.param 0) .int)) (.set (.var 0) (.int 1)) t := by
  intro h
  cases h with
  | set hx _ =>
    cases hx with
    | var hx => simp [Ctx.update] at hx

end BoltMut
