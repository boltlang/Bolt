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

end BoltMut
