//! Lowering a decision graph to Conduit.

use std::collections::{BTreeSet, HashMap};

use boltlang_hir_def::hir;
use boltlang_mir::{ExprId, Local};
use boltlang_mir_mono::MonoTy;
use boltlang_pattern::decision::{
    Bound, Case, Ctor, Decision, Node, NodeId, Place, PlaceId, Value,
};
use conduit_ir::{Atom, CtorId, Lit, Mutability, Prim, Ty, build::JoinPoint};

use super::{FAIL, Lower, R, Val, field_ty, int_lit, parse_float, tuple_field};
use crate::{
    ErrorKind,
    ty::{Scalar, none_index},
};

/// Lowering one decision. A node that more than one test leads to is a
/// join point, so that the code after it is made once. Its parameters are
/// the fields of variants it reads that it can't read itself, since a
/// field of a variant can only be read where a `case` found the variant.
pub(super) struct DecisionLower<'d, 'db> {
    d: &'d Decision<'db>,
    /// Whether a value no arm takes goes to the callback, as [`FAIL`].
    pub(super) fail_arm: bool,
    scrut: Val,
    /// The guards of the arms of a `match`.
    pub(super) guards: Vec<Option<ExprId>>,
    place_tys: Vec<MonoTy<'db>>,
    joins: HashMap<NodeId, (JoinPoint, Vec<PlaceId>)>,
}

impl<'d, 'db> DecisionLower<'d, 'db> {
    pub(super) fn new<'a, 'm>(
        l: &mut Lower<'a, 'm, 'db>,
        d: &'d Decision<'db>,
        scrut: Val,
        t: MonoTy<'db>,
    ) -> R<Self> {
        let mut place_tys: Vec<MonoTy<'db>> = Vec::new();
        for place in &d.places {
            let ty = match place {
                Place::Scrutinee => t.clone(),
                Place::Field { of, ctor, index } => {
                    let of_ty = &place_tys[*of as usize];
                    field_ty(l, of_ty, *ctor, *index)?
                }
            };
            place_tys.push(ty);
        }
        Ok(DecisionLower {
            d,
            fail_arm: false,
            scrut,
            guards: Vec::new(),
            place_tys,
            joins: HashMap::new(),
        })
    }

    pub(super) fn run<'a, 'm>(
        &mut self,
        l: &mut Lower<'a, 'm, 'db>,
        arm: &mut dyn FnMut(&mut Lower<'a, 'm, 'db>, u32) -> R<()>,
    ) -> R<()> {
        let d = self.d;
        // How many tests lead to each node, and an order in which a node
        // comes after every node it leads to.
        let mut preds = vec![0u32; d.nodes.len()];
        let mut order = Vec::new();
        let mut seen = vec![false; d.nodes.len()];
        let mut stack = vec![(d.root, false)];
        while let Some((n, done)) = stack.pop() {
            if done {
                order.push(n);
                continue;
            }
            if seen[n as usize] {
                continue;
            }
            seen[n as usize] = true;
            stack.push((n, true));
            for c in successors(&d.nodes[n as usize]) {
                preds[c as usize] += 1;
                stack.push((c, false));
            }
        }
        let shared: Vec<NodeId> =
            order.iter().copied().filter(|&n| preds[n as usize] > 1 && n != d.root).collect();
        // The join points, the nodes that the others lead to first, so that
        // each is in scope where it's jumped to.
        for &n in &shared {
            let params = self.params(n);
            let mut tys = Vec::new();
            let mut kept = Vec::new();
            for &p in &params {
                if let Some(t) = l.cty(&self.place_tys[p as usize])? {
                    tys.push(t);
                    kept.push(p);
                }
            }
            let jp = l.b.join(&tys);
            self.joins.insert(n, (jp, kept));
        }
        self.emit(l, d.root, &mut HashMap::new(), arm)?;
        for &n in &shared {
            let (jp, params) = &self.joins[&n];
            l.b.switch_to(jp.block());
            let mut env: HashMap<PlaceId, Val> = HashMap::new();
            for (&p, &v) in params.iter().zip(jp.params()) {
                env.insert(p, Some(Atom::Var(v)));
            }
            self.emit_node(l, n, &mut env, arm)?;
        }
        Ok(())
    }

    /// The places a join point for `n` needs: the fields of variants read
    /// at or after it, whose variant was found before it.
    fn params(&self, n: NodeId) -> Vec<PlaceId> {
        let d = self.d;
        let mut used = BTreeSet::new();
        let mut switched = BTreeSet::new();
        let mut stack = vec![n];
        let mut seen = BTreeSet::new();
        while let Some(m) = stack.pop() {
            if !seen.insert(m) {
                continue;
            }
            let node = &d.nodes[m as usize];
            match node {
                Node::Switch { place, .. } => {
                    used.insert(*place);
                    switched.insert(*place);
                }
                Node::Equals { place, .. } | Node::Range { place, .. } => {
                    used.insert(*place);
                }
                Node::Guard { bindings, .. } | Node::Arm { bindings, .. } => {
                    used.extend(bindings.iter().map(|(_, p)| *p));
                }
                Node::Fail => {}
            }
            stack.extend(successors(node));
        }
        let mut params = BTreeSet::new();
        for p in used {
            let mut at = p;
            loop {
                match d.places[at as usize] {
                    Place::Scrutinee => break,
                    Place::Field { of, ctor, .. } => {
                        if matches!(ctor, Ctor::Variant(_)) && !switched.contains(&of) {
                            params.insert(at);
                            break;
                        }
                        at = of;
                    }
                }
            }
        }
        params.into_iter().collect()
    }

    /// The value of a place: read once on each path.
    fn place<'a, 'm>(
        &self,
        l: &mut Lower<'a, 'm, 'db>,
        p: PlaceId,
        env: &mut HashMap<PlaceId, Val>,
    ) -> R<Val> {
        if let Some(v) = env.get(&p) {
            return Ok(v.clone());
        }
        let v = match self.d.places[p as usize] {
            Place::Scrutinee => self.scrut.clone(),
            Place::Field { of, ctor, index } => {
                let obj = self.place(l, of, env)?;
                let of_ty = &self.place_tys[of as usize];
                let (ctor, index) = tuple_field(l.cx.db, of_ty, ctor, index)?;
                // The `x` of a `Some(x)` that's a `nullable host`.
                if l.nullable_host(of_ty) {
                    let obj = obj.ok_or_else(|| ErrorKind::unsupported("this host value"))?;
                    let v = Some(l.host_unwrap(obj));
                    env.insert(p, v.clone());
                    return Ok(v);
                }
                let Some(Ty::Data(id, ..)) = l.cty(of_ty)? else {
                    return Err(ErrorKind::unsupported("a field of this type"));
                };
                let c = match ctor {
                    Ctor::Variant(v) => v.index,
                    Ctor::Tuple(_) | Ctor::TupleEnd | Ctor::TupleNamed(_) | Ctor::Struct(_) => 0,
                };
                // A field of a type with no values is none.
                match (obj, l.cty(&self.place_tys[p as usize])?) {
                    (Some(obj), Some(_)) => {
                        Some(Atom::Var(l.b.get(CtorId { data: id, index: c }, index, obj)))
                    }
                    _ => None,
                }
            }
        };
        env.insert(p, v.clone());
        Ok(v)
    }

    fn emit<'a, 'm>(
        &self,
        l: &mut Lower<'a, 'm, 'db>,
        n: NodeId,
        env: &mut HashMap<PlaceId, Val>,
        arm: &mut dyn FnMut(&mut Lower<'a, 'm, 'db>, u32) -> R<()>,
    ) -> R<()> {
        if let Some((jp, params)) = self.joins.get(&n) {
            let mut args = Vec::new();
            for &p in params {
                args.extend(self.place(l, p, env)?);
            }
            l.b.jump(jp, args);
            return Ok(());
        }
        self.emit_node(l, n, env, arm)
    }

    fn emit_node<'a, 'm>(
        &self,
        l: &mut Lower<'a, 'm, 'db>,
        n: NodeId,
        env: &mut HashMap<PlaceId, Val>,
        arm: &mut dyn FnMut(&mut Lower<'a, 'm, 'db>, u32) -> R<()>,
    ) -> R<()> {
        match &self.d.nodes[n as usize] {
            Node::Switch { place, cases, default } => {
                let t = &self.place_tys[*place as usize];
                let v = self
                    .place(l, *place, env)?
                    .ok_or_else(|| ErrorKind::unsupported("this match"))?;
                // A `Char` is tested by its code point.
                let v = match (&t, cases.first()) {
                    (MonoTy::Adt(..), Some((Case::Char(_), _))) => {
                        let Some(Ty::Data(id, ..)) = l.cty(t)? else {
                            return Err(ErrorKind::unsupported("this match"));
                        };
                        Atom::Var(l.b.get(CtorId { data: id, index: 0 }, 0, v))
                    }
                    _ => v,
                };
                // Conduit's `case` is on an `i32`, so an 8- or 16-bit integer
                // is widened, by its sign, and its tags are its values.
                // A `nullable host` is tested by whether it's null, `None`.
                let nullable = l.nullable_host(t);
                let v = if nullable { l.host_is_null(v) } else { v };
                let v = match Scalar::of(t)? {
                    Some(sc @ Scalar::Int { bits: 8 | 16, .. }) => {
                        Atom::Var(l.b.convert(Ty::I32, sc.sign(), v))
                    }
                    _ => v,
                };
                let data = match l.cty(t)? {
                    Some(Ty::Data(id, ..))
                        if !matches!(cases.first(), Some((Case::Char(_), _))) && !nullable =>
                    {
                        Some(id)
                    }
                    _ => None,
                };
                let case = l.b.case(v);
                for (c, child) in cases {
                    match (c, data) {
                        (Case::Variant(variant), Some(id)) => {
                            l.b.arm(case, CtorId { data: id, index: variant.index });
                        }
                        (Case::Variant(variant), None) if nullable => {
                            let null = variant.index == none_index(l.cx.db, variant.parent);
                            l.b.arm(case, null as i64);
                        }
                        (Case::Variant(variant), None) => {
                            l.b.arm(case, variant.index as i64);
                        }
                        (Case::Bool(b), _) => {
                            l.b.arm(case, *b as i64);
                        }
                        (Case::Int(i), _) => {
                            l.b.arm(case, *i as i64);
                        }
                        (Case::Char(ch), _) => {
                            l.b.arm(case, *ch as u32 as i64);
                        }
                    }
                    self.emit(l, *child, &mut env.clone(), arm)?;
                }
                match default {
                    Some(other) => {
                        l.b.default(case);
                        self.emit(l, *other, &mut env.clone(), arm)?;
                    }
                    None if data.is_none() => {
                        // The cases cover every value, but an `i32` has more.
                        l.b.default(case);
                        l.b.unreachable();
                    }
                    None => {}
                }
                Ok(())
            }
            Node::Equals { place, value, then, otherwise } => {
                let s = Scalar::of(&self.place_tys[*place as usize])?;
                let v = self
                    .place(l, *place, env)?
                    .ok_or_else(|| ErrorKind::unsupported("this match"))?;
                let x = match (value, s) {
                    (Value::Float(text), Some(Scalar::F32)) => Lit::f32(parse_float(text)? as f32),
                    (Value::Float(text), Some(Scalar::F64)) => Lit::f64(parse_float(text)?),
                    (Value::Str(_), _) => return Err(ErrorKind::unsupported("string patterns")),
                    _ => return Err(ErrorKind::unsupported("this pattern")),
                };
                let c = l.b.prim(Prim::Eq, [v, Atom::Lit(x)]);
                self.branch(l, Atom::Var(c), *then, *otherwise, env, arm)
            }
            Node::Range { place, start, end, inclusive, then, otherwise } => {
                let t = &self.place_tys[*place as usize];
                let mut v = self
                    .place(l, *place, env)?
                    .ok_or_else(|| ErrorKind::unsupported("this match"))?;
                let s = match Scalar::of(t)? {
                    Some(s) => s,
                    None => {
                        // A `Char`, by its code point.
                        let Some(Ty::Data(id, ..)) = l.cty(t)? else {
                            return Err(ErrorKind::unsupported("this pattern"));
                        };
                        v = Atom::Var(l.b.get(CtorId { data: id, index: 0 }, 0, v));
                        Scalar::Int { bits: 32, signed: false }
                    }
                };
                let sign = s.sign();
                let bound = |b: &Bound<'_>| -> R<Atom> {
                    Ok(match b {
                        Bound::Int(n) => int_lit(s, *n),
                        Bound::Char(c) => Atom::Lit(Lit::I32(*c as u32 as i32)),
                        Bound::Float(text) if s == Scalar::F32 => {
                            Atom::Lit(Lit::f32(parse_float(text)? as f32))
                        }
                        Bound::Float(text) => Atom::Lit(Lit::f64(parse_float(text)?)),
                        Bound::Const(_) => {
                            return Err(ErrorKind::unsupported("constants in ranges"));
                        }
                    })
                };
                let mut ok: Option<Atom> = None;
                if let Some(start) = start {
                    let c = l.b.prim(Prim::Ge(sign), [v.clone(), bound(start)?]);
                    ok = Some(Atom::Var(c));
                }
                if let Some(end) = end {
                    let p = if *inclusive { Prim::Le(sign) } else { Prim::Lt(sign) };
                    let c = Atom::Var(l.b.prim(p, [v.clone(), bound(end)?]));
                    ok = Some(match ok {
                        Some(a) => Atom::Var(l.b.prim(Prim::And, [a, c])),
                        None => c,
                    });
                }
                let ok = ok.unwrap_or(Atom::Lit(Lit::I32(1)));
                self.branch(l, ok, *then, *otherwise, env, arm)
            }
            Node::Guard { arm: index, bindings, then, otherwise, .. } => {
                self.bind(l, bindings, env)?;
                let Some(Some(guard)) = self.guards.get(*index as usize).copied() else {
                    return Err(ErrorKind::unsupported("a guard outside a `match`"));
                };
                let Some(c) = l.expr(guard)? else { return Ok(()) };
                self.branch(l, c, *then, *otherwise, env, arm)
            }
            Node::Arm { arm: index, bindings } => {
                self.bind(l, bindings, env)?;
                arm(l, *index)
            }
            Node::Fail if self.fail_arm => arm(l, FAIL),
            Node::Fail => {
                // Say so with the prelude's `panic`, then stop: reaching
                // this is defined, so it's a `trap`, not an `unreachable`.
                if let Some((panic, Ty::Data(id, m, margs, rows))) = &l.cx.panic {
                    let text = "no arm of a match matched";
                    let bytes: Vec<Atom> =
                        text.bytes().map(|b| Atom::Lit(Lit::I8(b as i8))).collect();
                    let xs = l.b.array_new(Ty::I8, Mutability::Imm, bytes);
                    let ctor = CtorId { data: *id, index: 0 };
                    let message = l.b.new_object_full(
                        ctor,
                        *m,
                        margs.clone(),
                        rows.clone(),
                        vec![Atom::Var(xs)],
                    );
                    l.b.call(*panic, [Atom::Var(message)]);
                }
                l.b.trap();
                Ok(())
            }
        }
    }

    fn bind<'a, 'm>(
        &self,
        l: &mut Lower<'a, 'm, 'db>,
        bindings: &[(hir::BindingId, PlaceId)],
        env: &mut HashMap<PlaceId, Val>,
    ) -> R<()> {
        for &(b, p) in bindings {
            let v = self.place(l, p, env)?;
            l.bind(Local::Binding(b), v)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn branch<'a, 'm>(
        &self,
        l: &mut Lower<'a, 'm, 'db>,
        c: Atom,
        then: NodeId,
        otherwise: NodeId,
        env: &mut HashMap<PlaceId, Val>,
        arm: &mut dyn FnMut(&mut Lower<'a, 'm, 'db>, u32) -> R<()>,
    ) -> R<()> {
        let case = l.b.case(c);
        l.b.arm(case, 0);
        self.emit(l, otherwise, &mut env.clone(), arm)?;
        l.b.default(case);
        self.emit(l, then, &mut env.clone(), arm)
    }
}

/// The nodes a node leads to.
fn successors(node: &Node<'_>) -> Vec<NodeId> {
    match node {
        Node::Switch { cases, default, .. } => {
            cases.iter().map(|(_, n)| *n).chain(default.iter().copied()).collect()
        }
        Node::Equals { then, otherwise, .. }
        | Node::Range { then, otherwise, .. }
        | Node::Guard { then, otherwise, .. } => vec![*then, *otherwise],
        Node::Arm { .. } | Node::Fail => Vec::new(),
    }
}
