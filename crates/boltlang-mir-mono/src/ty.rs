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
//! - **[`MonoRow`]** is a row of effects, with the instance's row
//!   parameters in it. Instances aren't specialized on rows either, so
//!   `fn map[E](f: fn() does E) does E` is one instance for every row.

use std::fmt;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::resolver::GenericDefId;
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    hir::Mutability,
    ids::{EffectId, TypeAliasId},
};
use boltlang_hir_ty::{
    AdtId, GenericParamId, Ty,
    lower::VariantDef,
    solve::{imm_of, normalize_assoc},
    ty::EffectTy,
};

/// A mutability in an instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum MonoMut {
    Imm,
    Mut,
    /// The `i`th mutability parameter of the instance.
    Param(u32),
}

/// An effect with its arguments, like `State[i32]`, in an instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct MonoEffect<'db> {
    pub effect: EffectId<'db>,
    pub args: Box<[MonoTy<'db>]>,
}

/// A row of effects in an instance: the effects it has, each once, and the
/// instance's row parameters it's made of, both sorted, so that equal rows
/// are equal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct MonoRow<'db> {
    pub effects: Box<[MonoEffect<'db>]>,
    /// The row parameters, from 0.
    pub params: Box<[u32]>,
}

impl<'db> MonoRow<'db> {
    /// No effects.
    pub fn nothing() -> Self {
        MonoRow::default()
    }

    /// The `i`th row parameter.
    pub fn param(i: u32) -> Self {
        MonoRow { effects: Box::new([]), params: Box::new([i]) }
    }

    /// The row of `effects` and `params`, sorted, each once. Of two with the
    /// same effect, the first stays: the checker gave them the same
    /// arguments.
    pub fn new(
        effects: impl IntoIterator<Item = MonoEffect<'db>>,
        params: impl IntoIterator<Item = u32>,
    ) -> Self {
        let mut known: Vec<MonoEffect<'db>> = Vec::new();
        for e in effects {
            if !known.iter().any(|k| k.effect == e.effect) {
                known.push(e);
            }
        }
        known.sort_by_key(|e| salsa::plumbing::AsId::as_id(&e.effect));
        let mut params: Vec<u32> = params.into_iter().collect();
        params.sort();
        params.dedup();
        MonoRow { effects: known.into(), params: params.into() }
    }

    pub fn is_nothing(&self) -> bool {
        self.effects.is_empty() && self.params.is_empty()
    }

    /// What either may perform.
    pub fn union(&self, other: &MonoRow<'db>) -> Self {
        MonoRow::new(
            self.effects.iter().chain(other.effects.iter()).cloned(),
            self.params.iter().chain(other.params.iter()).copied(),
        )
    }

    /// The row without `effect`.
    pub fn without(&self, effect: EffectId<'db>) -> Self {
        let effects = self.effects.iter().filter(|e| e.effect != effect).cloned();
        MonoRow::new(effects, self.params.iter().copied())
    }

    /// The row with each parameter replaced by the row `f` gives it, and
    /// the types in its effects' arguments by `ty`.
    pub fn map(
        &self,
        param: &mut dyn FnMut(u32) -> MonoRow<'db>,
        ty: &mut dyn FnMut(&MonoTy<'db>) -> MonoTy<'db>,
    ) -> Self {
        let effects: Vec<MonoEffect<'db>> = self
            .effects
            .iter()
            .map(|e| MonoEffect { effect: e.effect, args: e.args.iter().map(&mut *ty).collect() })
            .collect();
        let mut row = MonoRow::new(effects, []);
        for &p in self.params.iter() {
            row = row.union(&param(p));
        }
        row
    }

    fn any_ty(&self, f: &mut dyn FnMut(&MonoTy<'db>) -> bool) -> bool {
        self.effects.iter().flat_map(|e| e.args.iter()).any(f)
    }
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
    /// `(A, B)` and its mutability, and `()` when empty, which is `Imm`.
    Tuple(Box<[MonoTy<'db>]>, MonoMut),
    Array(MonoMut, Box<MonoTy<'db>>),
    /// The parameters, the result, the mutability and the effects of a
    /// closure.
    Fn(Box<[MonoTy<'db>]>, Box<MonoTy<'db>>, MonoMut, MonoRow<'db>),
    /// A row of effects where a type goes, like the `R` of `Cont[B, U, R]`.
    Row(MonoRow<'db>),
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
        MonoTy::Tuple(Box::new([]), MonoMut::Imm)
    }

    /// Whether an erased type is somewhere in the type.
    pub fn has_erased(&self) -> bool {
        match self {
            MonoTy::Erased(_) | MonoTy::ErasedAssoc(..) => true,
            MonoTy::Builtin(_) | MonoTy::Never => false,
            MonoTy::Adt(_, args, ..) | MonoTy::Tuple(args, _) => {
                args.iter().any(MonoTy::has_erased)
            }
            MonoTy::Array(_, elem) => elem.has_erased(),
            MonoTy::Fn(params, ret, _, row) => {
                params.iter().any(MonoTy::has_erased)
                    || ret.has_erased()
                    || row.any_ty(&mut MonoTy::has_erased)
            }
            MonoTy::Row(row) => row.any_ty(&mut MonoTy::has_erased),
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
            MonoTy::Tuple(args, own) => MonoTy::Tuple(all(args, map, next), *own),
            MonoTy::Array(own, elem) => MonoTy::Array(*own, Box::new(elem.renumber(map, next))),
            MonoTy::Fn(params, ret, own, row) => {
                let params = all(params, map, next);
                let ret = Box::new(ret.renumber(map, next));
                let row = row.map(&mut MonoRow::param, &mut |t| t.renumber(map, next));
                MonoTy::Fn(params, ret, *own, row)
            }
            MonoTy::Row(row) => {
                MonoTy::Row(row.map(&mut MonoRow::param, &mut |t| t.renumber(map, next)))
            }
        }
    }

    /// The type with each of its mutabilities replaced by what `f` gives.
    pub fn map_muts(&self, f: &mut dyn FnMut(MonoMut) -> MonoMut) -> Self {
        let all = |tys: &[MonoTy<'db>], f: &mut dyn FnMut(MonoMut) -> MonoMut| {
            tys.iter().map(|t| t.map_muts(f)).collect::<Box<[_]>>()
        };
        match self {
            MonoTy::Erased(_) | MonoTy::Builtin(_) | MonoTy::Never => self.clone(),
            MonoTy::ErasedAssoc(alias, args) => MonoTy::ErasedAssoc(*alias, all(args, f)),
            MonoTy::Adt(adt, args, ms, own) => {
                let args = all(args, f);
                let ms = ms.iter().map(|&m| f(m)).collect();
                MonoTy::Adt(*adt, args, ms, f(*own))
            }
            MonoTy::Tuple(args, own) => {
                let args = all(args, f);
                MonoTy::Tuple(args, f(*own))
            }
            MonoTy::Array(own, elem) => {
                let own = f(*own);
                MonoTy::Array(own, Box::new(elem.map_muts(f)))
            }
            MonoTy::Fn(params, ret, own, row) => {
                let params = all(params, f);
                let ret = Box::new(ret.map_muts(f));
                let own = f(*own);
                let row = row.map(&mut MonoRow::param, &mut |t| t.map_muts(f));
                MonoTy::Fn(params, ret, own, row)
            }
            MonoTy::Row(row) => MonoTy::Row(row.map(&mut MonoRow::param, &mut |t| t.map_muts(f))),
        }
    }

    /// The type with each of its rows, like that of the closure in
    /// `Option[fn() does Log]`, replaced by a row parameter, numbered from
    /// `first` in the order they appear, an equal row by the same one.
    /// `out` gets the rows that were replaced, by their number.
    pub fn abstract_rows(&self, first: u32, out: &mut Vec<MonoRow<'db>>) -> Self {
        let param = |r: &MonoRow<'db>, out: &mut Vec<MonoRow<'db>>| {
            let at = out.iter().position(|o| o == r).unwrap_or_else(|| {
                out.push(r.clone());
                out.len() - 1
            });
            MonoRow::param(first + at as u32)
        };
        let all = |tys: &[MonoTy<'db>], out: &mut Vec<MonoRow<'db>>| -> Box<[MonoTy<'db>]> {
            tys.iter().map(|t| t.abstract_rows(first, out)).collect()
        };
        match self {
            MonoTy::Adt(adt, args, muts, own) => {
                MonoTy::Adt(*adt, all(args, out), muts.clone(), *own)
            }
            MonoTy::Tuple(elems, own) => MonoTy::Tuple(all(elems, out), *own),
            MonoTy::Array(m, elem) => MonoTy::Array(*m, Box::new(elem.abstract_rows(first, out))),
            MonoTy::Fn(params, ret, m, row) => {
                let params = all(params, out);
                let ret = Box::new(ret.abstract_rows(first, out));
                MonoTy::Fn(params, ret, *m, param(row, out))
            }
            MonoTy::Row(row) => MonoTy::Row(param(row, out)),
            MonoTy::ErasedAssoc(alias, args) => MonoTy::ErasedAssoc(*alias, all(args, out)),
            MonoTy::Builtin(_) | MonoTy::Never | MonoTy::Erased(_) => self.clone(),
        }
    }

    /// The type with each row parameter replaced by the row `f` gives it.
    pub fn map_rows(&self, f: &mut dyn FnMut(u32) -> MonoRow<'db>) -> Self {
        let all = |tys: &[MonoTy<'db>], f: &mut dyn FnMut(u32) -> MonoRow<'db>| {
            tys.iter().map(|t| t.map_rows(f)).collect::<Box<[_]>>()
        };
        match self {
            MonoTy::Erased(_) | MonoTy::Builtin(_) | MonoTy::Never => self.clone(),
            MonoTy::ErasedAssoc(alias, args) => MonoTy::ErasedAssoc(*alias, all(args, f)),
            MonoTy::Adt(adt, args, ms, own) => MonoTy::Adt(*adt, all(args, f), ms.clone(), *own),
            MonoTy::Tuple(args, own) => MonoTy::Tuple(all(args, f), *own),
            MonoTy::Array(own, elem) => MonoTy::Array(*own, Box::new(elem.map_rows(f))),
            MonoTy::Fn(params, ret, own, row) => {
                let params = all(params, f);
                let ret = Box::new(ret.map_rows(f));
                let row = map_row(row, f);
                MonoTy::Fn(params, ret, *own, row)
            }
            MonoTy::Row(row) => MonoTy::Row(map_row(row, f)),
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
            MonoTy::Tuple(args, m) => param(m) || args.iter().any(MonoTy::has_mut_param),
            MonoTy::Array(m, elem) => param(m) || elem.has_mut_param(),
            MonoTy::Fn(params, ret, m, row) => {
                param(m)
                    || params.iter().any(MonoTy::has_mut_param)
                    || ret.has_mut_param()
                    || row.any_ty(&mut MonoTy::has_mut_param)
            }
            MonoTy::Row(row) => row.any_ty(&mut MonoTy::has_mut_param),
        }
    }

    /// The type as the type checker writes it, to choose impls with, with
    /// each mutability parameter as `params`. `None` if it has an erased
    /// type.
    /// Rows are left out, as `Nothing`: impls don't depend on them.
    pub(crate) fn to_ty(&self, params: Mutability) -> Option<Ty<'db>> {
        self.to_ty_with(&|_| Ty::Mutability(params), None)
    }

    /// [`MonoTy::to_ty`], with `param` giving each mutability parameter,
    /// and `row`, if given, each row parameter. Without it, rows are
    /// `Nothing`.
    pub(crate) fn to_ty_with(
        &self,
        param: &dyn Fn(u32) -> Ty<'db>,
        row: Option<&dyn Fn(u32) -> Ty<'db>>,
    ) -> Option<Ty<'db>> {
        let m = |m: &MonoMut| match m {
            MonoMut::Imm => Ty::Mutability(Mutability::Imm),
            MonoMut::Mut => Ty::Mutability(Mutability::Mut),
            MonoMut::Param(i) => param(*i),
        };
        let all = |tys: &[MonoTy<'db>]| -> Option<Box<[Ty<'db>]>> {
            tys.iter().map(|t| t.to_ty_with(param, row)).collect()
        };
        let row_ty = |r: &MonoRow<'db>| -> Option<Ty<'db>> {
            let Some(row) = row else { return Some(Ty::nothing()) };
            let effects = r
                .effects
                .iter()
                .map(|e| Some(EffectTy { effect: e.effect, args: all(&e.args)? }))
                .collect::<Option<Vec<_>>>()?;
            Some(Ty::row_of(effects, r.params.iter().map(|&i| row(i))))
        };
        Some(match self {
            MonoTy::Erased(_) | MonoTy::ErasedAssoc(..) => return None,
            MonoTy::Builtin(b) => Ty::Builtin(*b),
            MonoTy::Never => Ty::Never,
            MonoTy::Adt(adt, args, ms, own) => {
                Ty::Adt(*adt, all(args)?, ms.iter().map(m).collect(), Box::new(m(own)))
            }
            MonoTy::Tuple(tys, own) => Ty::tuple(all(tys)?, m(own)),
            MonoTy::Array(own, elem) => {
                Ty::Array(Box::new(m(own)), Box::new(elem.to_ty_with(param, row)?))
            }
            MonoTy::Fn(ps, ret, own, r) => Ty::Fn(
                all(ps)?,
                Box::new(ret.to_ty_with(param, row)?),
                Box::new(m(own)),
                Box::new(row_ty(r)?),
            ),
            MonoTy::Row(r) => row_ty(r)?,
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
            MonoTy::Tuple(tys, own) => {
                if *own != MonoMut::Imm {
                    write!(f, "{own} ")?;
                }
                write!(f, "(")?;
                list(f, tys)?;
                if tys.len() == 1 {
                    write!(f, ",")?;
                }
                write!(f, ")")
            }
            MonoTy::Array(own, elem) => write!(f, "{own} [{}]", DisplayMonoTy { db, ty: elem }),
            MonoTy::Fn(params, ret, own, row) => {
                if *own != MonoMut::Imm {
                    write!(f, "{own} ")?;
                }
                write!(f, "fn(")?;
                list(f, params)?;
                write!(f, ") -> {}", DisplayMonoTy { db, ty: ret })?;
                if !row.is_nothing() {
                    write!(f, " does {}", row.display(db))?;
                }
                Ok(())
            }
            MonoTy::Row(row) => write!(f, "{}", row.display(db)),
        }
    }
}

/// `row` with each parameter replaced by the row `f` gives it, also in its
/// effects' arguments. [`MonoRow::map`] calls one of its two functions at a
/// time, so they can share `f`.
fn map_row<'db>(row: &MonoRow<'db>, f: &mut dyn FnMut(u32) -> MonoRow<'db>) -> MonoRow<'db> {
    let f = std::cell::RefCell::new(f);
    row.map(&mut |p| (f.borrow_mut())(p), &mut |t| t.map_rows(&mut **f.borrow_mut()))
}

impl<'db> MonoRow<'db> {
    /// The row as text: `Nothing`, or its effects and then its parameters,
    /// like `State[i32] + E0`.
    pub fn display(&self, db: &'db dyn Db) -> String {
        if self.is_nothing() {
            return "Nothing".into();
        }
        let effects = self.effects.iter().map(|e| {
            let name = e.effect.item(db).name.as_ref().map_or("?", |n| n.as_str()).to_string();
            if e.args.is_empty() {
                return name;
            }
            let args: Vec<String> = e.args.iter().map(|t| t.display(db).to_string()).collect();
            format!("{name}[{}]", args.join(", "))
        });
        let params = self.params.iter().map(|i| format!("E{i}"));
        effects.chain(params).collect::<Vec<_>>().join(" + ")
    }

    /// The row parameters in `ty`, in the order they appear, each once.
    pub fn params_in(ty: &MonoTy<'db>, out: &mut Vec<u32>) {
        ty.map_rows(&mut |i| {
            if !out.contains(&i) {
                out.push(i);
            }
            MonoRow::param(i)
        });
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

/// A type whose parameters are `params`, like the signature of an
/// operation in those of its effect, as a closed type, with `args` for
/// them and the impls visible from `root`.
pub fn closed_ty<'db>(
    db: &'db dyn Db,
    root: File,
    params: &[GenericParamId<'db>],
    args: &[MonoTy<'db>],
    ty: &Ty<'db>,
) -> Result<MonoTy<'db>, TyError> {
    let slots: Vec<Slot<'db>> = args.iter().cloned().map(Slot::Ty).collect();
    Subst { db, root, params, slots: &slots }.ty(ty)
}

/// What a parameter of a scheme is in an instance: a type, or one of the
/// instance's mutabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Slot<'db> {
    Ty(MonoTy<'db>),
    Mut(MonoMut),
    /// A row of effects: instances aren't specialized on rows, so it's one
    /// with the instance's row parameters in it.
    Row(MonoRow<'db>),
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
                // A row that's a type argument, like the `R` of
                // `Cont[B, U, R]`.
                Some(Slot::Row(r)) => MonoTy::Row(r.clone()),
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
            Ty::Tuple(tys, own) => MonoTy::Tuple(all(tys)?, self.mutability(own)?),
            Ty::Array(own, elem) => MonoTy::Array(self.mutability(own)?, Box::new(self.ty(elem)?)),
            Ty::Fn(params, ret, own, effects) => MonoTy::Fn(
                all(params)?,
                Box::new(self.ty(ret)?),
                self.mutability(own)?,
                self.row(effects)?,
            ),
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
            // A row as a type argument, as for a parameter above.
            Ty::Row(..) => MonoTy::Row(self.row(ty)?),
            Ty::Mutability(_) | Ty::Unwritten | Ty::Number(_) | Ty::Error => {
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
        // A mutability parameter of the instance that no slot is, like the
        // `'0` of `C = '0 Vec[i32]`, gets a slot of its own, so that it
        // stays a parameter in what the impl gives, like the `'v` of
        // `VecIter[T, 'v]`, instead of becoming `imm`.
        let mut extra: Vec<Slot<'db>> = Vec::new();
        for t in &args {
            t.map_muts(&mut |m| {
                let slot = Slot::Mut(m);
                if matches!(m, MonoMut::Param(_))
                    && !self.slots.contains(&slot)
                    && !extra.contains(&slot)
                {
                    extra.push(slot);
                }
                m
            });
        }
        if !extra.is_empty() {
            let slots: Vec<Slot<'db>> = self.slots.iter().cloned().chain(extra).collect();
            let wider = Subst { slots: &slots, ..*self };
            return wider.assoc_of(alias, &args);
        }
        self.assoc_of(alias, &args)
    }

    /// The associated type `alias` of closed `args`, which `back` can give
    /// to the checker.
    fn assoc_of(
        &self,
        alias: TypeAliasId<'db>,
        args: &[MonoTy<'db>],
    ) -> Result<MonoTy<'db>, TyError> {
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
        let at = |slot: Slot<'db>, none: Ty<'db>| -> Ty<'db> {
            match self.slots.iter().position(|s| *s == slot) {
                Some(at) if at < self.params.len() => Ty::Param(self.params[at]),
                Some(at) => Ty::Bound((at - self.params.len()) as u32),
                None => none,
            }
        };
        let param = |i: u32| at(Slot::Mut(MonoMut::Param(i)), Ty::Mutability(Mutability::Imm));
        let row = |i: u32| at(Slot::Row(MonoRow::param(i)), Ty::nothing());
        t.to_ty_with(&param, Some(&row)).ok_or(TyError::Unknown)
    }

    /// A row of effects of the body, in the instance.
    pub fn row(&self, ty: &Ty<'db>) -> Result<MonoRow<'db>, TyError> {
        Ok(match ty {
            Ty::Row(effects, rest) => {
                let effects = effects
                    .iter()
                    .map(|e| {
                        let args = e.args.iter().map(|t| self.ty(t)).collect::<Result<_, _>>()?;
                        Ok(MonoEffect { effect: e.effect, args })
                    })
                    .collect::<Result<Vec<_>, TyError>>()?;
                let mut row = MonoRow::new(effects, []);
                for part in rest.iter() {
                    row = row.union(&self.row(part)?);
                }
                row
            }
            Ty::Param(_) | Ty::Bound(_) => match self.slot(ty) {
                Some(Slot::Row(r)) => r.clone(),
                Some(Slot::Ty(MonoTy::Row(r))) => r.clone(),
                _ => return Err(TyError::Unknown),
            },
            // A row that nothing decided has no effects, its least solution;
            // and the checker reported the error.
            Ty::Var(_) | Ty::Error | Ty::Never => MonoRow::nothing(),
            _ => return Err(TyError::Unknown),
        })
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
