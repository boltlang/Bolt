//! The types the checker works with. Unlike the type references in
//! hir-def, they point at definitions instead of naming them.

use std::fmt;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    ids::{EnumId, StructId},
    resolver::GenericDefId,
};

/// An inference variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct TyVar(pub u32);

/// What an inference variable can become. An integer literal without a
/// suffix has an `Int` variable, which defaults to `i32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum VarKind {
    General,
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
    /// A struct or enum with its type arguments.
    Adt(AdtId<'db>, Box<[Ty<'db>]>),
    Builtin(BuiltinType),
    /// `(A, B)`; the unit type is the empty tuple.
    Tuple(Box<[Ty<'db>]>),
    /// `[T]` or `[T; N]`. Lengths aren't checked yet.
    Array(Box<Ty<'db>>),
    /// `fn(A, B) -> C`
    Fn(Box<[Ty<'db>]>, Box<Ty<'db>>),
    /// A type parameter as seen inside the item that declares it, where it
    /// stands for any type and only equals itself.
    Param(GenericParamId<'db>),
    /// A type variable bound by an inferred [`Scheme`](crate::Scheme).
    Bound(u32),
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

    pub fn is_unit(&self) -> bool {
        matches!(self, Ty::Tuple(tys) if tys.is_empty())
    }

    /// Call `f` on every type directly inside this one.
    pub fn walk_children(&self, mut f: impl FnMut(&Ty<'db>)) {
        match self {
            Ty::Adt(_, args) | Ty::Tuple(args) => args.iter().for_each(f),
            Ty::Array(elem) => f(elem),
            Ty::Fn(params, ret) => {
                params.iter().for_each(&mut f);
                f(ret);
            }
            Ty::Var(_) | Ty::Builtin(_) | Ty::Param(_) | Ty::Bound(_) | Ty::Never | Ty::Error => {}
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
            Ty::Adt(id, args) => Ty::Adt(*id, fold_all(args, f)),
            Ty::Tuple(tys) => Ty::Tuple(fold_all(tys, f)),
            Ty::Array(elem) => Ty::Array(Box::new(elem.fold(f))),
            Ty::Fn(params, ret) => Ty::Fn(fold_all(params, f), Box::new(ret.fold(f))),
            Ty::Var(_) | Ty::Builtin(_) | Ty::Param(_) | Ty::Bound(_) | Ty::Never | Ty::Error => self.clone(),
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
    pub fn display_with<'a>(&'a self, db: &'db dyn Db, names: &'a TyNames) -> DisplayTy<'a, 'db> {
        DisplayTy { ty: self, db, names: Some(names) }
    }
}

/// A type the user can't see the name of: one still being inferred, or a
/// type variable of an inferred function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unknown {
    Var(TyVar),
    Bound(u32),
}

/// Names for the unknown types of a message, so that they read like type
/// parameters: `a`, `b`, ... in the order they first appear. Names that the
/// type parameters in the message already have are skipped.
#[derive(Debug, Default)]
pub struct TyNames {
    names: Vec<(Unknown, String)>,
}

impl TyNames {
    /// Names for the unknown types in `tys`, which are shown in this order.
    pub fn new<'db>(db: &'db dyn Db, tys: &[&Ty<'db>]) -> Self {
        let mut taken = Vec::new();
        let mut unknowns = Vec::new();
        for ty in tys {
            ty.any(&mut |t| {
                match t {
                    Ty::Param(p) => taken.push(param_name(db, *p).to_string()),
                    Ty::Var(v) => unknowns.push(Unknown::Var(*v)),
                    Ty::Bound(i) => unknowns.push(Unknown::Bound(*i)),
                    _ => {}
                }
                false
            });
        }
        let mut out = TyNames::default();
        let mut candidates = (0..).map(bound_name).filter(|n| !taken.contains(n));
        for unknown in unknowns {
            if !out.names.iter().any(|(u, _)| *u == unknown) {
                out.names.push((unknown, candidates.next().unwrap()));
            }
        }
        out
    }

    fn get(&self, unknown: Unknown) -> Option<&str> {
        self.names.iter().find(|(u, _)| *u == unknown).map(|(_, n)| n.as_str())
    }
}

fn param_name<'db>(db: &'db dyn Db, param: GenericParamId<'db>) -> &'db boltlang_hir_def::Name {
    &param.def.generic_params(db).type_params[param.index as usize].name
}

pub struct DisplayTy<'a, 'db> {
    ty: &'a Ty<'db>,
    db: &'db dyn Db,
    names: Option<&'a TyNames>,
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
            Ty::Adt(id, args) => {
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
            Ty::Builtin(ty) => write!(f, "{ty}"),
            Ty::Tuple(tys) => {
                f.write_str("(")?;
                list(f, tys)?;
                if tys.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            Ty::Array(elem) => write!(f, "[{}]", show(elem)),
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
            Ty::Never => f.write_str("!"),
            Ty::Error => f.write_str("{error}"),
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
