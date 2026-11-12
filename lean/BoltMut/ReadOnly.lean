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
too, and a value of a type parameter `T`, seen as `ro T`
(`HasType.newRo_tparam`). So a tuple of generic values, like the
`match (a, b)` of #415, can be a `ro` literal with nothing assumed.
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
    (.sub (.var (t := .cell .mutable t) (by simp [Ctx.update, muOf])) (.ro (m := .mutable) hs hr))

/-- What a `ro` literal can be typed as: its value's type `s`, with the
object `mut`, or seen as `ro`. -/
theorem HasType.newRo_inv {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx}
    {x : Nat} {e : Expr} {t : Ty} (h : HasType fns σ Δ io Γ (Expr.newRo x e) t) :
    ∃ s, HasType fns σ Δ io Γ e s ∧ Sub Δ (.cell .mutable s) t := by
  obtain ⟨t₁, t₂, h₁, h₂, hs⟩ := h.letE_inv
  obtain ⟨s, he, _, hs₁⟩ := h₁.new_inv
  obtain ⟨s', hx, hs₂⟩ := h₂.var_inv
  simp only [Ctx.update, ite_true, Option.some.injEq] at hx
  subst hx
  exact ⟨s, he, hs₁.trans (hs₂.trans hs)⟩

/-- A `ro` literal can hold a value of a type parameter, with no
assumption: `x : T` is seen as `ro T`, which a `ro` object can hold. -/
theorem HasType.newRo_tparam {fns : List Fn} {σ : HeapTy} {Δ : Assume} {io : Bool} {Γ : Ctx}
    {x y i : Nat} (hy : Γ y = some (.tparam i)) :
    HasType fns σ Δ io Γ (Expr.newRo x (.var y)) (.cell .readonly (.roParam i)) :=
  .newRo (.var hy) ⟨.mutable, .inl rfl, .refl _⟩ (.tparam i) (.refl _)

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
  obtain ⟨s, hp, hd, _⟩ := h.new_inv
  obtain ⟨a, b, hx, _, hs⟩ := hp.pair_inv
  -- Nothing is seen as `ro` in what's `imm` all the way down.
  have hd' := hx.var_update.atMost_imm (hs.atMost_imm hd).1
  exact nomatch Le.none_immutable hd'.1

/-- A `ro` literal holding a `T` with nothing assumed: `ro (ro T)`, as in
#415's `match (a, b)`. -/
example : HasType [] [] .none false (Ctx.empty.update 0 (.tparam 0))
    (Expr.newRo 1 (.var 0)) (.cell .readonly (.roParam 0)) :=
  .newRo_tparam (by simp [Ctx.update])

/-- With `T ⊑ ro` assumed, the literal can hold `T` itself. -/
example : HasType [] [] { tys := [(0, .readonly)] } false (Ctx.empty.update 0 (.tparam 0))
    (Expr.newRo 1 (.var 0)) (.cell .readonly (.tparam 0)) :=
  .newRo (t := .tparam 0) (.var (by simp [Ctx.update])) ⟨.mutable, .inl rfl, .refl _⟩ (.refl _)
    ⟨.readonly, .inr (by simp), .refl _⟩

end BoltMut
