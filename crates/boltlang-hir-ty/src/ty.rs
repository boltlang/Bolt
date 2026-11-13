//! The types the checker works with. Unlike the type references in
//! hir-def, they point at definitions instead of naming them.

use std::fmt;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    builtin_type::BuiltinType,
    hir::TupleItem,
    ids::{EffectId, EnumId, ItemContainerId, StructId, TypeAliasId},
    resolver::GenericDefId,
};

pub use boltlang_hir_def::hir::Mutability;

use crate::variance::Variances;

/// An inference variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct TyVar(pub u32);

/// What an inference variable can become. An integer literal without a
/// suffix has an `Int` variable, which defaults to `i32`. A `Mutability`
/// variable stands for the mutability of an array, and defaults to `Imm`.
/// An `Effect` variable stands for a row of effects, and defaults to the
/// empty row: a function that nothing says does something does nothing.
/// A pack variable stands for the positional or the named fields that a
/// pack spread in a tuple gives: it becomes a [`Ty::Tuple`] of their types,
/// or a [`Ty::Record`] of the named ones. A `Name` variable stands for the
/// name of a positional parameter, and becomes a [`Ty::ParamName`]: see
/// [`Unifier::settle_names`](crate::unify::Unifier::settle_names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum VarKind {
    General,
    Int,
    Float,
    Mutability,
    Effect,
    PositionalPack,
    NamedPack,
    Name,
}

/// The parameters of a function type: `(x: i32, *Ts, sep: String, **Ks)`.
/// The positional ones come first, each with its name, then a pack for the
/// other positional arguments, then the keyword-only ones in the order of
/// their names, then a pack for the other keyword arguments. A pack that's
/// solved is spliced in, as in a tuple: a function type has one form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct Params<'db> {
    pub positional: Box<[Ty<'db>]>,
    /// The name of each positional parameter, which it can be passed by:
    /// [`Ty::ParamName`], or a variable of kind [`VarKind::Name`] while it's
    /// inferred. As many as `positional`.
    pub names: Box<[Ty<'db>]>,
    /// `*Ts`: a positional pack.
    pub rest: Option<Box<Ty<'db>>>,
    pub keyword: Box<[(Name, Ty<'db>)]>,
    /// `**Ks`: a named pack.
    pub keyword_rest: Option<Box<Ty<'db>>>,
}

impl<'db> Params<'db> {
    /// Positional parameters without names, and nothing else.
    pub fn positional(tys: impl Into<Box<[Ty<'db>]>>) -> Self {
        let positional: Box<[Ty<'db>]> = tys.into();
        let names = positional.iter().map(|_| Ty::ParamName(None)).collect();
        Params { positional, names, rest: None, keyword: Box::new([]), keyword_rest: None }
    }

    /// The parameters, with the packs that are solved spliced in. `names`
    /// are those of `positional`; the parameters a pack becomes have none.
    pub fn new(
        positional: Vec<Ty<'db>>,
        names: Vec<Ty<'db>>,
        rest: Option<Ty<'db>>,
        keyword: Vec<(Name, Ty<'db>)>,
        keyword_rest: Option<Ty<'db>>,
    ) -> Self {
        debug_assert_eq!(positional.len(), names.len());
        // A pack that stands for what it's solved to is spliced in, through
        // a tuple; otherwise the parameters are what's given.
        let solved =
            |t: &Option<Ty<'db>>| matches!(t, Some(Ty::Tuple(..) | Ty::Record(..) | Ty::Error));
        if !solved(&rest) && !solved(&keyword_rest) {
            let mut keyword = keyword;
            keyword.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
            return Params {
                positional: positional.into(),
                names: names.into(),
                rest: rest.map(Box::new),
                keyword: keyword.into(),
                keyword_rest: keyword_rest.map(Box::new),
            };
        }
        let mut tys = positional;
        tys.extend(rest.map(|r| Ty::Spread(PackHalf::Positional, Box::new(r))));
        tys.extend(keyword_rest.map(|r| Ty::Spread(PackHalf::Named, Box::new(r))));
        let tuple = Ty::record(tys.into(), keyword, Ty::Mutability(Mutability::Imm));
        Params::from_tuple(&tuple).with_names(names)
    }

    /// The parameters with `names` for the first positional ones, and none
    /// for the others.
    pub fn with_names(mut self, names: Vec<Ty<'db>>) -> Self {
        let mut names = names.into_iter();
        self.names =
            self.positional.iter().map(|_| names.next().unwrap_or(Ty::ParamName(None))).collect();
        self
    }

    /// The parameters as a tuple type, which unifies like the arguments of
    /// a call: `(i32, *Ts, sep: String, **Ks)`. It isn't a value, so its
    /// mutability is [`Ty::Unwritten`], which isn't shown. The names of the
    /// positional parameters aren't in it.
    pub fn as_tuple(&self) -> Ty<'db> {
        let mut tys = self.positional.to_vec();
        tys.extend(self.rest.iter().map(|r| Ty::Spread(PackHalf::Positional, r.clone())));
        tys.extend(self.keyword_rest.iter().map(|r| Ty::Spread(PackHalf::Named, r.clone())));
        Ty::record(tys.into(), self.keyword.to_vec(), Ty::Unwritten)
    }

    /// The parameters of a tuple type made by [`Params::as_tuple`], or by
    /// unifying one: its first positional pack is the rest, and so is its
    /// first named one. The positional ones have no names.
    pub fn from_tuple(ty: &Ty<'db>) -> Self {
        let Some(parts) = ty.tuple_parts() else { return Params::positional([]) };
        let mut positional = Vec::new();
        let mut rest = None;
        for t in parts.positional {
            match t {
                Ty::Spread(_, pack) if rest.is_none() => rest = Some(pack),
                t => positional.push(t),
            }
        }
        Params {
            names: positional.iter().map(|_| Ty::ParamName(None)).collect(),
            positional: positional.into(),
            rest,
            keyword: parts.named.into(),
            keyword_rest: parts.rests.into_iter().next().map(Box::new),
        }
    }

    /// Whether it only has positional parameters, like most functions.
    pub fn is_positional(&self) -> bool {
        self.rest.is_none() && self.keyword.is_empty() && self.keyword_rest.is_none()
    }

    /// The name of the `i`th positional parameter, if it's known to have
    /// one.
    pub fn name(&self, i: usize) -> Option<&Name> {
        match self.names.get(i) {
            Some(Ty::ParamName(name)) => name.as_ref(),
            _ => None,
        }
    }

    /// Every type in it: the positional ones, the rest, the keyword ones
    /// and the rest of them. Not the names.
    pub fn tys(&self) -> impl Iterator<Item = &Ty<'db>> {
        self.positional
            .iter()
            .chain(self.rest.as_deref())
            .chain(self.keyword.iter().map(|(_, t)| t))
            .chain(self.keyword_rest.as_deref())
    }

    /// The parameters with each type replaced by what `f` gives, and the
    /// packs that become what they stand for spliced in. The names stay.
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        if self.is_positional() {
            let positional = self.positional.iter().map(&mut f).collect::<Box<[_]>>();
            return Params { positional, names: self.names.clone(), ..Params::positional([]) };
        }
        Params::new(
            self.positional.iter().map(&mut f).collect(),
            self.names.to_vec(),
            self.rest.as_deref().map(&mut f),
            self.keyword.iter().map(|(n, t)| (n.clone(), f(t))).collect(),
            self.keyword_rest.as_deref().map(&mut f),
        )
    }
}

/// Where an argument of a call goes, as [`place_args`] finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgPlace {
    /// Where it's written: a positional one in its place, and a named one
    /// by its name, as a keyword argument.
    Written,
    /// A named argument for the positional parameter with its name.
    Positional(usize),
    /// A named argument for a positional parameter that an argument before
    /// it is for already.
    Twice,
}

/// Where the arguments of a call go, as [`place_args`] finds it: a place
/// for each argument, and the positional parameters that no argument is
/// for, which get their defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgPlaces {
    pub places: Vec<ArgPlace>,
    pub defaulted: Vec<usize>,
}

/// Where each of the arguments `items` of a call goes, after `first`
/// arguments that come before them, like the receiver of `x.f(y)`, for a
/// function with `count` positional parameters, whose names `name` gives.
/// A named argument goes to the positional parameter with its name, if the
/// arguments before it fill the parameters before that one, or those have
/// defaults, as `has_default` says: `f(1, z: 3)` passes `z` by name when
/// `f(x, y = 2, z)`, and gives `y` its default. With a spread, the places
/// aren't known, so every argument goes where it's written, and nothing
/// gets a default.
pub fn place_args<T>(
    count: usize,
    name: impl Fn(usize) -> Option<Name>,
    has_default: impl Fn(usize) -> bool,
    first: usize,
    items: &[TupleItem<T>],
) -> ArgPlaces {
    let mut places = vec![ArgPlace::Written; items.len()];
    let mut defaulted = Vec::new();
    if items.iter().any(|i| matches!(i, TupleItem::Spread(..))) {
        return ArgPlaces { places, defaulted };
    }
    let given = first + items.iter().filter(|i| matches!(i, TupleItem::Positional(_))).count();
    let names: Vec<Option<Name>> = (0..count).map(name).collect();
    for (item, place) in items.iter().zip(&mut places) {
        if let TupleItem::Named(n, _) = item
            && names[..given.min(count)].iter().any(|m| m.as_ref() == Some(n))
        {
            *place = ArgPlace::Twice;
        }
    }
    for (i, name) in names.iter().enumerate().skip(given) {
        let found = items
            .iter()
            .position(|item| matches!(item, TupleItem::Named(n, _) if name.as_ref() == Some(n)));
        match found {
            Some(j) if places[j] == ArgPlace::Written => places[j] = ArgPlace::Positional(i),
            _ if has_default(i) => defaulted.push(i),
            _ => break,
        }
    }
    // A name given again, after the argument that went to its parameter.
    for j in 0..items.len() {
        let TupleItem::Named(n, _) = &items[j] else { continue };
        let placed = items.iter().zip(&places).any(|(item, place)| {
            matches!((item, place), (TupleItem::Named(m, _), ArgPlace::Positional(_)) if m == n)
        });
        if placed && places[j] == ArgPlace::Written {
            places[j] = ArgPlace::Twice;
        }
    }
    ArgPlaces { places, defaulted }
}

/// Which fields of a tuple a [`Ty::Spread`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum PackHalf {
    Positional,
    Named,
}

/// The parts of a tuple type, as [`Ty::tuple_parts`] gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TupleParts<'db> {
    /// The positional fields, with spreads of positional packs among them.
    pub positional: Vec<Ty<'db>>,
    /// The named fields, in the order of their names.
    pub named: Vec<(Name, Ty<'db>)>,
    /// The named packs spread in it.
    pub rests: Vec<Ty<'db>>,
    pub mutability: Ty<'db>,
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
    /// A type of the host, declared with `#[link(..)] type JsString;`: a
    /// value the host gives, like a JavaScript string, that Bolt can only
    /// hold and pass back. Each declaration is a type of its own, so two
    /// of them are never equal. Nothing in Bolt can change one, so it's
    /// always `imm`.
    Host(TypeAliasId<'db>),
    /// `(A, B)` and its mutability: like a struct with numeric fields, a
    /// `mut` tuple's fields can be assigned, and an `imm` one holds nothing
    /// `mut`. The unit type is the empty tuple, which is always `Imm`.
    Tuple(Box<[Ty<'db>]>, Box<Ty<'db>>),
    /// A tuple with named fields, like `(i32, x: i32)`: its positional
    /// fields, its named fields in the order of their names, and its
    /// mutability, as for a tuple. It has a named field, or a pack of them
    /// among its positional ones, like `(**R)`: a tuple without either is
    /// a [`Ty::Tuple`], so that each type has one form.
    Record(Box<[Ty<'db>]>, Box<[(Name, Ty<'db>)]>, Box<Ty<'db>>),
    /// Among the positional fields of a tuple, any number of fields that a
    /// pack stands for: positional ones, like the `*Ts` of `(T, *Ts)`, or
    /// named ones, like the `**R` of `(x: i32, **R)`, which come after the
    /// positional fields. The pack is a parameter, a variable or a bound
    /// variable: one that's solved is spliced in, since a solved
    /// positional pack is a [`Ty::Tuple`] of its types, and a solved named
    /// one a [`Ty::Record`] of its fields.
    Spread(PackHalf, Box<Ty<'db>>),
    /// An array: its mutability, then the type of its elements. The
    /// mutability is [`Ty::Mutability`], a variable, a parameter, or the
    /// error type. Lengths aren't checked yet.
    Array(Box<Ty<'db>>, Box<Ty<'db>>),
    /// `Mut` or `Imm`, the mutability of an array, which is a type of its
    /// own kind: see `site/docs/internals/designs/mutability/`.
    Mutability(Mutability),
    /// The mutability of an array type written without `mut` or `imm`, as
    /// lowered on its own. What it means depends on where the type is
    /// written, so the item that has the type replaces it with
    /// [`Ty::fill_unwritten`]: a parameter of a function, a variable in a
    /// body, or `Imm`. Only type aliases keep it, for the types they're
    /// used in to replace.
    Unwritten,
    /// The name of a positional parameter of a function type, like the `x`
    /// of `fn(x: i32)`, or `None` for one that has none. It's only in
    /// [`Params::names`]. Names aren't types: a function with names fits
    /// where fewer are asked for (see `site/docs/internals/compiler/type-checking/types.mdx`).
    ParamName(Option<Name>),
    /// `fn(A, B) -> C does E`: the parameters, the result, the mutability
    /// and the effects. A closure is `mut` if it can change what it
    /// captured, or capture something that can change, and `imm` otherwise;
    /// named functions capture nothing and are `imm`. As for arrays, an
    /// `imm` value can't hold a `mut fn`. The mutability doesn't restrict
    /// calling it. The effects are a row: [`Ty::Row`], a variable or a
    /// parameter.
    Fn(Box<Params<'db>>, Box<Ty<'db>>, Box<Ty<'db>>, Box<Ty<'db>>),
    /// A row of effects, which is a type of kind `Effect`: the effects that
    /// are known, like `Log + State[i32]`, and the rows it's made of that
    /// stand for any row, like the `E` of `Log + E`, which are parameters or
    /// variables bound by a scheme. Each effect is in it once, and both
    /// lists are in the order of [`Ty::row_of`]; the empty row is `Nothing`.
    /// A row made of rows still being inferred is a variable, and how it's
    /// made is a constraint (see `site/docs/internals/compiler/type-checking/effects.mdx`).
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
    /// `ro T` (#320): a `T` seen as `ro`, for a type whose view can't be
    /// taken apart yet, like a type parameter. A `T` can be used where a
    /// `ro T` is expected, and what's read through a `ro [T]` is a `ro T`,
    /// so a generic function can't give it back as a `T`. The view of a
    /// known type is that type with its arrays, tuples and structs `ro`
    /// ([`Ty::ro_view`]). In a body, the checker replaces it with a
    /// variable and a constraint, like `imm T`.
    Ro(Box<Ty<'db>>),
    /// `mut T` or `'m T` (#459): a `T` with the outer mutability `m`,
    /// whatever its own, for a type whose own can't be replaced yet, like a
    /// type parameter. The mutability is first. Once `T` is known, it's
    /// that type with `m` instead of its own ([`Ty::with_outer`]), `T`
    /// itself for one without a mutability, like `i32`, and its `ro` view
    /// when `m` is `ro`. Like `imm T`, it only equals itself, and in a body
    /// the checker replaces it with a variable and a constraint.
    WithMut(Box<Ty<'db>>, Box<Ty<'db>>),
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

/// How one row fits in another ([`Ty::row_fit`]): the effects of the one
/// that the other has too, paired with the other's, whose arguments must be
/// the same, and what it has that the other doesn't, as effects and as
/// rows that stand for any row.
#[derive(Debug, Default)]
pub struct RowFit<'a, 'db> {
    pub same: Vec<(&'a EffectTy<'db>, EffectTy<'db>)>,
    pub missing_effects: Vec<EffectTy<'db>>,
    pub missing_parts: Vec<Ty<'db>>,
}

impl RowFit<'_, '_> {
    /// Whether it fits: nothing is missing.
    pub fn fits(&self) -> bool {
        self.missing_effects.is_empty() && self.missing_parts.is_empty()
    }
}

impl<'db> Ty<'db> {
    pub fn unit() -> Self {
        Ty::tuple(Box::new([]), Ty::Mutability(Mutability::Imm))
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
            Box::new(Params::positional(params)),
            Box::new(ret),
            Box::new(Ty::Mutability(Mutability::Imm)),
            Box::new(effects),
        )
    }

    /// An `imm` function with the parameters `params`, from them to `ret`
    /// with the effects `effects`.
    pub fn func_with(params: Params<'db>, ret: Ty<'db>, effects: Ty<'db>) -> Self {
        Ty::Fn(
            Box::new(params),
            Box::new(ret),
            Box::new(Ty::Mutability(Mutability::Imm)),
            Box::new(effects),
        )
    }

    /// The type with each mutability inside it that isn't written seen as
    /// `ro`, as in a type written `ro` (#320), whose view is deep.
    pub fn ro_inside(&self, variances: Variances<'db>) -> Ty<'db> {
        self.ro_view(variances, true)
    }

    /// Each `ro T` in the type whose `T` is known replaced by the view of
    /// it (#320), and each `'m T` by `T` with the mutability `m` (#459). A
    /// `ro T` or `'m T` of what can't be taken apart, like a type parameter
    /// or a variable, stays.
    pub fn normalize_ro(&self, variances: Variances<'db>) -> Ty<'db> {
        if !self.any(&mut |t| matches!(t, Ty::Ro(_) | Ty::WithMut(..))) {
            return self.clone();
        }
        self.fold(&mut |t| {
            if let Ty::WithMut(m, inner) = t {
                let inner = inner.normalize_ro(variances);
                return Some(Ty::with_outer((**m).clone(), inner, variances));
            }
            let Ty::Ro(inner) = t else { return None };
            let inner = inner.normalize_ro(variances);
            Some(match inner {
                Ty::Param(_)
                | Ty::Bound(_)
                | Ty::App(..)
                | Ty::Assoc(..)
                | Ty::Imm(_)
                | Ty::Var(_)
                | Ty::Error => Ty::Ro(Box::new(inner)),
                inner => inner.ro_view(variances, false).normalize_ro(variances),
            })
        })
    }

    /// The type seen as `ro` (#320): its arrays, tuples and structs `ro`,
    /// what they hold seen the same way, and a type that can't be taken
    /// apart yet, like a type parameter, as `ro T`. With `unwritten`, only
    /// the mutabilities that aren't written are, as in a type written `ro`,
    /// where one written otherwise is kept, and reported if it can't be
    /// held. A closure stays as it is. The arguments of a struct or enum
    /// are seen as `ro` where its variance lets them become a supertype
    /// (#440), and kept elsewhere, like the `T` of
    /// `struct Handler[T] { f: fn(T) }`.
    pub fn ro_view(&self, variances: Variances<'db>, unwritten: bool) -> Ty<'db> {
        let ro = Ty::Mutability(Mutability::ReadOnly);
        let m = |m: &Ty<'db>| match unwritten && !matches!(m, Ty::Unwritten) {
            true => m.clone(),
            false => ro.clone(),
        };
        let view = |t: &Ty<'db>| t.ro_view(variances, unwritten);
        match self {
            Ty::Array(mu, elem) => Ty::array(m(mu), view(elem)),
            Ty::Tuple(tys, mu) => Ty::tuple(tys.iter().map(view).collect(), m(mu)),
            Ty::Record(tys, named, mu) => Ty::record(
                tys.iter().map(view).collect(),
                named.iter().map(|(n, t)| (n.clone(), view(t))).collect(),
                m(mu),
            ),
            Ty::Adt(id, args, ms, own) => {
                let varies = |i: usize| variances.get(*id, i).keeps_direction();
                let ms = ms
                    .iter()
                    .enumerate()
                    .map(|(j, mu)| if varies(args.len() + j) { m(mu) } else { mu.clone() })
                    .collect();
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(i, a)| if varies(i) { view(a) } else { a.clone() })
                    .collect();
                // An enum is a value, always `imm`, and so is an `imm struct`.
                let own = match id {
                    AdtId::Struct(s) if !variances.always_imm(*s) => m(own),
                    AdtId::Struct(_) | AdtId::Enum(_) => (**own).clone(),
                };
                Ty::Adt(*id, args, ms, Box::new(own))
            }
            Ty::Param(_) | Ty::Bound(_) | Ty::App(..) | Ty::Assoc(..) | Ty::Var(_) | Ty::Imm(_) => {
                Ty::Ro(Box::new(self.clone()))
            }
            _ => self.clone(),
        }
    }

    /// `ty` with the outer mutability `m`, as `'m T` is once `T` is known
    /// (#459): an array, struct, tuple or function with `m` instead of its
    /// own, seen as `ro` all the way down when `m` is `ro`, and a type
    /// without a mutability, like `i32` or an enum, as it is. A type that
    /// can't be taken apart yet, like a type parameter, is `'m T`, or
    /// `ro T` when `m` is `ro`.
    pub fn with_outer(m: Ty<'db>, ty: Ty<'db>, variances: Variances<'db>) -> Ty<'db> {
        let ro = matches!(m, Ty::Mutability(Mutability::ReadOnly));
        let replaced = match ty {
            Ty::WithMut(_, inner) => return Ty::with_outer(m, *inner, variances),
            Ty::Param(_)
            | Ty::Bound(_)
            | Ty::App(..)
            | Ty::Assoc(..)
            | Ty::Imm(_)
            | Ty::Ro(_)
            | Ty::Var(_) => {
                return match ro {
                    true => Ty::Ro(Box::new(ty)),
                    false => Ty::WithMut(Box::new(m), Box::new(ty)),
                };
            }
            Ty::Adt(AdtId::Struct(s), args, ms, _) if !variances.always_imm(s) => {
                Ty::Adt(AdtId::Struct(s), args, ms, Box::new(m))
            }
            Ty::Array(_, elem) => Ty::Array(Box::new(m), elem),
            Ty::Tuple(tys, _) if !tys.is_empty() => Ty::Tuple(tys, Box::new(m)),
            Ty::Record(tys, named, _) => Ty::Record(tys, named, Box::new(m)),
            Ty::Fn(params, ret, _, effects) => Ty::Fn(params, ret, Box::new(m), effects),
            ty => return ty,
        };
        match ro {
            true => replaced.ro_view(variances, false),
            false => replaced,
        }
    }

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

    /// How the row `self` fits in `sup`, when neither is a variable. A row
    /// fits in itself and in one made of it, and no effects fit anywhere.
    pub fn row_fit(&self, sup: &Ty<'db>) -> Option<RowFit<'_, 'db>> {
        if *self == Ty::Error || *sup == Ty::Error || self == sup {
            return Some(RowFit::default());
        }
        if let Ty::Row(_, rest) = sup
            && rest.contains(self)
        {
            return Some(RowFit::default());
        }
        let (effects, parts) = self.row_parts()?;
        let (allowed, allowed_parts) = sup.row_parts()?;
        let mut fit = RowFit::default();
        for e in effects {
            match allowed.iter().find(|a| a.effect == e.effect) {
                Some(a) => fit.same.push((e, a.clone())),
                None => fit.missing_effects.push(e.clone()),
            }
        }
        // A row that stands for any row fits only in a row made of it: a
        // parameter like `E` isn't known to be in any other row.
        fit.missing_parts = parts.into_iter().filter(|p| !allowed_parts.contains(p)).collect();
        Some(fit)
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
    /// A tuple of `tys`. The unit type is always `Imm`, so that it has
    /// one form.
    pub fn tuple(tys: Box<[Ty<'db>]>, mutability: Ty<'db>) -> Self {
        if tys.iter().any(|t| matches!(t, Ty::Spread(..))) {
            return Ty::record(tys, Vec::new(), mutability);
        }
        let mutability = if tys.is_empty() { Ty::Mutability(Mutability::Imm) } else { mutability };
        Ty::Tuple(tys, Box::new(mutability))
    }

    /// A tuple with the positional fields `tys` and the named fields
    /// `named`. Without named fields, or spreads of named packs, it's a
    /// tuple. A spread of a pack that's solved is spliced in, and a field
    /// given twice that way is kept twice, for unification to reject.
    pub fn record(
        tys: Box<[Ty<'db>]>,
        mut named: Vec<(Name, Ty<'db>)>,
        mutability: Ty<'db>,
    ) -> Self {
        let mut positional = Vec::new();
        let mut rests = Vec::new();
        let mut todo: Vec<Ty<'db>> = tys.into_vec();
        todo.reverse();
        while let Some(t) = todo.pop() {
            match t {
                Ty::Spread(PackHalf::Positional, pack) => match *pack {
                    Ty::Tuple(tys, _) => todo.extend(tys.into_vec().into_iter().rev()),
                    // What's unknown because of an error.
                    Ty::Error => {}
                    pack => positional.push(Ty::Spread(PackHalf::Positional, Box::new(pack))),
                },
                Ty::Spread(PackHalf::Named, pack) => match *pack {
                    Ty::Tuple(tys, _) if tys.is_empty() => {}
                    Ty::Record(tys, fields, _) => {
                        named.extend(fields.into_vec());
                        todo.extend(tys.into_vec().into_iter().rev());
                    }
                    Ty::Error => {}
                    pack => rests.push(Ty::Spread(PackHalf::Named, Box::new(pack))),
                },
                t => positional.push(t),
            }
        }
        if named.is_empty() && rests.is_empty() {
            let mutability =
                if positional.is_empty() { Ty::Mutability(Mutability::Imm) } else { mutability };
            return Ty::Tuple(positional.into(), Box::new(mutability));
        }
        named.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
        positional.extend(rests);
        Ty::Record(positional.into(), named.into(), Box::new(mutability))
    }

    /// The parts of a tuple type: its positional fields, with the spreads
    /// of positional packs among them, its named fields, the named packs
    /// spread in it, and its mutability. `None` for other types.
    pub fn tuple_parts(&self) -> Option<TupleParts<'db>> {
        let (tys, named, mutability): (&[Ty<'db>], &[(Name, Ty<'db>)], _) = match self {
            Ty::Tuple(tys, m) => (tys, &[], m),
            Ty::Record(tys, named, m) => (tys, named, m),
            _ => return None,
        };
        let mut parts = TupleParts {
            positional: Vec::new(),
            named: named.to_vec(),
            rests: Vec::new(),
            mutability: (**mutability).clone(),
        };
        for t in tys {
            match t {
                Ty::Spread(PackHalf::Named, pack) => parts.rests.push((**pack).clone()),
                t => parts.positional.push(t.clone()),
            }
        }
        Some(parts)
    }

    /// The tuple of `parts`.
    pub fn from_parts(parts: TupleParts<'db>) -> Self {
        let mut tys = parts.positional;
        tys.extend(parts.rests.into_iter().map(|r| Ty::Spread(PackHalf::Named, Box::new(r))));
        Ty::record(tys.into(), parts.named, parts.mutability)
    }

    /// Whether this is the spread of a pack, as a field of a tuple.
    pub fn is_spread(&self) -> bool {
        matches!(self, Ty::Spread(..))
    }

    /// Whether this is a tuple with a pack spread in it, like `(T, *Ts)`.
    pub fn has_spread(&self) -> bool {
        match self {
            Ty::Tuple(tys, _) | Ty::Record(tys, ..) => {
                tys.iter().any(|t| matches!(t, Ty::Spread(..)))
            }
            _ => false,
        }
    }

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
            Ty::Array(mutability, _)
            | Ty::Adt(AdtId::Struct(_), _, _, mutability)
            | Ty::Tuple(_, mutability)
            | Ty::Record(_, _, mutability) => Some(mutability),
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
        matches!(self, Ty::Tuple(tys, _) if tys.is_empty())
    }

    /// Call `f` on every type directly inside this one.
    pub fn walk_children(&self, mut f: impl FnMut(&Ty<'db>)) {
        match self {
            Ty::Adt(_, args, mutabilities, mutability) => {
                args.iter().for_each(&mut f);
                mutabilities.iter().for_each(&mut f);
                f(mutability);
            }
            Ty::Tuple(args, mutability) => {
                args.iter().for_each(&mut f);
                f(mutability);
            }
            Ty::Record(args, named, mutability) => {
                args.iter().for_each(&mut f);
                named.iter().for_each(|(_, t)| f(t));
                f(mutability);
            }
            Ty::App(head, args) => {
                f(head);
                args.iter().for_each(f);
            }
            Ty::Array(mutability, elem) => {
                f(mutability);
                f(elem);
            }
            Ty::Fn(params, ret, mutability, effects) => {
                params.tys().for_each(&mut f);
                f(ret);
                f(mutability);
                f(effects);
            }
            Ty::Row(effects, rest) => {
                effects.iter().flat_map(|e| e.args.iter()).for_each(&mut f);
                rest.iter().for_each(f);
            }
            Ty::Assoc(_, args) => args.iter().for_each(f),
            Ty::Imm(ty) | Ty::Ro(ty) | Ty::Spread(_, ty) => f(ty),
            Ty::WithMut(m, ty) => {
                f(m);
                f(ty);
            }
            Ty::Var(_)
            | Ty::Builtin(_)
            | Ty::Host(_)
            | Ty::Mutability(_)
            | Ty::Unwritten
            | Ty::ParamName(_)
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
            Ty::Tuple(tys, mutability) => Ty::tuple(fold_all(tys, f), mutability.fold(f)),
            // A pack can become what it stands for, which is spliced in.
            Ty::Record(tys, named, mutability) => Ty::record(
                fold_all(tys, f),
                named.iter().map(|(n, t)| (n.clone(), t.fold(f))).collect(),
                mutability.fold(f),
            ),
            Ty::Spread(half, pack) => Ty::Spread(*half, Box::new(pack.fold(f))),
            Ty::Array(mutability, elem) => Ty::array(mutability.fold(f), elem.fold(f)),
            Ty::Fn(params, ret, mutability, effects) => Ty::Fn(
                Box::new({
                    let names = params.names.iter().map(|n| n.fold(f)).collect();
                    params.map(|t| t.fold(f)).with_names(names)
                }),
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
            Ty::Ro(ty) => Ty::Ro(Box::new(ty.fold(f))),
            Ty::WithMut(m, ty) => Ty::WithMut(Box::new(m.fold(f)), Box::new(ty.fold(f))),
            Ty::Var(_)
            | Ty::Builtin(_)
            | Ty::Host(_)
            | Ty::Mutability(_)
            | Ty::Unwritten
            | Ty::ParamName(_)
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

    /// Whether the error type is in this type, which means an error about
    /// it was already reported.
    pub(crate) fn has_error(&self) -> bool {
        self.any(&mut |t| *t == Ty::Error)
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
                Ty::Tuple(tys, m) => {
                    mutability(db, m, found);
                    tys.iter().for_each(|t| collect(db, t, found));
                }
                Ty::Record(tys, named, m) => {
                    mutability(db, m, found);
                    tys.iter().for_each(|t| collect(db, t, found));
                    named.iter().for_each(|(_, t)| collect(db, t, found));
                }
                Ty::Adt(_, args, ms, m) => {
                    mutability(db, m, found);
                    args.iter().for_each(|t| collect(db, t, found));
                    ms.iter().for_each(|m| mutability(db, m, found));
                }
                Ty::Fn(params, ret, m, e) => {
                    mutability(db, m, found);
                    params.tys().for_each(|t| collect(db, t, found));
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
            Ty::Mutability(Mutability::ReadOnly) => "ro".into(),
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
            Ty::Host(alias) => match &alias.item(db).name {
                Some(name) => write!(f, "{name}"),
                None => f.write_str("?"),
            },
            Ty::Tuple(tys, mutability) => {
                if !tys.is_empty() {
                    self.write_mutability(f, mutability)?;
                }
                f.write_str("(")?;
                list(f, tys)?;
                // `(A,)`, but `(*Ts)`, which isn't one field.
                if tys.len() == 1 && !matches!(tys[0], Ty::Spread(..)) {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            // The spreads of named packs after the named fields.
            Ty::Record(tys, named, mutability) => {
                self.write_mutability(f, mutability)?;
                let is_rest = |t: &&Ty<'_>| matches!(t, Ty::Spread(PackHalf::Named, _));
                let parts: Vec<String> = tys
                    .iter()
                    .filter(|t| !is_rest(t))
                    .map(|t| show(t).to_string())
                    .chain(named.iter().map(|(name, ty)| format!("{name}: {}", show(ty))))
                    .chain(tys.iter().filter(is_rest).map(|t| show(t).to_string()))
                    .collect();
                write!(f, "({})", parts.join(", "))
            }
            Ty::Spread(PackHalf::Positional, pack) => write!(f, "*{}", show(pack)),
            Ty::Spread(PackHalf::Named, pack) => write!(f, "**{}", show(pack)),
            Ty::Array(mutability, elem) => {
                self.write_mutability(f, mutability)?;
                write!(f, "[{}]", show(elem))
            }
            // On its own, like the `mut` of `Iterable[[i32], mut]` (#453).
            Ty::Mutability(Mutability::Mut) => f.write_str("mut"),
            Ty::Mutability(Mutability::Imm) => f.write_str("imm"),
            Ty::Mutability(Mutability::ReadOnly) => f.write_str("ro"),
            Ty::Unwritten => f.write_str("_"),
            Ty::ParamName(name) => f.write_str(name.as_ref().map_or("_", |n| n.as_str())),
            Ty::Fn(params, ret, mutability, effects) => {
                // An `imm` function is shown as just `fn(...)`, since most
                // are.
                let explicit = self.names.is_some_and(|n| n.explicit);
                if explicit || *mutability.as_ref() != Ty::Mutability(Mutability::Imm) {
                    self.write_mutability(f, mutability)?;
                }
                // `fn(i32, *Ts, sep: String, **Ks)`
                let parts: Vec<String> = params
                    .positional
                    .iter()
                    .enumerate()
                    .map(|(i, t)| match params.name(i) {
                        Some(name) => format!("{name}: {}", show(t)),
                        None => show(t).to_string(),
                    })
                    .chain(params.rest.iter().map(|r| format!("*{}", show(r))))
                    .chain(params.keyword.iter().enumerate().map(|(i, (name, t))| {
                        // A `*` when there's no positional pack before them.
                        let star = i == 0 && params.rest.is_none();
                        format!("{}{name}: {}", if star { "*, " } else { "" }, show(t))
                    }))
                    .chain(params.keyword_rest.iter().map(|r| format!("**{}", show(r))))
                    .collect();
                write!(f, "fn({}) -> {}", parts.join(", "), show(ret))?;
                // No effects, or effects that are unknown and not shared, are
                // shown as nothing.
                match self.shown_effects(effects) {
                    Some(row) => write!(f, " does {row}"),
                    None => Ok(()),
                }
            }
            Ty::Row(..) => write!(f, "{}", self.shown_effects(self.ty).unwrap_or("Nothing".into())),
            // A mutability parameter on its own, like the `'v` of
            // `Iterable[[T], 'v]` (#453), is shown as a mutability.
            Ty::Param(param)
                if param.index as usize >= param.def.generic_params(db).type_params.len() =>
            {
                let m = self.shown_mutability(self.ty);
                f.write_str(&m.unwrap_or_else(|| "'_".into()))
            }
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
                Ty::Fn(..) | Ty::Imm(_) | Ty::Ro(_) => write!(f, "imm ({})", show(ty)),
                _ => write!(f, "imm {}", show(ty)),
            },
            Ty::Ro(ty) => match ty.as_ref() {
                Ty::Fn(..) | Ty::Imm(_) | Ty::Ro(_) => write!(f, "ro ({})", show(ty)),
                _ => write!(f, "ro {}", show(ty)),
            },
            Ty::WithMut(m, ty) => {
                self.write_mutability(f, m)?;
                match ty.as_ref() {
                    Ty::Fn(..) | Ty::Imm(_) | Ty::Ro(_) | Ty::WithMut(..) => {
                        write!(f, "({})", show(ty))
                    }
                    _ => write!(f, "{}", show(ty)),
                }
            }
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
            Ty::Mutability(Mutability::ReadOnly) => Some("ro".into()),
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
