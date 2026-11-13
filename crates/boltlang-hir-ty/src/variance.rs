//! How the arguments of a struct or enum can change (#440): the variance of
//! each of its type and mutability parameters, from where its fields use
//! them, as `Ty.varIn` in `lean/BoltMut/Variance.lean` says.
//!
//! A parameter keeps the direction of the place it's in when it's in a
//! tuple, or in an object that's `imm`, `ro`, or whose mutability is the
//! struct's own or a parameter that can become `ro`: what they hold can be
//! seen as a supertype (`Sub.ro`, `Sub.immCell`). A function's parameters
//! turn it the other way round, and what it gives keeps it (`Sub.fn`). In
//! a `mut` object, or one whose mutability is a parameter that can't
//! change, it can't change at all: what's there is read and written. So
//! `T` is covariant in `struct Box[T] { item: T }`, contravariant in
//! `struct Handler[T] { f: fn(T) -> () }`, and invariant in a struct that
//! has both.
//!
//! A mutability parameter can only become `ro`, and only in objects that
//! are seen as they are: not as a closure's mutability, nor in a function's
//! parameters. The struct's own mutability isn't a parameter here: the
//! whole struct is an object of it, so its arguments can only change when
//! the struct isn't `mut` ([`arg_variance`]).
//!
//! A struct whose fields have another uses that one's variance, with its
//! arguments in place. For structs that refer to each other, that's a least
//! fixed point: they start out using nothing, and Salsa iterates until it
//! stops changing, as for [`crate::holds`].

use boltlang_base_db::Db;
use boltlang_hir_def::{
    ids::{EnumId, StructId},
    resolver::GenericDefId,
};

use crate::{
    lower::{first_hidden, raw_field_types},
    ty::{AdtId, Mutability, Ty},
};

/// How an argument can change where a parameter is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Variance {
    /// Not used, so it can change any way.
    Unused,
    /// As the place it's in: to a supertype where a supertype fits.
    Co,
    /// The other way round, like a function's parameters.
    Contra,
    /// Not at all.
    Inv,
}

impl Variance {
    /// Used both ways is used as neither.
    pub fn join(self, other: Variance) -> Variance {
        match (self, other) {
            (Variance::Unused, v) | (v, Variance::Unused) => v,
            (a, b) if a == b => a,
            _ => Variance::Inv,
        }
    }

    /// The other way round.
    pub fn flip(self) -> Variance {
        match self {
            Variance::Co => Variance::Contra,
            Variance::Contra => Variance::Co,
            v => v,
        }
    }

    /// A use that varies as `inner` in a place that varies as `self`.
    pub fn then(self, inner: Variance) -> Variance {
        match inner {
            Variance::Unused => Variance::Unused,
            Variance::Co => self,
            Variance::Contra => self.flip(),
            Variance::Inv => Variance::Inv,
        }
    }

    /// Whether an argument here can become a supertype, like `ro`.
    pub fn keeps_direction(self) -> bool {
        matches!(self, Variance::Unused | Variance::Co)
    }
}

/// Where the variances of structs and enums are found: the database, or
/// nowhere, where every argument is invariant, as for unit tests of the
/// unifier.
#[derive(Clone, Copy, Default)]
pub struct Variances<'db>(Option<&'db dyn Db>);

impl std::fmt::Debug for Variances<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "Variances" } else { "Variances::none()" })
    }
}

impl<'db> Variances<'db> {
    pub fn of(db: &'db dyn Db) -> Self {
        Variances(Some(db))
    }

    /// Whether the values of the struct `s` are always `imm`, as for an
    /// `imm struct` like `Char`: its view is itself. Without a database,
    /// none is.
    pub fn always_imm(self, s: boltlang_hir_def::ids::StructId<'db>) -> bool {
        self.0.is_some_and(|db| s.item(db).is_imm)
    }

    pub fn none() -> Self {
        Variances(None)
    }

    /// The variance of the `index`th parameter of `adt`: its type
    /// parameters, then the mutabilities it declares.
    pub fn get(self, adt: AdtId<'db>, index: usize) -> Variance {
        match self.0 {
            Some(db) => adt_variance(db, adt).get(index).copied().unwrap_or(Variance::Inv),
            None => Variance::Inv,
        }
    }
}

/// How the `index`th argument of a value of `adt` whose own mutability is
/// `own` can change. Only one that isn't `mut` lets it: the whole struct is
/// an object of its own mutability, and what a `mut` object holds is read
/// and written. An enum is a value, always `imm`.
pub fn arg_variance<'db>(
    variances: Variances<'db>,
    adt: AdtId<'db>,
    own: &Ty<'db>,
    index: usize,
) -> Variance {
    match own {
        Ty::Mutability(Mutability::Imm | Mutability::ReadOnly) => variances.get(adt, index),
        _ => Variance::Inv,
    }
}

/// The variance of each parameter of `adt`: its type parameters, then the
/// mutabilities it declares.
pub fn adt_variance<'db>(db: &'db dyn Db, adt: AdtId<'db>) -> &'db [Variance] {
    match adt {
        AdtId::Struct(s) => struct_variance(db, s),
        AdtId::Enum(e) => enum_variance(db, e),
    }
}

#[salsa::tracked(returns(ref), cycle_initial = struct_variance_initial)]
fn struct_variance<'db>(db: &'db dyn Db, s: StructId<'db>) -> Box<[Variance]> {
    variance_of(db, AdtId::Struct(s))
}

/// Structs that refer to themselves start out using nothing.
fn struct_variance_initial<'db>(
    db: &'db dyn Db,
    _id: salsa::Id,
    s: StructId<'db>,
) -> Box<[Variance]> {
    unused(db, GenericDefId::Struct(s))
}

#[salsa::tracked(returns(ref), cycle_initial = enum_variance_initial)]
fn enum_variance<'db>(db: &'db dyn Db, e: EnumId<'db>) -> Box<[Variance]> {
    variance_of(db, AdtId::Enum(e))
}

fn enum_variance_initial<'db>(db: &'db dyn Db, _id: salsa::Id, e: EnumId<'db>) -> Box<[Variance]> {
    unused(db, GenericDefId::Enum(e))
}

fn unused<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Box<[Variance]> {
    vec![Variance::Unused; first_hidden(db, def) as usize].into()
}

/// The variances from the fields of `adt`. Whether a mutability parameter
/// can become `ro` decides how what's in an object of it can change, so
/// it's a fixed point too: each round uses the last one's, starting from
/// every parameter unused, and only adds to them.
fn variance_of<'db>(db: &'db dyn Db, adt: AdtId<'db>) -> Box<[Variance]> {
    let def = match adt {
        AdtId::Struct(s) => GenericDefId::Struct(s),
        AdtId::Enum(e) => GenericDefId::Enum(e),
    };
    let fields = raw_field_types(db, adt);
    let mut current = unused(db, def).into_vec();
    loop {
        let mut walk = Walk {
            db,
            def,
            types: def.generic_params(db).type_params.len(),
            own: first_hidden(db, def) as usize,
            current: &current,
            out: vec![Variance::Unused; current.len()],
        };
        for field in &fields {
            walk.ty(field, Variance::Co);
        }
        if walk.out == current {
            return current.into();
        }
        current = walk.out;
    }
}

/// One round of [`variance_of`]: where the fields use each parameter.
struct Walk<'a, 'db> {
    db: &'db dyn Db,
    def: GenericDefId<'db>,
    /// How many type parameters there are; the mutabilities it declares
    /// come after them, and its own mutability is `own`.
    types: usize,
    own: usize,
    /// The variances of the last round.
    current: &'a [Variance],
    out: Vec<Variance>,
}

impl<'db> Walk<'_, 'db> {
    /// The parameter `ty` is, if it's one of this struct's other than its
    /// own mutability.
    fn param(&self, ty: &Ty<'db>) -> Option<usize> {
        match ty {
            Ty::Param(p) if p.def == self.def && (p.index as usize) < self.own => {
                Some(p.index as usize)
            }
            _ => None,
        }
    }

    fn add(&mut self, index: usize, v: Variance) {
        self.out[index] = self.out[index].join(v);
    }

    /// A mutability used as that of an object or closure, in a place that
    /// varies as `v`. It can only become `ro` where it's seen as it is.
    fn mutability(&mut self, m: &Ty<'db>, v: Variance) {
        if v == Variance::Unused {
            return;
        }
        if let Some(i) = self.param(m) {
            self.add(i, if v == Variance::Co { Variance::Co } else { Variance::Inv });
        }
    }

    /// How what an object of mutability `m` holds varies, in a place that
    /// varies as `v`: as it is in one that isn't `mut`, or whose mutability
    /// can become `ro`, and not at all otherwise.
    fn inside(&self, m: &Ty<'db>, v: Variance) -> Variance {
        match m {
            Ty::Mutability(Mutability::Imm | Mutability::ReadOnly) => v,
            Ty::Param(p) if p.def == self.def && p.index as usize == self.own => v,
            _ => match self.param(m) {
                Some(i) if self.current[i].keeps_direction() => v,
                _ => Variance::Inv,
            },
        }
    }

    /// Where `ty`, in a place that varies as `v`, uses each parameter. In
    /// an argument of a parameter that isn't used, nothing is.
    fn ty(&mut self, ty: &Ty<'db>, v: Variance) {
        if v == Variance::Unused {
            return;
        }
        match ty {
            Ty::Param(_) => match self.param(ty) {
                Some(i) if i < self.types => self.add(i, v),
                Some(i) => self.add(i, Variance::Inv),
                None => {}
            },
            Ty::Array(m, elem) => {
                self.mutability(m, v);
                self.ty(elem, self.inside(m, v));
            }
            Ty::Tuple(tys, m) => {
                self.mutability(m, v);
                let inner = self.inside(m, v);
                tys.iter().for_each(|t| self.ty(t, inner));
            }
            Ty::Record(tys, named, m) => {
                self.mutability(m, v);
                let inner = self.inside(m, v);
                tys.iter().chain(named.iter().map(|(_, t)| t)).for_each(|t| self.ty(t, inner));
            }
            // What another struct holds varies as it does, inside an object
            // of its mutability; an enum is a value.
            Ty::Adt(adt, args, ms, m) => {
                let inner = match adt {
                    AdtId::Struct(_) => {
                        self.mutability(m, v);
                        self.inside(m, v)
                    }
                    AdtId::Enum(_) => v,
                };
                let theirs = adt_variance(self.db, *adt);
                let of = |i: usize| theirs.get(i).copied().unwrap_or(Variance::Inv);
                for (i, arg) in args.iter().enumerate() {
                    self.ty(arg, inner.then(of(i)));
                }
                for (j, mu) in ms.iter().enumerate() {
                    self.mutability(mu, inner.then(of(args.len() + j)));
                }
            }
            // A closure's own mutability is kept as it is, unless it's the
            // struct's (#320).
            Ty::Fn(params, ret, m, effects) => {
                self.mutability(m, Variance::Inv);
                params.tys().for_each(|t| self.ty(t, v.flip()));
                self.ty(ret, v);
                self.ty(effects, v);
            }
            // `ro T` varies as `T`: seen as `ro` twice is seen as `ro` once.
            Ty::Ro(inner) => self.ty(inner, v),
            Ty::Row(effects, rest) => {
                effects.iter().flat_map(|e| e.args.iter()).for_each(|t| self.ty(t, Variance::Inv));
                rest.iter().for_each(|t| self.ty(t, v));
            }
            // What's taken apart later, like `imm T`, `'m T` or `F[T]`, can't vary.
            Ty::Imm(_) | Ty::WithMut(..) | Ty::App(..) | Ty::Assoc(..) | Ty::Spread(..) => {
                ty.walk_children(|t| self.ty(t, Variance::Inv))
            }
            Ty::Var(_)
            | Ty::Bound(_)
            | Ty::Builtin(_)
            | Ty::Host(_)
            | Ty::Mutability(_)
            | Ty::Unwritten
            | Ty::ParamName(_)
            | Ty::Number(_)
            | Ty::Never
            | Ty::Error => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use boltlang_base_db::TestDb;
    use boltlang_hir_def::item_tree::{ModItem, item_tree};

    use super::*;
    use Variance::*;

    /// The variance of each struct and enum in `source`, in order.
    fn variances(source: &str) -> Vec<Vec<Variance>> {
        let db = TestDb::new();
        let file = db.file("/main.bolt", source);
        let tree = item_tree(&db, file);
        tree.top_level
            .iter()
            .filter_map(|item| match item {
                ModItem::Struct(idx) => Some(AdtId::Struct(StructId::new(&db, file, *idx))),
                ModItem::Enum(idx) => Some(AdtId::Enum(EnumId::new(&db, file, *idx))),
                _ => None,
            })
            .map(|adt| adt_variance(&db, adt).to_vec())
            .collect()
    }

    #[test]
    fn held_is_covariant() {
        assert_eq!(variances("struct Box[T] { item: T }"), [vec![Co]]);
        assert_eq!(variances("struct Frozen[T] { items: imm [T] }"), [vec![Co]]);
        assert_eq!(variances("struct Seen[T] { items: ro [T] }"), [vec![Co]]);
        assert_eq!(variances("struct Vec[T] { items: [T], len: i32 }"), [vec![Co]]);
        assert_eq!(variances("struct Pair[A, B] { a: A, b: (B, i32) }"), [vec![Co, Co]]);
    }

    #[test]
    fn closures_take_contravariantly_and_give_covariantly() {
        assert_eq!(variances("struct Handler[T] { f: fn(T) -> () }"), [vec![Contra]]);
        assert_eq!(variances("struct Lazy[T] { get: fn() -> T }"), [vec![Co]]);
        assert_eq!(variances("struct Both[T] { a: T, f: fn(T) -> () }"), [vec![Inv]]);
        assert_eq!(variances("struct Twice[T] { f: fn(fn(T) -> ()) -> () }"), [vec![Co]]);
    }

    #[test]
    fn unused_parameters() {
        assert_eq!(variances("struct Tag[T] { id: i32 }"), [vec![Unused]]);
        // What's in an argument of a parameter that isn't used isn't used
        // either, mutabilities and closures too.
        assert_eq!(
            variances(
                "struct Tag[T] { id: i32 }
                 struct Outer['v, T] { a: Tag['v [i32]], b: Tag['v fn(T) -> ()], c: Tag[imm T] }"
            ),
            [vec![Unused], vec![Unused, Unused]]
        );
    }

    #[test]
    fn mutability_parameters() {
        // `'v` can become `ro`, and `T` with it.
        assert_eq!(variances("struct VecIter[T, 'v] { data: 'v [T], index: i32 }"), [vec![Co, Co]]);
        // As a closure's mutability, `'v` can't, so neither can what's in
        // an object of it.
        assert_eq!(
            variances("struct Keep[T, 'v] { data: 'v [T], f: 'v fn() -> () }"),
            [vec![Inv, Inv]]
        );
    }

    #[test]
    fn other_structs_and_enums() {
        assert_eq!(
            variances(
                "struct Box[T] { item: T }
                 struct Handler[T] { f: fn(T) -> () }
                 struct Outer[A, B] { a: Box[A], b: Handler[B] }"
            ),
            [vec![Co], vec![Contra], vec![Co, Contra]]
        );
        assert_eq!(
            variances(
                "enum Opt[T] { No, Yes(T) }
                 struct List[T] { head: T, tail: Opt[List[T]] }"
            ),
            [vec![Co], vec![Co]]
        );
    }
}
