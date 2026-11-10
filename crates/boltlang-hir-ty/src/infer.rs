//! Inference of the types in a body: walks its expressions and patterns,
//! checking them against an expected type where one is known, and gives
//! the constraints it finds to the [`Solver`].

mod handler;
mod pat;

use std::collections::HashMap;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    arena::ArenaMap,
    body::{Body, DefWithBody, DefWithBodyId},
    builtin_type::{BuiltinInt, BuiltinType, BuiltinUint},
    hir::{
        BinaryOp, Binding, BindingId, Expr, ExprId, LabelId, Literal, Pat, PatId, Path,
        PathSegment, RecordField, Statement, TupleItem, TypeRefId,
    },
    ids::{ClassId, ConstId, FunctionId, ImplId, ItemContainerId, StructId},
    resolver::{
        GenericDefId, Hidden, OverloadSet, Resolver, TypeNs, TypeResolution, ValueNs,
        ValueResolution,
    },
    scope::expr_scopes,
    signatures::function_signature,
};

use crate::{
    check::{InferenceCycle, function_scheme},
    holds::struct_atoms_of,
    lang::{LangClass, char_ty, range_inclusive_struct, range_struct, string_ty},
    lower::{
        ClassConstraint, FieldsKind, LowerDiagnostic, ParamDefaults, TyLoweringContext, VariantDef,
        const_ty, field_types, field_visible, generic_predicates, identity_args,
        named_mutabilities, private_field, struct_args, struct_mutability, subst_params,
    },
    rows::RowConstraint,
    scheme::{Scheme, generic_param_ids},
    solve::{
        AtMostConstraint, AtMostSource, Constraint, ConstraintSource, ExprOrPat, FieldConstraint,
        Held, ImmConstraint, Origin, OverloadConstraint, Reason, Solver, SpreadConstraint,
        TuplePart, TypeError,
    },
    ty::{AdtId, ArgPlace, ArgPlaces, Mutability, Params, Ty, VarKind, place_args},
};

/// The arguments of a call: only positional ones, or with named ones and
/// spreads, like `f(1, sep: ",")` or `f(*xs)`.
#[derive(Clone, Copy)]
enum CallArgs<'a> {
    Positional(&'a [ExprId]),
    Items(&'a [TupleItem<ExprId>]),
}

/// The types found in a body, and what went wrong.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct InferenceResult<'db> {
    pub expr_types: ArenaMap<Expr, Ty<'db>>,
    pub pat_types: ArenaMap<Pat, Ty<'db>>,
    pub binding_types: ArenaMap<Binding, Ty<'db>>,
    /// The operation each operation pattern of an effect arm names, like
    /// the `yield(x)` of `effect yield(x), k`.
    pub operations: ArenaMap<Pat, FunctionId<'db>>,
    /// The function a call like `x.f(y)` calls, when it's `f(x, y)`.
    pub method_calls: ArenaMap<Expr, FunctionId<'db>>,
    /// The parameters a call of a function leaves out, which get their
    /// defaults, by the call: the function, and the indices of its declared
    /// parameters, in order.
    pub defaults: ArenaMap<Expr, (FunctionId<'db>, Box<[u32]>)>,
    /// The impl that a use of a class method or constant uses, by the
    /// expression that names it and the class, when the types decide it.
    /// In a generic function, like `fn f(x) = show(x)`, they don't.
    pub instances: Vec<(ExprId, ClassId<'db>, ImplId<'db>)>,
    /// The function that a use of a name with several functions is, by the
    /// expression that names it, when the types decide it (#275).
    pub overloads: ArenaMap<Expr, FunctionId<'db>>,
    /// The types that a use of a function or constant gives the parameters
    /// of its scheme, then its bound variables, by the expression that names
    /// it, like `[i32]` for the `id` of `id(1)`. In a generic function they
    /// can be its own parameters, like `[T]` for `show(x)` with `x: T`: the
    /// code generator and the evaluator replace them with the types it's
    /// used with, and then know which impl a class method uses. A function
    /// inferred together with the body, like itself, has none.
    pub type_args: ArenaMap<Expr, Box<[Ty<'db>]>>,
    /// The `mut e` that make an array or struct `mut`, like `mut [1]`, `mut
    /// Point { x: 1 }` or `mut Wrapper(1)`, each with its `e`. The others
    /// are errors.
    pub mut_literals: Vec<(ExprId, ExprId)>,
    /// What each `e?` is about, by the `?`.
    pub tries: ArenaMap<Expr, Try<'db>>,
    pub diagnostics: Vec<InferenceDiagnostic<'db>>,
}

/// What an `e?` takes apart, and what it gives back when it can't go on.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Try<'db> {
    pub kind: TryKind<'db>,
    /// The type of the function's result, which the `None` or `Err` it
    /// returns has.
    pub ret: Ty<'db>,
}

/// Whether an `e?` is on an `Option` or a `Result`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub enum TryKind<'db> {
    Option(boltlang_hir_def::ids::EnumId<'db>),
    Result(boltlang_hir_def::ids::EnumId<'db>),
}

/// Why an `e?` can't be checked.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum BadTry<'db> {
    /// `e` is neither an `Option` nor a `Result`.
    NotOptionOrResult(Ty<'db>),
    /// The function gives something else than what `?` returns: an
    /// `Option` for an `Option`, and a `Result` for a `Result`.
    WrongReturn { result: bool, ret: Ty<'db> },
    /// What `e` is isn't known, nor what the function gives.
    Unknown,
}

impl<'db> InferenceResult<'db> {
    pub fn expr_ty(&self, expr: ExprId) -> &Ty<'db> {
        self.expr_types.get(expr).unwrap_or(&Ty::Error)
    }

    pub fn pat_ty(&self, pat: PatId) -> &Ty<'db> {
        self.pat_types.get(pat).unwrap_or(&Ty::Error)
    }

    pub fn binding_ty(&self, binding: BindingId) -> &Ty<'db> {
        self.binding_types.get(binding).unwrap_or(&Ty::Error)
    }

    /// The impl of `class` that the expression `expr` uses, if the types
    /// decide it.
    pub fn instance(&self, expr: ExprId, class: ClassId<'db>) -> Option<ImplId<'db>> {
        self.instances.iter().find(|(e, c, _)| *e == expr && *c == class).map(|(_, _, imp)| *imp)
    }

    /// Whether the `mut e` at `expr` makes its array or struct `mut`, rather
    /// than being an error: see [`InferenceResult::mut_literals`].
    pub fn is_mut_literal(&self, expr: ExprId) -> bool {
        self.mut_literals.iter().any(|(e, _)| *e == expr)
    }

    /// The types that the use of a function or constant at `expr` gives its
    /// scheme: see [`InferenceResult::type_args`].
    pub fn type_args(&self, expr: ExprId) -> Option<&[Ty<'db>]> {
        self.type_args.get(expr).map(|args| &**args)
    }

    /// Keep the impls, overloads and type arguments that `solver` chose for
    /// uses in the body `def`.
    pub(crate) fn add_instances(&mut self, def: DefWithBodyId<'db>, solver: &Solver<'db>) {
        for (origin, class, imp) in &solver.chosen {
            if origin.body == def
                && let ExprOrPat::Expr(expr) = origin.node
            {
                self.instances.push((expr, *class, *imp));
            }
        }
        for (origin, args) in &solver.instantiations {
            if origin.body == def
                && let ExprOrPat::Expr(expr) = origin.node
            {
                // Resolved here, since a body that isn't generalized, like a
                // file's, has its types resolved before.
                self.type_args
                    .insert(expr, args.iter().map(|t| solver.unifier.resolve(t)).collect());
            }
        }
        for (origin, method_call, f) in &solver.overload_choices {
            if origin.body == def
                && let ExprOrPat::Expr(expr) = origin.node
            {
                self.overloads.insert(expr, *f);
                if let Some(call) = method_call {
                    self.method_calls.insert(*call, *f);
                }
            }
        }
    }

    /// The function that a use of a name with several is, if the types
    /// decided it (#275).
    pub fn overload(&self, expr: ExprId) -> Option<FunctionId<'db>> {
        self.overloads.get(expr).copied()
    }

    /// Report the expressions and patterns whose type has a `mut` array or
    /// struct inside an `imm` one, like `[mut [1]]`: `imm` is deep, so what
    /// an `imm` value holds can't be changed either. Each such type is
    /// reported once, where it first appears, once the types are resolved.
    pub(crate) fn check_deep_immutability(&mut self, db: &'db dyn Db) {
        let mut seen = Vec::new();
        // What an "at most" error already reports, like the `mut [i32]` of
        // `wrap(mut [1])` for `wrap: forall a. a ⊑ imm => ...`.
        let reported: Vec<Ty<'db>> = self
            .diagnostics
            .iter()
            .filter_map(|d| match d {
                InferenceDiagnostic::Type(TypeError::NotAtMost { constraint, .. }) => {
                    Some(constraint.held.ty().clone())
                }
                _ => None,
            })
            .collect();
        // A literal after `mut` is reported with its `mut`.
        let outer = |e: ExprId| {
            self.mut_literals.iter().find(|(_, inner)| *inner == e).map_or(e, |(outer, _)| *outer)
        };
        let exprs = self.expr_types.iter().map(|(e, t)| (ExprOrPat::Expr(outer(e)), t));
        let pats = self.pat_types.iter().map(|(p, t)| (ExprOrPat::Pat(p), t));
        for (at, ty) in exprs.chain(pats) {
            if let Some((outer, inner)) = mut_inside_imm(db, ty)
                && !seen.contains(&outer)
                && !reported.contains(&inner)
            {
                seen.push(outer.clone());
                self.diagnostics.push(InferenceDiagnostic::MutInsideImm { at, ty: outer, inner });
            }
        }
    }

    /// Replace the types with `f` of them, once inference is done.
    pub(crate) fn map_types(&mut self, f: &mut impl FnMut(&Ty<'db>) -> Ty<'db>) {
        for ty in self
            .expr_types
            .values_mut()
            .chain(self.pat_types.values_mut())
            .chain(self.binding_types.values_mut())
        {
            *ty = f(ty);
        }
        for args in self.type_args.values_mut() {
            *args = args.iter().map(&mut *f).collect();
        }
        for t in self.tries.values_mut() {
            t.ret = f(&t.ret);
        }
        for diagnostic in &mut self.diagnostics {
            diagnostic.map_types(f);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum InferenceDiagnostic<'db> {
    Type(TypeError<'db>),
    /// A type reference in the body, like that of a `let`.
    Lower(LowerDiagnostic),
    UnresolvedName {
        expr: ExprOrPat,
    },
    /// A path whose last name is in another file, but not visible here.
    NotVisible {
        expr: ExprOrPat,
        hidden: Hidden,
    },
    NotAFunction {
        expr: ExprId,
        ty: Ty<'db>,
    },
    WrongArgCount {
        expr: ExprOrPat,
        expected: usize,
        found: usize,
    },
    NoSuchField {
        expr: ExprOrPat,
        name: Name,
    },
    MissingFields {
        expr: ExprId,
        names: Vec<Name>,
    },
    /// A label or loop to break out of that doesn't exist.
    BreakOutside {
        expr: ExprId,
    },
    /// An assignment to a variable, or to a field of one, that isn't `mut`.
    /// `expr` is the left side.
    AssignToImmutable {
        expr: ExprId,
        binding: BindingId,
    },
    /// `mut` before something other than an array or a struct, like the
    /// call of a function or an enum's variant: `mut f(x)`.
    NoMutability {
        expr: ExprId,
    },
    /// A type `ty` with a `mut` array or struct, `inner`, inside an `imm`
    /// one, which can't hold it: `imm` is deep.
    MutInsideImm {
        at: ExprOrPat,
        ty: Ty<'db>,
        inner: Ty<'db>,
    },
    /// An assignment whose left side isn't a variable, a field or an
    /// element, like `1 = 2` or `f() = 3`. `expr` is the left side.
    NotAPlace {
        expr: ExprId,
    },
    /// Something the checker doesn't support yet, like `?` or ranges.
    Unsupported {
        expr: ExprId,
    },
    /// Syntax that needs a class of the prelude, like `xs[i]` needs
    /// `Index`, without a prelude that defines it.
    NoLangClass {
        expr: ExprId,
        class: &'static str,
    },
    /// A field of a struct that's private to another file or package, used
    /// at `at`: written in a struct literal or a pattern, copied with `..`,
    /// or given to a tuple struct's constructor.
    PrivateField {
        at: ExprOrPat,
        strukt: StructId<'db>,
        name: Name,
    },
    /// An effect arm whose operation, like the `f` of `effect f(x), k`,
    /// names something that isn't an operation of an effect.
    NotAnOperation {
        pat: PatId,
    },
    /// A match with arms for some operations of an effect, but not all,
    /// reported at its first arm for the effect.
    MissingOperations {
        pat: PatId,
        effect: Option<Name>,
        missing: Vec<Name>,
    },
    /// An effect arm whose operation is the name of operations of several
    /// effects, like `get` for `State` and `Config`.
    AmbiguousOperation {
        pat: PatId,
    },
    /// Syntax that needs a type of the prelude, like `"hi"` needs `String`,
    /// without a prelude that defines it.
    NoLangType {
        at: ExprOrPat,
        ty: &'static str,
    },
    /// `x as T` where `x` or `T` is a struct or an enum, like a `Char`:
    /// `as` only converts between the primitive types.
    InvalidCast {
        expr: ExprId,
        from: Ty<'db>,
        to: Ty<'db>,
    },
    /// Functions without complete signatures in files that import each
    /// other call each other; one of them needs a signature.
    CyclicInference,
    /// An `e?` that can't be checked.
    BadTry {
        expr: ExprId,
        reason: BadTry<'db>,
    },
}

impl<'db> InferenceDiagnostic<'db> {
    pub(crate) fn map_types(&mut self, f: &mut impl FnMut(&Ty<'db>) -> Ty<'db>) {
        match self {
            InferenceDiagnostic::Type(error) => error.map_types(f),
            InferenceDiagnostic::NotAFunction { ty, .. } => *ty = f(ty),
            InferenceDiagnostic::InvalidCast { from, to, .. } => {
                *from = f(from);
                *to = f(to);
            }
            InferenceDiagnostic::MutInsideImm { ty, inner, .. } => {
                *ty = f(ty);
                *inner = f(inner);
            }
            InferenceDiagnostic::BadTry { reason, .. } => match reason {
                BadTry::NotOptionOrResult(ty) => *ty = f(ty),
                BadTry::WrongReturn { ret, .. } => *ret = f(ret),
                BadTry::Unknown => {}
            },
            _ => {}
        }
    }
}

/// The types of the functions being inferred together, which aren't
/// available from queries yet.
#[derive(Default)]
pub(crate) struct FnEnv<'db> {
    /// The functions of the group being inferred, not generalized yet.
    pub mono: HashMap<FunctionId<'db>, Ty<'db>>,
    /// Functions of the same file inferred before this group.
    pub done: HashMap<FunctionId<'db>, Scheme<'db>>,
}

/// The type an expression is expected to have, and why.
#[derive(Clone, Copy)]
struct Expectation<'a, 'db> {
    ty: &'a Ty<'db>,
    reason: Reason<'db>,
}

/// The first `mut` array, struct or function in `ty` that an `imm` array
/// or struct holds, with the type that holds it. What a function takes and
/// gives isn't looked into: its type says that, not what it holds, which
/// its mutability covers. What a struct holds is what its fields do
/// (`crate::holds`), not its type arguments: an `imm Handler[mut [i32]]`
/// for `struct Handler[T] { f: fn(T) }` holds nothing `mut`, and a
/// `mut VecIter[mut [i32], imm]` holds an `imm` array of `mut` ones.
pub fn mut_inside_imm<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<(Ty<'db>, Ty<'db>)> {
    fn has(ty: &Ty<'_>, mutability: Mutability) -> bool {
        ty.held_mutability() == Some(&Ty::Mutability(mutability))
    }
    /// Something `mut` that a value of type `ty` is, or holds as a value
    /// does, in an enum. A struct, tuple or array that isn't `mut` is
    /// looked into on its own.
    fn find_mut<'db>(ty: &Ty<'db>) -> Option<Ty<'db>> {
        // A `mut` on its own is a mutability argument, like that of
        // `VecIter[i32, mut]`, whose fields can have it.
        if has(ty, Mutability::Mut) || *ty == Ty::Mutability(Mutability::Mut) {
            return Some(ty.clone());
        }
        match ty {
            Ty::Adt(AdtId::Enum(_), args, ..) => args.iter().find_map(find_mut),
            _ => None,
        }
    }
    /// Something `mut` that `ty` holds where it must be at most `imm`.
    fn breaks<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Ty<'db>> {
        let imm = Ty::Mutability(Mutability::Imm);
        match ty {
            Ty::Array(m, elem) if **m == imm => find_mut(elem),
            Ty::Tuple(tys, m) if **m == imm => tys.iter().find_map(find_mut),
            Ty::Record(tys, named, m) if **m == imm => {
                tys.iter().chain(named.iter().map(|(_, t)| t)).find_map(find_mut)
            }
            Ty::Adt(AdtId::Struct(s), args, ms, m) => struct_atoms_of(db, *s, args, ms, m)
                .into_iter()
                .filter(|a| a.bound == imm)
                .find_map(|a| match a.held {
                    Held::Type(t) => find_mut(&t),
                    Held::Mutability(m) => (m == Ty::Mutability(Mutability::Mut)).then_some(m),
                }),
            _ => None,
        }
    }
    if matches!(ty, Ty::Fn(..)) {
        return None;
    }
    if let Some(inner) = breaks(db, ty) {
        return Some((ty.clone(), inner));
    }
    let mut found = None;
    ty.walk_children(|t| found = found.take().or_else(|| mut_inside_imm(db, t)));
    found
}

/// Whether the values of `def` are always `imm`: a variant, or an `imm
/// struct`.
fn is_always_imm<'db>(db: &'db dyn Db, def: VariantDef<'db>) -> bool {
    match def {
        VariantDef::Variant(_) => true,
        VariantDef::Struct(s) => s.item(db).is_imm,
    }
}

/// The mutability of an array literal: `Mut` for `mut [a, b]`, and `Imm`
/// for `[a, b]`.
fn literal_mutability<'db>(is_mut: bool) -> Ty<'db> {
    Ty::Mutability(if is_mut { Mutability::Mut } else { Mutability::Imm })
}

/// The reason for the `index`th of several expressions that must have the
/// same type, like the arms of a `match`: the reason they all have, or else
/// the first of them.
fn reason_for_nth<'db>(reason: Reason<'db>, first: ExprId, index: usize) -> Reason<'db> {
    match reason {
        Reason::None if index > 0 => Reason::SameAs(first),
        reason => reason,
    }
}

struct Breakable<'db> {
    label: Option<LabelId>,
    /// Whether it's a loop, which `break` without a label leaves.
    is_loop: bool,
    /// The type of the value `break` gives, if it can give one.
    ty: Option<Ty<'db>>,
    broken: bool,
}

pub(crate) struct InferenceContext<'a, 'db> {
    pub db: &'db dyn Db,
    pub def: DefWithBodyId<'db>,
    pub body: &'db Body,
    pub solver: &'a mut Solver<'db>,
    env: &'a FnEnv<'db>,
    pub result: InferenceResult<'db>,
    return_ty: Ty<'db>,
    /// Why values returned must have `return_ty`.
    return_reason: Reason<'db>,
    breakables: Vec<Breakable<'db>>,
    /// Variables declared by a `let` without a value, like `let x;`, that
    /// haven't been assigned yet. The first `x = ...` initializes them, so
    /// it's allowed even if they aren't `mut`.
    uninitialized: Vec<BindingId>,
    /// Whether the callee being inferred is that of `mut Wrapper(x)`, so
    /// that the tuple struct it makes is `mut`. Taken by the struct's
    /// constructor; if nothing takes it, `mut` was before something else.
    mut_constructor: bool,
    /// The closures being inferred, innermost last.
    closures: Vec<ClosureFrame>,
    /// The effects the code being inferred can do: those of its function or
    /// closure. Each call adds those of what it calls, as `callee ≲ effects`.
    /// Top-level code and constants can do none.
    effects: Ty<'db>,
}

/// What a closure being inferred declares and uses from outside it.
#[derive(Default)]
struct ClosureFrame {
    declared: Vec<BindingId>,
    captured: Vec<BindingId>,
}

impl<'a, 'db> InferenceContext<'a, 'db> {
    pub fn new(
        db: &'db dyn Db,
        def: DefWithBodyId<'db>,
        solver: &'a mut Solver<'db>,
        env: &'a FnEnv<'db>,
    ) -> Self {
        let body = boltlang_hir_def::body::body(db, def);
        InferenceContext {
            db,
            def,
            body,
            solver,
            env,
            result: InferenceResult::default(),
            return_ty: Ty::Error,
            return_reason: Reason::None,
            breakables: Vec::new(),
            uninitialized: Vec::new(),
            mut_constructor: false,
            closures: Vec::new(),
            effects: Ty::nothing(),
        }
    }

    /// Note that `binding` is declared, in the closures being inferred.
    pub(crate) fn declare(&mut self, binding: BindingId) {
        for frame in &mut self.closures {
            frame.declared.push(binding);
        }
    }

    /// Note that `binding` is used: the closures being inferred that it's
    /// declared outside of capture it.
    fn use_binding(&mut self, binding: BindingId) {
        for frame in &mut self.closures {
            if !frame.declared.contains(&binding) && !frame.captured.contains(&binding) {
                frame.captured.push(binding);
            }
        }
    }

    /// The file of the body being inferred.
    pub(crate) fn file(&self) -> File {
        self.def.def(self.db).file(self.db)
    }

    fn origin(&self, expr: ExprId) -> Origin<'db> {
        Origin::new(self.def, ExprOrPat::Expr(expr))
    }

    fn new_var(&mut self) -> Ty<'db> {
        self.solver.new_var(VarKind::General)
    }

    /// A variable for the mutability of an array.
    fn new_mutability(&mut self) -> Ty<'db> {
        self.solver.new_var(VarKind::Mutability)
    }

    /// The expected type, or a new variable when nothing is expected.
    fn expected_or_var(&mut self, expected: Option<Expectation<'_, 'db>>) -> Ty<'db> {
        expected.map(|e| e.ty.clone()).unwrap_or_else(|| self.new_var())
    }

    pub(crate) fn unresolved(&mut self, expr: ExprOrPat) {
        self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr });
    }

    /// `path` doesn't resolve: say so, or that what it names isn't visible.
    pub(crate) fn unresolved_path(
        &mut self,
        expr: ExprOrPat,
        resolver: &Resolver<'db>,
        path: &Path,
    ) {
        match resolver.hidden(self.db, path) {
            Some(hidden) => {
                self.result.diagnostics.push(InferenceDiagnostic::NotVisible { expr, hidden })
            }
            None => {
                let expr = match expr {
                    ExprOrPat::Expr(e) => ExprOrPat::Expr(self.unresolved_part(e, resolver, path)),
                    pat => pat,
                };
                self.unresolved(expr)
            }
        }
    }

    /// The part of a path like `nope.push`, written as fields, that doesn't
    /// resolve: the first that refers to nothing, like `nope`, rather than
    /// the whole, whose name is `push`.
    fn unresolved_part(&self, expr: ExprId, resolver: &Resolver<'db>, path: &Path) -> ExprId {
        let (mut expr, mut len) = (expr, path.segments.len());
        while let Expr::Field { expr: base, .. } = &self.body[expr] {
            len -= 1;
            if len == 0 {
                break;
            }
            let prefix = Path { segments: path.segments[..len].into() };
            if resolver.resolves(self.db, &prefix) {
                break;
            }
            expr = *base;
        }
        expr
    }

    /// The type of `def`'s struct or enum, with new variables for its type
    /// arguments and, for a struct, its mutability arguments, and the
    /// mutability `mutability`. Also gives what to substitute in its field
    /// types: the arguments, then for a struct its mutability arguments and
    /// its mutability (see [`struct_args`]).
    pub(crate) fn fresh_adt(
        &mut self,
        def: VariantDef<'db>,
        mutability: Ty<'db>,
    ) -> (Ty<'db>, Box<[Ty<'db>]>) {
        let (adt, generic_def) = def.adt();
        // An `imm struct` is always `imm`.
        let mutability = match adt {
            AdtId::Struct(s) if s.item(self.db).is_imm => Ty::Mutability(Mutability::Imm),
            _ => mutability,
        };
        let count = generic_def.generic_params(self.db).type_params.len();
        let args: Vec<Ty<'db>> = (0..count).map(|_| self.new_var()).collect();
        let named = named_mutabilities(self.db, generic_def).len();
        let mutabilities: Vec<Ty<'db>> = (0..named).map(|_| self.new_mutability()).collect();
        let ty = Ty::adt(adt, args.clone(), mutabilities, mutability);
        let subst = match &ty {
            Ty::Adt(AdtId::Struct(_), args, mutabilities, mutability) => {
                struct_args(args, mutabilities, mutability)
            }
            _ => args,
        };
        (ty, subst.into())
    }

    /// An array or struct of mutability `mutability` that holds values of
    /// type `ty` is made at `at`: they must be at most its mutability.
    fn holds(&mut self, ty: &Ty<'db>, mutability: &Ty<'db>, at: ExprId) {
        let constraint =
            AtMostConstraint { held: Held::Type(ty.clone()), bound: mutability.clone() };
        let source = AtMostSource::Object(constraint.clone());
        self.solver.add(Constraint::AtMost { constraint, source, origin: self.origin(at) });
    }

    /// A struct of type `ty` is made at `at`: what its fields hold must be
    /// at most their mutability, like `'v ⊑ m` and `T ⊑ 'v` for
    /// `VecIter[T, 'v]` (`crate::holds`).
    fn struct_holds(&mut self, ty: &Ty<'db>, at: ExprId) {
        if let Ty::Adt(AdtId::Struct(s), args, mutabilities, mutability) = ty {
            for constraint in struct_atoms_of(self.db, *s, args, mutabilities, mutability) {
                let source = AtMostSource::Object(constraint.clone());
                self.solver.add(Constraint::AtMost { constraint, source, origin: self.origin(at) });
            }
        }
    }

    /// A value of the struct or enum of `def`, with the type arguments
    /// `args`, is made at `at`, so the arguments must meet the bounds of its
    /// type parameters, like the `Iterator` of `struct Zip[A: Iterator]`. An
    /// error points at the bound.
    fn require_bounds(&mut self, def: VariantDef<'db>, args: &[Ty<'db>], at: ExprId) {
        let (_, generic_def) = def.adt();
        let predicates = generic_predicates(self.db, generic_def);
        for (predicate, &type_ref) in predicates.predicates.iter().zip(predicates.sources.iter()) {
            let source = ConstraintSource::Declared { def: generic_def, type_ref };
            let origin = Origin { reason: Reason::RequiredBy(source), ..self.origin(at) };
            let constraint = predicate.map(|t| subst_params(t, generic_def, args));
            self.solver.add(Constraint::Class { constraint, origin });
        }
        // Bounds like the `T: imm` of `struct Frozen[T: imm]`.
        for (bound, type_ref) in predicates.at_most.iter() {
            let source = ConstraintSource::Declared { def: generic_def, type_ref: *type_ref };
            let origin = Origin { reason: Reason::RequiredBy(source), ..self.origin(at) };
            let constraint = bound.map(|t| subst_params(t, generic_def, args));
            let source = AtMostSource::Needed(constraint.clone());
            self.solver.add(Constraint::AtMost { constraint, source, origin });
        }
    }

    fn resolver(&self, expr: ExprId) -> Resolver<'db> {
        Resolver::for_expr(self.db, self.def, expr)
    }

    /// The row of the effects written after a closure's `does`.
    fn lower_row(&mut self, expr: ExprId, terms: &[TypeRefId]) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let mut ctx = TyLoweringContext::new(self.db, &resolver, &self.body.store);
        let row = ctx.lower_row(terms);
        self.result.diagnostics.extend(ctx.diagnostics.into_iter().map(InferenceDiagnostic::Lower));
        row
    }

    fn lower_ty(&mut self, expr: ExprId, type_ref: TypeRefId) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let mut ctx = TyLoweringContext::new(self.db, &resolver, &self.body.store);
        // A body can use the named mutabilities of its function.
        if let DefWithBody::Function(f) = self.def.def(self.db) {
            ctx = ctx.with_mutabilities(GenericDefId::Function(f));
        }
        let ty = ctx.lower_ty(type_ref);
        self.result.diagnostics.extend(ctx.diagnostics.into_iter().map(InferenceDiagnostic::Lower));
        // An array without `mut` or `imm` in a body can be either, which
        // the value decides.
        let ty = ty.fill_unwritten(&mut || self.solver.new_var(VarKind::Mutability));
        let origin = self.origin(expr);
        self.solver.flatten_assoc(&ty, origin)
    }

    /// Infer a function body with the given parameter and return types.
    pub fn infer_fn_body(&mut self, params: &[Ty<'db>], ret: &Ty<'db>, effects: &Ty<'db>) {
        self.effects = effects.clone();
        // The associated types in the signature, like `C.Item`, are decided
        // by the solver, which knows the `where` clauses and impls.
        let origin = self.origin(self.body.body_expr);
        let ret = &self.solver.flatten_assoc(ret, origin);
        let mut params = params.iter();
        let mut tys = Vec::new();
        for &pat in self.body.params.iter() {
            let ty = params.next().cloned().unwrap_or(Ty::Error);
            let ty = self.solver.flatten_assoc(&ty, origin);
            self.check_pat(pat, &ty);
            tys.push(ty);
        }
        // A default has the type of its parameter. It's evaluated where the
        // function is called, so what it does, the function does.
        for (default, ty) in self.body.defaults.iter().zip(&tys) {
            if let Some(default) = *default {
                self.check_expr(default, ty);
            }
        }
        self.return_ty = ret.clone();
        self.return_reason = match self.def.def(self.db) {
            DefWithBody::Function(f) if function_signature(self.db, f).ret_type.is_some() => {
                Reason::ReturnType
            }
            _ => Reason::None,
        };
        let body_expr = self.body.body_expr;
        self.check_expr_because(body_expr, ret, self.return_reason);
    }

    /// Infer a body that's an expression, like that of a const.
    pub fn infer_expr_body(&mut self, expected: Option<&Ty<'db>>) -> Ty<'db> {
        let body_expr = self.body.body_expr;
        let reason = match self.def.def(self.db) {
            DefWithBody::Const(_) => Reason::ConstType,
            _ => Reason::None,
        };
        match expected {
            Some(ty) => {
                self.check_expr_because(body_expr, ty, reason);
                ty.clone()
            }
            None => self.infer_expr(body_expr),
        }
    }

    pub fn check_expr(&mut self, expr: ExprId, expected: &Ty<'db>) {
        self.check_expr_because(expr, expected, Reason::None);
    }

    /// `e?`: the `T` of an `Option[T]` or a `Result[E, T]`. The function
    /// it's in must give the same kind, an `Option`, or a `Result` with the
    /// same `E`, which a `None` or an `Err` is returned as. When the type of
    /// `e` isn't known yet, the function's says which it is.
    fn infer_try(
        &mut self,
        expr: ExprId,
        inner: ExprId,
        value: BindingId,
        error: BindingId,
    ) -> Ty<'db> {
        let db = self.db;
        let file = self.file();
        let option = crate::lang::option_enum(db, file);
        let result = crate::lang::result_enum(db, file);
        let ty = self.infer_expr(inner);
        let ret = self.return_ty.clone();
        let enum_of = |t: &Ty<'db>| match t {
            Ty::Adt(AdtId::Enum(e), args, ..) => Some((*e, args.clone())),
            _ => None,
        };
        let enum_ty = |e, args: Vec<Ty<'db>>| Ty::adt(AdtId::Enum(e), args, [], Ty::Unwritten);
        let resolved = self.solver.unifier.shallow_resolve(&ty);
        let ret_resolved = self.solver.unifier.shallow_resolve(&ret);
        // Which kind it is, from `e`'s type, or else from the function's.
        let kind = match (enum_of(&resolved), &resolved) {
            (Some((e, _)), _) if Some(e) == option => Some(TryKind::Option(e)),
            (Some((e, _)), _) if Some(e) == result => Some(TryKind::Result(e)),
            (_, Ty::Error) => return Ty::Error,
            (_, Ty::Var(_)) => match enum_of(&ret_resolved) {
                Some((e, _)) if Some(e) == option => Some(TryKind::Option(e)),
                Some((e, _)) if Some(e) == result => Some(TryKind::Result(e)),
                _ => {
                    let reason = BadTry::Unknown;
                    self.result.diagnostics.push(InferenceDiagnostic::BadTry { expr, reason });
                    return Ty::Error;
                }
            },
            _ => None,
        };
        let Some(kind) = kind else {
            let reason = BadTry::NotOptionOrResult(resolved);
            self.result.diagnostics.push(InferenceDiagnostic::BadTry { expr, reason });
            return Ty::Error;
        };
        let origin = self.origin(inner);
        // What `e` has, and the error it passes on, if any.
        let (value_ty, error_ty) = match kind {
            TryKind::Option(e) => {
                let t = self.new_var();
                self.solver.equal(&enum_ty(e, vec![t.clone()]), &ty, origin);
                (t, Ty::unit())
            }
            TryKind::Result(e) => {
                let (err, t) = (self.new_var(), self.new_var());
                self.solver.equal(&enum_ty(e, vec![err.clone(), t.clone()]), &ty, origin);
                (t, err)
            }
        };
        // The function must give the same kind, with the same error.
        let ret_origin = Origin { reason: self.return_reason, ..self.origin(expr) };
        match (kind, enum_of(&ret_resolved), &ret_resolved) {
            (TryKind::Option(e), Some((r, _)), _) | (TryKind::Result(e), Some((r, _)), _)
                if e == r =>
            {
                let expected = match kind {
                    TryKind::Option(_) => vec![self.new_var()],
                    TryKind::Result(_) => vec![error_ty.clone(), self.new_var()],
                };
                self.solver.equal(&ret, &enum_ty(e, expected), ret_origin);
            }
            (_, _, Ty::Var(_)) => {
                let expected = match kind {
                    TryKind::Option(e) => enum_ty(e, vec![self.new_var()]),
                    TryKind::Result(e) => enum_ty(e, vec![error_ty.clone(), self.new_var()]),
                };
                self.solver.equal(&ret, &expected, ret_origin);
            }
            (_, _, Ty::Error) => {}
            _ => {
                let result = matches!(kind, TryKind::Result(_));
                let reason = BadTry::WrongReturn { result, ret: ret_resolved };
                self.result.diagnostics.push(InferenceDiagnostic::BadTry { expr, reason });
            }
        }
        self.result.binding_types.insert(value, value_ty.clone());
        self.result.binding_types.insert(error, error_ty);
        self.result.tries.insert(expr, Try { kind, ret });
        value_ty
    }

    fn check_expr_because(&mut self, expr: ExprId, expected: &Ty<'db>, reason: Reason<'db>) {
        let actual = self.infer_expr_expect(expr, Some(Expectation { ty: expected, reason }));
        self.solver.equal(expected, &actual, Origin { reason, ..self.origin(expr) });
    }

    pub fn infer_expr(&mut self, expr: ExprId) -> Ty<'db> {
        self.infer_expr_expect(expr, None)
    }

    /// Infer the type of an expression. `expected` is a hint, for branches
    /// and lambdas; the caller checks the result against it.
    fn infer_expr_expect(
        &mut self,
        expr: ExprId,
        expected: Option<Expectation<'_, 'db>>,
    ) -> Ty<'db> {
        let ty = self.infer_expr_inner(expr, expected);
        self.result.expr_types.insert(expr, ty.clone());
        ty
    }

    fn infer_expr_inner(
        &mut self,
        expr: ExprId,
        expected: Option<Expectation<'_, 'db>>,
    ) -> Ty<'db> {
        let body = self.body;
        let reason = expected.map_or(Reason::None, |e| e.reason);
        match &body[expr] {
            Expr::Missing => Ty::Error,
            Expr::Path(path) => self.infer_path(expr, path),
            Expr::Literal(lit) => self.literal_ty(ExprOrPat::Expr(expr), lit),
            Expr::Block { statements, tail, label, .. } => {
                if let Some(label) = label {
                    let ty = self.expected_or_var(expected);
                    self.breakables.push(Breakable {
                        label: Some(*label),
                        is_loop: false,
                        ty: Some(ty.clone()),
                        broken: false,
                    });
                    let actual =
                        self.infer_block(statements, *tail, Some(Expectation { ty: &ty, reason }));
                    self.breakables.pop();
                    self.solver.equal(&ty, &actual, self.origin(expr));
                    ty
                } else {
                    self.infer_block(statements, *tail, expected)
                }
            }
            Expr::Call { callee, args } => {
                self.infer_call(expr, *callee, CallArgs::Positional(args), false)
            }
            Expr::CallWith { callee, args } => {
                self.infer_call(expr, *callee, CallArgs::Items(args), false)
            }
            Expr::Field { expr: base, name } => self.infer_field(expr, *base, name),
            Expr::Try { expr: inner, value, error } => self.infer_try(expr, *inner, *value, *error),
            Expr::Cast { expr: inner, type_ref } => {
                let from = self.infer_expr(*inner);
                let to = self.lower_ty(expr, *type_ref);
                // A struct or enum isn't a number, even an `imm struct`
                // that holds one, like `Char`: it has functions for that,
                // like `to_u32`.
                let from = self.solver.unifier.shallow_resolve(&from);
                let adt = |ty: &Ty<'db>| match ty {
                    Ty::Adt(id, ..) => Some(*id),
                    _ => None,
                };
                let known = |ty: &Ty<'db>| !matches!(ty, Ty::Var(_) | Ty::Error | Ty::Never);
                if known(&from)
                    && known(&to)
                    && (adt(&from).is_some() || adt(&to).is_some())
                    && adt(&from) != adt(&to)
                {
                    self.result.diagnostics.push(InferenceDiagnostic::InvalidCast {
                        expr,
                        from,
                        to: to.clone(),
                    });
                }
                to
            }
            Expr::Index { base, index } => {
                // Through the impl of `Index` for the type, arrays too.
                let base_ty = self.infer_expr(*base);
                self.check_expr(*index, &Ty::Builtin(BuiltinType::Int(BuiltinInt::I32)));
                self.lang_assoc(LangClass::Index, &base_ty, expr).unwrap_or(Ty::Error)
            }
            // `-a` calls the function `-` in scope, like the prelude's
            // `fn -a = neg(a)`.
            Expr::Prefix { op, expr: inner } => {
                let ty = self.infer_expr(*inner);
                self.infer_operator_call(expr, op, *inner, ty, &[])
            }
            // `imm e`: a copy of `e` with each array and struct `imm`.
            Expr::Imm { expr: inner } => {
                let from = self.infer_expr(*inner);
                let to = self.new_var();
                let constraint = ImmConstraint { from, to: to.clone() };
                self.solver.add(Constraint::Imm { constraint, origin: self.origin(expr) });
                to
            }
            // `mut [1]`, `mut Point { x: 1 }` and `mut Wrapper(1)` make an
            // array or struct `mut`. `mut` before anything else is an error:
            // a copy is `to_mut(e)` or `to_mut_deep(e)`.
            Expr::Mut { expr: inner } => {
                let literal = match &body[*inner] {
                    Expr::Array { .. }
                    | Expr::ArrayRepeat { .. }
                    | Expr::Record { .. }
                    | Expr::Tuple { .. }
                    | Expr::NamedTuple { .. } => true,
                    Expr::Call { callee, .. } => self.constructs_struct(*callee),
                    _ => false,
                };
                if literal {
                    self.result.mut_literals.push((expr, *inner));
                    let ty = self.infer_literal(expr, *inner, expected, true);
                    self.result.expr_types.insert(*inner, ty.clone());
                    ty
                } else {
                    self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
                    self.infer_expr(*inner)
                }
            }
            Expr::Binary { op, lhs, rhs } => self.infer_binary(expr, op, *lhs, *rhs),
            // `a..b` is the prelude's `Range { start: a, end: b }`, and `a..=b`
            // its `RangeInclusive`. The other ranges have no types in std
            // yet.
            Expr::Range { lhs: Some(start), rhs: Some(end), inclusive } => {
                self.infer_range(expr, *start, *end, *inclusive)
            }
            Expr::Range { lhs, rhs, .. } => {
                for e in lhs.iter().chain(rhs.iter()) {
                    self.infer_expr(*e);
                }
                self.result.diagnostics.push(InferenceDiagnostic::Unsupported { expr });
                Ty::Error
            }
            Expr::Array { .. }
            | Expr::ArrayRepeat { .. }
            | Expr::Record { .. }
            | Expr::Tuple { .. }
            | Expr::NamedTuple { .. } => self.infer_literal(expr, expr, expected, false),
            Expr::If { condition, then_branch, else_branch } => {
                self.check_expr(*condition, &Ty::bool());
                match else_branch {
                    Some(else_branch) => {
                        let ty = self.expected_or_var(expected);
                        self.check_expr_because(*then_branch, &ty, reason);
                        self.check_expr_because(
                            *else_branch,
                            &ty,
                            reason_for_nth(reason, *then_branch, 1),
                        );
                        ty
                    }
                    None => {
                        self.check_expr(*then_branch, &Ty::unit());
                        Ty::unit()
                    }
                }
            }
            Expr::Let { pat, expr: value } => {
                let ty = self.infer_expr(*value);
                self.check_pat(*pat, &ty);
                Ty::bool()
            }
            Expr::Match { expr: scrutinee, arms, effect_arms } => {
                // With effect arms, the match handles what its scrutinee does:
                // that runs with effects of its own.
                let inner = (!effect_arms.is_empty()).then(|| self.solver.new_var(VarKind::Effect));
                let outer = inner.clone().map(|inner| std::mem::replace(&mut self.effects, inner));
                let scrutinee_ty = self.infer_expr(*scrutinee);
                if let Some(outer) = outer {
                    self.effects = outer;
                }
                let ty = self.expected_or_var(expected);
                for (i, arm) in arms.iter().enumerate() {
                    self.check_pat(arm.pat, &scrutinee_ty);
                    if let Some(guard) = arm.guard {
                        self.check_expr(guard, &Ty::bool());
                    }
                    self.check_expr_because(arm.expr, &ty, reason_for_nth(reason, arms[0].expr, i));
                }
                if let Some(inner) = inner {
                    self.check_effect_arms(expr, effect_arms, inner, &ty);
                }
                if arms.is_empty() { Ty::Never } else { ty }
            }
            Expr::While { condition, body: loop_body, label } => {
                self.breakables.push(Breakable {
                    label: *label,
                    is_loop: true,
                    ty: None,
                    broken: false,
                });
                self.check_expr(*condition, &Ty::bool());
                self.check_expr(*loop_body, &Ty::unit());
                self.breakables.pop();
                Ty::unit()
            }
            Expr::Loop { body: loop_body, label } => {
                let ty = self.expected_or_var(expected);
                self.breakables.push(Breakable {
                    label: *label,
                    is_loop: true,
                    ty: Some(ty.clone()),
                    broken: false,
                });
                self.check_expr(*loop_body, &Ty::unit());
                let breakable = self.breakables.pop().unwrap();
                // A loop that's never left doesn't give a value.
                if breakable.broken { ty } else { Ty::Never }
            }
            Expr::For { pat, iterable, body: loop_body, label } => {
                // Through the impl of `Iterable` for the type, arrays too,
                // whose iterator gives the items with `Iterator`.
                let iterable_ty = self.infer_expr(*iterable);
                let elem = self
                    .lang_assoc(LangClass::Iterable, &iterable_ty, *iterable)
                    .and_then(|iter| self.lang_assoc(LangClass::Iterator, &iter, expr))
                    .unwrap_or(Ty::Error);
                self.check_pat(*pat, &elem);
                self.breakables.push(Breakable {
                    label: *label,
                    is_loop: true,
                    ty: None,
                    broken: false,
                });
                self.check_expr(*loop_body, &Ty::unit());
                self.breakables.pop();
                Ty::unit()
            }
            Expr::Break { expr: value, label } => {
                let target = match label {
                    Some(name) => {
                        let scopes = expr_scopes(self.db, self.def);
                        let id = scopes
                            .scope_for(expr)
                            .and_then(|s| scopes.resolve_label(s, Some(name)));
                        self.breakables.iter().rposition(|b| b.label.is_some() && b.label == id)
                    }
                    None => self.breakables.iter().rposition(|b| b.is_loop),
                };
                match target {
                    Some(i) => {
                        self.breakables[i].broken = true;
                        let ty = self.breakables[i].ty.clone().unwrap_or_else(Ty::unit);
                        match value {
                            Some(value) => self.check_expr(*value, &ty),
                            None => self.solver.equal(&ty, &Ty::unit(), self.origin(expr)),
                        }
                    }
                    None => {
                        if let Some(value) = value {
                            self.infer_expr(*value);
                        }
                        self.result.diagnostics.push(InferenceDiagnostic::BreakOutside { expr });
                    }
                }
                Ty::Never
            }
            Expr::Continue { .. } => Ty::Never,
            Expr::Return { expr: value } => {
                let ret = self.return_ty.clone();
                match value {
                    Some(value) => self.check_expr_because(*value, &ret, self.return_reason),
                    None => self.solver.equal(
                        &ret,
                        &Ty::unit(),
                        Origin { reason: self.return_reason, ..self.origin(expr) },
                    ),
                }
                Ty::Never
            }
            Expr::Lambda { .. } => self.infer_lambda(expr, expected),
            Expr::MacroCall { .. } => Ty::Error,
        }
    }

    /// A closure or an anonymous function, like `fn (x) = x + 1`. Its
    /// parameter types come from what it's written with, or else from
    /// the function type it's expected to have.
    fn infer_lambda(&mut self, expr: ExprId, expected: Option<Expectation<'_, 'db>>) -> Ty<'db> {
        let Expr::Lambda { params, param_types, ret_type, effects, body: lambda_body, .. } =
            &self.body[expr]
        else {
            unreachable!("only called for a lambda")
        };
        let (hint_params, hint_names, hint_ret) =
            match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                // A lambda only has positional parameters for now.
                Some(Ty::Fn(ps, r, _, _))
                    if ps.is_positional() && ps.positional.len() == params.len() =>
                {
                    (Some(ps.positional), Some(ps.names), Some(*r))
                }
                _ => (None, None, None),
            };
        // The names of the parameters are those of the function type
        // it's checked against, or else those of its patterns.
        let names = hint_names.map(|n| n.into_vec()).unwrap_or_else(|| {
            let body = self.body;
            params
                .iter()
                .map(|&p| match &body[p] {
                    Pat::Bind { id, subpat: None } => Ty::ParamName(Some(body[*id].name.clone())),
                    _ => Ty::ParamName(None),
                })
                .collect()
        });
        self.closures.push(ClosureFrame::default());
        let mut param_tys = Vec::new();
        for (i, (&pat, type_ref)) in params.iter().zip(param_types.iter()).enumerate() {
            let ty = match (type_ref, &hint_params) {
                (Some(type_ref), _) => self.lower_ty(expr, *type_ref),
                (None, Some(hints)) => hints[i].clone(),
                (None, None) => self.new_var(),
            };
            self.check_pat(pat, &ty);
            param_tys.push(ty);
        }
        let ret = match (ret_type, hint_ret) {
            (Some(type_ref), _) => self.lower_ty(expr, *type_ref),
            (None, Some(hint)) => hint,
            (None, None) => self.new_var(),
        };
        // The effects that aren't written are inferred from what the
        // body calls, which adds to the closure's effects, not to
        // those of the code around it.
        let effects = match effects {
            Some(terms) => self.lower_row(expr, terms),
            None => self.solver.new_var(VarKind::Effect),
        };
        let ret_reason = ret_type.map_or(Reason::None, Reason::Annotation);
        let outer_return = std::mem::replace(&mut self.return_ty, ret.clone());
        let outer_reason = std::mem::replace(&mut self.return_reason, ret_reason);
        let outer_breakables = std::mem::take(&mut self.breakables);
        let outer_effects = std::mem::replace(&mut self.effects, effects.clone());
        self.check_expr_because(*lambda_body, &ret, ret_reason);
        self.effects = outer_effects;
        self.breakables = outer_breakables;
        self.return_reason = outer_reason;
        self.return_ty = outer_return;
        let frame = self.closures.pop().unwrap();
        let body = self.body;
        let closure = Ty::Fn(
            Box::new(Params::positional(param_tys).with_names(names)),
            Box::new(ret),
            Box::new(self.new_mutability()),
            Box::new(effects),
        );
        // What the closure captured is at most its mutability, and a
        // `let mut` variable it shares makes it `mut`.
        let Ty::Fn(_, _, own, _) = &closure else { unreachable!() };
        let origin = self.origin(expr);
        if frame.captured.iter().any(|&b| body[b].mutable) {
            self.solver.equal(own, &Ty::Mutability(Mutability::Mut), origin);
        }
        for &b in &frame.captured {
            let constraint = AtMostConstraint {
                held: Held::Type(self.result.binding_ty(b).clone()),
                bound: (**own).clone(),
            };
            self.solver.add(Constraint::AtMost {
                constraint,
                source: AtMostSource::Closure(closure.clone()),
                origin,
            });
        }
        closure
    }

    fn infer_block(
        &mut self,
        statements: &[Statement],
        tail: Option<ExprId>,
        expected: Option<Expectation<'_, 'db>>,
    ) -> Ty<'db> {
        let mut diverges = false;
        for statement in statements {
            match statement {
                Statement::Let { pat, type_ref, initializer, else_branch } => {
                    let ty = match type_ref {
                        Some(type_ref) => {
                            let at = initializer.unwrap_or(self.body.body_expr);
                            self.lower_ty(at, *type_ref)
                        }
                        None => self.new_var(),
                    };
                    if let Some(init) = initializer {
                        self.check_expr_because(
                            *init,
                            &ty,
                            type_ref.map_or(Reason::None, Reason::Annotation),
                        );
                    } else if let Pat::Bind { id, subpat: None } = &self.body[*pat] {
                        self.uninitialized.push(*id);
                    }
                    if let Some(else_branch) = else_branch {
                        self.check_expr(*else_branch, &Ty::Never);
                    }
                    self.check_pat(*pat, &ty);
                }
                Statement::Expr { expr, has_semi } => {
                    let ty = self.infer_expr(*expr);
                    if matches!(self.solver.unifier.shallow_resolve(&ty), Ty::Never) {
                        diverges = true;
                    } else if !has_semi {
                        // A block-like expression used as a statement.
                        self.solver.equal(&Ty::unit(), &ty, self.origin(*expr));
                    }
                }
                Statement::Item => {}
            }
        }
        match (tail, expected) {
            // Check the last expression itself, so that errors point at it.
            (Some(tail), Some(expected)) => {
                self.check_expr_because(tail, expected.ty, expected.reason);
                expected.ty.clone()
            }
            (Some(tail), None) => self.infer_expr(tail),
            (None, _) if diverges => Ty::Never,
            (None, _) => Ty::unit(),
        }
    }

    fn literal_ty(&mut self, at: ExprOrPat, lit: &Literal) -> Ty<'db> {
        let suffixed = |suffix: &Option<Name>| {
            suffix.as_ref().and_then(|s| BuiltinType::by_name(s.as_str())).map(Ty::Builtin)
        };
        match lit {
            Literal::Int(_, suffix) => {
                suffixed(suffix).unwrap_or_else(|| self.solver.new_var(VarKind::Int))
            }
            Literal::Float(_, suffix) => {
                suffixed(suffix).unwrap_or_else(|| self.solver.new_var(VarKind::Float))
            }
            Literal::Char(_) => {
                let file = self.file();
                char_ty(self.db, file).unwrap_or_else(|| {
                    self.result
                        .diagnostics
                        .push(InferenceDiagnostic::NoLangType { at, ty: "Char" });
                    Ty::Error
                })
            }
            Literal::Byte(_) => Ty::Builtin(BuiltinType::Uint(BuiltinUint::U8)),
            Literal::String(_) | Literal::CString(_) => {
                let file = self.file();
                string_ty(self.db, file).unwrap_or_else(|| {
                    self.result
                        .diagnostics
                        .push(InferenceDiagnostic::NoLangType { at, ty: "String" });
                    Ty::Error
                })
            }
            Literal::ByteString(_) => Ty::array(
                literal_mutability(false),
                Ty::Builtin(BuiltinType::Uint(BuiltinUint::U8)),
            ),
        }
    }

    /// The type of a function, instantiated for this use.
    fn function_ty(&mut self, expr: ExprId, f: FunctionId<'db>) -> Ty<'db> {
        let origin = self.origin(expr);
        match instantiate_function(self.db, self.solver, self.env, f, origin) {
            Ok(ty) => ty,
            Err(InferenceCycle) => {
                self.result.diagnostics.push(InferenceDiagnostic::CyclicInference);
                Ty::Error
            }
        }
    }

    /// The type of a use of a name that several functions have, like
    /// `push` (#275): a new variable, which an [`OverloadConstraint`]
    /// decides once the types leave one of them. `method_call` is the call
    /// if the use is the method of `x.f(y)`.
    fn overloaded_ty(
        &mut self,
        expr: ExprId,
        set: OverloadSet<'db>,
        method_call: Option<ExprId>,
    ) -> Ty<'db> {
        let all: Vec<_> = set.functions(self.db).iter().map(|&f| (f, self.probe_ty(f))).collect();
        let ty = self.new_var();
        let constraint = OverloadConstraint {
            candidates: all.clone(),
            ty: ty.clone(),
            all,
            method_call,
            args: Vec::new(),
        };
        self.solver.add_overload(constraint, self.origin(expr));
        ty
    }

    /// The type of `f` with new variables, to try against a use, without
    /// adding what it needs.
    fn probe_ty(&mut self, f: FunctionId<'db>) -> Ty<'db> {
        if let Some(ty) = self.env.mono.get(&f) {
            return ty.clone();
        }
        if let Some(scheme) = self.env.done.get(&f) {
            return scheme.probe(self.solver);
        }
        match function_scheme(self.db, f) {
            Ok(scheme) => scheme.probe(self.solver),
            // It fits anything, so a use stays ambiguous rather than wrong.
            Err(InferenceCycle) => Ty::Error,
        }
    }

    /// Choose the overloads that the types decide so far, so that what they
    /// decide is known for what follows.
    fn solve_overloads(&mut self) {
        if self.solver.has_pending_overloads() {
            solve_overloads(self.db, self.solver, self.env);
        }
    }

    fn infer_path(&mut self, expr: ExprId, path: &Path) -> Ty<'db> {
        let resolver = self.resolver(expr);
        match resolver.resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(value)) => self.value_ty(expr, value),
            // `Vec.grow`, a function of a type that isn't visible here.
            Some(ValueResolution::Type(TypeResolution { unresolved: Some(_), .. }))
                if resolver.hidden(self.db, path).is_some() =>
            {
                self.unresolved_path(ExprOrPat::Expr(expr), &resolver, path);
                Ty::Error
            }
            Some(ValueResolution::Type(resolution)) => self.type_as_value(expr, resolution),
            None => {
                self.unresolved_path(ExprOrPat::Expr(expr), &resolver, path);
                Ty::Error
            }
        }
    }

    fn value_ty(&mut self, expr: ExprId, value: ValueNs<'db>) -> Ty<'db> {
        match value {
            ValueNs::Local(binding) => {
                self.use_binding(binding);
                self.result.binding_types.get(binding).cloned().unwrap_or(Ty::Error)
            }
            ValueNs::Function(f) => self.function_ty(expr, f),
            ValueNs::Overloaded(set) => self.overloaded_ty(expr, set, None),
            ValueNs::Const(c) => self.const_value_ty(expr, c),
            // Const generics aren't supported yet.
            ValueNs::ConstParam { .. } => Ty::Error,
            ValueNs::Variant(v) => self.constructor_ty(expr, VariantDef::Variant(v), false),
            ValueNs::Builtin(v) => Ty::Builtin(v.ty()),
        }
    }

    /// The type of a constant used as a value. A class constant, like the
    /// `ZERO` of `class Zero[T] { const ZERO: T; }`, is generic in the
    /// class's parameters and needs an instance, like a class method.
    fn const_value_ty(&mut self, expr: ExprId, c: ConstId<'db>) -> Ty<'db> {
        let ty = const_ty(self.db, c).ty.clone();
        let ItemContainerId::Class(class) = c.container(self.db) else { return ty };
        let def = GenericDefId::Class(class);
        let scheme = Scheme {
            params: generic_param_ids(self.db, [def]),
            predicates: Box::new([ClassConstraint { class, args: identity_args(self.db, def) }]),
            ..Scheme::mono(ty)
        };
        scheme.instantiate(self.solver, self.origin(expr))
    }

    /// The type of a struct or variant used as a value: a function for a
    /// tuple struct, the type itself for a unit one. A struct is `imm`, or
    /// `mut` when it's made by `mut Wrapper(x)`. `expr` is where it's used,
    /// which must meet the bounds of the struct or enum.
    fn constructor_ty(&mut self, expr: ExprId, def: VariantDef<'db>, is_mut: bool) -> Ty<'db> {
        let (_, generic_def) = def.adt();
        if is_mut && is_always_imm(self.db, def) {
            self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
        }
        let (adt_ty, args) = self.fresh_adt(def, literal_mutability(is_mut));
        self.struct_holds(&adt_ty, expr);
        self.require_bounds(def, &args, expr);
        let fields = field_types(self.db, def);
        match fields.kind {
            FieldsKind::Unit => adt_ty,
            FieldsKind::Tuple => {
                // Making a tuple struct gives all of its fields.
                let file = self.file();
                if let (VariantDef::Struct(strukt), Some(name)) =
                    (def, private_field(self.db, def, file))
                {
                    self.result.diagnostics.push(InferenceDiagnostic::PrivateField {
                        at: ExprOrPat::Expr(expr),
                        strukt,
                        name,
                    });
                }
                let params: Box<[Ty<'db>]> = fields
                    .fields
                    .iter()
                    .map(|(_, t)| subst_params(t, generic_def, &args))
                    .collect();
                Ty::func(params, adt_ty)
            }
            // A record struct can't be used as a value.
            FieldsKind::Record => Ty::Error,
        }
    }

    /// Report the field `index` of `def` at `at` if it's private here.
    pub(crate) fn check_field_visible(
        &mut self,
        at: ExprOrPat,
        def: VariantDef<'db>,
        index: usize,
    ) {
        let file = self.file();
        if let VariantDef::Struct(strukt) = def
            && !field_visible(self.db, def, index, file)
        {
            let name = field_types(self.db, def).fields[index].0.clone();
            self.result.diagnostics.push(InferenceDiagnostic::PrivateField { at, strukt, name });
        }
    }

    /// `start..end` or `start..=end`: the prelude's `Range` or
    /// `RangeInclusive`, made like a struct literal.
    fn infer_range(
        &mut self,
        expr: ExprId,
        start: ExprId,
        end: ExprId,
        inclusive: bool,
    ) -> Ty<'db> {
        let file = self.file();
        let (found, name) = match inclusive {
            false => (range_struct(self.db, file), "Range"),
            true => (range_inclusive_struct(self.db, file), "RangeInclusive"),
        };
        let Some(s) = found else {
            self.infer_expr(start);
            self.infer_expr(end);
            let at = ExprOrPat::Expr(expr);
            self.result.diagnostics.push(InferenceDiagnostic::NoLangType { at, ty: name });
            return Ty::Error;
        };
        let def = VariantDef::Struct(s);
        let (ty, args) = self.fresh_adt(def, literal_mutability(false));
        self.struct_holds(&ty, expr);
        let fields = field_types(self.db, def);
        for (name, e) in [("start", start), ("end", end)] {
            let field = fields.field(&Name::new(name)).map(|(_, t)| t.clone());
            let field =
                field.map_or(Ty::Error, |t| subst_params(&t, GenericDefId::Struct(s), &args));
            self.check_expr(e, &field);
        }
        ty
    }

    /// A path that names a type used as a value: a constructor.
    fn type_as_value(&mut self, expr: ExprId, resolution: TypeResolution<'db>) -> Ty<'db> {
        match (resolution.ty, resolution.unresolved) {
            (TypeNs::Struct(s), None) => {
                let is_mut = std::mem::take(&mut self.mut_constructor);
                self.constructor_ty(expr, VariantDef::Struct(s), is_mut)
            }
            (TypeNs::Variant(v), None) => self.constructor_ty(expr, VariantDef::Variant(v), false),
            _ => {
                self.unresolved(ExprOrPat::Expr(expr));
                Ty::Error
            }
        }
    }

    /// Whether `expr` is a path that doesn't name a value, like `Color` or
    /// a module, so that `expr.name` is a longer path. `expr` can be such a
    /// longer path itself, like the `m.Color` of `m.Color.Red`.
    fn path_extended(&self, expr: ExprId, name: &Name) -> Option<Path> {
        let path = match &self.body[expr] {
            Expr::Path(path) => path.clone(),
            Expr::Field { expr: base, name: inner } => self.path_extended(*base, inner)?,
            _ => return None,
        };
        let resolver = self.resolver(expr);
        if let Some(ValueResolution::Value(_)) = resolver.resolve_path_in_value_ns(self.db, &path) {
            return None;
        }
        let mut segments = path.segments.to_vec();
        segments.push(PathSegment { name: name.clone(), generic_args: None });
        Some(Path { segments: segments.into() })
    }

    fn infer_field(&mut self, expr: ExprId, base: ExprId, name: &Name) -> Ty<'db> {
        if let Some(path) = self.path_extended(base, name) {
            return self.infer_path(expr, &path);
        }
        let base_ty = self.infer_expr(base);
        // The solver finds the field once the type is known; until then, a
        // variable stands for the field's type.
        let field = self.new_var();
        let constraint = FieldConstraint {
            ty: base_ty,
            name: name.clone(),
            field: field.clone(),
            assign: false,
        };
        self.solver.add(Constraint::Field { constraint, origin: self.origin(expr) });
        field
    }

    /// The array, tuple, struct literal or call `literal`, made `mut` with
    /// `is_mut`, as `mut [1]`, `mut (1, 2)`, `mut Point { x: 1 }` or
    /// `mut Wrapper(1)` do.
    /// `expr` is where it's reported: the `mut` around it, if there is one.
    fn infer_literal(
        &mut self,
        expr: ExprId,
        literal: ExprId,
        expected: Option<Expectation<'_, 'db>>,
        is_mut: bool,
    ) -> Ty<'db> {
        let body = self.body;
        let reason = expected.map_or(Reason::None, |e| e.reason);
        let mutability = literal_mutability(is_mut);
        match &body[literal] {
            Expr::Array { exprs } => {
                let elem = match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                    Some(Ty::Array(_, elem)) => *elem,
                    _ => self.new_var(),
                };
                for (i, e) in exprs.iter().enumerate() {
                    self.check_expr_because(*e, &elem, reason_for_nth(reason, exprs[0], i));
                }
                self.holds(&elem, &mutability, expr);
                Ty::array(mutability, elem)
            }
            Expr::ArrayRepeat { value, count } => {
                let elem = self.infer_expr(*value);
                self.check_expr(*count, &Ty::Builtin(BuiltinType::Uint(BuiltinUint::Usize)));
                self.holds(&elem, &mutability, expr);
                Ty::array(mutability, elem)
            }
            Expr::Record { path, fields, spread } => {
                self.infer_record(expr, path.as_ref(), fields, *spread, is_mut)
            }
            Expr::Call { callee, args } => {
                self.infer_call(expr, *callee, CallArgs::Positional(args), is_mut)
            }
            // A tuple with named fields or spreads, which the solver makes
            // once it knows what's spread.
            Expr::NamedTuple { items } => {
                let hint = expected.map(|e| self.solver.unifier.shallow_resolve(e.ty));
                let (hint_pos, hint_named) = match hint {
                    Some(Ty::Record(tys, named, _)) => (tys.into_vec(), named.into_vec()),
                    _ => (Vec::new(), Vec::new()),
                };
                // After a spread, a position isn't known.
                let mut position = Some(0);
                let mut parts = Vec::new();
                for item in items.iter() {
                    match item {
                        TupleItem::Positional(e) => {
                            let hint = position.and_then(|i| hint_pos.get(i));
                            position = position.map(|i| i + 1);
                            let ty = self
                                .infer_expr_expect(*e, hint.map(|ty| Expectation { ty, reason }));
                            self.holds(&ty, &mutability, expr);
                            parts.push(TuplePart::Positional(ty));
                        }
                        TupleItem::Named(name, e) => {
                            let hint = hint_named.iter().find(|(n, _)| n == name).map(|(_, t)| t);
                            let ty = self
                                .infer_expr_expect(*e, hint.map(|ty| Expectation { ty, reason }));
                            self.holds(&ty, &mutability, expr);
                            parts.push(TuplePart::Named(name.clone(), ty));
                        }
                        TupleItem::Spread(spread, e) => {
                            position = None;
                            parts.push(TuplePart::Spread(*spread, self.infer_expr(*e)));
                        }
                    }
                }
                let result = self.new_var();
                let constraint =
                    SpreadConstraint { parts, mutability, result: result.clone(), value: true };
                self.solver.add(Constraint::Spread { constraint, origin: self.origin(expr) });
                result
            }
            Expr::Tuple { exprs } => {
                let hints: Option<Vec<Ty<'db>>> =
                    match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                        Some(Ty::Tuple(tys, _)) if tys.len() == exprs.len() => Some(tys.into_vec()),
                        _ => None,
                    };
                let tys: Box<[Ty<'db>]> = exprs
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        self.infer_expr_expect(
                            *e,
                            hints.as_ref().map(|h| Expectation { ty: &h[i], reason }),
                        )
                    })
                    .collect();
                for ty in tys.iter() {
                    self.holds(ty, &mutability, expr);
                }
                Ty::tuple(tys, mutability)
            }
            _ => unreachable!("only arrays, tuples, struct literals and calls are literals"),
        }
    }

    /// Whether `callee` names a struct, which a call of makes a tuple
    /// struct, like the `Wrapper` of `Wrapper(1)` or `m.Wrapper(1)`.
    fn constructs_struct(&self, callee: ExprId) -> bool {
        let path = match &self.body[callee] {
            Expr::Path(path) => path.clone(),
            Expr::Field { expr: base, name } => match self.path_extended(*base, name) {
                Some(path) => path,
                None => return false,
            },
            _ => return false,
        };
        matches!(
            self.resolver(callee).resolve_path_in_value_ns(self.db, &path),
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), unresolved: None }))
        )
    }

    fn infer_call(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        args: CallArgs<'_>,
        is_mut: bool,
    ) -> Ty<'db> {
        // `x.f(y)`: a field holding a function, a longer path, or `f(x, y)`.
        if let Expr::Field { expr: receiver, name } = &self.body[callee] {
            let (receiver, name) = (*receiver, name.clone());
            if self.path_extended(receiver, &name).is_none() {
                let receiver_ty = self.infer_expr(receiver);
                let is_field = match self.solver.unifier.shallow_resolve(&receiver_ty) {
                    // A private field isn't one here, so `x.f(y)` is `f(x, y)`.
                    Ty::Adt(AdtId::Struct(s), ..) => {
                        let def = VariantDef::Struct(s);
                        let file = self.file();
                        field_types(self.db, def)
                            .field(&name)
                            .is_some_and(|(i, _)| field_visible(self.db, def, i, file))
                    }
                    _ => false,
                };
                if !is_field {
                    if is_mut {
                        self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
                    }
                    return self.infer_method_call(
                        expr,
                        callee,
                        receiver,
                        &receiver_ty,
                        &name,
                        args,
                    );
                }
            }
        }
        // A tuple struct's constructor takes `mut_constructor`.
        self.mut_constructor = is_mut;
        let callee_ty = self.infer_expr(callee);
        if std::mem::take(&mut self.mut_constructor) {
            self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
        }
        let function = match &self.body[callee] {
            Expr::Path(path) => match self.resolver(callee).resolve_path_in_value_ns(self.db, path)
            {
                Some(ValueResolution::Value(ValueNs::Function(f))) => Some(f),
                _ => None,
            },
            _ => None,
        };
        let ty = self.apply(expr, callee, &callee_ty, function, args, None);
        self.solve_overloads();
        ty
    }

    /// `x.f(y)` as `f(x, y)`, with the `f` in scope. Functions for a type are
    /// found like any other: by importing them.
    fn infer_method_call(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        receiver: ExprId,
        receiver_ty: &Ty<'db>,
        name: &Name,
        args: CallArgs<'_>,
    ) -> Ty<'db> {
        let path = Path::from_name(name.clone());
        let mut function = None;
        let callee_ty = match self.resolver(expr).resolve_path_in_value_ns(self.db, &path) {
            Some(ValueResolution::Value(ValueNs::Function(f))) => {
                function = Some(f);
                self.result.method_calls.insert(expr, f);
                self.function_ty(callee, f)
            }
            Some(ValueResolution::Value(ValueNs::Overloaded(set))) => {
                self.overloaded_ty(callee, set, Some(expr))
            }
            // A local holding a function works too.
            Some(ValueResolution::Value(value)) => self.value_ty(callee, value),
            _ => {
                if !matches!(self.solver.unifier.shallow_resolve(receiver_ty), Ty::Error) {
                    self.unresolved(ExprOrPat::Expr(callee));
                }
                Ty::Error
            }
        };
        self.result.expr_types.insert(callee, callee_ty.clone());
        let ty = self.apply(
            expr,
            callee,
            &callee_ty,
            function,
            args,
            Some((receiver, receiver_ty, false)),
        );
        self.solve_overloads();
        ty
    }

    /// Call a value of type `callee_ty` with `args`, after `receiver` if
    /// there's one. `function` is the function it is, if it's known.
    fn apply(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        callee_ty: &Ty<'db>,
        function: Option<FunctionId<'db>>,
        args: CallArgs<'_>,
        // The receiver of `x.f(y)` or the left operand of `a + b`, and
        // whether the other arguments must have its type, as for `+`.
        receiver: Option<(ExprId, &Ty<'db>, bool)>,
    ) -> Ty<'db> {
        // Named arguments, spreads, or a function with more than positional
        // parameters: the arguments are checked as a tuple.
        // A function with defaults too, which the call may leave out.
        let resolved = self.solver.unifier.shallow_resolve(callee_ty);
        let takes_more = matches!(&resolved, Ty::Fn(params, ..) if !params.is_positional())
            || function.is_some_and(|f| !ParamDefaults::of(self.db, f).is_empty());
        let args = match args {
            CallArgs::Positional(args) if !takes_more => args,
            CallArgs::Items(items) => {
                return self.apply_general(expr, callee, callee_ty, function, items, receiver);
            }
            CallArgs::Positional(args) => {
                let items: Vec<_> = args.iter().map(|&e| TupleItem::Positional(e)).collect();
                return self.apply_general(expr, callee, callee_ty, function, &items, receiver);
            }
        };
        let count = args.len() + receiver.is_some() as usize;
        let (params, ret, effects) = match resolved {
            Ty::Fn(params, ret, _, effects) => (params.positional, *ret, *effects),
            // A number literal's type isn't known yet, but it's no function.
            Ty::Var(v) if self.solver.unifier.var_kind(v) == Some(VarKind::General) => {
                let params: Box<[Ty<'db>]> = (0..count).map(|_| self.new_var()).collect();
                let ret = self.new_var();
                // Calling a function doesn't need it to be `mut` or `imm`.
                let mutability = self.new_mutability();
                let effects = self.solver.new_var(VarKind::Effect);
                let fn_ty = Ty::Fn(
                    Box::new(Params::positional(params.clone())),
                    Box::new(ret.clone()),
                    Box::new(mutability),
                    Box::new(effects.clone()),
                );
                self.solver.equal(&fn_ty, callee_ty, self.origin(callee));
                // A use of a name with several functions is checked against
                // the arguments where they are, once one is chosen (#275).
                let at = receiver.map(|(r, _, _)| r).into_iter().chain(args.iter().copied());
                let at = at.map(|e| self.origin(e)).collect();
                self.solver.attach_overload_args(callee_ty, at);
                (params, ret, effects)
            }
            Ty::Error | Ty::Never => {
                for &arg in args {
                    self.infer_expr(arg);
                }
                return Ty::Error;
            }
            other => {
                self.result
                    .diagnostics
                    .push(InferenceDiagnostic::NotAFunction { expr: callee, ty: other });
                for &arg in args {
                    self.infer_expr(arg);
                }
                return Ty::Error;
            }
        };
        // The call does what the function does.
        let constraint = RowConstraint::contains(effects, self.effects.clone());
        self.solver.add(Constraint::Row { constraint, origin: self.origin(expr) });
        if params.len() != count {
            self.result.diagnostics.push(InferenceDiagnostic::WrongArgCount {
                expr: ExprOrPat::Expr(expr),
                expected: params.len(),
                found: count,
            });
        }
        let reason = |index: usize| match (function, receiver) {
            // `b` of `a + b` must be what `a` is.
            (_, Some((receiver, _, true))) if index > 0 => Reason::SameAs(receiver),
            (Some(function), _) => Reason::Argument { function, index: index as u32 },
            (None, _) => Reason::None,
        };
        let mut params = params.iter();
        if let Some((receiver, receiver_ty, _)) = receiver
            && let Some(param) = params.next()
        {
            self.solver.equal(
                param,
                receiver_ty,
                Origin { reason: reason(0), ..self.origin(receiver) },
            );
        }
        let first = receiver.is_some() as usize;
        for (i, &arg) in args.iter().enumerate() {
            // The arguments before may leave one of a name's functions, whose
            // parameter types the rest are then checked against, so that a
            // closure knows the types of its parameters.
            self.solve_overloads();
            match params.next() {
                Some(param) => self.check_expr_because(arg, param, reason(first + i)),
                None => {
                    self.infer_expr(arg);
                }
            }
        }
        // The arguments may decide associated types in the result, like the
        // `imm [i32]` of `to_imm(mut [1])`, which what follows should know.
        self.solver.solve_pending_assoc();
        ret
    }

    /// A call with named arguments or spreads, or of a function with
    /// keyword-only parameters or packs: the arguments, as a tuple, must be
    /// what the parameters are, as a tuple, like `(i32, *Ts, sep: String)`:
    /// by position, by name, and with the packs taking the rest.
    fn apply_general(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        callee_ty: &Ty<'db>,
        function: Option<FunctionId<'db>>,
        items: &[TupleItem<ExprId>],
        receiver: Option<(ExprId, &Ty<'db>, bool)>,
    ) -> Ty<'db> {
        let infer_all = |this: &mut Self| {
            for item in items {
                this.infer_expr(item.value());
            }
        };
        let (params, ret, effects) = match self.solver.unifier.shallow_resolve(callee_ty) {
            Ty::Fn(params, ret, _, effects) => (*params, *ret, *effects),
            // What isn't known yet takes the arguments as they're given. Its
            // positional ones are unknown with a spread.
            Ty::Var(v) if self.solver.unifier.var_kind(v) == Some(VarKind::General) => {
                if items.iter().any(|i| matches!(i, TupleItem::Spread(..))) {
                    infer_all(self);
                    let origin = self.origin(expr);
                    self.solver.errors.push(TypeError::TypeNeeded { origin });
                    return Ty::Error;
                }
                let positional = items.iter().filter(|i| matches!(i, TupleItem::Positional(_)));
                let count = positional.count() + receiver.is_some() as usize;
                let positional = (0..count).map(|_| self.new_var()).collect();
                let names = (0..count).map(|_| Ty::ParamName(None)).collect();
                let keyword = items
                    .iter()
                    .filter_map(|i| match i {
                        TupleItem::Named(n, _) => Some((n.clone(), self.new_var())),
                        _ => None,
                    })
                    .collect();
                let params = Params::new(positional, names, None, keyword, None);
                let ret = self.new_var();
                let effects = self.solver.new_var(VarKind::Effect);
                let fn_ty = Ty::Fn(
                    Box::new(params.clone()),
                    Box::new(ret.clone()),
                    Box::new(self.new_mutability()),
                    Box::new(effects.clone()),
                );
                self.solver.equal(&fn_ty, callee_ty, self.origin(callee));
                (params, ret, effects)
            }
            Ty::Error | Ty::Never => {
                infer_all(self);
                return Ty::Error;
            }
            other => {
                self.result
                    .diagnostics
                    .push(InferenceDiagnostic::NotAFunction { expr: callee, ty: other });
                infer_all(self);
                return Ty::Error;
            }
        };
        // The call does what the function does.
        let constraint = RowConstraint::contains(effects, self.effects.clone());
        self.solver.add(Constraint::Row { constraint, origin: self.origin(expr) });
        // The arguments, as a tuple: a positional one is checked against its
        // parameter while its place is known, before a spread. A named one
        // for a positional parameter comes after the positional ones, in
        // the order of the parameters.
        let mut parts = Vec::new();
        let mut position: Option<usize> = Some(0);
        if let Some((_, receiver_ty, _)) = receiver {
            parts.push(TuplePart::Positional(receiver_ty.clone()));
            position = Some(1);
        }
        let names = self.solver.unifier.names_now(&params.names);
        let first = receiver.is_some() as usize;
        // The defaults of the function, if it's known: those of the
        // parameters the call leaves out.
        let defaults = function.map(|f| ParamDefaults::of(self.db, f)).unwrap_or_default();
        let ArgPlaces { places, defaulted } = place_args(
            params.positional.len(),
            |i| names[i].clone(),
            |i| defaults.has_positional(i),
            first,
            items,
        );
        let given =
            |name: &Name| items.iter().any(|i| matches!(i, TupleItem::Named(n, _) if n == name));
        let no_spread = !items.iter().any(|i| matches!(i, TupleItem::Spread(..)));
        let keyword_defaulted: Vec<&(Name, u32)> =
            defaults.keyword.iter().filter(|(n, _)| no_spread && !given(n)).collect();
        // An argument given twice is reported on its own, and so is one by a
        // name that no parameter has.
        let twice = places.contains(&ArgPlace::Twice);
        let unknown = |name: &Name| {
            params.keyword_rest.is_none() && !params.keyword.iter().any(|(n, _)| n == name)
        };
        // The positional parameters that no argument is for, by position or
        // by name, and that have no name.
        let given = first + items.iter().filter(|i| matches!(i, TupleItem::Positional(_))).count();
        let unnamed = (given..params.positional.len()).any(|i| {
            names[i].is_none()
                && !defaulted.contains(&i)
                && !places.contains(&ArgPlace::Positional(i))
        });
        let mut unknowns = false;
        let mut placed = Vec::new();
        for (item, &place) in items.iter().zip(&places) {
            match (item, place) {
                (TupleItem::Named(name, e), ArgPlace::Positional(i)) => {
                    let hint = Expectation { ty: &params.positional[i], reason: Reason::None };
                    let ty = self.infer_expr_expect(*e, Some(hint));
                    self.solver.unifier.require_name(&params.names[i], name);
                    placed.push((i, ty));
                    continue;
                }
                (TupleItem::Named(name, e), ArgPlace::Twice) => {
                    self.infer_expr(*e);
                    let origin = self.origin(*e);
                    let error = TypeError::DuplicateArgument { origin, name: name.clone() };
                    self.solver.errors.push(error);
                    continue;
                }
                (TupleItem::Named(name, e), ArgPlace::Written) if unknown(name) => {
                    self.infer_expr(*e);
                    let origin = self.origin(*e);
                    let error = TypeError::UnknownArgument { origin, name: name.clone(), unnamed };
                    self.solver.errors.push(error);
                    unknowns = true;
                    continue;
                }
                _ => {}
            }
            match item {
                TupleItem::Positional(e) => {
                    let hint = position.and_then(|i| params.positional.get(i));
                    position = position.map(|i| i + 1);
                    let hint = hint.map(|ty| Expectation { ty, reason: Reason::None });
                    let ty = self.infer_expr_expect(*e, hint);
                    parts.push(TuplePart::Positional(ty));
                }
                TupleItem::Named(name, e) => {
                    let hint = params.keyword.iter().find(|(n, _)| n == name).map(|(_, t)| t);
                    let hint = hint.map(|ty| Expectation { ty, reason: Reason::None });
                    let ty = self.infer_expr_expect(*e, hint);
                    parts.push(TuplePart::Named(name.clone(), ty));
                }
                TupleItem::Spread(spread, e) => {
                    position = None;
                    parts.push(TuplePart::Spread(*spread, self.infer_expr(*e)));
                }
            }
        }
        if twice || unknowns {
            return ret;
        }
        placed.sort_by_key(|(i, _)| *i);
        parts.extend(placed.into_iter().map(|(_, ty)| TuplePart::Positional(ty)));
        // The arguments must be the parameters that the call doesn't leave
        // out, and the others get their defaults.
        let params = if defaulted.is_empty() && keyword_defaulted.is_empty() {
            params
        } else {
            let mut used: Vec<u32> = defaulted
                .iter()
                .filter_map(|&i| defaults.positional[i])
                .chain(keyword_defaulted.iter().map(|(_, i)| *i))
                .collect();
            used.sort();
            if let Some(f) = function {
                self.result.defaults.insert(expr, (f, used.into()));
            }
            let kept = |i: &usize| !defaulted.contains(i);
            let (positional, names) = (0..params.positional.len())
                .filter(kept)
                .map(|i| (params.positional[i].clone(), params.names[i].clone()))
                .unzip();
            let keyword = params
                .keyword
                .iter()
                .filter(|(n, _)| !keyword_defaulted.iter().any(|(m, _)| m == n))
                .cloned()
                .collect();
            let rest = params.rest.map(|r| *r);
            let keyword_rest = params.keyword_rest.map(|r| *r);
            Params::new(positional, names, rest, keyword, keyword_rest)
        };
        let args = self.new_var();
        let constraint = SpreadConstraint {
            parts,
            mutability: Ty::Unwritten,
            result: args.clone(),
            value: false,
        };
        self.solver.add(Constraint::Spread { constraint, origin: self.origin(expr) });
        self.solver.equal(&params.as_tuple(), &args, self.origin(expr));
        self.solver.solve_pending_assoc();
        ret
    }

    fn infer_binary(&mut self, expr: ExprId, op: &BinaryOp, lhs: ExprId, rhs: ExprId) -> Ty<'db> {
        match op {
            // `a + b`, `a < b` and `a <$> b` call the functions `+`, `<` and
            // `<$>` in scope, like the prelude's `fn a + b = add(a, b)`, for
            // every type: only the operations that the prelude's impls use,
            // like `i32_add`, are built in.
            BinaryOp::Operator(name) => {
                let ty = self.infer_expr(lhs);
                self.infer_operator_call(expr, name, lhs, ty, &[rhs])
            }
            BinaryOp::Logic(_) => {
                self.check_expr(lhs, &Ty::bool());
                self.check_expr(rhs, &Ty::bool());
                Ty::bool()
            }
            BinaryOp::Assign(op) => {
                let ty = self.infer_expr(lhs);
                match op {
                    // `a += b` is `a = a + b`.
                    Some(op) => {
                        let result = self.infer_operator_call(expr, op, lhs, ty.clone(), &[rhs]);
                        self.solver.equal(
                            &ty,
                            &result,
                            Origin { reason: Reason::SameAs(lhs), ..self.origin(expr) },
                        );
                    }
                    _ => self.check_expr_because(rhs, &ty, Reason::SameAs(lhs)),
                }
                self.check_place(lhs, op.is_none());
                Ty::unit()
            }
        }
    }

    /// Whether the functions that `value` names, with `arity` parameters,
    /// all give an associated type of their first parameter's type, like the
    /// `T.Output` of the prelude's `fn a + b = add(a, b)`.
    fn gives_operand_assoc(&self, value: ValueNs<'db>, arity: usize) -> bool {
        let functions = match value {
            ValueNs::Function(f) => vec![f],
            ValueNs::Overloaded(set) => set.functions(self.db).clone(),
            _ => return false,
        };
        let mut found = false;
        for f in functions {
            if self.env.mono.contains_key(&f) {
                return false;
            }
            let scheme = match self.env.done.get(&f) {
                Some(scheme) => scheme,
                None => match function_scheme(self.db, f) {
                    Ok(scheme) => scheme,
                    Err(_) => return false,
                },
            };
            let Ty::Fn(params, ret, _, _) = &scheme.ty else { return false };
            if !params.is_positional() || params.positional.len() != arity {
                continue;
            }
            match &**ret {
                Ty::Assoc(_, args) if args.first() == params.positional.first() => found = true,
                _ => return false,
            }
        }
        found
    }

    /// `lhs <op> rhs`, or `<op> lhs` when `rest` is empty, as a call of the
    /// function named `name` in scope, with `lhs` of type `lhs_ty` already
    /// inferred.
    fn infer_operator_call(
        &mut self,
        expr: ExprId,
        name: &Name,
        lhs: ExprId,
        lhs_ty: Ty<'db>,
        rest: &[ExprId],
    ) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let mut function = None;
        let mut gives_operand_assoc = false;
        let callee_ty =
            match resolver.resolve_path_in_value_ns(self.db, &Path::from_name(name.clone())) {
                Some(ValueResolution::Value(value)) => {
                    if let ValueNs::Function(f) = value {
                        function = Some(f);
                    }
                    gives_operand_assoc = self.gives_operand_assoc(value, rest.len() + 1);
                    self.value_ty(expr, value)
                }
                _ => {
                    self.unresolved(ExprOrPat::Expr(expr));
                    Ty::Error
                }
            };
        // An operator whose operands have one type, like `fn(T, T)` for `+`,
        // says that the right one must be what the left one is.
        let same = match self.solver.unifier.shallow_resolve(&callee_ty) {
            Ty::Fn(params, ..) => {
                let ps = &params.positional;
                params.is_positional() && ps.len() == 2 && ps[0] == ps[1]
            }
            _ => false,
        };
        let args = CallArgs::Positional(rest);
        let ty = self.apply(expr, expr, &callee_ty, function, args, Some((lhs, &lhs_ty, same)));
        self.solve_overloads();
        // While `lhs` is a number literal whose type isn't known, like the `1`
        // of `let x: i64 = 1 << 40`, an operator that gives an associated type
        // of it, like `+`'s `T.Output`, gives its type, as every number type's
        // impl does: so that what the result must be decides the literal's
        // type, as in Rust.
        if gives_operand_assoc
            && let Ty::Var(v) = self.solver.unifier.shallow_resolve(&lhs_ty)
            && matches!(self.solver.unifier.var_kind(v), Some(VarKind::Int | VarKind::Float))
        {
            self.solver.equal(&lhs_ty, &ty, self.origin(expr));
            return lhs_ty;
        }
        ty
    }

    /// Report an assignment to `place` if it can't be assigned to. `plain`
    /// is whether it's `=` rather than a compound assignment like `+=`.
    ///
    /// Arrays, structs and tuples are shared, so assigning to an element or
    /// a field changes the object: it must be `mut`, and the variable it's
    /// in doesn't matter. Otherwise the place must be a variable that's
    /// `mut`.
    fn check_place(&mut self, place: ExprId, plain: bool) {
        let body = self.body;
        let mut root = place;
        loop {
            match &body[root] {
                Expr::Field { expr: base, .. } => {
                    let base = *base;
                    // A path like `Color.Red` has no type of its own.
                    let Some(base_ty) = self.result.expr_types.get(base) else {
                        root = base;
                        continue;
                    };
                    match self.solver.unifier.resolve(base_ty) {
                        // A struct or tuple is shared, like an array.
                        ty @ (Ty::Adt(AdtId::Struct(_), ..) | Ty::Tuple(..) | Ty::Record(..)) => {
                            let mutability = ty.mutability().unwrap().clone();
                            return self.solver.require_mut(&ty, &mutability, self.origin(base));
                        }
                        // Decided with the field constraint, once the type is
                        // known.
                        Ty::Var(_) => return self.solver.assign_field(self.origin(root)),
                        Ty::Error | Ty::Never => return,
                        // An enum has no fields to assign.
                        _ => root = base,
                    }
                }
                Expr::Index { base, .. } => return self.check_mut_index(root, *base),
                _ => break,
            }
        }
        let path = match &body[root] {
            Expr::Path(path) => path,
            // Already reported, or unknown until macros are expanded.
            Expr::Missing | Expr::MacroCall { .. } => return,
            _ => {
                self.result.diagnostics.push(InferenceDiagnostic::NotAPlace { expr: place });
                return;
            }
        };
        let binding = match self.resolver(root).resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(ValueNs::Local(binding))) => binding,
            // An unresolved name is already reported.
            None => return,
            // A function, constant, variant or type.
            Some(_) => {
                self.result.diagnostics.push(InferenceDiagnostic::NotAPlace { expr: place });
                return;
            }
        };
        if !body[binding].mutable {
            // The first `x = ...` after `let x;` initializes `x`.
            if plain
                && root == place
                && let Some(i) = self.uninitialized.iter().position(|&b| b == binding)
            {
                self.uninitialized.swap_remove(i);
                return;
            }
            self.result
                .diagnostics
                .push(InferenceDiagnostic::AssignToImmutable { expr: place, binding });
        }
    }

    /// `expr`, an element of which is assigned to at `place`, needs an impl
    /// of `IndexMut`. An array or struct that isn't `mut` is reported as such
    /// instead, and one whose mutability isn't known yet becomes `mut`.
    fn check_mut_index(&mut self, place: ExprId, expr: ExprId) {
        // A type that an associated type decides, like that of
        // `let ys = to_imm(xs)`, may be known once what's pending is solved.
        if let Ty::Var(_) = self.solver.unifier.shallow_resolve(self.result.expr_ty(expr)) {
            self.solver.solve();
        }
        let ty = self.solver.unifier.resolve(self.result.expr_ty(expr));
        if matches!(ty, Ty::Error | Ty::Never) {
            return;
        }
        // An array or struct that isn't `mut` can't be changed, which says
        // more than that there's no impl for it; and one whose mutability
        // isn't known yet becomes `mut`.
        if let Some(mutability) = ty.mutability() {
            let changeable = matches!(
                self.solver.unifier.shallow_resolve(mutability),
                Ty::Mutability(Mutability::Mut) | Ty::Var(_) | Ty::Error | Ty::Never
            );
            self.solver.require_mut(&ty, mutability, self.origin(expr));
            if !changeable {
                return;
            }
        }
        let file = self.file();
        // Without `Index`, reading the element already reported it.
        let Some(class) = LangClass::IndexMut.class(self.db, file) else { return };
        let constraint = ClassConstraint { class, args: Box::new([ty]) };
        self.solver.add(Constraint::Class { constraint, origin: self.origin(place) });
    }

    /// The associated type of `lang`, a class of the prelude that syntax
    /// uses, for `arg`, which must have an instance of it at `at`. Without a
    /// prelude that defines the class, that's reported, and it's `None`.
    fn lang_assoc(&mut self, lang: LangClass, arg: &Ty<'db>, at: ExprId) -> Option<Ty<'db>> {
        let file = self.file();
        let Some((class, alias)) =
            lang.class(self.db, file).and_then(|c| Some((c, lang.assoc(self.db, c)?)))
        else {
            self.result
                .diagnostics
                .push(InferenceDiagnostic::NoLangClass { expr: at, class: lang.name() });
            return None;
        };
        let origin = self.origin(at);
        let constraint = ClassConstraint { class, args: Box::new([arg.clone()]) };
        self.solver.add(Constraint::Class { constraint, origin });
        Some(self.solver.flatten_assoc(&Ty::Assoc(alias, Box::new([arg.clone()])), origin))
    }

    fn infer_record(
        &mut self,
        expr: ExprId,
        path: Option<&Path>,
        fields: &[RecordField],
        spread: Option<ExprId>,
        is_mut: bool,
    ) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let def = match path.and_then(|p| resolver.resolve_path_in_type_ns(self.db, p)) {
            Some(TypeResolution { ty: TypeNs::Struct(s), unresolved: None }) => {
                Some(VariantDef::Struct(s))
            }
            Some(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }) => {
                Some(VariantDef::Variant(v))
            }
            _ => None,
        };
        let Some(def) = def else {
            match path {
                Some(path) => self.unresolved_path(ExprOrPat::Expr(expr), &resolver, path),
                None => self.unresolved(ExprOrPat::Expr(expr)),
            }
            for field in fields {
                self.infer_expr(field.expr);
            }
            if let Some(spread) = spread {
                self.infer_expr(spread);
            }
            return Ty::Error;
        };
        let (_, generic_def) = def.adt();
        // A variant can't be `mut`: an enum's values can't be changed; nor
        // can those of an `imm struct`.
        if is_mut && is_always_imm(self.db, def) {
            self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
        }
        let (ty, args) = self.fresh_adt(def, literal_mutability(is_mut));
        let field_tys = field_types(self.db, def);
        let mut seen = Vec::new();
        for field in fields {
            match field_tys.field(&field.name) {
                Some((index, field_ty)) => {
                    seen.push(index);
                    self.check_field_visible(ExprOrPat::Expr(field.expr), def, index);
                    let field_ty = subst_params(field_ty, generic_def, &args);
                    let reason = Reason::Field { variant: def, index: index as u32 };
                    self.check_expr_because(field.expr, &field_ty, reason);
                }
                None => {
                    self.infer_expr(field.expr);
                    self.result.diagnostics.push(InferenceDiagnostic::NoSuchField {
                        expr: ExprOrPat::Expr(field.expr),
                        name: field.name.clone(),
                    });
                }
            }
        }
        match spread {
            // The fields not given are copied from `base`. The copy is
            // shallow: an array, struct or function in a field is shared,
            // not copied. So `base` can have another mutability only if no
            // such field has the struct's own, or `Line { ..l }` with a
            // `mut` `l` would be an `imm Line` holding `l`'s `mut Point`.
            Some(spread) => {
                // The fields not given are copied, so they must be visible.
                let file = self.file();
                let hidden = (0..field_tys.fields.len())
                    .find(|&i| !seen.contains(&i) && !field_visible(self.db, def, i, file));
                if let Some(hidden) = hidden {
                    self.check_field_visible(ExprOrPat::Expr(spread), def, hidden);
                }
                let base = match (&ty, def) {
                    // The fields not given that have a mutability the struct
                    // declares are shared too, so `base` has the same ones.
                    (
                        Ty::Adt(id @ AdtId::Struct(_), args, mutabilities, _),
                        VariantDef::Struct(s),
                    ) => {
                        let own = struct_mutability(self.db, s);
                        let shares = field_tys
                            .fields
                            .iter()
                            .enumerate()
                            .any(|(i, (_, t))| !seen.contains(&i) && t.any(&mut |t| *t == own));
                        if shares {
                            ty.clone()
                        } else {
                            Ty::adt(*id, args.clone(), mutabilities.clone(), self.new_mutability())
                        }
                    }
                    (ty, _) => ty.clone(),
                };
                self.check_expr(spread, &base)
            }
            None => {
                let missing: Vec<Name> = field_tys
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !seen.contains(i))
                    .map(|(_, (name, _))| name.clone())
                    .collect();
                if !missing.is_empty() {
                    self.result
                        .diagnostics
                        .push(InferenceDiagnostic::MissingFields { expr, names: missing });
                }
            }
        }
        // Once the fields are known, so that a mutability argument is that
        // of what's given, like the `'v` of `Iter { data: xs }` for `xs: 'v
        // [i32]`, and isn't decided by them first.
        self.struct_holds(&ty, expr);
        self.require_bounds(def, &args, expr);
        ty
    }

    /// Resolve all types once solving is done.
    pub fn finish(mut self) -> InferenceResult<'db> {
        let unifier = &self.solver.unifier;
        self.result.map_types(&mut |t| unifier.resolve(t));
        self.result
    }

    pub fn take_result(&mut self) -> InferenceResult<'db> {
        std::mem::take(&mut self.result)
    }
}

/// The type of the function `f`, instantiated for a use at `origin`: its
/// type while its group is inferred, or its scheme's.
pub(crate) fn instantiate_function<'db>(
    db: &'db dyn Db,
    solver: &mut Solver<'db>,
    env: &FnEnv<'db>,
    f: FunctionId<'db>,
    origin: Origin<'db>,
) -> Result<Ty<'db>, InferenceCycle> {
    if let Some(ty) = env.mono.get(&f) {
        return Ok(ty.clone());
    }
    if let Some(scheme) = env.done.get(&f) {
        return Ok(scheme.instantiate(solver, origin));
    }
    Ok(function_scheme(db, f)?.instantiate(solver, origin))
}

/// Choose the functions of the uses of names with several that the types
/// decide, adding what each needs, until no more are decided (#275).
pub(crate) fn solve_overloads<'db>(db: &'db dyn Db, solver: &mut Solver<'db>, env: &FnEnv<'db>) {
    loop {
        solver.solve();
        let ready = solver.narrow_overloads();
        if ready.is_empty() {
            return;
        }
        for (constraint, origin, f) in ready {
            let ty = instantiate_function(db, solver, env, f, origin).unwrap_or(Ty::Error);
            // A call is checked argument by argument, so that a mismatch is
            // at the argument, as for a function without overloads.
            let call = solver.unifier.shallow_resolve(&constraint.ty);
            match (&call, &ty) {
                (Ty::Fn(args, ret, m, e), Ty::Fn(params, own_ret, own_m, own_e))
                    if args.is_positional()
                        && params.is_positional()
                        && args.positional.len() == params.positional.len()
                        && constraint.args.len() == args.positional.len() =>
                {
                    let pairs = args.positional.iter().zip(params.positional.iter());
                    for (index, ((arg, param), at)) in pairs.zip(&constraint.args).enumerate() {
                        let reason = Reason::Argument { function: f, index: index as u32 };
                        solver.equal(param, arg, Origin { reason, ..*at });
                    }
                    solver.equal(ret, own_ret, origin);
                    solver.equal(m, own_m, origin);
                    solver.equal(e, own_e, origin);
                }
                _ => solver.equal(&constraint.ty, &ty, origin),
            }
            solver.overload_choices.push((origin, constraint.method_call, f));
        }
    }
}
