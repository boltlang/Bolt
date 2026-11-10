//! Converting a value between the type a use of an operation gives it and
//! the type the operation's declaration has, where the operation's own
//! type parameters are `any` (see [`MonoTy::Any`]).
//!
//! A `perform` of `fn wrap[T](x: T) -> Option[T]` at `T = i32` boxes its
//! argument, and its result, an `Option[any]`, becomes an `Option[i32]` by
//! matching it and unboxing what it holds. Values that can't change are
//! rebuilt this way: data, tuples and arrays, the last copied. A closure
//! is wrapped in one that converts its arguments and results. A `mut`
//! value with a type parameter of the operation in it can't be converted,
//! since the handler would get a copy of it rather than the value itself.
//!
//! An `Option` of a type of the host is a `nullable host`, not data, so
//! its conversion to an `Option[any]` makes `None` of a null and `Some`
//! of a host value, boxed, and the other way gives a null for `None`.
//!
//! Each conversion of data, an array or a closure is a function of its
//! own, so that a type that holds itself, like `List[T]`, is converted by
//! one that calls itself. They're all found before the bodies are lowered,
//! from the operations' uses, and built after.

use std::collections::{BTreeMap, HashMap};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{ids::EnumVariantId, signatures::enum_signature};
use boltlang_hir_ty::{AdtId, lower::VariantDef};
use boltlang_mir_mono::{MonoMut, MonoRow, MonoTy, field_types};
use conduit_ir::{
    Atom, CtorId, Func, FuncId, Lit, Module, MutParams, Mutability, Op, Sig, Ty, build::FuncBuilder,
};

use crate::{
    ErrorKind, finish,
    ty::{PLACEHOLDER, Types, is_nullable_host, none_index, placeholder},
};

/// How a value of one type becomes one of the other.
enum Step {
    /// It's already the same in Conduit.
    Same,
    Box,
    Unbox,
    /// A call of the conversion function of the two types.
    Call,
}

/// The conversion functions of a program, by the types they convert from
/// and to.
#[derive(Default)]
pub(crate) struct Convs<'db> {
    funcs: HashMap<(MonoTy<'db>, MonoTy<'db>), FuncId>,
    /// For a conversion of closures, the function of the closure it makes.
    wrappers: HashMap<(MonoTy<'db>, MonoTy<'db>), FuncId>,
    /// The conversions, in the order they were found.
    order: Vec<(MonoTy<'db>, MonoTy<'db>)>,
}

/// What finding and building conversions needs.
pub(crate) struct ConvCx<'a, 'db> {
    pub db: &'db dyn Db,
    pub root: File,
    pub types: &'a mut Types<'db>,
}

fn unsupported(what: &str) -> ErrorKind {
    ErrorKind::unsupported(what)
}

impl<'db> ConvCx<'_, 'db> {
    fn value(&mut self, t: &MonoTy<'db>) -> Result<Option<Ty>, ErrorKind> {
        self.types.value(self.db, self.root, t)
    }

    fn step(&mut self, from: &MonoTy<'db>, to: &MonoTy<'db>) -> Result<Step, ErrorKind> {
        // Without an operation's type parameter, a value is given as it is,
        // even where its mutability differs.
        if from == to || !from.has_any() && !to.has_any() {
            return Ok(Step::Same);
        }
        match (from, to) {
            (MonoTy::Any(_), MonoTy::Any(_)) => Ok(Step::Same),
            (_, MonoTy::Any(_)) => Ok(Step::Box),
            (MonoTy::Any(_), _) => Ok(Step::Unbox),
            _ if self.value(from)? == self.value(to)? => Ok(Step::Same),
            _ => Ok(Step::Call),
        }
    }

    /// The types of the fields of each constructor of `t`, a data type or
    /// a tuple, in the order of the constructors.
    fn ctor_fields(&self, t: &MonoTy<'db>) -> Result<Vec<Vec<MonoTy<'db>>>, ErrorKind> {
        let unknown = |_| unsupported("a field whose type isn't known");
        Ok(match t {
            MonoTy::Adt(AdtId::Enum(e), args, ..) => {
                let n = enum_signature(self.db, *e).variants.len() as u32;
                (0..n)
                    .map(|index| {
                        let v = VariantDef::Variant(EnumVariantId { parent: *e, index });
                        field_types(self.db, self.root, v, args, &[], MonoMut::Imm)
                    })
                    .collect::<Result<_, _>>()
                    .map_err(unknown)?
            }
            MonoTy::Adt(AdtId::Struct(s), args, ms, own) => {
                let v = VariantDef::Struct(*s);
                vec![field_types(self.db, self.root, v, args, ms, *own).map_err(unknown)?]
            }
            MonoTy::Tuple(elems, _) | MonoTy::Record(elems, ..) => vec![elems.to_vec()],
            _ => return Err(unsupported("converting this type")),
        })
    }
}

/// Whether `from` and `to` are the same kind of type, with the same
/// number of parts, so that one can be converted to the other part by
/// part.
fn same_shape(from: &MonoTy<'_>, to: &MonoTy<'_>) -> bool {
    match (from, to) {
        (MonoTy::Adt(a, xs, ..), MonoTy::Adt(b, ys, ..)) => a == b && xs.len() == ys.len(),
        (MonoTy::Tuple(xs, _), MonoTy::Tuple(ys, _)) => xs.len() == ys.len(),
        (MonoTy::Record(xs, a, _), MonoTy::Record(ys, b, _)) => xs.len() == ys.len() && a == b,
        (MonoTy::Array(..), MonoTy::Array(..)) => true,
        (MonoTy::Fn(ps, ..), MonoTy::Fn(qs, ..)) => ps.len() == qs.len(),
        _ => false,
    }
}

/// Why a value of type `t` can't be converted, if it can't.
fn check(t: &MonoTy<'_>) -> Result<(), ErrorKind> {
    if t.has_mut_param() {
        return Err(unsupported(
            "an operation's type parameter in a value whose mutability is a parameter",
        ));
    }
    let mut rows = false;
    t.map_rows(&mut |_| {
        rows = true;
        MonoRow::nothing()
    });
    if rows {
        return Err(unsupported(
            "an operation's type parameter in a closure whose effects are a parameter",
        ));
    }
    let changes = match t {
        MonoTy::Adt(AdtId::Struct(_), .., own) | MonoTy::Array(own, _) => *own != MonoMut::Imm,
        MonoTy::Tuple(elems, own) | MonoTy::Record(elems, _, own) => {
            !elems.is_empty() && *own != MonoMut::Imm
        }
        _ => false,
    };
    if changes {
        return Err(unsupported("an operation's type parameter in a `mut` value"));
    }
    Ok(())
}

impl<'db> Convs<'db> {
    /// Find the conversion functions that converting a value of type
    /// `from` to `to` needs, giving each a name in `names` and a signature
    /// in `sigs`.
    pub(crate) fn plan(
        &mut self,
        cx: &mut ConvCx<'_, 'db>,
        from: &MonoTy<'db>,
        to: &MonoTy<'db>,
        names: &mut BTreeMap<FuncId, String>,
        sigs: &mut Vec<(FuncId, (Sig, MutParams, u32))>,
    ) -> Result<(), ErrorKind> {
        if !matches!(cx.step(from, to)?, Step::Call) {
            return Ok(());
        }
        let key = (from.clone(), to.clone());
        if self.funcs.contains_key(&key) {
            return Ok(());
        }
        if !same_shape(from, to) {
            return Err(unsupported("converting between these types"));
        }
        check(from)?;
        check(to)?;
        let (Some(ft), Some(tt)) = (cx.value(from)?, cx.value(to)?) else {
            return Err(unsupported("converting a type with no values"));
        };
        let db = cx.db;
        let mut fresh = |name: String| {
            let id = FuncId(names.len() as u64);
            names.insert(id, name);
            id
        };
        let name = format!("convert[{}, {}]", from.display(db), to.display(db));
        let id = fresh(name.clone());
        self.funcs.insert(key.clone(), id);
        self.order.push(key.clone());
        sigs.push((id, (Sig::new(vec![ft.clone()], vec![tt]), MutParams::default(), 0)));
        match (from, to) {
            (MonoTy::Fn(ps, _, r, _, _), MonoTy::Fn(qs, _, s, _, row)) => {
                let wrapper = fresh(format!("{name}::closure"));
                self.wrappers.insert(key, wrapper);
                // It takes the closure it converts first.
                let mut sig = cx.types.fn_sig(cx.db, cx.root, qs, s, row)?;
                sig.params.insert(0, ft);
                sigs.push((wrapper, (sig, MutParams::default(), 0)));
                // Its arguments go the other way.
                for (p, q) in ps.iter().zip(qs.iter()) {
                    self.plan(cx, q, p, names, sigs)?;
                }
                self.plan(cx, r, s, names, sigs)?;
            }
            (MonoTy::Array(_, a), MonoTy::Array(_, b)) => self.plan(cx, a, b, names, sigs)?,
            _ => {
                let froms = cx.ctor_fields(from)?;
                let tos = cx.ctor_fields(to)?;
                for (fs, ts) in froms.iter().zip(&tos) {
                    for (f, t) in fs.iter().zip(ts) {
                        self.plan(cx, f, t, names, sigs)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `v`, of type `from`, as a value of type `to`, with what
    /// [`Convs::plan`] found.
    pub(crate) fn emit(
        &self,
        cx: &mut ConvCx<'_, 'db>,
        b: &mut FuncBuilder<'_, Module>,
        v: Option<Atom>,
        from: &MonoTy<'db>,
        to: &MonoTy<'db>,
    ) -> Result<Option<Atom>, ErrorKind> {
        Ok(match cx.step(from, to)? {
            Step::Same => v,
            // A value of a type with no values, like `()`, is boxed as a
            // placeholder.
            Step::Box => {
                let (t, v) = match (cx.value(from)?, v) {
                    (Some(t), Some(v)) => (t, v),
                    _ => (PLACEHOLDER, placeholder()),
                };
                Some(Atom::Var(b.boxed(t, v)))
            }
            Step::Unbox => match (cx.value(to)?, v) {
                (Some(t), Some(v)) => Some(Atom::Var(b.unbox(t, v))),
                _ => None,
            },
            Step::Call => {
                let key = (from.clone(), to.clone());
                let (Some(&f), Some(v)) = (self.funcs.get(&key), v) else {
                    return Err(unsupported("converting this value"));
                };
                Some(Atom::Var(b.call1(f, vec![v])))
            }
        })
    }

    /// The bodies of the conversion functions, and of the closures they
    /// make, once `module` has their signatures.
    pub(crate) fn build(
        &self,
        cx: &mut ConvCx<'_, 'db>,
        module: &Module,
    ) -> Result<Vec<(FuncId, Func)>, ErrorKind> {
        let mut out = Vec::new();
        for key in &self.order {
            let (from, to) = key;
            let id = self.funcs[key];
            let mut b = builder(module, id);
            let x = Atom::Var(b.param(0));
            match (from, to) {
                (MonoTy::Fn(ps, _, r, ..), MonoTy::Fn(qs, _, s, ..)) => {
                    let Some(Ty::Fn(_, m)) = cx.value(to)? else {
                        return Err(unsupported("converting this closure"));
                    };
                    let wrapper = self.wrappers[key];
                    let c = b.closure(wrapper, m, vec![x]);
                    b.ret(vec![Atom::Var(c)]);
                    out.push((id, finish(b)?));
                    out.push((wrapper, self.wrapper(cx, module, wrapper, ps, r, qs, s)?));
                    continue;
                }
                (MonoTy::Array(_, a), MonoTy::Array(_, e)) => {
                    self.array(cx, &mut b, x, to, a, e)?
                }
                _ if is_nullable_host(cx.db, cx.root, from)
                    || is_nullable_host(cx.db, cx.root, to) =>
                {
                    self.nullable_host(cx, &mut b, x, from, to)?
                }
                _ => self.data(cx, &mut b, x, from, to)?,
            }
            out.push((id, finish(b)?));
        }
        Ok(out)
    }

    /// Rebuild data or a tuple: match it, convert its fields, and make the
    /// same constructor of the other type with them.
    fn data(
        &self,
        cx: &mut ConvCx<'_, 'db>,
        b: &mut FuncBuilder<'_, Module>,
        x: Atom,
        from: &MonoTy<'db>,
        to: &MonoTy<'db>,
    ) -> Result<(), ErrorKind> {
        let (Some(Ty::Data(fid, ..)), Some(Ty::Data(tid, m, margs, rows))) =
            (cx.value(from)?, cx.value(to)?)
        else {
            return Err(unsupported("converting this data"));
        };
        let froms = cx.ctor_fields(from)?;
        let tos = cx.ctor_fields(to)?;
        let case = (froms.len() > 1).then(|| b.case(x.clone()));
        for (index, (fs, ts)) in froms.iter().zip(&tos).enumerate() {
            let (fc, tc) = (
                CtorId { data: fid, index: index as u32 },
                CtorId { data: tid, index: index as u32 },
            );
            if let Some(case) = case {
                b.arm(case, fc);
            }
            let fields = b.fields(fc, x.clone());
            let mut converted = Vec::new();
            for ((v, f), t) in fields.into_iter().zip(fs).zip(ts) {
                converted.extend(self.emit(cx, b, Some(Atom::Var(v)), f, t)?);
            }
            let obj = b.new_object_full(tc, m, margs.clone(), rows.clone(), converted);
            b.ret(vec![Atom::Var(obj)]);
        }
        Ok(())
    }

    /// Convert between an `Option` of a type of the host, a `nullable host`,
    /// and an `Option` of `any`, which is data.
    fn nullable_host(
        &self,
        cx: &mut ConvCx<'_, 'db>,
        b: &mut FuncBuilder<'_, Module>,
        x: Atom,
        from: &MonoTy<'db>,
        to: &MonoTy<'db>,
    ) -> Result<(), ErrorKind> {
        let MonoTy::Adt(AdtId::Enum(option), ..) = from else {
            return Err(unsupported("converting this `Option`"));
        };
        let none = none_index(cx.db, *option);
        let some = 1 - none;
        let field = |t: &MonoTy<'db>| match t {
            MonoTy::Adt(_, args, ..) => args[0].clone(),
            _ => unreachable!("an `Option`"),
        };
        let (f, t) = (field(from), field(to));
        if is_nullable_host(cx.db, cx.root, from) {
            let Some(Ty::Data(tid, m, margs, rows)) = cx.value(to)? else {
                return Err(unsupported("converting this `Option`"));
            };
            let null = b.op(Op::HostIsNull, [x.clone()])[0];
            let case = b.case(null);
            b.arm(case, 1);
            let ctor = CtorId { data: tid, index: none };
            let obj = b.new_object_full(ctor, m, margs.clone(), rows.clone(), Vec::<Atom>::new());
            b.ret(vec![Atom::Var(obj)]);
            b.default(case);
            let v = Atom::Var(b.op(Op::HostUnwrap, [x])[0]);
            let v = self.emit(cx, b, Some(v), &f, &t)?;
            let ctor = CtorId { data: tid, index: some };
            let obj = b.new_object_full(ctor, m, margs, rows, Vec::from_iter(v));
            b.ret(vec![Atom::Var(obj)]);
        } else {
            let Some(Ty::Data(fid, ..)) = cx.value(from)? else {
                return Err(unsupported("converting this `Option`"));
            };
            let case = b.case(x.clone());
            b.arm(case, CtorId { data: fid, index: none });
            let null = b.op(Op::HostNull, ())[0];
            b.ret(vec![Atom::Var(null)]);
            b.arm(case, CtorId { data: fid, index: some });
            let ctor = CtorId { data: fid, index: some };
            let v = Atom::Var(b.get(ctor, 0, x));
            let v = self.emit(cx, b, Some(v), &f, &t)?;
            b.ret(Vec::from_iter(v));
        }
        Ok(())
    }

    /// Copy an array, converting each element, into a new one.
    fn array(
        &self,
        cx: &mut ConvCx<'_, 'db>,
        b: &mut FuncBuilder<'_, Module>,
        xs: Atom,
        to: &MonoTy<'db>,
        from_elem: &MonoTy<'db>,
        to_elem: &MonoTy<'db>,
    ) -> Result<(), ErrorKind> {
        let Some(Ty::Array(elem, m)) = cx.value(to)? else {
            return Err(unsupported("converting this array"));
        };
        let n = b.array_len(xs.clone());
        let out = b.array_alloc(*elem, n);
        let next = b.join_rec(&[Ty::I32]);
        b.jump(&next, vec![Atom::Lit(Lit::I32(0))]);
        b.switch_to(next.block());
        let i = next.params()[0];
        let more = b.lt_s(i, n);
        let case = b.case(more);
        b.arm(case, 0);
        // An array of `any` is `mut` in Conduit (see `holding_any`).
        let done = match m {
            Mutability::Imm => b.array_freeze(out),
            _ => out,
        };
        b.ret(vec![Atom::Var(done)]);
        b.default(case);
        let x = b.array_get(xs, i);
        let y =
            self.emit(cx, b, Some(Atom::Var(x)), from_elem, to_elem)?.unwrap_or_else(placeholder);
        b.array_set(out, i, y);
        let i = b.add(i, Atom::Lit(Lit::I32(1)));
        b.jump(&next, vec![Atom::Var(i)]);
        Ok(())
    }

    /// The function of a closure that converts `f`, of type
    /// `fn(ps) -> r`, to one of type `fn(qs) -> s`: it converts its
    /// arguments to `ps`, applies `f`, and converts what that gives to `s`.
    #[allow(clippy::too_many_arguments)]
    fn wrapper(
        &self,
        cx: &mut ConvCx<'_, 'db>,
        module: &Module,
        id: FuncId,
        ps: &[MonoTy<'db>],
        r: &MonoTy<'db>,
        qs: &[MonoTy<'db>],
        s: &MonoTy<'db>,
    ) -> Result<Func, ErrorKind> {
        let mut b = builder(module, id);
        let f = b.param(0);
        let mut next = 1;
        let mut args = Vec::new();
        for (p, q) in ps.iter().zip(qs) {
            let v = match cx.value(q)? {
                Some(_) => {
                    next += 1;
                    Some(Atom::Var(b.param(next - 1)))
                }
                None => None,
            };
            args.extend(self.emit(cx, &mut b, v, q, p)?);
        }
        let results = b.apply(f, args);
        let result = results.first().map(|&v| Atom::Var(v));
        let result = self.emit(cx, &mut b, result, r, s)?;
        b.ret(Vec::from_iter(result));
        finish(b)
    }
}

fn builder(module: &Module, id: FuncId) -> FuncBuilder<'_, Module> {
    let sig = &module.funcs[&id];
    let mut b = FuncBuilder::new(module, sig.sig().params, sig.results.clone());
    b.set_row(sig.row.clone());
    b
}
