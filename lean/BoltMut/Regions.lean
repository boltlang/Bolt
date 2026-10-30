import BoltMut.Immutability

/-!
# Regions (#209)

A *region* is the mutability of the values a block makes and that nothing
outside the block has (`site/docs/internals/type-checking/mutability/regions.mdx`).
A region's objects are made by `new true e` (`HasType.newR`) and changed
by `set` (`HasType.setR`), like `mut` ones, and what they hold is at most
the region: so a region can't hold `mut` objects. One that would have to,
`mut ⊑ r`, is just `mut`, as the checker decides right away
(`Le.eq_mutable` in `Solver.lean`, #286).

When a block ends, its region becomes what the code around it needs. If
that's `mut` or another region, the block's objects simply are that. If
it's `imm`, the block ends with a freeze that copies them (#292), so the
heap typing only ever grows, and soundness is proven as for any other
step. That the copy can be left out, since nothing else has the
originals, is the second part of #292.

A generic function can be given a region (#285), change what has a
changeable mutability parameter (`HasType.setP`) and make values with one
(`HasType.newP`), as rule 1 of the regions page says.
-/

namespace BoltMut

/-- The examples' region `0` exists: the heap typing has its marker. -/
theorem region0_below : ∀ m ∈ [Mu.region 0], m.Below [(Mu.immutable, Ty.unit)].length :=
  fun m hm r hr => by simp at hm; subst hm; simp [Mu.mentions] at hr; subst hr; simp

/-- `let xs = [1] in r; xs[0] = 2`: an object of a region can be changed,
like a `mut` one. -/
example : HasType [] [(.immutable, .unit)] .none false Ctx.empty
    (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.newR (r := 0) .int trivial (by simp)) (.setR (.var rfl) .int)

/-- But it can't hold a `mut` object, since it becomes `imm` when its region
is freed: `[mut [1]] in r` is rejected. Such an object makes the array `mut`
instead, which is what the checker decides (#286). -/
example : ¬ HasType [] [] .none false Ctx.empty
    (.new true (.new true (.int 1))) (.cell (.region 0) (.cell .mutable .int)) := by
  intro h
  cases h with
  | newR _ hd => exact nomatch Le.below_region hd.1

/-! ## Generic functions with a region's objects (#285)

`fn fill(xs, x) { xs[0] = x; }` is generic over a changeable `'m` and a
`T`, as rule 1 of the regions page makes it: its parameter is the pair
`(xs, x)`, of type `('m [T], T)`. -/
def fill : Fn :=
  { assume := { changeable := [0] }, param := 0,
    argTy := .prod (.cell (.param 0) (.tparam 0)) (.tparam 0), retTy := .unit,
    body := .set (.fst (.var 0)) (.snd (.var 0)) }

/-- It changes what it's given, since `'m` is changeable. -/
theorem fill_ok : FnsOk [fill] := by
  intro fn hfn
  simp only [List.mem_singleton] at hfn
  subst hfn
  exact ⟨⟨trivial, trivial⟩, .setP (.fst (.var rfl)) (by simp [fill]) (.snd (.var rfl)),
    by simp [Fn.NoRegions, Ty.NoRegion, Mu.NoRegion, fill], by simp [Expr.Scoped, fill]⟩

theorem fill_sat (m : Mu) (hm : m.Changeable .none) : Sat .none fill.assume [m] [.int] := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro p h; simp [fill] at h
  · intro p h; simp [fill] at h
  · intro i
    cases i with
    | zero => trivial
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial
  · intro i h; simp [fill] at h
  · intro i h; simp [fill] at h
  · intro i h
    simp [fill] at h
    subst h
    exact hm

/-- `let xs = [0] in r; fill(xs, 1); xs`: the block gives `fill` an object
of its region, which `fill` changes. The block's value has the region's
type, so it can be frozen when the block ends. -/
example : HasType [fill] [(.immutable, .unit)] .none false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.letE 1 (.call 0 [.region 0] [.int] (.pair (.var 0) (.int 1))) (.var 0)))
    (.cell (.region 0) .int) :=
  .letE (.newR .int trivial (by simp))
    (.letE (.call rfl (fill_sat _ trivial) (.pair (.var rfl) .int) region0_below) (.var rfl))

/-- Freed, that's `imm [i32]`. -/
example : (Ty.cell (.region 0) .int).free 0 .immutable = .cell .immutable .int := rfl

/-- But `fill` can't be given an `imm` object. -/
example : ¬ Sat .none fill.assume [.immutable] [.int] := by
  intro h
  exact h.2.2.2.2.2 0 (by simp [fill])

/-- `fn build(n) -> 'ρ [i32] { let xs = mut [n]; xs[0] = 5; xs }`, with a
changeable `'ρ` that the caller chooses, like `squares` on the regions
page. -/
def build : Fn :=
  { assume := { changeable := [0] }, param := 0, argTy := .int, retTy := .cell (.param 0) .int,
    body := .letE 1 (.new true (.var 0)) (.letE 2 (.set (.var 1) (.int 5)) (.var 1)) }

theorem build_ok : FnsOk [build] := by
  intro fn hfn
  simp only [List.mem_singleton] at hfn
  subst hfn
  exact ⟨⟨trivial, ⟨trivial, trivial⟩, trivial⟩,
    .letE (.newP (.var rfl) (by simp [build]) trivial) (.letE (.setP (.var rfl) (by simp [build]) .int) (.var rfl)),
    by simp [Fn.NoRegions, Ty.NoRegion, Mu.NoRegion, build], by simp [Expr.Scoped, build]⟩

/-- A caller that wants an `imm` array can't call it as it is, since `imm`
isn't changeable... -/
example : ¬ Sat .none build.assume [.immutable] [] := by
  intro h
  exact h.2.2.2.2.2 0 (by simp [build])

/-- ...but it can call it with a region, `r`, and freeze it: the call is
the block, and the freeze copies the array into an `imm` one (#292). A
caller that wants `mut` would give `'ρ` that instead, with no freeze. -/
example : HasType [build] [(.immutable, .unit)] .none false Ctx.empty
    (.freeze 0 (.cell (.region 0) .int) (.call 0 [.region 0] [] (.int 3))) (.cell .immutable .int) := by
  refine .freeze (t := .cell (.region 0) .int) ?_ ⟨(fun h => nomatch h), trivial⟩ ⟨(fun _ h => nomatch h), trivial⟩
    (by simp) (by intro r h; simp [Ty.mentions, Mu.mentions] at h; subst h; simp)
  show HasType _ _ _ _ _ _ (build.retTy.inst [.region 0] [])
  refine .call rfl ⟨?_, ?_, ?_, ?_, ?_, ?_⟩ .int region0_below
  · intro p h; simp [build] at h
  · intro p h; simp [build] at h
  · intro i; simp only [List.getElem?_nil, Option.getD_none]; trivial
  · intro i h; simp [build] at h
  · intro i h; simp [build] at h
  · intro i h
    simp [build] at h
    subst h
    trivial

/-! ## Freezing -/

/-- `let ys: imm [i32] = { let xs = mut [1]; xs[0] = 2; xs }`: the block's
region becomes `imm`, so it ends with a freeze, and `ys` is an `imm`
array. -/
example : HasType [] [(.immutable, .unit)] .none false Ctx.empty
    (.freeze 0 (.cell (.region 0) .int)
      (.letE 0 (.new true (.int 1)) (.letE 1 (.set (.var 0) (.int 2)) (.var 0))))
    (.cell .immutable .int) :=
  .freeze (t := .cell (.region 0) .int) (.letE (.newR .int trivial (by simp)) (.letE (.setR (.var rfl) .int) (.var rfl)))
    ⟨(fun h => nomatch h), trivial⟩ ⟨(fun _ h => nomatch h), trivial⟩ (by simp) (by intro r h; simp [Ty.mentions, Mu.mentions] at h; subst h; simp)

/-- The same block as written: `block` names its region `1` inside, and
makes one when it starts, from an empty heap. -/
example : HasType [] [] .none false Ctx.empty
    (.block false 0 (.cell (.region 1) .int)
      (.letE 0 (.new true (.int 1)) (.letE 1 (.set (.var 0) (.int 2)) (.var 0))))
    (.cell .immutable .int) :=
  .block (Γb := Ctx.empty) (fun _ _ h => h) (fun _ _ h => nomatch h) (fun _ h => by simp [Expr.HasLoc] at h)
    (.letE (.newR .int trivial (by simp)) (.letE (.setR (.var rfl) .int) (.var rfl)))
    ⟨(fun h => nomatch h), trivial⟩ ⟨(fun _ h => nomatch h), trivial⟩
    (by intro r h; simp [Ty.mentions, Mu.mentions] at h; subst h; simp)

/-- A closure can't be copied, so a freeze can't make a function type that
mentions the region `imm`: here one that takes the region's arrays, which
could change the copy if it were given it. -/
example (e : Expr) (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.freeze 0 (.prod (.fn .immutable (.cell (.region 0) .int) .unit) (.cell (.region 0) .int)) e) t := by
  intro h
  cases h with
  | freeze _ hf _ _ _ => simp [Ty.Freezable, Ty.mentions, Mu.mentions] at hf

/-- `{ let xs = mut [1]; let f = || xs[0] = 2; xs }`: a closure that changes
the region's array has the region as its mutability (rule 5 of the
regions page), so it can't be in the frozen value, and nothing after the
block can reach the original through it. -/
example : HasType [] [(.immutable, .unit)] .none false Ctx.empty
    (.freeze 0 (.cell (.region 0) .int)
      (.letE 0 (.new true (.int 1)) (.letE 1 (.lam true 2 (.set (.var 0) (.int 2))) (.var 0))))
    (.cell .immutable .int) := by
  refine .freeze (t := .cell (.region 0) .int) (.letE (.newR .int trivial (by simp)) (.letE (.lam (a := .unit) (μ := .region 0) (.inr ⟨rfl, 0, rfl⟩)
    (fun _ _ h => h) ?_ ?_ (fun _ h => by simp [Expr.HasLoc] at h) (.setR (.var rfl) .int)
    (fun r h => by simp [Mu.mentions] at h; subst h; simp)) (.var rfl)))
    ⟨(fun h => nomatch h), trivial⟩ ⟨(fun _ h => nomatch h), trivial⟩ (by simp) (by intro r h; simp [Ty.mentions, Mu.mentions] at h; subst h; simp)
  · intro _ y s hy
    simp only [Ctx.update] at hy
    split at hy
    · cases hy; exact ⟨.refl _, trivial⟩
    · cases hy
  · intro y s hy
    simp only [Ctx.update] at hy
    split at hy
    · cases hy
      exact ⟨fun r' hr => by simpa [Ty.mentions, Mu.mentions] using hr, trivial, trivial⟩
    · cases hy

end BoltMut
