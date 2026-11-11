import BoltMut.Typing
import BoltMut.Lemmas

/-!
# Read-only literals (#320)

A literal that holds something that may change, like `(xs, 1)` for a `mut`
`xs`, can't be `imm`. It needn't be `mut` either: nothing else has a name
for a new object, so it can be made `ro`, which no one can change through
its name.

No object is made `ro` (`Mu.readonly`): a `ro` literal is a new `mut`
object whose only name is seen as `ro` from the start,
`let x = mut e in x`. So it needs no rule of its own, and everything proven
about the language holds for it as it is. `HasType.newRo` is the rule it
gets.

What it can hold is what a `ro` view can see: a `mut` array, seen as `ro`
too, but not a value of a type parameter `T` that the function doesn't
assume to be at most `ro` (`newRo_tparam`). A `T` can't be seen as `ro`,
since nothing says what's in it, so a `ro` literal doesn't help with a
tuple of generic values, like the `match (a, b)` of #415.
-/

namespace BoltMut

/-- `e` as a `ro` literal: a new `mut` object, named `x`, seen as `ro`. -/
def Expr.newRo (x : Nat) (e : Expr) : Expr := .letE x (.new true e) (.var x)

/-- A `ro` literal of a value of type `t` has type `ro t'`, for any `t'`
that `t` can be seen as and that a `ro` object can hold. -/
theorem HasType.newRo {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx}
    {x : Nat} {e : Expr} {t t' : Ty}
    (he : HasType fns σ Δ io Γ e t) (hm : AtMost Δ .mutable t) (hs : Sub Δ t t')
    (hr : AtMost Δ .readonly t') :
    HasType fns σ Δ io Γ (Expr.newRo x e) (.cell .readonly t') :=
  .letE (.new he hm)
    (.varSub (s := .cell .mutable t) (by simp [Ctx.update, muOf]) (.ro (m := .mutable) hs hr))

/-- What a `ro` literal can be typed as: its value's type `s`, with the
object `mut`, or seen as `ro`. -/
theorem HasType.newRo_inv {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx}
    {x : Nat} {e : Expr} {t : Ty} (h : HasType fns σ Δ io Γ (Expr.newRo x e) t) :
    ∃ s, HasType fns σ Δ io Γ e s ∧ Sub Δ (.cell .mutable s) t := by
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | new he _ =>
      cases h₂ with
      | var hx =>
        simp only [Ctx.update, ite_true, Option.some.injEq] at hx
        subst hx
        exact ⟨_, he, .refl _⟩
      | varSub hx hs =>
        simp only [Ctx.update, ite_true, Option.some.injEq] at hx
        subst hx
        exact ⟨_, he, hs⟩

/-- A type parameter is only seen as itself. -/
theorem Sub.tparam_inv {Δ : Assume} {i : Nat} {t : Ty} (h : Sub Δ (.tparam i) t) :
    t = .tparam i := by
  cases h
  rfl

/-- What a variable of a type parameter can be typed as: that parameter. -/
theorem HasType.var_tparam {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx}
    {x i : Nat} {t : Ty} (hx : Γ x = some (.tparam i)) (h : HasType fns σ Δ io Γ (.var x) t) :
    t = .tparam i := by
  cases h with
  | var hx' => rw [hx] at hx'; exact (Option.some.inj hx').symm
  | varSub hx' hs => rw [hx] at hx'; cases hx'; exact hs.tparam_inv

/-- Without assumptions, a `ro` literal can't hold a value of a type
parameter: `x : T` can't be seen as anything but `T`, and `T` may be `mut`,
which is more than `ro`. Like an `imm` one (#247), it needs `T ⊑ ro`. -/
theorem newRo_tparam {fns : List Fn} {σ : HeapTy} {io : Bool} {Γ : Ctx} {x y i : Nat} {t : Ty}
    (hy : Γ y = some (.tparam i)) :
    ¬ HasType fns σ .none io Γ (Expr.newRo x (.var y)) (.cell .readonly t) := by
  intro h
  obtain ⟨s, he, hs⟩ := h.newRo_inv
  have := HasType.var_tparam hy he
  subst this
  cases hs with
  | ro hs hr =>
    have := hs.tparam_inv
    subst this
    obtain ⟨μ', hμ', hle⟩ := hr
    rcases hμ' with rfl | hin
    · rcases Le.none_inv hle with h | h | h <;> exact nomatch h
    · exact nomatch hin

/-! ## Examples -/

/-- `let xs = mut [1]; (xs, 1)` as a `ro` literal: `ro (ro [i32], i32)`.
Through it, `xs` is seen as `ro` too, so neither can be changed. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 1)) (Expr.newRo 1 (.pair (.var 0) (.int 1))))
    (.cell .readonly (.prod (.cell .readonly .int) .int)) :=
  .letE (.new .int trivial)
    (.newRo (.pair (.var (t := .cell .mutable .int) (by simp [Ctx.update, muOf])) .int)
      ⟨⟨.refl _, trivial⟩, trivial⟩
      (.prod (.ro (.refl _) trivial) (.refl _))
      ⟨⟨.refl _, trivial⟩, trivial⟩)

/-- The same literal can't be `imm`: an `imm` object can't hold a `mut`
array, nor one seen as `ro`, since it may change. -/
example (t : Ty) : ¬ HasType [] [] .none false
    ((Ctx.empty.update 0 (.cell .mutable .int)))
    (.new false (.pair (.var 0) (.int 1))) t := by
  intro h
  cases h with
  | new hp hd =>
    cases hp with
    | pair hx _ =>
      cases hx with
      | var hx =>
        simp only [Ctx.update, ite_true, Option.some.injEq] at hx
        subst hx
        exact nomatch Le.none_immutable hd.1.1
      | varSub hx hs =>
        simp only [Ctx.update, ite_true, Option.some.injEq] at hx
        subst hx
        rcases hs.cell_inv with rfl | ⟨_, rfl, _⟩ <;> exact nomatch Le.none_immutable hd.1.1

/-- A `ro` literal holding a `T` is fine when the function assumes
`T ⊑ ro`. -/
example : HasType [] [] { tys := [(0, .readonly)] } false (Ctx.empty.update 0 (.tparam 0))
    (Expr.newRo 1 (.var 0)) (.cell .readonly (.tparam 0)) :=
  .newRo (t := .tparam 0) (.var (by simp [Ctx.update])) ⟨.mutable, .inl rfl, .refl _⟩ (.refl _)
    ⟨.readonly, .inr (by simp), .refl _⟩

end BoltMut
