import BoltMut.Purity

/-!
# Examples

What the rules accept and reject, as in Bolt.
-/

namespace BoltMut

/-- `let xs = mut [1]; xs[0] = 2`: a `mut` object can be changed. -/
example : HasType [] [] .none false Ctx.empty (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.new .int trivial) (.set (.var rfl) .int)

/-- `let xs = [1]; xs[0] = 2`: an `imm` one can't. -/
example (t : Ty) :
    ¬ HasType [] [] .none false Ctx.empty (.letE 0 (.new false (.int 1)) (.set (.var 0) (.int 2))) t := by
  intro h
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | new _ _ =>
      cases h₂ with
      | setP _ hi _ => simp [Assume.none] at hi
      | set hx _ | setR hx _ =>
        cases hx with
        | var hx => simp [Ctx.update, muOf] at hx

/-- `[mut [1]]`: an `imm` object can't hold a `mut` one, since `imm` is
deep. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty (.new false (.new true (.int 1))) t := by
  intro h
  cases h with
  | new hin hd =>
    cases hin with
    | newP _ hi _ => simp [Assume.none] at hi
    | new _ _ | newR _ _ => exact nomatch Le.none_immutable hd.1

/-- A generic function that reads what it's given:
`fn first(xs: 'm [i32]) -> i32 = xs[0]`, with the mutability `'m` as its
parameter `0`. -/
def first : Fn :=
  { assume := {}, param := 0, argTy := .cell (.param 0) .int, retTy := .int, body := .get (.var 0) }

/-- Arguments that only need every type argument to be well-formed. -/
theorem sat_none (Δ : Assume) (ms : List Mu) : Sat Δ {} ms [] := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro p h; simp at h
  · intro p h; simp at h
  · intro i; simp only [List.getElem?_nil, Option.getD_none]; trivial
  · intro i h; simp at h
  · intro i h; simp at h
  · intro i h; simp at h

/-- What a closure captures of `x : t`, when `t` mentions no region and no
parameter. -/
theorem captured_update {Δ : Assume} (x : Nat) (t : Ty) (hn : t.NoRegion) (hp : t.ParamsNoRegion Δ) (μ : Mu) :
    ∀ y s, Ctx.empty.update x t y = some s → s.RegionsIn μ ∧ s.ParamsNoRegion Δ := by
  intro y s hy
  simp only [Ctx.update] at hy
  split at hy
  · cases hy; exact ⟨hn.regionsIn μ, hp⟩
  · cases hy

/-- `mut` and `imm` mention no region. -/
theorem muOf_below (b : Bool) (n : Nat) : (muOf b).Below n := by
  intro r h
  cases b <;> simp [muOf, Mu.mentions] at h

theorem below_nil_regions {n : Nat} {ms : List Mu} (h : ∀ m ∈ ms, ∀ r, m.mentions r = false) :
    ∀ m ∈ ms, m.Below n := fun m hm r hr => by rw [h m hm r] at hr; cases hr

/-- It can be called with a `mut` object and with an `imm` one. -/
example : HasType [first] [] .none false Ctx.empty (.call 0 [.mutable] [] (.new true (.int 1))) .int := by
  show HasType _ _ _ _ _ _ (first.retTy.inst [.mutable] [])
  exact .call rfl (sat_none _ _) (.new .int trivial) (below_nil_regions (by simp [Mu.mentions]))

example : HasType [first] [] .none false Ctx.empty (.call 0 [.immutable] [] (.new false (.int 1))) .int := by
  show HasType _ _ _ _ _ _ (first.retTy.inst [.immutable] [])
  exact .call rfl (sat_none _ _) (.new .int trivial) (below_nil_regions (by simp [Mu.mentions]))

/-- And with an object of a region (#285). -/
example : HasType [first] [(.immutable, .unit)] .none false Ctx.empty
    (.call 0 [.region 0] [] (.new true (.int 1))) .int := by
  show HasType _ _ _ _ _ _ (first.retTy.inst [.region 0] [])
  exact .call rfl (sat_none _ _) (.newR .int trivial (by simp))
    (fun m hm r hr => by simp at hm; subst hm; simp [Mu.mentions] at hr; subst hr; simp)

/-- A closure in a generic function that captures a `T`, as in
`fn hold[T](x: T) = || x`, needs `T` to be a parameter that can't be given
a region: otherwise, once `T` is a region's type, it would capture a
region's value without being of that region. -/
example (t : Ty) : ¬ HasType [] [] {} false (Ctx.empty.update 0 (.tparam 0)) (.lam true 1 (.var 0)) t := by
  intro h
  cases h with
  | lam _ hc _ hrc _ hb =>
    cases hb with
    | var hx =>
      simp only [Ctx.update] at hx
      have hΓ := hc 0 _ hx
      simp only [Ctx.update, ite_true, Option.some.injEq] at hΓ
      subst hΓ
      exact nomatch (hrc 0 _ hx).2

example : HasType [] [] { noRegionTys := [0] } false (Ctx.empty.update 0 (.tparam 0)) (.lam true 1 (.var 0))
    (.fn .mutable .unit (.tparam 0)) :=
  .lam (.inl rfl) (fun _ _ h => h) (fun h => (h rfl).elim)
    (captured_update 0 (.tparam 0) trivial (by simp [Ty.ParamsNoRegion]) _) (fun _ h => nomatch h) (.var rfl)
    (muOf_below true _)

/-- So a call can't give it one. -/
example : ¬ Sat .none { noRegionTys := [0] } [] [.cell (.region 0) .int] := by
  intro h
  have := (h.2.2.2.2.1 0 (by simp)).1
  simp [Ty.NoRegion, Mu.NoRegion] at this

/-- But a generic function can't change what it's given:
`fn poke(xs: 'm [i32]) { xs[0] = 1; }` is rejected, since `'m` may be `imm`.
It can if it assumes `'m` is changeable (`fill` in `Regions.lean`). -/
example (t : Ty) :
    ¬ HasType [] [] .none false (Ctx.empty.update 0 (.cell (.param 0) .int)) (.set (.var 0) (.int 1)) t := by
  intro h
  cases h with
  | setP _ hi _ => simp [Assume.none] at hi
  | set hx _ | setR hx _ =>
    cases hx with
    | var hx => simp [Ctx.update] at hx

/-! ## Closures -/

/-- `let c = mut [0]; || c[0] = 1`: a closure that changes what it captured
is `mut`. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.lam true 1 (.set (.var 0) (.int 1)))) (.fn .mutable .unit .unit) :=
  .letE (.new .int trivial) (.lam (.inl rfl) (fun _ _ h => h) (fun h => (h rfl).elim)
    (captured_update 0 (.cell .mutable .int) ⟨trivial, trivial⟩ ⟨trivial, trivial⟩ _)
    (fun _ h => by simp [Expr.HasLoc] at h) (.set (.var rfl) .int) (muOf_below true _))

/-- It can't be `imm`: an `imm` closure only sees what's `imm` all the way
down, and `c` is `mut`. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.lam false 1 (.set (.var 0) (.int 1)))) t := by
  intro h
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | newP _ hi _ => simp [Assume.none] at hi
    | new _ _ | newR _ _ =>
      cases h₂ with
      | lam hk _ himm _ _ hb =>
        rcases hk with hk | ⟨hk, _⟩
        · subst hk
          cases hb with
          | setP _ hi _ => simp [Assume.none] at hi
          | set hx _ | setR hx _ =>
            cases hx with
            | var hx =>
              simp only [Ctx.update] at hx
              exact nomatch Le.none_immutable (himm (fun h => nomatch h) 0 _ hx).1
        · cases hk

/-- The hole from #244: `let c = mut [0]; [|| c[0] = 1]`, an `imm` object
holding a closure that changes `c`, is rejected, since the closure is
`mut`. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.new false (.lam true 1 (.set (.var 0) (.int 1))))) t := by
  intro h
  cases h with
  | letE _ h₂ =>
    cases h₂ with
    | new hl hd =>
      cases hl with
      | lam hk _ _ _ _ _ =>
        have hμ := Le.none_immutable hd
        subst hμ
        rcases hk with h | ⟨_, _, _, h⟩ <;> exact nomatch h

/-- `let xs = [1]; [|| xs[0]]`: an `imm` closure that reads an `imm` object
can be held by an `imm` one. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new false (.int 1)) (.new false (.lam false 1 (.get (.var 0)))))
    (.cell .immutable (.fn .immutable .unit .int)) := by
  refine .letE (.new .int trivial) (.new (.lam (.inl rfl) (fun _ _ h => h) ?_
    (captured_update 0 (.cell .immutable .int) ⟨trivial, trivial⟩ ⟨trivial, trivial⟩ _)
    (fun _ h => by simp [Expr.HasLoc] at h) (.get (.var rfl)) (muOf_below false _)) (.refl _))
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
    (.letE 0 (.new true (.pair (.new true (.int 1)) (.int 0)))
      (.set (.var 0) (.pair (.fst (.get (.var 0))) (.int 5)))) .unit :=
  .letE (.new (.pair (.new .int trivial) .int) ⟨⟨.top _, trivial⟩, trivial⟩)
    (.set (.var rfl) (.pair (.fst (.get (.var rfl))) .int))

/-- #246: `Line { n: 5, ..l }` with a `mut` `l` copies `l.a`, a `mut Point`,
into an `imm Line`, so it's rejected: what's shared keeps its type. -/
example (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.pair (.new true (.int 1)) (.int 0)))
      (.new false (.pair (.fst (.get (.var 0))) (.int 5)))) t := by
  intro h
  cases h with
  | letE h₁ h₂ =>
    cases h₁ with
    | newP _ hi _ => simp [Assume.none] at hi
    | new hl _ | newR hl _ =>
      cases hl with
      | pair hp₁ _ =>
        cases hp₁ with
        | newP _ hi _ => simp [Assume.none] at hi
        | new _ _ | newR _ _ =>
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
                    exact nomatch Le.none_immutable hd.1.1

/-- But `Line { a: Point { x: 0 }, ..l }` gives the shared field anew, and
copies only the number. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.pair (.new true (.int 1)) (.int 0)))
      (.new false (.pair (.new false (.int 0)) (.snd (.get (.var 0))))))
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
    ¬ HasType [] [] .none false (Ctx.empty.update 0 (.tparam 0)) (.new false (.var 0)) t := by
  intro h
  cases h with
  | new hx hd =>
    cases hx with
    | var hx =>
      simp only [Ctx.update, ite_true, Option.some.injEq] at hx
      subst hx
      obtain ⟨μ', hμ', hle⟩ := hd
      rcases hμ' with rfl | hmem
      · exact nomatch Le.none_immutable hle
      · simp [Assume.none] at hmem

/-- `mut Box { item: x }` is fine without one: a `mut` box can hold
anything. -/
example : HasType [] [] .none false (Ctx.empty.update 0 (.tparam 0)) (.new true (.var 0))
    (.cell .mutable (.tparam 0)) :=
  .new (.var rfl) ⟨.mutable, .inl rfl, .refl _⟩

/-- `fn wrap[T](x: T) -> Box[T] where T ⊑ imm = Box { item: x }`: assuming
`T ⊑ imm`, the `imm` box is fine. -/
def wrapAssume : Assume := { tys := [(0, .immutable)] }

def wrap : Fn :=
  { assume := wrapAssume, param := 0, argTy := .tparam 0, retTy := .cell .immutable (.tparam 0),
    body := .new false (.var 0) }

theorem wrap_ok : FnsOk [wrap] := by
  intro fn hfn
  simp at hfn
  subst hfn
  exact ⟨trivial, .new (.var rfl) ⟨.immutable, .inr (by simp [wrap, wrapAssume]), .refl _⟩⟩

/-- `wrap[imm [i32]](x)` meets the assumption. -/
example : HasType [wrap] [] .none false Ctx.empty
    (.call 0 [] [.cell .immutable .int] (.new false (.int 1))) (.cell .immutable (.cell .immutable .int)) := by
  show HasType _ _ _ _ _ _ (wrap.retTy.inst [] [.cell .immutable .int])
  refine .call rfl ⟨?_, ?_, ?_, ?_, ?_, ?_⟩ (.new .int trivial) (fun _ h => by simp at h)
  · intro p h; simp [wrap, wrapAssume] at h
  · intro p hp
    simp [wrap, wrapAssume] at hp
    subst hp
    exact ⟨.refl _, trivial⟩
  · intro i
    cases i with
    | zero => exact ⟨.bot _, trivial⟩
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial
  · intro i h; simp [wrap, wrapAssume] at h
  · intro i h; simp [wrap, wrapAssume] at h
  · intro i h; simp [wrap, wrapAssume] at h

/-- `wrap[mut [i32]](x)` doesn't: a `mut` array can't go in an `imm` box. -/
example (e : Expr) (t : Ty) :
    ¬ HasType [wrap] [] .none false Ctx.empty (.call 0 [] [.cell .mutable .int] e) t := by
  intro h
  cases h with
  | call hf hs _ =>
    simp at hf
    subst hf
    have := hs.2.1 (0, .immutable) (by simp [wrap, wrapAssume])
    exact nomatch Le.none_immutable this.1

/-- A field with a mutability of its own: `struct VecIter['v] { vec: 'v [i32],
next: i32 }` is `cell m (prod (cell 'v int) int)`. A `mut` iterator can hold
the array it's given, whatever `'v` is. -/
example : HasType [] [] .none false (Ctx.empty.update 0 (.cell (.param 0) .int))
    (.new true (.pair (.var 0) (.int 0))) (.cell .mutable (.prod (.cell (.param 0) .int) .int)) :=
  .new (.pair (.var rfl) .int) ⟨⟨.top _, trivial⟩, trivial⟩

/-- An `imm` one can't, since `'v` may be `mut`... -/
example (t : Ty) : ¬ HasType [] [] .none false (Ctx.empty.update 0 (.cell (.param 0) .int))
    (.new false (.pair (.var 0) (.int 0))) t := by
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

/-- ...unless the function assumes `'v ⊑ imm`. -/
example : HasType [] [] { mus := [(.param 0, .immutable)] } false (Ctx.empty.update 0 (.cell (.param 0) .int))
    (.new false (.pair (.var 0) (.int 0))) (.cell .immutable (.prod (.cell (.param 0) .int) .int)) :=
  .new (.pair (.var rfl) .int) ⟨⟨.assume (by simp [muOf]), trivial⟩, trivial⟩

end BoltMut
