//! The types the checker works with. Unlike the type references in
//! hir-def, they point at definitions instead of naming them.

use std::fmt;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    ids::{EffectId, EnumId, ItemContainerId, StructId, TypeAliasId},
    resolver::GenericDefId,
};

pub use boltlang_hir_def::hir::Mutability;

/// An inference variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct TyVar(pub u32);

/// What an inference variable can become. An integer literal without a
/// suffix has an `Int` variable, which defaults to `i32`. A `Mutability`
/// variable stands for the mutability of an array, and defaults to `Imm`.
/// An `Effect` variable stands for a row of effects, and defaults to the
/// empty row: a function that nothing says does something does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum VarKind {
    General,
    Int,
    Float,
    Mutability,
    Effect,
}

/// An effect in a row, with its arguments, like `State[i32]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct EffectTy<'db> {
    pub effect: EffectId<'db>,
    pub args: Box<[Ty<'db>]>,
}

impl<'db> EffectTy<'db> {
    /// The order of effects in a row. It only has to be the same for the same
    /// effects, so it's the order the effects were interned in.
    fn key(&self) -> salsa::Id {
        salsa::plumbing::AsId::as_id(&self.effect)
    }
}

/// Whether a number literal is an integer or a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum NumberKind {
    Int,
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum AdtId<'db> {
    Struct(StructId<'db>),
    Enum(EnumId<'db>),
}

/// The `index`th type parameter of `def`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct GenericParamId<'db> {
    pub def: GenericDefId<'db>,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Ty<'db> {
    /// A type still being inferred.
    Var(TyVar),
    /// A struct or enum with its type arguments, its mutability arguments,
    /// and its mutability. It can have fewer type arguments than parameters:
    /// `Adt(Option, [], [], Imm)` is the type constructor `Option`, which is
    /// of kind `Type -> Type`. The mutabilities aren't type arguments, so
    /// they don't change the kind.
    ///
    /// Only a struct has a mutability, like an array: an enum's is always
    /// `Imm`, since its values can't be changed in place. Only a struct has
    /// mutability arguments too, one for each it declares, like the `'v` of
    /// `struct VecIter[T, 'v] { data: 'v Vec[T] }` (#243): `VecIter[i32,
    /// imm]` has `[imm]`.
    Adt(AdtId<'db>, Box<[Ty<'db>]>, Box<[Ty<'db>]>, Box<Ty<'db>>),
    /// A type parameter or variable applied to arguments, like `F[A]`. The
    /// head is never a struct or enum, nor another `App`: [`Ty::app`] makes
    /// those an `Adt` or one `App`, so that each type has one form.
    App(Box<Ty<'db>>, Box<[Ty<'db>]>),
    Builtin(BuiltinType),
    /// `(A, B)`; the unit type is the empty tuple.
    Tuple(Box<[Ty<'db>]>),
    /// An array: its mutability, then the type of its elements. The
    /// mutability is [`Ty::Mutability`], a variable, a parameter, or the
    /// error type. Lengths aren't checked yet.
    Array(Box<Ty<'db>>, Box<Ty<'db>>),
    /// `Mut` or `Imm`, the mutability of an array, which is a type of its
    /// own kind: see `site/docs/internals/type-checking/mutability/`.
    Mutability(Mutability),
    /// The mutability of an array type written without `mut` or `imm`, as
    /// lowered on its own. What it means depends on where the type is
    /// written, so the item that has the type replaces it with
    /// [`Ty::fill_unwritten`]: a parameter of a function, a variable in a
    /// body, or `Imm`. Only type aliases keep it, for the types they're
    /// used in to replace.
    Unwritten,
    /// `fn(A, B) -> C does E`: the parameters, the result, the mutability
    /// and the effects. A closure is `mut` if it can change what it
    /// captured, or capture something that can change, and `imm` otherwise;
    /// named functions capture nothing and are `imm`. As for arrays, an
    /// `imm` value can't hold a `mut fn`. The mutability doesn't restrict
    /// calling it. The effects are a row: [`Ty::Row`], a variable or a
    /// parameter.
    Fn(Box<[Ty<'db>]>, Box<Ty<'db>>, Box<Ty<'db>>, Box<Ty<'db>>),
    /// A row of effects, which is a type of kind `Effect`: the effects that
    /// are known, like `Log + State[i32]`, and the rows it's made of that
    /// stand for any row, like the `E` of `Log + E`, which are parameters or
    /// variables bound by a scheme. Each effect is in it once, and both
    /// lists are in the order of [`Ty::row_of`]; the empty row is `Nothing`.
    /// A row made of rows still being inferred is a variable, and how it's
    /// made is a constraint (see `site/docs/internals/effects.mdx`).
    Row(Box<[EffectTy<'db>]>, Box<[Ty<'db>]>),
    /// An associated type of a class applied to types, like `C.Item` for
    /// `type Item;` of `class Index[C]`: the type alias declared in the
    /// class, and the class's arguments. Which type it is depends on the
    /// impl for the arguments. In a body, the checker replaces it with a
    /// variable and a constraint that the solver decides once an impl is
    /// found (see [`Constraint::Assoc`](crate::solve::Constraint::Assoc)).
    /// When the arguments are type parameters with a `where` clause, like
    /// `C` in `fn first[C](c: C) -> C.Item where Index[C]`, it stays as it
    /// is, and only equals itself.
    Assoc(TypeAliasId<'db>, Box<[Ty<'db>]>),
    /// `imm T`: the type of a copy of a `T` that can't be changed, all the
    /// way down, which `imm e` makes. It's only for a type that `imm` can't
    /// be taken apart yet, like a type parameter: the checker gives `imm`
    /// of a known type as that type, with its arrays and structs `imm`, so
    /// that `imm (mut [mut [i32]])` is `imm [imm [i32]]`. Like an
    /// associated type, it only equals itself, since two types can have the
    /// same `imm`. In a body, the checker replaces it with a variable and a
    /// constraint, which gives `T` itself when `T` is `imm`.
    Imm(Box<Ty<'db>>),
    /// A type parameter as seen inside the item that declares it, where it
    /// stands for any type and only equals itself.
    Param(GenericParamId<'db>),
    /// A type variable bound by an inferred [`Scheme`](crate::Scheme).
    Bound(u32),
    /// The type of a number literal whose type nothing decided, which was
    /// given the default `i32` or `f64`. Only in type errors, where it reads
    /// as `{integer}`: `let a: f64 = 1;` didn't find an `i32`.
    Number(NumberKind),
    /// `!`, the type of `return` and other expressions that don't finish.
    /// It fits wherever any type is expected.
    Never,
    /// A type that couldn't be found. It fits everywhere, so that one
    /// error doesn't cause others.
    Error,
}

impl<'db> Ty<'db> {
    pub fn unit() -> Self {
        Ty::Tuple(Box::new([]))
    }

    pub fn bool() -> Self {
        Ty::Builtin(BuiltinType::Bool)
    }

    /// An `imm` function from `params` to `ret` without effects.
    pub fn func(params: impl Into<Box<[Ty<'db>]>>, ret: Ty<'db>) -> Self {
        Ty::func_does(params, ret, Ty::nothing())
    }

    /// An `imm` function from `params` to `ret` with the effects `effects`,
    /// like a named function.
    pub fn func_does(params: impl Into<Box<[Ty<'db>]>>, ret: Ty<'db>, effects: Ty<'db>) -> Self {
        Ty::Fn(
            params.into(),
            Box::new(ret),
            Box::new(Ty::Mutability(Mutability::Imm)),
            Box::new(effects),
        )
    }

    /// `imm T` of a type that `imm` can't take apart: `imm (imm T)` is
    /// `imm T`.
    pub fn imm_of_unknown(ty: Ty<'db>) -> Self {
        match ty {
            Ty::Imm(_) => ty,
            ty => Ty::Imm(Box::new(ty)),
        }
    }

    /// The row without effects, `Nothing`.
    pub fn nothing() -> Self {
        Ty::Row(Box::new([]), Box::new([]))
    }

    /// The row of `effects`, which are all known. Each effect must be in it
    /// once: see [`Ty::union_rows`] for rows that can share effects.
    pub fn row(effects: impl IntoIterator<Item = EffectTy<'db>>) -> Self {
        Ty::row_of(effects, [])
    }

    /// The row of `effects` and the rows `rest`, in its one form: rows in
    /// `rest` that are known are taken apart, an effect already there keeps
    /// its arguments, both lists are sorted, and a row that's one row of
    /// `rest` and nothing else is that row.
    pub fn row_of(
        effects: impl IntoIterator<Item = EffectTy<'db>>,
        rest: impl IntoIterator<Item = Ty<'db>>,
    ) -> Self {
        let mut known: Vec<EffectTy<'db>> = Vec::new();
        let add = |e: EffectTy<'db>, known: &mut Vec<EffectTy<'db>>| {
            if !known.iter().any(|k| k.effect == e.effect) {
                known.push(e);
            }
        };
        for e in effects {
            add(e, &mut known);
        }
        let mut parts: Vec<Ty<'db>> = Vec::new();
        let mut todo: Vec<Ty<'db>> = rest.into_iter().collect();
        while let Some(part) = todo.pop() {
            match part {
                Ty::Row(es, rs) => {
                    for e in es.into_vec() {
                        add(e, &mut known);
                    }
                    todo.extend(rs.into_vec());
                }
                part if !parts.contains(&part) => parts.push(part),
                _ => {}
            }
        }
        known.sort_by_key(EffectTy::key);
        parts.sort_by_key(rest_key);
        if known.is_empty() && parts.len() == 1 {
            return parts.pop().unwrap();
        }
        Ty::Row(known.into(), parts.into())
    }

    /// The known effects of a row and the rows it's made of, for a row that's
    /// not a variable: a lone parameter is a row made of itself.
    pub fn row_parts(&self) -> Option<(&[EffectTy<'db>], Vec<Ty<'db>>)> {
        match self {
            Ty::Row(effects, rest) => Some((effects, rest.to_vec())),
            Ty::Param(_) | Ty::Bound(_) => Some((&[], vec![self.clone()])),
            _ => None,
        }
    }

    /// The effects of both lists, with the arguments of an effect in both
    /// from `a`. The pairs of arguments that must be the same, for the
    /// effects in both, are given to `same`.
    pub fn union_rows(
        a: &[EffectTy<'db>],
        b: &[EffectTy<'db>],
        mut same: impl FnMut(&EffectTy<'db>, &EffectTy<'db>),
    ) -> Vec<EffectTy<'db>> {
        let mut out: Vec<EffectTy<'db>> = a.to_vec();
        for e in b {
            match a.iter().find(|x| x.effect == e.effect) {
                Some(x) => same(x, e),
                None => out.push(e.clone()),
            }
        }
        out
    }

    /// An array of `element` with the mutability `mutability`.
    pub fn array(mutability: Ty<'db>, element: Ty<'db>) -> Self {
        Ty::Array(Box::new(mutability), Box::new(element))
    }

    /// A struct or enum applied to `args`. `mutabilities` and `mutability`
    /// are a struct's; an enum has none, and its mutability is `Imm`.
    pub fn adt(
        id: AdtId<'db>,
        args: impl Into<Box<[Ty<'db>]>>,
        mutabilities: impl Into<Box<[Ty<'db>]>>,
        mutability: Ty<'db>,
    ) -> Self {
        let (mutabilities, mutability) = match id {
            AdtId::Struct(_) => (mutabilities.into(), mutability),
            AdtId::Enum(_) => (Box::default(), Ty::Mutability(Mutability::Imm)),
        };
        Ty::Adt(id, args.into(), mutabilities, Box::new(mutability))
    }

    /// The mutability of an array or a struct.
    pub fn mutability(&self) -> Option<&Ty<'db>> {
        match self {
            Ty::Array(mutability, _) | Ty::Adt(AdtId::Struct(_), _, _, mutability) => {
                Some(mutability)
            }
            _ => None,
        }
    }

    /// The mutability of what a value of this type holds directly: that of
    /// an array, a struct, or a function, whose own is that of what it
    /// captured.
    pub fn held_mutability(&self) -> Option<&Ty<'db>> {
        match self {
            Ty::Fn(_, _, mutability, _) => Some(mutability),
            ty => ty.mutability(),
        }
    }

    /// Replace each [`Ty::Unwritten`] with what `fill` gives, in the order
    /// they're written.
    pub fn fill_unwritten(&self, fill: &mut impl FnMut() -> Ty<'db>) -> Ty<'db> {
        self.fold(&mut |t| matches!(t, Ty::Unwritten).then(&mut *fill))
    }

    /// Whether [`Ty::Unwritten`] is in this type.
    pub fn has_unwritten(&self) -> bool {
        self.any(&mut |t| matches!(t, Ty::Unwritten))
    }

    /// `head` applied to `args`, in its normal form: `Option` applied to
    /// `i32` is `Option[i32]`, and `F[A]` applied to `B` is `F[A, B]`.
    pub fn app(head: Ty<'db>, args: impl IntoIterator<Item = Ty<'db>>) -> Self {
        let mut args = args.into_iter().peekable();
        if args.peek().is_none() {
            return head;
        }
        match head {
            Ty::Adt(id, xs, ms, m) => {
                Ty::Adt(id, xs.into_vec().into_iter().chain(args).collect(), ms, m)
            }
            Ty::App(head, xs) => Ty::App(head, xs.into_vec().into_iter().chain(args).collect()),
            // An error applied to anything is still an error.
            Ty::Error => Ty::Error,
            head => Ty::App(Box::new(head), args.collect()),
        }
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Ty::Tuple(tys) if tys.is_empty())
    }

    /// Call `f` on every type directly inside this one.
    pub fn walk_children(&self, mut f: impl FnMut(&Ty<'db>)) {
        match self {
            Ty::Adt(_, args, mutabilities, mutability) => {
                args.iter().for_each(&mut f);
                mutabilities.iter().for_each(&mut f);
                f(mutability);
            }
            Ty::Tuple(args) => args.iter().for_each(f),
            Ty::App(head, args) => {
                f(head);
                args.iter().for_each(f);
            }
            Ty::Array(mutability, elem) => {
                f(mutability);
                f(elem);
            }
            Ty::Fn(params, ret, mutability, effects) => {
                params.iter().for_each(&mut f);
                f(ret);
                f(mutability);
                f(effects);
            }
            Ty::Row(effects, rest) => {
                effects.iter().flat_map(|e| e.args.iter()).for_each(&mut f);
                rest.iter().for_each(f);
            }
            Ty::Assoc(_, args) => args.iter().for_each(f),
            Ty::Imm(ty) => f(ty),
            Ty::Var(_)
            | Ty::Builtin(_)
            | Ty::Mutability(_)
            | Ty::Unwritten
            | Ty::Param(_)
            | Ty::Bound(_)
            | Ty::Number(_)
            | Ty::Never
            | Ty::Error => {}
        }
    }

    /// Rebuild this type, replacing each type inside it, including itself,
    /// by `f`. When `f` returns `None`, the type is rebuilt from its parts.
    pub fn fold(&self, f: &mut impl FnMut(&Ty<'db>) -> Option<Ty<'db>>) -> Ty<'db> {
        if let Some(ty) = f(self) {
            return ty;
        }
        let fold_all = |tys: &[Ty<'db>], f: &mut _| tys.iter().map(|t| t.fold(f)).collect();
        match self {
            Ty::Adt(id, args, mutabilities, mutability) => Ty::Adt(
                *id,
                fold_all(args, f),
                fold_all(mutabilities, f),
                Box::new(mutability.fold(f)),
            ),
            // The head can become a struct or enum, like `F` in `F[A]` when
            // `F` is replaced by `Option`.
            Ty::App(head, args) => Ty::app(head.fold(f), fold_all(args, f).into_vec()),
            Ty::Tuple(tys) => Ty::Tuple(fold_all(tys, f)),
            Ty::Array(mutability, elem) => Ty::array(mutability.fold(f), elem.fold(f)),
            Ty::Fn(params, ret, mutability, effects) => Ty::Fn(
                fold_all(params, f),
                Box::new(ret.fold(f)),
                Box::new(mutability.fold(f)),
                Box::new(effects.fold(f)),
            ),
            // A part of the row can become a row with effects, like `E` when
            // it's replaced by `Log`, so the row is made again.
            Ty::Row(effects, rest) => Ty::row_of(
                effects
                    .iter()
                    .map(|e| EffectTy { effect: e.effect, args: fold_all(&e.args, f) })
                    .collect::<Vec<_>>(),
                fold_all(rest, f).into_vec(),
            ),
            Ty::Assoc(alias, args) => Ty::Assoc(*alias, fold_all(args, f)),
            Ty::Imm(ty) => Ty::Imm(Box::new(ty.fold(f))),
            Ty::Var(_)
            | Ty::Builtin(_)
            | Ty::Mutability(_)
            | Ty::Unwritten
            | Ty::Param(_)
            | Ty::Bound(_)
            | Ty::Number(_)
            | Ty::Never
            | Ty::Error => self.clone(),
        }
    }

    /// Whether `pred` holds for this type or a type inside it.
    pub fn any(&self, pred: &mut impl FnMut(&Ty<'db>) -> bool) -> bool {
        if pred(self) {
            return true;
        }
        let mut found = false;
        self.walk_children(|t| found = found || t.any(pred));
        found
    }

    /// A value that shows this type as Bolt code. Types still being
    /// inferred are shown as `?0`, `?1`, ...; see [`Ty::display_with`] for
    /// readable names.
    pub fn display(&self, db: &'db dyn Db) -> DisplayTy<'_, 'db> {
        DisplayTy { ty: self, db, names: None }
    }

    /// Like [`Ty::display`], with the names `names` gives unknown types.
    pub fn display_with<'a>(
        &'a self,
        db: &'db dyn Db,
        names: &'a TyNames<'db>,
    ) -> DisplayTy<'a, 'db> {
        DisplayTy { ty: self, db, names: Some(names) }
    }
}

/// A type the user can't see the name of: one still being inferred, or a
/// type variable of an inferred function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unknown<'db> {
    Var(TyVar),
    Bound(u32),
    /// A hidden mutability parameter, which only gets a name when it's
    /// shared.
    Param(GenericParamId<'db>),
}

/// Names for the unknown types of a message, so that they read like type
/// parameters: `a`, `b`, ... in the order they first appear. Names that the
/// type parameters in the message already have are skipped.
///
/// An unknown mutability is shown only when it's shared: one that appears
/// more than once gets a name like `'m`, so that `fn(Bool, 'm [a], 'm [a])
/// -> 'm [a]` says that the arrays have the same mutability. One that
/// appears once is shown as nothing, like `[a]`.
#[derive(Debug, Default)]
pub struct TyNames<'db> {
    names: Vec<(Unknown<'db>, String)>,
    mutabilities: Vec<(Unknown<'db>, String)>,
    /// Names for unknown rows, like `e`, which are only shown when they're
    /// shared, like mutabilities.
    effects: Vec<(Unknown<'db>, String)>,
    /// Whether every mutability is shown, even an unknown one used once and
    /// the `imm` of a function.
    explicit: bool,
}

impl<'db> TyNames<'db> {
    /// Like [`TyNames::new`], but showing every mutability: for types that
    /// differ only there, like an `imm fn()` and a generic `fn()`.
    pub fn explicit(db: &'db dyn Db, tys: &[&Ty<'db>]) -> Self {
        let mut names = Self::with_mode(db, tys, true);
        names.explicit = true;
        names
    }

    /// Names for the unknown types in `tys`, which are shown in this order.
    pub fn new(db: &'db dyn Db, tys: &[&Ty<'db>]) -> Self {
        Self::with_mode(db, tys, false)
    }

    fn with_mode(db: &'db dyn Db, tys: &[&Ty<'db>], explicit: bool) -> Self {
        #[derive(Default)]
        struct Found<'db> {
            taken: Vec<String>,
            unknowns: Vec<Unknown<'db>>,
            taken_mutabilities: Vec<String>,
            mutabilities: Vec<Unknown<'db>>,
            effects: Vec<Unknown<'db>>,
        }
        fn effects<'db>(db: &'db dyn Db, row: &Ty<'db>, found: &mut Found<'db>) {
            match row {
                Ty::Var(v) => found.effects.push(Unknown::Var(*v)),
                Ty::Bound(i) => found.effects.push(Unknown::Bound(*i)),
                Ty::Param(p) => found.taken.push(param_name(db, *p)),
                Ty::Row(known, rest) => {
                    for e in known.iter() {
                        e.args.iter().for_each(|t| collect(db, t, found));
                    }
                    rest.iter().for_each(|r| effects(db, r, found));
                }
                _ => row.walk_children(|t| collect(db, t, found)),
            }
        }
        fn mutability<'db>(db: &'db dyn Db, m: &Ty<'db>, found: &mut Found<'db>) {
            match m {
                Ty::Var(v) => found.mutabilities.push(Unknown::Var(*v)),
                Ty::Bound(i) => found.mutabilities.push(Unknown::Bound(*i)),
                Ty::Param(p) => match mutability_name(db, *p) {
                    Some(name) => found.taken_mutabilities.push(format!("'{name}")),
                    None => found.mutabilities.push(Unknown::Param(*p)),
                },
                _ => {}
            }
        }
        fn collect<'db>(db: &'db dyn Db, ty: &Ty<'db>, found: &mut Found<'db>) {
            match ty {
                Ty::Param(p) => found.taken.push(param_name(db, *p)),
                Ty::Var(v) => found.unknowns.push(Unknown::Var(*v)),
                Ty::Bound(i) => found.unknowns.push(Unknown::Bound(*i)),
                Ty::Array(m, elem) => {
                    mutability(db, m, found);
                    collect(db, elem, found);
                }
                Ty::Adt(_, args, ms, m) => {
                    mutability(db, m, found);
                    args.iter().for_each(|t| collect(db, t, found));
                    ms.iter().for_each(|m| mutability(db, m, found));
                }
                Ty::Fn(params, ret, m, e) => {
                    mutability(db, m, found);
                    params.iter().for_each(|t| collect(db, t, found));
                    collect(db, ret, found);
                    effects(db, e, found);
                }
                _ => ty.walk_children(|t| collect(db, t, found)),
            }
        }
        let mut found = Found::default();
        for ty in tys {
            collect(db, ty, &mut found);
        }
        let mut out = TyNames::default();
        let mut candidates = (0..).map(bound_name).filter(|n| !found.taken.contains(n));
        for unknown in found.unknowns {
            if !out.names.iter().any(|(u, _)| *u == unknown) {
                out.names.push((unknown, candidates.next().unwrap()));
            }
        }
        // From `'m`, like a name written in a signature, so that they don't
        // read like the type variables `a`, `b`, ...
        let mut candidates = (12..)
            .map(|i| format!("'{}", bound_name(i)))
            .filter(|n| !found.taken_mutabilities.contains(n));
        for &unknown in &found.mutabilities {
            let shared =
                explicit || found.mutabilities.iter().filter(|u| **u == unknown).count() > 1;
            if shared && !out.mutabilities.iter().any(|(u, _)| *u == unknown) {
                out.mutabilities.push((unknown, candidates.next().unwrap()));
            }
        }
        // From `e`, as in `forall a e.`, skipping the names already taken.
        let taken: Vec<String> = out.names.iter().map(|(_, n)| n.clone()).collect();
        let mut candidates =
            (4..).map(bound_name).filter(|n| !found.taken.contains(n) && !taken.contains(n));
        for &unknown in &found.effects {
            let shared = found.effects.iter().filter(|u| **u == unknown).count() > 1;
            if shared && !out.effects.iter().any(|(u, _)| *u == unknown) {
                out.effects.push((unknown, candidates.next().unwrap()));
            }
        }
        out
    }

    /// The name of an unknown row, if it's shared.
    fn effect(&self, unknown: Unknown<'db>) -> Option<&str> {
        self.effects.iter().find(|(u, _)| *u == unknown).map(|(_, n)| n.as_str())
    }

    /// The name of an unknown mutability, if it's shared.
    fn mutability(&self, unknown: Unknown<'db>) -> Option<&str> {
        self.mutabilities.iter().find(|(u, _)| *u == unknown).map(|(_, n)| n.as_str())
    }

    fn get(&self, unknown: Unknown<'db>) -> Option<&str> {
        self.names.iter().find(|(u, _)| *u == unknown).map(|(_, n)| n.as_str())
    }

    /// A mutability on its own, as in a bound like `a ⊑ imm`: `mut`, `imm`,
    /// or its name, like `'m`.
    pub fn mutability_name(&self, db: &'db dyn Db, m: &Ty<'db>) -> String {
        let named = |unknown| self.mutability(unknown).map(str::to_string);
        match m {
            Ty::Mutability(Mutability::Mut) => "mut".into(),
            Ty::Mutability(Mutability::Imm) => "imm".into(),
            Ty::Param(p) => match mutability_name(db, *p) {
                Some(name) => format!("'{name}"),
                None => named(Unknown::Param(*p)).unwrap_or_else(|| "'_".into()),
            },
            Ty::Var(v) => named(Unknown::Var(*v)).unwrap_or_else(|| "'_".into()),
            Ty::Bound(i) => named(Unknown::Bound(*i)).unwrap_or_else(|| "'_".into()),
            _ => "{error}".into(),
        }
    }

    /// A row on its own, as in a constraint like `Log ≲ e`: its effects,
    /// `Nothing`, or its name.
    pub fn row_name(&self, db: &'db dyn Db, row: &Ty<'db>) -> String {
        let shown = DisplayTy { ty: row, db, names: Some(self) }.shown_effects(row);
        shown.unwrap_or_else(|| match row {
            Ty::Row(..) => "Nothing".into(),
            _ => "_".into(),
        })
    }

    /// The names of the type variables of a scheme, then those of its rows,
    /// in order.
    pub fn bound_names(&self) -> impl Iterator<Item = &str> {
        self.names
            .iter()
            .chain(self.effects.iter())
            .filter(|(u, _)| matches!(u, Unknown::Bound(_)))
            .map(|(_, n)| n.as_str())
    }
}

/// Whether `param` is a mutability, named like the `'m` of `'m Point` or
/// hidden, rather than a declared type parameter.
pub fn is_hidden_param<'db>(db: &'db dyn Db, param: GenericParamId<'db>) -> bool {
    param.index as usize >= param.def.generic_params(db).type_params.len()
}

/// The name of a mutability parameter that the function or impl names,
/// like the `m` of `'m Point`, without its `'`.
fn mutability_name<'db>(
    db: &'db dyn Db,
    param: GenericParamId<'db>,
) -> Option<boltlang_hir_def::Name> {
    let index =
        (param.index as usize).checked_sub(param.def.generic_params(db).type_params.len())?;
    crate::lower::named_mutabilities(db, param.def).get(index).cloned()
}

/// The name of a type parameter. A hidden mutability parameter is never
/// shown in a type, but has a name for completeness.
fn param_name<'db>(db: &'db dyn Db, param: GenericParamId<'db>) -> String {
    let params = &param.def.generic_params(db).type_params;
    match params.get(param.index as usize) {
        Some(p) => p.name.to_string(),
        None => format!("m{}", param.index as usize - params.len()),
    }
}

pub struct DisplayTy<'a, 'db> {
    ty: &'a Ty<'db>,
    db: &'db dyn Db,
    names: Option<&'a TyNames<'db>>,
}

impl fmt::Display for DisplayTy<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db;
        let show = |ty| DisplayTy { ty, db, names: self.names };
        let list = |f: &mut fmt::Formatter<'_>, tys: &[Ty<'_>]| {
            for (i, ty) in tys.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}", DisplayTy { ty, db, names: self.names })?;
            }
            Ok(())
        };
        let named = |unknown| self.names.and_then(|n| n.get(unknown));
        match self.ty {
            Ty::Var(var) => match named(Unknown::Var(*var)) {
                Some(name) => f.write_str(name),
                None => write!(f, "?{}", var.0),
            },
            Ty::Adt(id, args, mutabilities, mutability) => {
                // An `imm struct` is always `imm`, like an enum.
                if let AdtId::Struct(s) = id
                    && !s.item(db).is_imm
                {
                    self.write_mutability(f, mutability)?;
                }
                let name = match id {
                    AdtId::Struct(id) => id.item(db).name.as_ref(),
                    AdtId::Enum(id) => id.item(db).name.as_ref(),
                };
                write!(f, "{}", name.map_or("?", |n| n.as_str()))?;
                // Like before a type, a mutability argument that's unknown or
                // generic and used once isn't shown, unless one after it is,
                // where it's `'_`.
                let mutabilities: Vec<Option<String>> =
                    mutabilities.iter().map(|m| self.shown_mutability(m)).collect();
                let shown = mutabilities.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
                if !args.is_empty() || shown > 0 {
                    f.write_str("[")?;
                    list(f, args)?;
                    for (i, m) in mutabilities[..shown].iter().enumerate() {
                        if i > 0 || !args.is_empty() {
                            f.write_str(", ")?;
                        }
                        f.write_str(m.as_deref().unwrap_or("'_"))?;
                    }
                    f.write_str("]")?;
                }
                Ok(())
            }
            Ty::App(head, args) => {
                write!(f, "{}[", show(head))?;
                list(f, args)?;
                f.write_str("]")
            }
            Ty::Builtin(ty) => write!(f, "{ty}"),
            Ty::Tuple(tys) => {
                f.write_str("(")?;
                list(f, tys)?;
                if tys.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            Ty::Array(mutability, elem) => {
                self.write_mutability(f, mutability)?;
                write!(f, "[{}]", show(elem))
            }
            // Only inside an array, where it's shown by the array.
            Ty::Mutability(Mutability::Mut) => f.write_str("Mut"),
            Ty::Mutability(Mutability::Imm) => f.write_str("Imm"),
            Ty::Unwritten => f.write_str("_"),
            Ty::Fn(params, ret, mutability, effects) => {
                // An `imm` function is shown as just `fn(...)`, since most
                // are.
                let explicit = self.names.is_some_and(|n| n.explicit);
                if explicit || *mutability.as_ref() != Ty::Mutability(Mutability::Imm) {
                    self.write_mutability(f, mutability)?;
                }
                f.write_str("fn(")?;
                list(f, params)?;
                write!(f, ") -> {}", show(ret))?;
                // No effects, or effects that are unknown and not shared, are
                // shown as nothing.
                match self.shown_effects(effects) {
                    Some(row) => write!(f, " does {row}"),
                    None => Ok(()),
                }
            }
            Ty::Row(..) => write!(f, "{}", self.shown_effects(self.ty).unwrap_or("Nothing".into())),
            Ty::Param(param) => write!(f, "{}", param_name(db, *param)),
            // `C.Item`, or `Convert[A, B].Item` for a class with several
            // parameters.
            Ty::Assoc(alias, args) => {
                let name = alias.item(db).name.as_ref().map_or("?", |n| n.as_str());
                match &args[..] {
                    [arg] if !matches!(arg, Ty::Fn(..)) => write!(f, "{}.{name}", show(arg)),
                    _ => {
                        let class = match alias.container(db) {
                            ItemContainerId::Class(c) => {
                                c.item(db).name.as_ref().map_or("?", |n| n.as_str())
                            }
                            _ => "?",
                        };
                        write!(f, "{class}[")?;
                        list(f, args)?;
                        write!(f, "].{name}")
                    }
                }
            }
            Ty::Imm(ty) => match ty.as_ref() {
                Ty::Fn(..) | Ty::Imm(_) => write!(f, "imm ({})", show(ty)),
                _ => write!(f, "imm {}", show(ty)),
            },
            Ty::Bound(index) => match named(Unknown::Bound(*index)) {
                Some(name) => f.write_str(name),
                None => write!(f, "{}", bound_name(*index)),
            },
            Ty::Number(NumberKind::Int) => f.write_str("{integer}"),
            Ty::Number(NumberKind::Float) => f.write_str("{float}"),
            Ty::Never => f.write_str("!"),
            Ty::Error => f.write_str("{error}"),
        }
    }
}

impl DisplayTy<'_, '_> {
    /// How the effects of a function are shown after `does`, like `Log +
    /// State[i32]` or a name like `e` for an unknown row that's shared.
    /// `None` for no effects, or an unknown row used once.
    fn shown_effects(&self, row: &Ty<'_>) -> Option<String> {
        // A row that's also a type argument, like the `R` of `Cont[B, U, R]`,
        // has the name it has there.
        let shared = |unknown| {
            self.names
                .and_then(|n| n.effect(unknown).or_else(|| n.get(unknown)))
                .map(str::to_string)
        };
        match row {
            Ty::Row(effects, rest) if effects.is_empty() && rest.is_empty() => None,
            Ty::Row(effects, rest) => {
                let mut shown: Vec<String> = effects
                    .iter()
                    .map(|e| {
                        let name = e.effect.item(self.db).name.as_ref().map_or("?", |n| n.as_str());
                        if e.args.is_empty() {
                            return name.to_string();
                        }
                        let args: Vec<String> = e
                            .args
                            .iter()
                            .map(|ty| DisplayTy { ty, db: self.db, names: self.names }.to_string())
                            .collect();
                        format!("{name}[{}]", args.join(", "))
                    })
                    .collect();
                // A part of a row that stands for any row is shown by its
                // name, which it has since it's also elsewhere, or as `_`.
                shown.extend(rest.iter().map(|r| self.shown_effects(r).unwrap_or("_".into())));
                Some(shown.join(" + "))
            }
            Ty::Param(p) => Some(param_name(self.db, *p)),
            Ty::Var(v) => shared(Unknown::Var(*v)),
            Ty::Bound(i) => shared(Unknown::Bound(*i)),
            Ty::Error => Some("{error}".into()),
            _ => None,
        }
    }

    /// `mut ` or `imm ` before an array or struct, a name like `'m ` for one
    /// that's named or shared, or nothing for one that's unknown or generic.
    fn write_mutability(&self, f: &mut fmt::Formatter<'_>, mutability: &Ty<'_>) -> fmt::Result {
        match self.shown_mutability(mutability) {
            Some(m) => write!(f, "{m} "),
            None => Ok(()),
        }
    }

    /// How a mutability is shown: `mut`, `imm`, or a name like `'m` for one
    /// that's named or shared. `None` for one that's unknown or generic and
    /// used once, which isn't shown.
    fn shown_mutability(&self, mutability: &Ty<'_>) -> Option<String> {
        let shared = |unknown| self.names.and_then(|n| n.mutability(unknown)).map(str::to_string);
        match mutability {
            Ty::Mutability(Mutability::Mut) => Some("mut".into()),
            Ty::Mutability(Mutability::Imm) => Some("imm".into()),
            Ty::Param(p) => match mutability_name(self.db, *p) {
                Some(name) => Some(format!("'{name}")),
                None => shared(Unknown::Param(*p)),
            },
            Ty::Var(v) => shared(Unknown::Var(*v)),
            Ty::Bound(i) => shared(Unknown::Bound(*i)),
            _ => None,
        }
    }
}

/// The order of the rows a row is made of: parameters, then variables, then
/// the variables of a scheme. It only has to be the same for the same rows.
fn rest_key(t: &Ty<'_>) -> (u8, Option<salsa::Id>, u32) {
    match t {
        Ty::Param(p) => (0, Some(salsa::plumbing::AsId::as_id(&p.def)), p.index),
        Ty::Var(v) => (1, None, v.0),
        Ty::Bound(i) => (2, None, *i),
        _ => (3, None, 0),
    }
}

/// `a`, `b`, ..., `z`, `a1`, ...
pub(crate) fn bound_name(index: u32) -> String {
    let letter = (b'a' + (index % 26) as u8) as char;
    match index / 26 {
        0 => letter.to_string(),
        n => format!("{letter}{n}"),
    }
}
