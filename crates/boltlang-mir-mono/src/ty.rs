//! The types of an instance: closed, with nothing left to infer.
//!
//! The type checker's [`Ty`] has inference variables, type parameters,
//! associated types and more. A backend only needs what a value is at run
//! time, so an instance's types are [`MonoTy`]s:
//!
//! - **[`MonoTy::Erased`]** is a type that was erased: a type parameter of
//!   a function that recurses polymorphically, which is compiled once for
//!   every type (see [`crate::collect`]). At run time it's a boxed value of
//!   any type. Erased types are numbered, so that a dictionary for `Show[A]`
//!   isn't mistaken for one for `Show[B]`. Users can't write them.
//! - **[`MonoMut::Param`]** is a mutability parameter of the instance.
//!   Instances aren't specialized on mutabilities, so `fn f(xs: 'm [i32])`
//!   is one instance for `mut` and `imm` arrays.

use std::fmt;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::resolver::GenericDefId;
use boltlang_hir_def::{builtin_type::BuiltinType, hir::Mutability, ids::TypeAliasId};
use boltlang_hir_ty::{
    AdtId, GenericParamId, Ty,
    lower::VariantDef,
    solve::{imm_of, normalize_assoc},
};

/// A mutability in an instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum MonoMut {
    Imm,
    Mut,
    /// The `i`th mutability parameter of the instance.
    Param(u32),
}

/// A type in an instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum MonoTy<'db> {
    Builtin(BuiltinType),
    /// A struct or enum: its type arguments, mutability arguments and
    /// mutability, as in [`Ty::Adt`]. A type constructor, like the
    /// argument of `Functor[Option]`, has fewer type arguments than
    /// parameters.
    Adt(AdtId<'db>, Box<[MonoTy<'db>]>, Box<[MonoMut]>, MonoMut),
    /// `(A, B)`, and `()` when empty.
    Tuple(Box<[MonoTy<'db>]>),
    Array(MonoMut, Box<MonoTy<'db>>),
    /// The parameters, the result, and the mutability of a closure.
    Fn(Box<[MonoTy<'db>]>, Box<MonoTy<'db>>, MonoMut),
    /// The type of an expression that never gives a value, like `return`.
    Never,
    /// The `i`th erased type of the instance: a value of any type, boxed.
    Erased(u32),
    /// An associated type of erased types, like `A.Item`, which no impl can
    /// decide: a boxed value too.
    ErasedAssoc(TypeAliasId<'db>, Box<[MonoTy<'db>]>),
}

impl<'db> MonoTy<'db> {
    pub fn unit() -> Self {
        MonoTy::Tuple(Box::new([]))
    }

    /// Whether an erased type is somewhere in the type.
    pub fn has_erased(&self) -> bool {
        match self {
            MonoTy::Erased(_) | MonoTy::ErasedAssoc(..) => true,
            MonoTy::Builtin(_) | MonoTy::Never => false,
            MonoTy::Adt(_, args, ..) | MonoTy::Tuple(args) => args.iter().any(MonoTy::has_erased),
            MonoTy::Array(_, elem) => elem.has_erased(),
            MonoTy::Fn(params, ret, _) => params.iter().any(MonoTy::has_erased) || ret.has_erased(),
        }
    }

    /// The type with its erased types numbered in the order they appear,
    /// from `next`, with `map` keeping what each one became.
    pub(crate) fn renumber(&self, map: &mut Vec<(u32, u32)>, next: &mut u32) -> Self {
        let all = |tys: &[MonoTy<'db>], map: &mut Vec<(u32, u32)>, next: &mut u32| {
            tys.iter().map(|t| t.renumber(map, next)).collect()
        };
        match self {
            MonoTy::Erased(i) => match map.iter().find(|(old, _)| old == i) {
                Some((_, new)) => MonoTy::Erased(*new),
                None => {
                    map.push((*i, *next));
                    *next += 1;
                    MonoTy::Erased(*next - 1)
                }
            },
            MonoTy::ErasedAssoc(alias, args) => MonoTy::ErasedAssoc(*alias, all(args, map, next)),
            MonoTy::Builtin(_) | MonoTy::Never => self.clone(),
            MonoTy::Adt(adt, args, ms, own) => {
                MonoTy::Adt(*adt, all(args, map, next), ms.clone(), *own)
            }
            MonoTy::Tuple(args) => MonoTy::Tuple(all(args, map, next)),
            MonoTy::Array(own, elem) => MonoTy::Array(*own, Box::new(elem.renumber(map, next))),
            MonoTy::Fn(params, ret, own) => {
                MonoTy::Fn(all(params, map, next), Box::new(ret.renumber(map, next)), *own)
            }
        }
    }

    /// Whether a mutability parameter is somewhere in the type.
    pub fn has_mut_param(&self) -> bool {
        let param = |m: &MonoMut| matches!(m, MonoMut::Param(_));
        match self {
            MonoTy::Erased(_) | MonoTy::Builtin(_) | MonoTy::Never => false,
            MonoTy::ErasedAssoc(_, args) => args.iter().any(MonoTy::has_mut_param),
            MonoTy::Adt(_, args, ms, m) => {
                param(m) || ms.iter().any(param) || args.iter().any(MonoTy::has_mut_param)
            }
            MonoTy::Tuple(args) => args.iter().any(MonoTy::has_mut_param),
            MonoTy::Array(m, elem) => param(m) || elem.has_mut_param(),
            MonoTy::Fn(params, ret, m) => {
                param(m) || params.iter().any(MonoTy::has_mut_param) || ret.has_mut_param()
            }
        }
    }

    /// The type as the type checker writes it, to choose impls with, with
    /// each mutability parameter as `params`. `None` if it has an erased
    /// type.
    pub(crate) fn to_ty(&self, params: Mutability) -> Option<Ty<'db>> {
        self.to_ty_with(&|_| Ty::Mutability(params))
    }

    /// [`MonoTy::to_ty`], with `param` giving each mutability parameter.
    pub(crate) fn to_ty_with(&self, param: &dyn Fn(u32) -> Ty<'db>) -> Option<Ty<'db>> {
        let m = |m: &MonoMut| match m {
            MonoMut::Imm => Ty::Mutability(Mutability::Imm),
            MonoMut::Mut => Ty::Mutability(Mutability::Mut),
            MonoMut::Param(i) => param(*i),
        };
        let all = |tys: &[MonoTy<'db>]| -> Option<Box<[Ty<'db>]>> {
            tys.iter().map(|t| t.to_ty_with(param)).collect()
        };
        Some(match self {
            MonoTy::Erased(_) | MonoTy::ErasedAssoc(..) => return None,
            MonoTy::Builtin(b) => Ty::Builtin(*b),
            MonoTy::Never => Ty::Never,
            MonoTy::Adt(adt, args, ms, own) => {
                Ty::Adt(*adt, all(args)?, ms.iter().map(m).collect(), Box::new(m(own)))
            }
            MonoTy::Tuple(tys) => Ty::Tuple(all(tys)?),
            MonoTy::Array(own, elem) => {
                Ty::Array(Box::new(m(own)), Box::new(elem.to_ty_with(param)?))
            }
            // Instances don't keep effects, which only the checker needs.
            MonoTy::Fn(ps, ret, own) => Ty::Fn(
                all(ps)?,
                Box::new(ret.to_ty_with(param)?),
                Box::new(m(own)),
                Box::new(Ty::nothing()),
            ),
        })
    }

    pub fn display(&self, db: &'db dyn Db) -> impl fmt::Display + '_ {
        DisplayMonoTy { db, ty: self }
    }
}

impl fmt::Display for MonoMut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonoMut::Imm => write!(f, "imm"),
            MonoMut::Mut => write!(f, "mut"),
            MonoMut::Param(i) => write!(f, "'{i}"),
        }
    }
}

struct DisplayMonoTy<'a, 'db> {
    db: &'db dyn Db,
    ty: &'a MonoTy<'db>,
}

impl fmt::Display for DisplayMonoTy<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db;
        let list = |f: &mut fmt::Formatter<'_>, tys: &[MonoTy<'_>]| -> fmt::Result {
            for (i, t) in tys.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", DisplayMonoTy { db, ty: t })?;
            }
            Ok(())
        };
        match self.ty {
            MonoTy::Erased(i) => write!(f, "any{i}"),
            MonoTy::ErasedAssoc(alias, args) => {
                let name = alias.item(db).name.clone();
                write!(f, "any({}[", name.as_ref().map_or("?", |n| n.as_str()))?;
                list(f, args)?;
                write!(f, "])")
            }
            MonoTy::Never => write!(f, "!"),
            MonoTy::Builtin(b) => write!(f, "{b}"),
            MonoTy::Adt(adt, args, ms, own) => {
                // An enum, and an `imm struct`, are always `imm`.
                if *own != MonoMut::Imm {
                    write!(f, "{own} ")?;
                }
                write!(f, "{}", adt_name(db, *adt))?;
                if !args.is_empty() || !ms.is_empty() {
                    write!(f, "[")?;
                    list(f, args)?;
                    for (i, m) in ms.iter().enumerate() {
                        if i > 0 || !args.is_empty() {
                            write!(f, ", ")?;
                        }
                        write!(f, "{m}")?;
                    }
                    write!(f, "]")?;
                }
                Ok(())
            }
            MonoTy::Tuple(tys) => {
                write!(f, "(")?;
                list(f, tys)?;
                if tys.len() == 1 {
                    write!(f, ",")?;
                }
                write!(f, ")")
            }
            MonoTy::Array(own, elem) => write!(f, "{own} [{}]", DisplayMonoTy { db, ty: elem }),
            MonoTy::Fn(params, ret, own) => {
                if *own != MonoMut::Imm {
                    write!(f, "{own} ")?;
                }
                write!(f, "fn(")?;
                list(f, params)?;
                write!(f, ") -> {}", DisplayMonoTy { db, ty: ret })
            }
        }
    }
}

fn adt_name(db: &dyn Db, adt: AdtId<'_>) -> String {
    let name = match adt {
        AdtId::Struct(s) => s.item(db).name.clone(),
        AdtId::Enum(e) => e.item(db).name.clone(),
    };
    name.map_or_else(|| "?".to_string(), |n| n.as_str().to_string())
}

/// The types of the fields of `def`, a struct or a variant of an enum, in a
/// value of type `MonoTy::Adt(_, args, mutabilities, own)`: with `args` for
/// the type parameters, `mutabilities` for those a struct declares, and
/// `own` for the struct's own. An enum's fields don't depend on its
/// mutability, which is always `imm`.
pub fn field_types<'db>(
    db: &'db dyn Db,
    root: File,
    def: VariantDef<'db>,
    args: &[MonoTy<'db>],
    mutabilities: &[MonoMut],
    own: MonoMut,
) -> Result<Vec<MonoTy<'db>>, TyError> {
    let generic = match def {
        VariantDef::Struct(id) => GenericDefId::Struct(id),
        VariantDef::Variant(v) => GenericDefId::Enum(v.parent),
    };
    let mut slots: Vec<Slot<'db>> = args.iter().cloned().map(Slot::Ty).collect();
    if let VariantDef::Struct(_) = def {
        slots.extend(mutabilities.iter().map(|&m| Slot::Mut(m)));
        slots.push(Slot::Mut(own));
    }
    let params: Vec<GenericParamId<'db>> =
        (0..slots.len() as u32).map(|index| GenericParamId { def: generic, index }).collect();
    let subst = Subst { db, root, params: &params, slots: &slots };
    boltlang_hir_ty::lower::field_types(db, def).fields.iter().map(|(_, t)| subst.ty(t)).collect()
}

/// What a parameter of a scheme is in an instance: a type, or one of the
/// instance's mutabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Slot<'db> {
    Ty(MonoTy<'db>),
    Mut(MonoMut),
    /// A row of effects, which instances don't keep.
    Row,
}

/// Why a type couldn't be made closed.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum TyError {
    /// A type the checker didn't decide, which happens after a type error.
    Unknown,
}

/// Turns the checker's types in a body into an instance's: each parameter
/// of the body's scheme, `params` and then its bound variables, becomes
/// its slot, and associated types become what the impls visible from
/// `root` make them.
pub(crate) struct Subst<'a, 'db> {
    pub db: &'db dyn Db,
    pub root: File,
    pub params: &'a [GenericParamId<'db>],
    pub slots: &'a [Slot<'db>],
}

impl<'db> Subst<'_, 'db> {
    fn slot(&self, ty: &Ty<'db>) -> Option<&Slot<'db>> {
        match ty {
            Ty::Param(p) => self.params.iter().position(|q| q == p).and_then(|i| self.slots.get(i)),
            Ty::Bound(i) => self.slots.get(self.params.len() + *i as usize),
            _ => None,
        }
    }

    pub fn ty(&self, ty: &Ty<'db>) -> Result<MonoTy<'db>, TyError> {
        let all = |tys: &[Ty<'db>]| -> Result<Box<[MonoTy<'db>]>, TyError> {
            tys.iter().map(|t| self.ty(t)).collect()
        };
        Ok(match ty {
            Ty::Param(_) | Ty::Bound(_) => match self.slot(ty) {
                Some(Slot::Ty(t)) => t.clone(),
                _ => return Err(TyError::Unknown),
            },
            Ty::Builtin(b) => MonoTy::Builtin(*b),
            Ty::Never => MonoTy::Never,
            Ty::Adt(adt, args, ms, own) => MonoTy::Adt(
                *adt,
                all(args)?,
                ms.iter().map(|m| self.mutability(m)).collect::<Result<_, _>>()?,
                self.mutability(own)?,
            ),
            Ty::Tuple(tys) => MonoTy::Tuple(all(tys)?),
            Ty::Array(own, elem) => MonoTy::Array(self.mutability(own)?, Box::new(self.ty(elem)?)),
            Ty::Fn(params, ret, own, _) => {
                MonoTy::Fn(all(params)?, Box::new(self.ty(ret)?), self.mutability(own)?)
            }
            // `F[A]` with `F` a type constructor, like `Option`.
            Ty::App(head, args) => match self.ty(head)? {
                MonoTy::Adt(adt, given, ms, own) => {
                    let mut given = given.into_vec();
                    given.extend(all(args)?);
                    MonoTy::Adt(adt, given.into(), ms, own)
                }
                _ => return Err(TyError::Unknown),
            },
            Ty::Assoc(alias, args) => self.assoc(*alias, args)?,
            // `imm T`, once `T` is known: what `imm` makes of it.
            Ty::Imm(inner) => {
                let inner = self.back(&self.ty(inner)?)?;
                match imm_of(self.db, self.root, &inner) {
                    Ty::Imm(_) => return Err(TyError::Unknown),
                    ty => self.ty(&ty)?,
                }
            }
            // A type that nothing decides, like the elements of `[]`, can't
            // change what the code does.
            Ty::Var(_) => MonoTy::unit(),
            // A row is only ever inside a function type, which drops it.
            Ty::Mutability(_) | Ty::Unwritten | Ty::Row(..) | Ty::Number(_) | Ty::Error => {
                return Err(TyError::Unknown);
            }
        })
    }

    /// An associated type, like `C.Item`, once its arguments are known.
    /// With an erased argument it's erased too, since no impl can be
    /// chosen for it. With a mutability parameter, it's what the impls give
    /// for both mutabilities, if that's the same.
    fn assoc(&self, alias: TypeAliasId<'db>, args: &[Ty<'db>]) -> Result<MonoTy<'db>, TyError> {
        let args: Vec<MonoTy<'db>> = args.iter().map(|t| self.ty(t)).collect::<Result<_, _>>()?;
        if args.iter().any(MonoTy::has_erased) {
            return Ok(MonoTy::ErasedAssoc(alias, args.into()));
        }
        let args: Box<[Ty<'db>]> = args.iter().map(|t| self.back(t)).collect::<Result<_, _>>()?;
        let ty = normalize_assoc(self.db, self.root, &Ty::Assoc(alias, args));
        if matches!(ty, Ty::Assoc(..)) {
            return Err(TyError::Unknown);
        }
        self.ty(&ty)
    }

    /// A closed type as one of the checker's, for it to work on. A
    /// mutability parameter stays the body's own, as the checker would have
    /// it, so that what it gives can use it, like the `'v` of `VecIter[T,
    /// 'v]` that an impl gives, and maps back to the instance's.
    pub(crate) fn back(&self, t: &MonoTy<'db>) -> Result<Ty<'db>, TyError> {
        let param = |i: u32| -> Ty<'db> {
            let at = self.slots.iter().position(|s| *s == Slot::Mut(MonoMut::Param(i)));
            match at {
                Some(at) if at < self.params.len() => Ty::Param(self.params[at]),
                Some(at) => Ty::Bound((at - self.params.len()) as u32),
                None => Ty::Mutability(Mutability::Imm),
            }
        };
        t.to_ty_with(&param).ok_or(TyError::Unknown)
    }

    pub fn mutability(&self, ty: &Ty<'db>) -> Result<MonoMut, TyError> {
        Ok(match ty {
            Ty::Mutability(Mutability::Imm) | Ty::Unwritten => MonoMut::Imm,
            Ty::Mutability(Mutability::Mut) => MonoMut::Mut,
            Ty::Param(_) | Ty::Bound(_) => match self.slot(ty) {
                Some(Slot::Mut(m)) => *m,
                _ => return Err(TyError::Unknown),
            },
            // A mutability left undecided is `imm`, as the checker defaults it.
            Ty::Var(_) | Ty::Error => MonoMut::Imm,
            _ => return Err(TyError::Unknown),
        })
    }
}
