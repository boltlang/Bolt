//! The types the checker works with. Unlike the type references in
//! hir-def, they point at definitions instead of naming them.

use std::fmt;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    ids::{EnumId, StructId},
    resolver::GenericDefId,
};

pub use boltlang_hir_def::hir::Mutability;

/// An inference variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct TyVar(pub u32);

/// What an inference variable can become. An integer literal without a
/// suffix has an `Int` variable, which defaults to `i32`. A `Mutability`
/// variable stands for the mutability of an array, and defaults to `Imm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum VarKind {
    General,
    Int,
    Float,
    Mutability,
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
    /// A struct or enum with its type arguments, and its mutability. It can
    /// have fewer arguments than parameters: `Adt(Option, [], Imm)` is the
    /// type constructor `Option`, which is of kind `Type -> Type`. The
    /// mutability isn't an argument, so it doesn't change the kind. Only a
    /// struct has one, like an array: an enum's is always `Imm`, since its
    /// values can't be changed in place.
    Adt(AdtId<'db>, Box<[Ty<'db>]>, Box<Ty<'db>>),
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
    /// own kind: see `site/docs/internals/type-checking/mutability.mdx`.
    Mutability(Mutability),
    /// The mutability of an array type written without `mut` or `imm`, as
    /// lowered on its own. What it means depends on where the type is
    /// written, so the item that has the type replaces it with
    /// [`Ty::fill_unwritten`]: a parameter of a function, a variable in a
    /// body, or `Imm`. Only type aliases keep it, for the types they're
    /// used in to replace.
    Unwritten,
    /// `fn(A, B) -> C`
    Fn(Box<[Ty<'db>]>, Box<Ty<'db>>),
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

    /// An array of `element` with the mutability `mutability`.
    pub fn array(mutability: Ty<'db>, element: Ty<'db>) -> Self {
        Ty::Array(Box::new(mutability), Box::new(element))
    }

    /// A struct or enum applied to `args`. `mutability` is a struct's; an
    /// enum's is always `Imm`.
    pub fn adt(id: AdtId<'db>, args: impl Into<Box<[Ty<'db>]>>, mutability: Ty<'db>) -> Self {
        let mutability = match id {
            AdtId::Struct(_) => mutability,
            AdtId::Enum(_) => Ty::Mutability(Mutability::Imm),
        };
        Ty::Adt(id, args.into(), Box::new(mutability))
    }

    /// The mutability of an array or a struct.
    pub fn mutability(&self) -> Option<&Ty<'db>> {
        match self {
            Ty::Array(mutability, _) | Ty::Adt(AdtId::Struct(_), _, mutability) => Some(mutability),
            _ => None,
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
            Ty::Adt(id, xs, m) => Ty::Adt(id, xs.into_vec().into_iter().chain(args).collect(), m),
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
            Ty::Adt(_, args, mutability) => {
                args.iter().for_each(&mut f);
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
            Ty::Fn(params, ret) => {
                params.iter().for_each(&mut f);
                f(ret);
            }
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
            Ty::Adt(id, args, mutability) => Ty::Adt(*id, fold_all(args, f), Box::new(mutability.fold(f))),
            // The head can become a struct or enum, like `F` in `F[A]` when
            // `F` is replaced by `Option`.
            Ty::App(head, args) => Ty::app(head.fold(f), fold_all(args, f).into_vec()),
            Ty::Tuple(tys) => Ty::Tuple(fold_all(tys, f)),
            Ty::Array(mutability, elem) => Ty::array(mutability.fold(f), elem.fold(f)),
            Ty::Fn(params, ret) => Ty::Fn(fold_all(params, f), Box::new(ret.fold(f))),
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
    pub fn display_with<'a>(&'a self, db: &'db dyn Db, names: &'a TyNames<'db>) -> DisplayTy<'a, 'db> {
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
}

impl<'db> TyNames<'db> {
    /// Names for the unknown types in `tys`, which are shown in this order.
    pub fn new(db: &'db dyn Db, tys: &[&Ty<'db>]) -> Self {
        #[derive(Default)]
        struct Found<'db> {
            taken: Vec<String>,
            unknowns: Vec<Unknown<'db>>,
            taken_mutabilities: Vec<String>,
            mutabilities: Vec<Unknown<'db>>,
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
                Ty::Adt(_, args, m) => {
                    mutability(db, m, found);
                    args.iter().for_each(|t| collect(db, t, found));
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
        let mut candidates =
            (12..).map(|i| format!("'{}", bound_name(i))).filter(|n| !found.taken_mutabilities.contains(n));
        for &unknown in &found.mutabilities {
            let shared = found.mutabilities.iter().filter(|u| **u == unknown).count() > 1;
            if shared && !out.mutabilities.iter().any(|(u, _)| *u == unknown) {
                out.mutabilities.push((unknown, candidates.next().unwrap()));
            }
        }
        out
    }

    /// The name of an unknown mutability, if it's shared.
    fn mutability(&self, unknown: Unknown<'db>) -> Option<&str> {
        self.mutabilities.iter().find(|(u, _)| *u == unknown).map(|(_, n)| n.as_str())
    }

    fn get(&self, unknown: Unknown<'db>) -> Option<&str> {
        self.names.iter().find(|(u, _)| *u == unknown).map(|(_, n)| n.as_str())
    }

    /// The names of the type variables of a scheme, in order.
    pub fn bound_names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().filter(|(u, _)| matches!(u, Unknown::Bound(_))).map(|(_, n)| n.as_str())
    }
}

/// Whether `param` is a mutability, named like the `'m` of `'m Point` or
/// hidden, rather than a declared type parameter.
pub fn is_hidden_param<'db>(db: &'db dyn Db, param: GenericParamId<'db>) -> bool {
    param.index as usize >= param.def.generic_params(db).type_params.len()
}

/// The name of a mutability parameter that the function or impl names,
/// like the `m` of `'m Point`, without its `'`.
fn mutability_name<'db>(db: &'db dyn Db, param: GenericParamId<'db>) -> Option<boltlang_hir_def::Name> {
    let index = (param.index as usize).checked_sub(param.def.generic_params(db).type_params.len())?;
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
            Ty::Adt(id, args, mutability) => {
                if let AdtId::Struct(_) = id {
                    self.write_mutability(f, mutability)?;
                }
                let name = match id {
                    AdtId::Struct(id) => id.item(db).name.as_ref(),
                    AdtId::Enum(id) => id.item(db).name.as_ref(),
                };
                write!(f, "{}", name.map_or("?", |n| n.as_str()))?;
                if !args.is_empty() {
                    f.write_str("[")?;
                    list(f, args)?;
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
            Ty::Fn(params, ret) => {
                f.write_str("fn(")?;
                list(f, params)?;
                write!(f, ") -> {}", show(ret))
            }
            Ty::Param(param) => write!(f, "{}", param_name(db, *param)),
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
    /// `mut ` or `imm ` before an array or struct, a name like `'m ` for one
    /// that's named or shared, or nothing for one that's unknown or generic.
    fn write_mutability(&self, f: &mut fmt::Formatter<'_>, mutability: &Ty<'_>) -> fmt::Result {
        let shared = |unknown| self.names.and_then(|n| n.mutability(unknown));
        match mutability {
            Ty::Mutability(Mutability::Mut) => f.write_str("mut "),
            Ty::Mutability(Mutability::Imm) => f.write_str("imm "),
            Ty::Param(p) => match mutability_name(self.db, *p) {
                Some(name) => write!(f, "'{name} "),
                None => shared(Unknown::Param(*p)).map_or(Ok(()), |n| write!(f, "{n} ")),
            },
            Ty::Var(v) => shared(Unknown::Var(*v)).map_or(Ok(()), |n| write!(f, "{n} ")),
            Ty::Bound(i) => shared(Unknown::Bound(*i)).map_or(Ok(()), |n| write!(f, "{n} ")),
            _ => Ok(()),
        }
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
