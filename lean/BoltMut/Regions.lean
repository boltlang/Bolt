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

/-- `let xs = [1] in r; xs[0] = 2`: an object of a region can be changed,
like a `mut` one. -/
example : HasType [] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 1)) (.set (.var 0) (.int 2))) .unit :=
  .letE (.newR (r := 0) .int trivial) (.setR (.var rfl) .int)

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
  exact ⟨⟨trivial, trivial⟩, .setP (.fst (.var rfl)) (by simp [fill]) (.snd (.var rfl))⟩

theorem fill_sat (m : Mu) (hm : m.Changeable .none) : Sat .none fill.assume [m] [.int] := by
  refine ⟨?_, ?_, ?_, ?_⟩
  · intro p h; simp [fill] at h
  · intro p h; simp [fill] at h
  · intro i
    cases i with
    | zero => trivial
    | succ i => simp only [List.getElem?_cons_succ, List.getElem?_nil, Option.getD_none]; trivial
  · intro i h
    simp [fill] at h
    subst h
    exact hm

/-- `let xs = [0] in r; fill(xs, 1); xs`: the block gives `fill` an object
of its region, which `fill` changes. The block's value has the region's
type, so it can be frozen when the block ends. -/
example : HasType [fill] [] .none false Ctx.empty
    (.letE 0 (.new true (.int 0)) (.letE 1 (.call 0 [.region 0] [.int] (.pair (.var 0) (.int 1))) (.var 0)))
    (.cell (.region 0) .int) :=
  .letE (.newR .int trivial)
    (.letE (.call rfl (fill_sat _ trivial) (.pair (.var rfl) .int)) (.var rfl))

/-- Freed, that's `imm [i32]`. -/
example : (Ty.cell (.region 0) .int).free 0 .immutable = .cell .immutable .int := rfl

/-- But `fill` can't be given an `imm` object. -/
example : ¬ Sat .none fill.assume [.immutable] [.int] := by
  intro h
  exact h.2.2.2 0 (by simp [fill])

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
    .letE (.newP (.var rfl) (by simp [build]) trivial) (.letE (.setP (.var rfl) (by simp [build]) .int) (.var rfl))⟩

/-- A caller that wants an `imm` array can't call it as it is, since `imm`
isn't changeable... -/
example : ¬ Sat .none build.assume [.immutable] [] := by
  intro h
  exact h.2.2.2 0 (by simp [build])

/-- ...but it can call it with a region, `r`, and freeze it: the call is
the block, and the freeze copies the array into an `imm` one (#292). A
caller that wants `mut` would give `'ρ` that instead, with no freeze. -/
example : HasType [build] [] .none false Ctx.empty
    (.freeze 0 (.cell (.region 0) .int) (.call 0 [.region 0] [] (.int 3))) (.cell .immutable .int) := by
  refine .freeze ?_ trivial ⟨(fun _ h => nomatch h), trivial⟩
  show HasType _ _ _ _ _ _ (build.retTy.inst [.region 0] [])
  refine .call rfl ⟨?_, ?_, ?_, ?_⟩ .int
  · intro p h; simp [build] at h
  · intro p h; simp [build] at h
  · intro i; simp only [List.getElem?_nil, Option.getD_none]; trivial
  · intro i h
    simp [build] at h
    subst h
    trivial

/-! ## Freezing -/

/-- `let ys: imm [i32] = { let xs = mut [1]; xs[0] = 2; xs }`: the block's
region becomes `imm`, so it ends with a freeze, and `ys` is an `imm`
array. -/
example : HasType [] [] .none false Ctx.empty
    (.freeze 0 (.cell (.region 0) .int)
      (.letE 0 (.new true (.int 1)) (.letE 1 (.set (.var 0) (.int 2)) (.var 0))))
    (.cell .immutable .int) :=
  .freeze (.letE (.newR .int trivial) (.letE (.setR (.var rfl) .int) (.var rfl))) trivial
    ⟨(fun _ h => nomatch h), trivial⟩

/-- A closure can't be copied, so a freeze can't make a function type that
mentions the region `imm`: here one that takes the region's arrays, which
could change the copy if it were given it. -/
example (e : Expr) (t : Ty) : ¬ HasType [] [] .none false Ctx.empty
    (.freeze 0 (.prod (.fn .immutable (.cell (.region 0) .int) .unit) (.cell (.region 0) .int)) e) t := by
  intro h
  cases h with
  | freeze _ hf _ => simp [Ty.FnFree, Ty.mentions, Mu.mentions] at hf

/-- A closure that changes the region's array but whose type doesn't
mention the region is shared, not copied: `{ let xs = mut [1]; let f = ||
xs[0] = 2; xs }`. The copy is `imm`, and `f` changes the original. That's
sound, but the copy can't be left out here, which is what the rules of the
regions page for closures are for (#292). -/
example : HasType [] [] .none false Ctx.empty
    (.freeze 0 (.cell (.region 0) .int)
      (.letE 0 (.new true (.int 1)) (.letE 1 (.lam true 2 (.set (.var 0) (.int 2))) (.var 0))))
    (.cell .immutable .int) :=
  .freeze (.letE (.newR .int trivial) (.letE (.lam (a := .unit) (fun _ _ h => h) (fun h => nomatch h)
    (.setR (.var rfl) .int)) (.var rfl))) trivial ⟨(fun _ h => nomatch h), trivial⟩

end BoltMut
