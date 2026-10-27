import BoltMut.Immutability

/-!
# Examples

What the rules accept and reject, as in Bolt.
-/

namespace BoltMut

/-- `let xs = mut [1]; xs[0] = 2`: a `mut` object can be changed. -/
example : HasType [] [] false Ctx.empty (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.new .int trivial) (.set (.var rfl) .int)

/-- `let xs = [1]; xs[0] = 2`: an `imm` one can't. -/
example (t : Ty) :
    ¬ HasType [] [] false Ctx.empty (.letE 0 (.new false (.int 1)) (.set (.var 0) (.int 2))) t := by
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
example (t : Ty) : ¬ HasType [] [] false Ctx.empty (.new false (.new true (.int 1))) t := by
  intro h
  cases h with
  | new hin hd =>
    cases hin with
    | new _ _ =>
      simp [AtMost, Mu.Le, muOf] at hd

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
example : HasType [first] [] false Ctx.empty (.call 0 [.mutable] (.new true (.int 1))) .int := by
  show HasType _ _ _ _ _ (first.retTy.subst [.mutable])
  exact .call rfl rfl (.new .int trivial)

example : HasType [first] [] false Ctx.empty (.call 0 [.immutable] (.new false (.int 1))) .int := by
  show HasType _ _ _ _ _ (first.retTy.subst [.immutable])
  exact .call rfl rfl (.new .int trivial)

/-- But a generic function can't change what it's given:
`fn poke(xs: 'm [i32]) { xs[0] = 1; }` is rejected, since `'m` may be `imm`. -/
example (t : Ty) :
    ¬ HasType [] [] false (Ctx.empty.update 0 (.cell (.param 0) .int)) (.set (.var 0) (.int 1)) t := by
  intro h
  cases h with
  | set hx _ =>
    cases hx with
    | var hx => simp [Ctx.update] at hx

/-! ## Closures -/

/-- `let c = mut [0]; || c[0] = 1`: a closure that changes what it captured
is `mut`. -/
example : HasType [] [] false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.lam true 1 (.set (.var 0) (.int 1)))) (.fn .mutable .unit .unit) :=
  .letE (.new .int trivial) (.lam (.set (.var rfl) .int))

/-- It can't be `imm`: an `imm` closure doesn't see `c`, which is `mut`. -/
example (t : Ty) : ¬ HasType [] [] false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.lam false 1 (.set (.var 0) (.int 1)))) t := by
  intro h
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | new _ _ =>
      cases h₂ with
      | lam hb =>
        cases hb with
        | set hx _ =>
          cases hx with
          | var hx => simp [Ctx.update, Ctx.capture, DeepImm, muOf] at hx

/-- The hole from #244: `let c = mut [0]; [|| c[0] = 1]`, an `imm` object
holding a closure that changes `c`, is rejected, since the closure is
`mut`. -/
example (t : Ty) : ¬ HasType [] [] false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.new false (.lam true 1 (.set (.var 0) (.int 1))))) t := by
  intro h
  cases h with
  | letE _ h₂ =>
    cases h₂ with
    | new hl hd =>
      cases hl with
      | lam _ =>
        simp [AtMost, Mu.Le, muOf] at hd

/-- `let xs = [1]; [|| xs[0]]`: an `imm` closure that reads an `imm` object
can be held by an `imm` one. -/
example : HasType [] [] false Ctx.empty
    (.letE 0 (.new false (.int 1)) (.new false (.lam false 1 (.get (.var 0)))))
    (.cell .immutable (.fn .immutable .unit .int)) :=
  .letE (.new .int trivial)
    (.new (.lam (.get (.var (t := .cell .immutable .int) (by simp [Ctx.update, Ctx.capture, DeepImm, muOf])))) (.inl rfl))

/-! ## Structs

A struct is a cell holding the tuple of its fields. Here
`struct Point { x: i32 }` is `cell m int`, and
`struct Line { a: Point, n: i32 }` is `cell m (prod (cell m int) int)`: the
field `a` has the struct's mutability. -/

/-- `let p = mut Line { a: mut Point { x: 1 }, n: 0 }; p.n = 5`: changing a
field replaces the tuple, keeping the other field. -/
example : HasType [] [] false Ctx.empty
    (.letE 0 (.new true (.pair (.new true (.int 1)) (.int 0)))
      (.set (.var 0) (.pair (.fst (.get (.var 0))) (.int 5)))) .unit :=
  .letE (.new (.pair (.new .int trivial) .int) ⟨⟨.inr (.inl rfl), trivial⟩, trivial⟩)
    (.set (.var rfl) (.pair (.fst (.get (.var rfl))) .int))

/-- #246: `Line { n: 5, ..l }` with a `mut` `l` copies `l.a`, a `mut Point`,
into an `imm Line`, so it's rejected: what's shared keeps its type. -/
example (t : Ty) : ¬ HasType [] [] false Ctx.empty
    (.letE 0 (.new true (.pair (.new true (.int 1)) (.int 0)))
      (.new false (.pair (.fst (.get (.var 0))) (.int 5)))) t := by
  intro h
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | new hl _ =>
      cases hl with
      | pair hp₁ _ =>
        cases hp₁ with
        | new _ _ =>
          cases h₂ with
          | new hp hd =>
            cases hp with
            | pair ha _ =>
              cases ha with
              | fst hg =>
                cases hg with
                | get hv =>
                  cases hv with
                  | var hx =>
                    simp only [Ctx.update, ite_true, Option.some.injEq, Ty.cell.injEq, Ty.prod.injEq] at hx
                    obtain ⟨_, rfl, _⟩ := hx
                    simp [AtMost, Mu.Le, muOf] at hd

/-- But `Line { a: Point { x: 0 }, ..l }` gives the shared field anew, and
copies only the number. -/
example : HasType [] [] false Ctx.empty
    (.letE 0 (.new true (.pair (.new true (.int 1)) (.int 0)))
      (.new false (.pair (.new false (.int 0)) (.snd (.get (.var 0))))))
    (.cell .immutable (.prod (.cell .immutable .int) .int)) :=
  .letE (.new (.pair (.new .int trivial) .int) ⟨⟨.inr (.inl rfl), trivial⟩, trivial⟩)
    (.new (.pair (.new .int trivial) (.snd (.get (.var rfl)))) ⟨⟨.inl rfl, trivial⟩, trivial⟩)

end BoltMut
