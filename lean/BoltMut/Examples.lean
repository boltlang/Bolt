import BoltMut.Purity

/-!
# Examples

What the rules accept and reject, as in Bolt.
-/

namespace BoltMut

/-- `let xs = mut [1]; xs[0] = 2`: a `mut` object can be changed. -/
example : HasType [] [] .none false Ctx.empty (.letE 0 (.new .mutable (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.new .int trivial) (.set (.var rfl) .int)

/-- `let xs = [1]; xs[0] = 2`: an `imm` one can't. -/
example (t : Ty) :
    ¬ HasType [] [] .none false Ctx.empty (.letE 0 (.new .immutable (.int 1)) (.set (.var 0) (.int 2))) t := by
  intro h
  obtain ⟨t₁, _, h₁, h₂, _⟩ := h.letE_inv
  obtain ⟨_, _, _, _, hs₁⟩ := h₁.new_inv
  obtain ⟨_, hx, _, _⟩ := h₂.set_inv
  -- Seeing it as `ro` doesn't make it changeable either.
  cases (hs₁.trans hx.var_update).to_mut_cell

/-- `[mut [1]]`: an `imm` object can't hold a `mut` one, since `imm` is
deep. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty (.new .immutable (.new .mutable (.int 1))) t := by
  intro h
  obtain ⟨_, hin, hd, _, _⟩ := h.new_inv
  obtain ⟨_, _, _, _, hs⟩ := hin.new_inv
  exact nomatch Le.none_immutable (hs.atMost_imm hd).1

/-- A generic function that reads what it's given:
`fn first(xs: 'm [i32]) -> i32 = xs[0]`, with the mutability `'m` as its
parameter `0`. -/
def first : Fn :=
  { assume := {}, param := 0, argTy := .cell (.param 0) .int, retTy := .int, body := .get (.var 0) }

/-- Arguments that only need every type argument to be well-formed. -/
theorem sat_none (Δ : Assume) (ms : List Mu) : Sat Δ {} ms [] := by
  refine ⟨?_, ?_, ?_⟩
  · intro p h; simp at h
  · intro p h; simp at h
  · intro i; simp only [List.getElem?_nil, Option.getD_none]; trivial

/-- It can be called with a `mut` object and with an `imm` one. -/
example : HasType [first] [] .none false Ctx.empty (.call 0 [.mutable] [] (.new .mutable (.int 1))) .int := by
  show HasType _ _ _ _ _ _ (first.retTy.inst [.mutable] [])
  exact .call rfl (sat_none _ _) (.new .int trivial)

example : HasType [first] [] .none false Ctx.empty (.call 0 [.immutable] [] (.new .immutable (.int 1))) .int := by
  show HasType _ _ _ _ _ _ (first.retTy.inst [.immutable] [])
  exact .call rfl (sat_none _ _) (.new .int trivial)

/-- But a generic function can't change what it's given:
`fn poke(xs: 'm [i32]) { xs[0] = 1; }` is rejected, since `'m` may be `imm`.
-/
example (t : Ty) :
    ¬ HasType [] [] .none false (Ctx.empty.update 0 (.cell (.param 0) .int)) (.set (.var 0) (.int 1)) t := by
  intro h
  obtain ⟨_, hx, _, _⟩ := h.set_inv
  cases hx.var_update.to_mut_cell

/-! ## Closures -/

/-- `let c = mut [0]; || c[0] = 1`: a closure that changes what it captured
is `mut`. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 0)) (.lam true 1 (.set (.var 0) (.int 1)))) (.fn .mutable .unit .unit) :=
  .letE (.new .int trivial) (.lam (fun _ _ h => h) (fun h => nomatch h) (.set (.var rfl) .int))

/-- It can't be `imm`: an `imm` closure only sees what's `imm` all the way
down, and `c` is `mut`. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 0)) (.lam false 1 (.set (.var 0) (.int 1)))) t := by
  intro h
  obtain ⟨_, _, _, h₂, _⟩ := h.letE_inv
  obtain ⟨_, _, _, _, himm, hb, _⟩ := h₂.lam_inv
  obtain ⟨_, hx, _, _⟩ := hb.set_inv
  obtain ⟨s₀, hx, hs⟩ := hx.var_inv
  cases hs.to_mut_cell
  simp only [Ctx.update] at hx
  exact nomatch Le.none_immutable (himm rfl 0 _ hx).1

/-- The hole from #244: `let c = mut [0]; [|| c[0] = 1]`, an `imm` object
holding a closure that changes `c`, is rejected, since the closure is
`mut`. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 0)) (.new .immutable (.lam true 1 (.set (.var 0) (.int 1))))) t := by
  intro h
  obtain ⟨_, _, _, h₂, _⟩ := h.letE_inv
  obtain ⟨_, hl, hd, _, _⟩ := h₂.new_inv
  obtain ⟨_, _, _, _, _, _, hs⟩ := hl.lam_inv
  exact nomatch Le.none_immutable (hs.atMost_imm hd).none_fn

/-- `let xs = [1]; [|| xs[0]]`: an `imm` closure that reads an `imm` object
can be held by an `imm` one. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .immutable (.int 1)) (.new .immutable (.lam false 1 (.get (.var 0)))))
    (.cell .immutable (.fn .immutable .unit .int)) := by
  refine .letE (.new .int trivial) (.new (.lam (fun _ _ h => h) ?_ (.get (.var rfl))) (.inl (.refl _)))
  intro _ y s hy
  simp only [Ctx.update] at hy
  split at hy
  · cases hy
    exact ⟨.refl _, trivial⟩
  · cases hy

/-! ## Structs

A struct is a cell holding the tuple of its fields. Here
`struct Point { x: i32 }` is `cell m int`, and
`struct Line { a: Point, n: i32 }` is `cell m (prod (cell m int) int)`: the
field `a` has the struct's mutability. -/

/-- `let p = mut Line { a: mut Point { x: 1 }, n: 0 }; p.n = 5`: changing a
field replaces the tuple, keeping the other field. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.pair (.new .mutable (.int 1)) (.int 0)))
      (.set (.var 0) (.pair (.fst (.get (.var 0))) (.int 5)))) .unit :=
  .letE (.new (.pair (.new .int trivial) .int) ⟨⟨.top _, trivial⟩, trivial⟩)
    (.set (.var rfl) (.pair (.fst (.get (.var rfl))) .int))

/-- #246: `Line { n: 5, ..l }` with a `mut` `l` copies `l.a`, a `mut Point`,
into an `imm Line`, so it's rejected: what's shared keeps its type. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.pair (.new .mutable (.int 1)) (.int 0)))
      (.new .immutable (.pair (.fst (.get (.var 0))) (.int 5)))) t := by
  intro h
  obtain ⟨tl, _, hl, hbody, _⟩ := h.letE_inv
  -- `l` is a `mut` line holding `(p, 0)`, with `p` a `mut` point.
  obtain ⟨line, hpair, _, _, hline⟩ := hl.new_inv
  obtain ⟨pt, _, hpt, _, hfields⟩ := hpair.pair_inv
  obtain ⟨_, _, _, _, hmut⟩ := hpt.new_inv
  -- The new line is `imm` all the way down, and so is its first field.
  obtain ⟨_, hnew, hd, _, _⟩ := hbody.new_inv
  obtain ⟨a, _, ha, _, hnew'⟩ := hnew.pair_inv
  have hda := (hnew'.atMost_imm hd).1
  -- That field is `l.a`, read through `l`.
  obtain ⟨a₁, _, hfst, ha₁⟩ := ha.fst_inv
  obtain ⟨_, _, hget, hread⟩ := hfst.get_inv
  obtain ⟨a₂, b₂, rfl, ha₂, _⟩ := hread.to_prod
  -- What `l` holds is a subtype of what's read through it, also when `l`
  -- is seen as `ro`.
  have hheld : Sub .none line (.prod a₂ b₂) := by
    obtain ⟨_, _, h, hs, _⟩ := (hline.trans hget.var_update).of_cell
    cases h; exact hs
  obtain ⟨_, _, h, hpt', _⟩ := (hfields.trans hheld).of_prod
  cases h
  -- So the `mut` point has a subtype of what's `imm` all the way down: an
  -- `imm` object can't hold a `ro` one either.
  exact nomatch Le.none_immutable ((hmut.trans (hpt'.trans (ha₂.trans ha₁))).atMost_imm hda).1

/-- But `Line { a: Point { x: 0 }, ..l }` gives the shared field anew, and
copies only the number. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.pair (.new .mutable (.int 1)) (.int 0)))
      (.new .immutable (.pair (.new .immutable (.int 0)) (.snd (.get (.var 0))))))
    (.cell .immutable (.prod (.cell .immutable .int) .int)) :=
  .letE (.new (.pair (.new .int trivial) .int) ⟨⟨.top _, trivial⟩, trivial⟩)
    (.new (.pair (.new .int trivial) (.snd (.get (.var rfl)))) ⟨⟨.refl _, trivial⟩, trivial⟩)

/-! ## Type parameters and assumptions (#243)

`struct Box[T] { item: T }` is `cell m T`. Inside `fn wrap[T](x: T)`, `x` has
the type parameter `T`, `tparam 0`. -/

/-- #247: `Box { item: x }` makes an `imm` box holding a `T`, which may be
`mut`. Without an assumption about `T`, that's rejected where it's made,
whatever `wrap` is called with. -/
example (t : Ty) :
    ¬ HasType [] [] .none false (Ctx.empty.update 0 (.tparam 0)) (.new .immutable (.var 0)) t := by
  intro h
  obtain ⟨_, hx, hd, _, _⟩ := h.new_inv
  -- Seen as `ro T`, it would still be more than `imm`.
  obtain ⟨μ', hμ', hle⟩ := hx.var_update.atMost_imm hd
  rcases hμ' with rfl | hmem
  · exact nomatch Le.none_immutable hle
  · simp [Assume.none] at hmem

/-- `mut Box { item: x }` is fine without one: a `mut` box can hold
anything. -/
example : HasType [] [] .none false (Ctx.empty.update 0 (.tparam 0)) (.new .mutable (.var 0))
    (.cell .mutable (.tparam 0)) :=
  .new (.var rfl) ⟨.mutable, .inl rfl, .refl _⟩

/-- `fn wrap[T](x: T) -> Box[T] where T ⊑ imm = Box { item: x }`: assuming
`T ⊑ imm`, the `imm` box is fine. -/
def wrapAssume : Assume := { tys := [(0, .immutable)] }

def wrap : Fn :=
  { assume := wrapAssume, param := 0, argTy := .tparam 0, retTy := .cell .immutable (.tparam 0),
    body := .new .immutable (.var 0) }

theorem wrap_ok : FnsOk [wrap] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .new (.var rfl) ⟨.immutable, .inr (by simp [wrap, wrapAssume]), .refl _⟩⟩

/-- `wrap[imm [i32]](x)` meets the assumption. -/
example : HasType [wrap] [] .none false Ctx.empty
    (.call 0 [] [.cell .immutable .int] (.new .immutable (.int 1))) (.cell .immutable (.cell .immutable .int)) := by
  show HasType _ _ _ _ _ _ (wrap.retTy.inst [] [.cell .immutable .int])
  refine .call rfl ⟨?_, ?_, ?_⟩ (.new .int trivial)
  · intro p h; simp [wrap, wrapAssume] at h
  · intro p hp
    simp [wrap, wrapAssume] at hp
    subst hp
    exact ⟨.refl _, trivial⟩
  · intro i
    cases i with
    | zero => exact ⟨.bot _, trivial⟩
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial

/-- `wrap[mut [i32]](x)` doesn't: a `mut` array can't go in an `imm` box. -/
example (e : Expr) (t : Ty) :
    ¬ HasType [wrap] [] .none false Ctx.empty (.call 0 [] [.cell .mutable .int] e) t := by
  intro h
  obtain ⟨_, hf, hs, _, _⟩ := h.call_inv
  simp at hf
  subst hf
  have := hs.2.1 (0, .immutable) (by simp [wrap, wrapAssume])
  exact nomatch Le.none_immutable this.1

/-- A field with a mutability of its own: `struct VecIter['v] { vec: 'v [i32],
next: i32 }` is `cell m (prod (cell 'v int) int)`. A `mut` iterator can hold
the array it's given, whatever `'v` is. -/
example : HasType [] [] .none false (Ctx.empty.update 0 (.cell (.param 0) .int))
    (.new .mutable (.pair (.var 0) (.int 0))) (.cell .mutable (.prod (.cell (.param 0) .int) .int)) :=
  .new (.pair (.var rfl) .int) ⟨⟨.top _, trivial⟩, trivial⟩

/-- An `imm` one can't, since `'v` may be `mut`... -/
example (t : Ty) : ¬ HasType [] [] .none false (Ctx.empty.update 0 (.cell (.param 0) .int))
    (.new .immutable (.pair (.var 0) (.int 0))) t := by
  intro h
  obtain ⟨_, hp, hd, _, _⟩ := h.new_inv
  obtain ⟨_, _, hx, _, hs⟩ := hp.pair_inv
  -- Seen as `ro`, it would still be more than `imm`.
  exact nomatch Le.none_immutable (hx.var_update.atMost_imm (hs.atMost_imm hd).1).1

/-- ...unless the function assumes `'v ⊑ imm`. -/
example : HasType [] [] { mus := [(.param 0, .immutable)] } false (Ctx.empty.update 0 (.cell (.param 0) .int))
    (.new .immutable (.pair (.var 0) (.int 0))) (.cell .immutable (.prod (.cell (.param 0) .int) .int)) :=
  .new (.pair (.var rfl) .int) ⟨⟨.assume (by simp ), trivial⟩, trivial⟩

/-! ## Read-only views (#320)

A `mut` or `imm` object can be seen as `ro`, which reads it but can't
change it. -/

/-- `fn peek(xs: ro [i32]) -> i32 = xs[0]`. -/
def peek : Fn :=
  { assume := {}, param := 0, argTy := .cell .readonly .int, retTy := .int, body := .get (.var 0) }

theorem peek_ok : FnsOk [peek] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .get (.var rfl)⟩

/-- `let xs = mut [1]; peek(xs)` and `let xs = [1]; peek(xs)`: it takes
either, with no mutability parameter. -/
example (b : Bool) : HasType [peek] [] .none false Ctx.empty
    (.letE 0 (.new (muOf b) (.int 1)) (.call 0 [] [] (.var 0))) .int := by
  refine .letE (.new .int trivial (by cases b <;> lit_mu)) ?_
  show HasType _ _ _ _ _ _ (peek.retTy.inst [] [])
  exact .call rfl (sat_none _ _) (.sub (.var rfl) (.ro (.refl _) trivial))

/-- `fn total(xs: 'm [i32]) -> i32 = peek(xs)`: an object whose mutability
is a parameter can be seen as `ro` too, so a generic function can pass
what it's given on to one that takes `ro`. -/
def total : Fn :=
  { assume := {}, param := 0, argTy := .cell (.param 0) .int, retTy := .int,
    body := .call 0 [] [] (.var 0) }

theorem total_ok : FnsOk [peek, total] := by
  intro fn hfn
  simp only [List.mem_cons, List.not_mem_nil, or_false] at hfn
  rcases hfn with rfl | rfl
  · exact ⟨trivial, .get (.var rfl)⟩
  · refine ⟨trivial, ?_⟩
    show HasType _ _ _ _ _ _ (peek.retTy.inst [] [])
    exact .call rfl (sat_none _ _) (.sub (.var rfl) (.ro (.refl _) trivial))

/-- `xs[0] = 1` with `xs : ro [i32]` is rejected: a `ro` view can't change
what it sees. -/
example (t : Ty) :
    ¬ HasType [] [] .none false (Ctx.empty.update 0 (.cell .readonly .int)) (.set (.var 0) (.int 1)) t := by
  intro h
  obtain ⟨_, hx, _, _⟩ := h.set_inv
  cases hx.var_update.to_mut_cell

/-- `[xs]` with `xs : ro [i32]` is rejected: an `imm` object can't hold a
`ro` one, since what's seen as `ro` may change. -/
example (t : Ty) :
    ¬ HasType [] [] .none false (Ctx.empty.update 0 (.cell .readonly .int)) (.new .immutable (.var 0)) t := by
  intro h
  obtain ⟨_, hx, hd, _, _⟩ := h.new_inv
  -- However it's seen, `xs` is still `ro`.
  exact nomatch Le.none_immutable (hx.var_update.atMost_imm hd).1

/-- `fn first[T](xs: ro [T]) -> ro T = xs[0]`: a `ro` view of an array of
any `T`, with nothing assumed. What's read through it is `ro T`. -/
def firstRo : Fn :=
  { assume := {}, param := 0, argTy := .cell .readonly (.roParam 0), retTy := .roParam 0,
    body := .get (.var 0) }

theorem firstRo_ok : FnsOk [firstRo] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .get (.var rfl)⟩

/-- `let ys = mut [1]; let xs = mut [ys]; first(xs)`, with `T = mut [i32]`:
`xs` is seen as `ro [ro [i32]]`, and the result is `ro [i32]`, which can't
change `ys`. -/
example : HasType [firstRo] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 1)) (.letE 1 (.new .mutable (.var 0))
      (.call 0 [] [.cell .mutable .int] (.var 1))))
    (.cell .readonly .int) := by
  refine .letE (.new .int trivial)
    (.letE (.new (.var (t := .cell .mutable .int) (by simp [Ctx.update])) ⟨.refl _, trivial⟩) ?_)
  show HasType _ _ _ _ _ _ (firstRo.retTy.inst [] [.cell .mutable .int])
  refine .call rfl ⟨?_, ?_, ?_⟩
    (.sub (.var (t := .cell .mutable (.cell .mutable .int)) (by simp [Ctx.update]))
      (.ro (.ro (.refl _) trivial) ⟨.refl _, trivial⟩))
  · intro p h; simp [firstRo] at h
  · intro p h; simp [firstRo] at h
  · intro i
    cases i with
    | zero => exact ⟨.top _, trivial⟩
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial

/-- The case from #320: one type parameter for a `mut` and an `imm` value.
`fn both[T](p: (T, T))`, called with `(xs, ys)` where `xs` is `mut` and
`ys` is `imm`, takes `T` to be `ro [i32]`. -/
def both : Fn :=
  { assume := {}, param := 0, argTy := .prod (.tparam 0) (.tparam 0), retTy := .unit, body := .unit }

example : HasType [both] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 1)) (.letE 1 (.new .immutable (.int 2))
      (.call 0 [] [.cell .readonly .int] (.pair (.var 0) (.var 1))))) .unit := by
  refine .letE (.new .int trivial) (.letE (.new .int trivial) ?_)
  show HasType _ _ _ _ _ _ (both.retTy.inst [] [.cell .readonly .int])
  refine .call rfl ⟨?_, ?_, ?_⟩
    (.pair (.sub (.var (t := .cell .mutable .int) (by simp [Ctx.update])) (.ro (.refl _) trivial))
      (.sub (.var (t := .cell .immutable .int) (by simp [Ctx.update])) (.ro (.refl _) trivial)))
  · intro p h; simp [both] at h
  · intro p h; simp [both] at h
  · intro i
    cases i with
    | zero => exact ⟨.top _, trivial⟩
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial

/-! ## Freezing (#335)

`toImm false e` is `imm e`: it copies each `mut` object `e`'s value reaches
into a new `imm` one. -/

/-- `let xs = mut [1]; xs[0] = 2; imm xs`. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new .mutable (.int 1)) (.letE 1 (.set (.var 0) (.int 2)) (.toImm false (.var 0))))
    (.cell .immutable .int) :=
  .letE (.new .int trivial)
    (.letE (.set (.var rfl) .int) (.toImm (.var (t := .cell (muOf true) .int) rfl) (.cell (.asIs trivial))))

/-- `imm mut [mut [1]]` is deep: it gives `imm [imm [i32]]`. -/
example : HasType [] [] .none false Ctx.empty (.toImm false (.new .mutable (.new .mutable (.int 1))))
    (.cell .immutable (.cell .immutable .int)) :=
  .toImm (.new (.new .int trivial) ⟨.refl _, trivial⟩) (.cell (.cell (.asIs trivial)))

/-- `imm` of a `mut` closure is rejected: a closure can't be copied, so it
must be `imm` already. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty (.toImm false (.lam true 0 .unit)) t := by
  intro h
  obtain ⟨_, _, _, he, hd, _⟩ := h.toImm_inv
  obtain ⟨_, _, _, _, _, _, hs⟩ := he.lam_inv
  obtain ⟨_, _, rfl, _⟩ := hs.of_fn
  cases hd with
  | asIs h => exact nomatch Le.none_immutable h.none_fn

end BoltMut
