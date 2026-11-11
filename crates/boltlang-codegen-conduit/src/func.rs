//! Lowering the MIR of an instance to a Conduit function.

use std::collections::HashMap;

use boltlang_base_db::source_text;
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    expr_store::ExpressionStoreSourceMap,
    hir::{self, Literal, LogicOp},
    ids::{FunctionId, ItemContainerId},
};
use boltlang_hir_ty::{
    AdtId,
    builtin::{Builtin, PrimOp},
    lower::VariantDef,
};
use boltlang_mir::{
    Body, Callee, ExprId, ExprKind, ItemRef, LabelId, Local, Pattern, Place as MirPlace, Stmt,
};
use boltlang_mir_build::{lambda::Lambda, mir_bodies};
use boltlang_mir_mono::{Cx, InstanceDef, InstanceUses, MonoRow, MonoTy, Use, field_types};
use boltlang_mir_transform::shim_body;
use boltlang_pattern::decision::{Ctor, Decision, Node};
use conduit_ir::{
    Atom, CtorId, Func, Lit, Module, Mutability, Op, Prim, Row, Sign, Ty, build::FuncBuilder,
    build::JoinPoint,
};

use crate::{
    Ctx, Error, ErrorKind,
    conv::ConvCx,
    ty::{Scalar, Types, is_fieldless, is_nullable_host, mutability, placeholder},
};

mod decision;

use decision::DecisionLower;

type R<T> = Result<T, ErrorKind>;

/// The arm a decision's callback gets for a value that no arm matches,
/// when the decision is a `let`'s, whose `else` runs then.
const FAIL: u32 = u32::MAX;

/// A value: none for `()`, `!` and code that's never reached.
type Val = Option<Atom>;

/// Where a local is.
#[derive(Clone)]
enum LocalVal {
    Value(Val),
    /// A local that's assigned.
    Ref(conduit_ir::Var),
}

/// A loop or a labeled block that `break` can leave.
struct Target {
    label: LabelId,
    /// Where `break` goes, with the value if it gives one.
    exit: JoinPoint,
    /// Where `continue` goes, for a loop.
    next: Option<JoinPoint>,
}

/// Which function of an instance is lowered.
#[derive(Clone, Copy)]
pub(crate) enum Part<'a> {
    /// The instance's own.
    Body,
    /// That of one of its lambdas, lifted.
    Lambda(&'a Lambda),
    /// One of the functions of a `match` with effect arms, in the MIR body
    /// of that index.
    Handle(u32, ExprId, HandlePart),
}

/// What a function of a handler is.
#[derive(Clone, Copy)]
pub(crate) enum HandlePart {
    /// The scrutinee, as the closure the handler runs.
    Body,
    /// The value arms, as the handler's return clause.
    Ret,
    /// The arms of the `j`th operation of the `i`th effect it handles, as
    /// its clause.
    Clause(usize, usize),
}

/// The function of the instance `iu`, or one of its lambdas or handlers.
pub(crate) fn lower_body<'db>(
    cx: &Ctx<'db>,
    module: &Module,
    types: &mut Types<'db>,
    iu: &InstanceUses<'db>,
    part: Part<'_>,
) -> Result<Func, Error> {
    let db = cx.db;
    let error =
        |kind: ErrorKind| Error { instance: iu.instance.display(db).to_string(), at: None, kind };
    let def = iu.instance.source_body(db);
    let map = def.map(|def| &body_with_source_map(db, def).1);
    // A shim's MIR, which the compiler makes, rather than a body's.
    let (bodies, shim) = match iu.instance.def(db) {
        InstanceDef::Body(body) | InstanceDef::Default(body, _) => {
            (Some(mir_bodies(db, *body)), None)
        }
        InstanceDef::Shim(s) => (None, Some(shim_body(db, s, iu.instance.root(db)))),
    };
    // A default's MIR, and its number among the function's bodies.
    let default = match (iu.instance.def(db), bodies) {
        (InstanceDef::Default(_, param), Some(bodies)) => bodies.default(*param),
        _ => None,
    };
    let lcx = Cx::new(db, iu.instance, &cx.erased).map_err(|e| error(e.into()))?;
    // The MIR body a handler is in, and which of its functions this is.
    let (body_index, handle) = match part {
        Part::Body => (default.map_or(0, |(i, _)| i), None),
        Part::Lambda(lambda) => {
            let at = bodies.and_then(|b| {
                b.lambdas.iter().position(|m| {
                    matches!(m.owner, boltlang_mir::BodyOwner::Lambda { expr, .. } if expr == lambda.expr)
                })
            });
            (at.map_or(0, |i| i as u32 + 1), None)
        }
        Part::Handle(i, e, which) => (i, Some((e, which))),
    };
    let (id, mir) = match (bodies, &shim) {
        (_, Some(shim)) => (cx.funcs[&iu.instance], shim),
        (None, None) => unreachable!("an instance is of a body or a shim"),
        (Some(bodies), None) => match (part, handle) {
            (Part::Lambda(lambda), _) => {
                let mir = bodies
                    .lambda(lambda.expr)
                    .ok_or_else(|| error(ErrorKind::unsupported("a lambda without a body")))?;
                (cx.lambdas[&(iu.instance, lambda.expr)], mir)
            }
            (_, Some((e, which))) => {
                let fns = &cx.handles[&(iu.instance, body_index, e)];
                let id = match which {
                    HandlePart::Body => fns.body,
                    HandlePart::Ret => fns.ret,
                    HandlePart::Clause(effect, op) => fns.effects[effect].1[op],
                };
                let mir = bodies.nth(body_index).ok_or_else(|| {
                    error(ErrorKind::unsupported("a handler in a body that isn't there"))
                })?;
                (id, mir)
            }
            _ => (cx.funcs[&iu.instance], default.map_or(&bodies.body, |(_, mir)| mir)),
        },
    };
    let sig = &module.funcs[&id];
    let mut b = FuncBuilder::new(module, sig.sig().params, sig.results.clone());
    b.set_mut_params(sig.mut_params.clone());
    b.set_effect_params(sig.effect_params);
    b.set_row(sig.row.clone());
    let mut l = Lower {
        cx,
        lcx: &lcx,
        types,
        def,
        mir,
        map,
        b,
        is_lambda: matches!(part, Part::Lambda(_)),
        body_index,
        handle,
        locals: HashMap::new(),
        targets: Vec::new(),
        at: None,
        var_names: Default::default(),
    };
    if let Err(kind) = l.run() {
        return Err(Error { at: l.at.map(|e| l.snippet(e)), ..error(kind) });
    }
    let var_names = std::mem::take(&mut l.var_names);
    let mut f = crate::finish(l.b).map_err(error)?;
    f.debug.vars.extend(var_names.into_iter().map(|(v, n)| (v, n.into())));
    Ok(f)
}

struct Lower<'a, 'm, 'db> {
    cx: &'a Ctx<'db>,
    lcx: &'a Cx<'a, 'db>,
    types: &'a mut Types<'db>,
    /// The body in the source, unless this is a shim.
    def: Option<DefWithBodyId<'db>>,
    mir: &'a Body<'db>,
    map: Option<&'db ExpressionStoreSourceMap>,
    b: FuncBuilder<'m, Module>,
    /// Whether this is the function of a lambda, which takes its captures
    /// first.
    is_lambda: bool,
    /// Which MIR body of the instance this is: 0 for its own, and `i + 1`
    /// for its `i`th lambda's.
    body_index: u32,
    /// The `match` with effect arms whose function this is, if it's one.
    handle: Option<(ExprId, HandlePart)>,
    locals: HashMap<Local, LocalVal>,
    targets: Vec<Target>,
    /// The node being lowered, for errors.
    at: Option<ExprId>,
    /// The names of the variables of locals, for the text format.
    var_names: std::collections::BTreeMap<conduit_ir::Var, String>,
}

impl<'a, 'm, 'db> Lower<'a, 'm, 'db> {
    /// The source of a node, for errors: that of the HIR it came from.
    fn snippet(&self, e: ExprId) -> String {
        let db = self.cx.db;
        let (Some(def), Some(map)) = (self.def, self.map) else { return "?".to_string() };
        let text = source_text(db, def.def(db).file(db));
        self.mir[e]
            .origin
            .and_then(|o| map.expr_syntax(o))
            .map_or("?".to_string(), |p| text[p.start as usize..p.end as usize].to_string())
    }

    fn ended(&self) -> bool {
        self.b.is_ended(self.b.current())
    }

    /// The type of a node, in the instance.
    fn mono(&self, e: ExprId) -> R<MonoTy<'db>> {
        Ok(self.lcx.ty(&self.mir[e].ty)?)
    }

    fn mono_of(&self, t: &boltlang_hir_ty::Ty<'db>) -> R<MonoTy<'db>> {
        Ok(self.lcx.ty(t)?)
    }

    fn local_ty(&self, l: Local) -> R<MonoTy<'db>> {
        let decl =
            self.mir.local(l).ok_or_else(|| ErrorKind::unsupported("a local without a type"))?;
        self.mono_of(&decl.ty)
    }

    fn cty(&mut self, t: &MonoTy<'db>) -> R<Option<Ty>> {
        self.types.value(self.cx.db, self.cx.root, t)
    }

    /// What a use is in the instance.
    fn use_of(&self, item: &ItemRef<'db>, ty: &boltlang_hir_ty::Ty<'db>) -> R<Use<'db>> {
        let use_ty = self.mono_of(ty)?;
        Ok(self.lcx.item_use(item, &use_ty)?)
    }

    fn run(&mut self) -> R<()> {
        if let Some((e, which)) = self.handle {
            return self.run_handle(e, which);
        }
        let db = self.cx.db;
        let mut next = 0;
        // A lambda takes its captures first: a local that's assigned as its
        // ref.
        if self.is_lambda {
            for c in self.mir.captures.clone() {
                self.bind_param(c.local, c.by_ref, &mut next)?;
            }
        }
        // The parameters: each of a type with values is one of Conduit's.
        if !self.mir.param_kinds.is_empty() {
            self.bind_params(&mut next)?;
        } else {
            for local in self.mir.params.clone() {
                let p = self.next_param(&self.local_ty(local)?, &mut next)?;
                self.bind(local, p.map(Atom::Var))?;
            }
        }
        let result = self.expr(self.mir.root)?;
        if !self.ended() {
            // A constant, a function, a lambda or a shim: its value, if it
            // has one.
            match self.lcx.instance.body(db).map(|body| body.def(db)) {
                Some(DefWithBody::File(_)) if !self.is_lambda => self.b.ret(()),
                _ => self.b.ret(Vec::from_iter(result)),
            }
        }
        Ok(())
    }

    /// Bind the parameters of a function with keyword-only parameters,
    /// `*args` or `**opts` to what it's given, laid out flat as its function
    /// type is: the positional ones, then the arguments `*args` takes, then
    /// the keyword ones in the order of their names, of which `**opts`
    /// takes those that aren't parameters of their own. `next` is the
    /// next of Conduit's parameters.
    fn bind_params(&mut self, next: &mut usize) -> R<()> {
        use boltlang_hir_def::hir::ParamKind;
        let MonoTy::Fn(flat, names, ..) = self.lcx.instance_ty()? else {
            return Err(ErrorKind::unsupported("a function whose type isn't a function"));
        };
        let mut given = Vec::new();
        for t in flat.iter() {
            given.push(self.next_param(t, next)?.map(Atom::Var));
        }
        let split = flat.len() - names.len();
        let kinds = self.mir.param_kinds.clone();
        let positional = kinds.iter().filter(|(k, _)| *k == ParamKind::Positional).count();
        let keyword: Vec<&boltlang_hir_def::Name> = kinds
            .iter()
            .filter(|(k, _)| *k == ParamKind::Keyword)
            .filter_map(|(_, n)| n.as_ref())
            .collect();
        let named =
            |n: &boltlang_hir_def::Name| names.iter().position(|m| m == n).map(|i| split + i);
        let mut next_positional = 0;
        for (local, (kind, name)) in self.mir.params.clone().into_iter().zip(kinds.iter()) {
            let value = match kind {
                ParamKind::Positional => {
                    next_positional += 1;
                    given.get(next_positional - 1).cloned().flatten()
                }
                ParamKind::Keyword => name.as_ref().and_then(named).and_then(|i| given[i].clone()),
                // A tuple of what it takes.
                ParamKind::Rest => {
                    let fields = given[positional..split].iter().flatten().cloned().collect();
                    self.tuple_of(&self.local_ty(local)?, fields)?
                }
                ParamKind::KeywordRest => {
                    let fields = names
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| !keyword.contains(n))
                        .filter_map(|(i, _)| given[split + i].clone())
                        .collect();
                    self.tuple_of(&self.local_ty(local)?, fields)?
                }
            };
            self.bind(local, value)?;
        }
        Ok(())
    }

    /// A new tuple of type `t` with the fields `fields`, or none for `()`.
    fn tuple_of(&mut self, t: &MonoTy<'db>, fields: Vec<Atom>) -> R<Val> {
        match self.cty(t)? {
            Some(Ty::Data(id, m, margs, rows)) => {
                let ctor = CtorId { data: id, index: 0 };
                Ok(Some(Atom::Var(self.b.new_object_full(ctor, m, margs, rows, fields))))
            }
            None => Ok(None),
            Some(_) => Err(ErrorKind::unsupported("this tuple")),
        }
    }

    /// Bind the locals that a function of a handler is given first, each
    /// a parameter if its type has values: a local that's assigned as its
    /// ref.
    fn bind_given(&mut self, locals: &[Local], next: &mut usize) -> R<()> {
        for &local in locals {
            let mutable = self.mir.local(local).is_some_and(|d| d.mutable);
            self.bind_param(local, mutable, next)?;
        }
        Ok(())
    }

    /// Bind `local` to the next of Conduit's parameters, if its type has
    /// values, as its ref if `by_ref`.
    fn bind_param(&mut self, local: Local, by_ref: bool, next: &mut usize) -> R<()> {
        let slot = match self.next_param(&self.local_ty(local)?, next)? {
            Some(p) => {
                if let Some(name) = self.local_name(local) {
                    self.var_names.entry(p).or_insert(name);
                }
                if by_ref { LocalVal::Ref(p) } else { LocalVal::Value(Some(Atom::Var(p))) }
            }
            None => LocalVal::Value(None),
        };
        self.locals.insert(local, slot);
        Ok(())
    }

    /// The parameter `next` of Conduit's, which is then the one after it,
    /// for a value of type `t`, or none if `t` has no values.
    fn next_param(&mut self, t: &MonoTy<'db>, next: &mut usize) -> R<Option<conduit_ir::Var>> {
        if self.cty(t)?.is_none() {
            return Ok(None);
        }
        *next += 1;
        Ok(Some(self.b.param(*next - 1)))
    }

    /// A function of the handler of the `match` with effect arms `e`: its
    /// scrutinee, its value arms, or the arms of an operation.
    fn run_handle(&mut self, e: ExprId, which: HandlePart) -> R<()> {
        let db = self.cx.db;
        let mir = self.mir;
        let fns = &self.cx.handles[&(self.lcx.instance, self.body_index, e)];
        let ExprKind::Handle { body: scrutinee, value, arms, ops } = &mir[e].kind else {
            return Err(ErrorKind::unsupported("this handler"));
        };
        let mut next = 0;
        let ret = |l: &mut Self, v: Val| {
            if !l.ended() {
                l.b.ret(Vec::from_iter(v));
            }
        };
        match which {
            HandlePart::Body => {
                self.bind_given(&fns.captures, &mut next)?;
                let v = self.expr(*scrutinee)?;
                ret(self, v);
            }
            HandlePart::Ret => {
                self.bind_given(&fns.env, &mut next)?;
                let t = self.mono(*scrutinee)?;
                let v = match self.cty(&t)? {
                    Some(_) => Some(Atom::Var(self.b.param(next))),
                    None => None,
                };
                let guards: Vec<Option<ExprId>> = arms.iter().map(|a| a.guard).collect();
                let bodies: Vec<ExprId> = arms.iter().map(|a| a.body).collect();
                self.decide(value, v, t, false, &guards, &mut |l, arm| {
                    let v = l.expr(bodies[arm as usize])?;
                    ret(l, v);
                    Ok(())
                })?;
            }
            HandlePart::Clause(effect, index) => {
                self.bind_given(&fns.env, &mut next)?;
                let key = &fns.effects[effect].0;
                let op = self
                    .cx
                    .effects
                    .get(key)
                    .and_then(|(_, all)| all.get(index).copied())
                    .ok_or_else(|| ErrorKind::unsupported("this handler"))?;
                let op_arms = ops.iter().find(|o| o.op == op).ok_or_else(|| {
                    ErrorKind::unsupported("a handler without an arm for each operation")
                })?;
                // The arguments, matched as a tuple.
                let (tys, _) = crate::op_params(db, self.cx.root, key, op)?;
                let mut args = Vec::new();
                for t in &tys {
                    args.extend(self.next_param(t, &mut next)?.map(Atom::Var));
                }
                let k = Atom::Var(self.b.param(next));
                let t = crate::op_args_tuple(&tys);
                let scrutinee = if tys.is_empty() {
                    None
                } else {
                    let Some(Ty::Data(id, m, margs, rows)) = self.cty(&t)? else {
                        return Err(ErrorKind::unsupported("the arguments of this operation"));
                    };
                    let ctor = CtorId { data: id, index: 0 };
                    Some(Atom::Var(self.b.new_object_full(ctor, m, margs, rows, args)))
                };
                let guards: Vec<Option<ExprId>> = op_arms.arms.iter().map(|a| a.guard).collect();
                let bodies: Vec<ExprId> = op_arms.arms.iter().map(|a| a.body).collect();
                let conts = op_arms.conts.clone();
                self.decide(&op_arms.decision, scrutinee, t, false, &guards, &mut |l, arm| {
                    if let Some(c) = conts[arm as usize] {
                        l.bind(c, Some(k.clone()))?;
                    }
                    let v = l.expr(bodies[arm as usize])?;
                    ret(l, v);
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    fn local_name(&self, l: Local) -> Option<String> {
        let name = self.mir.local(l)?.name.as_ref()?;
        Some(crate::ident(name.as_str()))
    }

    /// A closure of the lifted lambda `lambda`, with the values it captures.
    fn closure(&mut self, e: ExprId, lambda: hir::ExprId) -> R<Val> {
        let f = self.cx.lambdas[&(self.lcx.instance, lambda)];
        let ExprKind::Closure { captures, .. } = &self.mir[e].kind else {
            return Err(ErrorKind::unsupported("this lambda"));
        };
        let values = self
            .passed(captures.iter().map(|c| c.local))
            .ok_or_else(|| ErrorKind::unsupported("a capture that isn't in scope"))?;
        let MonoTy::Fn(_, _, _, m, _) = self.mono(e)? else {
            return Err(ErrorKind::unsupported("a lambda whose type isn't a function"));
        };
        let (margs, rows) = (self.own_margs(), self.own_rows());
        Ok(Some(Atom::Var(self.b.closure_with(f, margs, rows, mutability(m), values))))
    }

    /// What a function that's given `locals` first gets for them: a local
    /// that's assigned as its ref. `None` if one isn't in scope.
    fn passed(&self, locals: impl IntoIterator<Item = Local>) -> Option<Vec<Atom>> {
        let mut out = Vec::new();
        for local in locals {
            match self.locals.get(&local)? {
                LocalVal::Value(Some(v)) => out.push(v.clone()),
                LocalVal::Ref(r) => out.push(Atom::Var(*r)),
                LocalVal::Value(None) => {}
            }
        }
        Some(out)
    }

    /// The instance's own mutability parameters, to give a lifted lambda,
    /// which has them too.
    fn own_margs(&self) -> Vec<Mutability> {
        (1..=self.lcx.instance.mut_params(self.cx.db) as u32).map(Mutability::Param).collect()
    }

    /// The instance's own row parameters, to give a lifted lambda or a
    /// handler, which have them too.
    fn own_rows(&self) -> Vec<Row> {
        (1..=self.lcx.instance.row_params(self.cx.db) as u32).map(Row::param).collect()
    }

    /// What a use gives the row parameters of an instance, in Conduit.
    fn rows(&mut self, rows: &[MonoRow<'db>]) -> Vec<Row> {
        rows.iter().map(|r| self.types.row(r)).collect()
    }

    fn bind(&mut self, local: Local, value: Val) -> R<()> {
        let mutable = self.mir.local(local).is_some_and(|d| d.mutable);
        let slot = if mutable {
            match value {
                Some(v) => LocalVal::Ref(self.b.ref_new(v)),
                None => LocalVal::Value(None),
            }
        } else {
            LocalVal::Value(value)
        };
        // The variable is shown by the local's name, unless it has one.
        let var = match &slot {
            LocalVal::Ref(v) | LocalVal::Value(Some(Atom::Var(v))) => Some(*v),
            LocalVal::Value(_) => None,
        };
        if let (Some(v), Some(name)) = (var, self.local_name(local)) {
            self.var_names.entry(v).or_insert(name);
        }
        self.locals.insert(local, slot);
        Ok(())
    }

    fn expr(&mut self, e: ExprId) -> R<Val> {
        let outer = self.at.replace(e);
        let v = self.expr_inner(e)?;
        if !self.ended() && matches!(self.mono(e)?, MonoTy::Never) {
            self.b.unreachable();
        }
        self.at = outer;
        Ok(if self.ended() { None } else { v })
    }

    /// The value of `e`, as an operand: an error for code that's never
    /// reached is the caller's to check with [`Lower::ended`].
    fn expr_inner(&mut self, e: ExprId) -> R<Val> {
        macro_rules! val {
            ($x:expr) => {{
                let v = $x?;
                if self.ended() {
                    return Ok(None);
                }
                v
            }};
        }
        let mir = self.mir;
        match &mir[e].kind {
            ExprKind::Literal(lit) => self.literal(e, lit),
            ExprKind::Bool(b) => Ok(Some(Atom::Lit(Lit::I32(*b as i32)))),
            ExprKind::Local(local) => match self.locals.get(local).cloned() {
                Some(LocalVal::Value(v)) => Ok(v),
                Some(LocalVal::Ref(r)) => Ok(Some(Atom::Var(self.b.ref_get(r)))),
                None => Err(ErrorKind::unsupported("a local that isn't in scope")),
            },
            ExprKind::Item(item) => self.item_value(e, item),
            ExprKind::Call { callee, args } => {
                if let ExprKind::Item(item) = &mir[*callee].kind {
                    let mut values = Vec::new();
                    for &a in args.iter() {
                        values.push(val!(self.expr(a)));
                    }
                    let result = self.mono(e)?;
                    return self.call_item(item, &mir[*callee].ty, values, &result);
                }
                // A value that's a function, like a closure: `apply` it.
                let f = val!(self.expr(*callee))
                    .ok_or_else(|| ErrorKind::unsupported("calling this"))?;
                let mut values = Vec::new();
                for &a in args.iter() {
                    values.push(val!(self.expr(a)));
                }
                let args: Vec<Atom> = values.into_iter().flatten().collect();
                let results = self.b.apply(f, args);
                if matches!(self.mono(e)?, MonoTy::Never) {
                    self.b.unreachable();
                }
                Ok(first(&results))
            }
            ExprKind::Construct { ctor, fields } => {
                let mut values = Vec::new();
                for &x in fields.iter() {
                    values.extend(val!(self.expr(x)));
                }
                match ctor {
                    Ctor::Tuple(0) => Ok(None),
                    Ctor::Tuple(_) | Ctor::Struct(_) => self.construct(e, 0, values),
                    Ctor::Variant(v) => self.variant(e, *v, values),
                    Ctor::TupleEnd | Ctor::TupleNamed(_) => {
                        Err(ErrorKind::unsupported("this tuple"))
                    }
                }
            }
            ExprKind::Array { elems } => {
                let mut values = Vec::new();
                for &x in elems.iter() {
                    values.push(val!(self.expr(x)).unwrap_or_else(placeholder));
                }
                let Some(Ty::Array(elem, m)) = self.cty(&self.mono(e)?)? else {
                    return Err(ErrorKind::unsupported("this array"));
                };
                Ok(Some(Atom::Var(self.b.array_new(*elem, m, values))))
            }
            ExprKind::ArrayRepeat { value, count } => {
                let v = val!(self.expr(*value)).unwrap_or_else(placeholder);
                let n = val!(self.expr(*count)).ok_or_else(|| ErrorKind::unsupported("this"))?;
                let Some(Ty::Array(elem, m)) = self.cty(&self.mono(e)?)? else {
                    return Err(ErrorKind::unsupported("this array"));
                };
                Ok(Some(Atom::Var(self.b.array_fill(*elem, m, n, v))))
            }
            ExprKind::Field { base, ctor, index } => {
                let t = self.mono(*base)?;
                let (ctor, index) = &tuple_field(self.cx.db, &t, *ctor, *index)?;
                let obj = val!(self.expr(*base));
                // The `x` of a `Some(x)` that's a `nullable host`.
                if self.nullable_host(&t) {
                    let obj = obj.ok_or_else(|| ErrorKind::unsupported("this host value"))?;
                    return Ok(Some(self.host_unwrap(obj)));
                }
                let mut data = self.data_ctor(&t)?;
                // A variant's field, which is read where a `case` found it.
                if let Ctor::Variant(v) = ctor {
                    data.index = v.index;
                }
                let ctor = data;
                let obj = obj.ok_or_else(|| ErrorKind::unsupported("a field of `()`"))?;
                Ok(Some(Atom::Var(self.b.get(ctor, *index, obj))))
            }
            ExprKind::Cast { expr: inner } => {
                let v = val!(self.expr(*inner));
                self.cast(*inner, e, v)
            }
            ExprKind::Closure { lambda, .. } => self.closure(e, *lambda),
            // A call of the deep copy of its type, unless `imm` doesn't change
            // it, when the value is shared as it is.
            ExprKind::Imm { expr: inner } => {
                let v = val!(self.expr(*inner));
                match self.lcx.imm_use(&mir[*inner].ty)? {
                    None => Ok(v),
                    Some(Use::Instance { instance, mutabilities, rows, dicts })
                        if dicts.is_empty() =>
                    {
                        let f = self.cx.funcs[&instance];
                        let margs = call_margs(&mutabilities)?;
                        let rows = self.rows(&rows);
                        let args = Vec::from_iter(v);
                        let results = self.b.call_with(f, margs, rows, args);
                        Ok(first(&results))
                    }
                    Some(_) => Err(ErrorKind::unsupported("this copy")),
                }
            }
            ExprKind::Block { stmts, tail, label } => {
                let exit = match label {
                    Some(label) => {
                        let tys: Vec<Ty> = self.cty(&self.mono(e)?)?.into_iter().collect();
                        let exit = self.b.join(&tys);
                        self.targets.push(Target { label: *label, exit, next: None });
                        true
                    }
                    None => false,
                };
                for s in stmts.iter() {
                    self.statement(s)?;
                    if self.ended() {
                        break;
                    }
                }
                let v = match tail {
                    Some(t) if !self.ended() => self.expr(*t)?,
                    _ => None,
                };
                if !exit {
                    return Ok(v);
                }
                let target = self.targets.pop().unwrap();
                if !self.ended() {
                    self.b.jump(&target.exit, Vec::from_iter(v));
                }
                self.b.switch_to(target.exit.block());
                Ok(first(target.exit.params()))
            }
            ExprKind::If { cond, then, else_ } => self.if_expr(e, *cond, *then, *else_),
            ExprKind::Logic { op, lhs, rhs } => {
                let l = val!(self.expr(*lhs)).ok_or_else(|| ErrorKind::unsupported("this"))?;
                let k = self.b.join(&[Ty::I32]);
                let case = self.b.case(l);
                // `a && b` is `b` when `a` is true, and `a || b` when it's false.
                let (short_tag, short) = match op {
                    LogicOp::And => (0, 0),
                    LogicOp::Or => (1, 1),
                };
                self.b.arm(case, short_tag);
                self.b.jump(&k, [short]);
                self.b.default(case);
                if let Some(r) = self.expr(*rhs)? {
                    self.b.jump(&k, [r]);
                }
                self.b.switch_to(k.block());
                Ok(Some(Atom::Var(k.param(0))))
            }
            ExprKind::Let { .. } => Err(ErrorKind::unsupported("`let` here")),
            // A `handle`, with the locals its clauses share, and a closure of
            // its scrutinee.
            ExprKind::Handle { .. } => {
                let fns = &self.cx.handles[&(self.lcx.instance, self.body_index, e)];
                let given = |l: &Self, locals: &[Local]| {
                    l.passed(locals.iter().copied())
                        .ok_or_else(|| ErrorKind::unsupported("a local that isn't in scope"))
                };
                let mut env = given(self, &fns.env)?;
                let captures = given(self, &fns.captures)?;
                let (margs, rows) = (self.own_margs(), self.own_rows());
                let body = self.b.closure_with(fns.body, margs, rows, Mutability::Mut, captures);
                env.push(Atom::Var(body));
                let results = self.b.handle_with(fns.handler, self.own_rows(), env);
                Ok(first(&results))
            }
            ExprKind::Match { scrutinee, decision, arms } => {
                let guards: Vec<Option<ExprId>> = arms.iter().map(|a| a.guard).collect();
                let bodies: Vec<ExprId> = arms.iter().map(|a| a.body).collect();
                self.match_expr(e, *scrutinee, decision, &guards, &bodies)
            }
            ExprKind::While { cond, body: inner, label } => {
                let exit = self.b.join(&[]);
                let next = self.b.join_rec(&[]);
                self.b.jump(&next, ());
                self.b.switch_to(next.block());
                self.targets.push(Target { label: *label, exit, next: Some(next) });
                let (exit_label, inner) = (self.targets.len() - 1, *inner);
                self.branch_on(
                    *cond,
                    &mut |l| {
                        l.expr(inner)?;
                        if !l.ended() {
                            l.b.jump(l.targets[exit_label].next.as_ref().unwrap(), ());
                        }
                        Ok(())
                    },
                    &mut |l| {
                        l.b.jump(&l.targets[exit_label].exit, ());
                        Ok(())
                    },
                )?;
                let target = self.targets.pop().unwrap();
                self.b.switch_to(target.exit.block());
                Ok(None)
            }
            ExprKind::For { pat, iterable, body: inner, label, iter, next } => {
                self.for_loop(pat, *iterable, *inner, *label, iter, next)
            }
            ExprKind::Loop { body: inner, label } => {
                let tys: Vec<Ty> = self.cty(&self.mono(e)?)?.into_iter().collect();
                let exit = self.b.join(&tys);
                let next = self.b.join_rec(&[]);
                self.b.jump(&next, ());
                self.b.switch_to(next.block());
                self.targets.push(Target { label: *label, exit, next: Some(next) });
                self.expr(*inner)?;
                let target = self.targets.pop().unwrap();
                if !self.ended() {
                    self.b.jump(target.next.as_ref().unwrap(), ());
                }
                self.b.switch_to(target.exit.block());
                Ok(first(target.exit.params()))
            }
            ExprKind::Break { label, value } => {
                let v = match value {
                    Some(x) => val!(self.expr(*x)),
                    None => None,
                };
                let i = self.target(*label, false)?;
                let exit = &self.targets[i].exit;
                self.b.jump(exit, Vec::from_iter(v));
                Ok(None)
            }
            ExprKind::Continue { label } => {
                let i = self.target(*label, true)?;
                let next = self.targets[i].next.as_ref().unwrap();
                self.b.jump(next, ());
                Ok(None)
            }
            ExprKind::Return { value } => {
                let v = match value {
                    Some(x) => val!(self.expr(*x)),
                    None => None,
                };
                self.b.ret(Vec::from_iter(v));
                Ok(None)
            }
            ExprKind::Assign { place, op, value } => {
                self.assign(place, op.as_ref(), *value)?;
                Ok(None)
            }
            ExprKind::Unsupported(what) => Err(ErrorKind::unsupported(*what)),
        }
    }

    /// The constructor of a struct or a tuple, whose fields are read and
    /// set.
    fn data_ctor(&mut self, t: &MonoTy<'db>) -> R<CtorId> {
        match self.cty(t)? {
            Some(Ty::Data(id, ..)) => Ok(CtorId { data: id, index: 0 }),
            _ => Err(ErrorKind::unsupported("a field of this type")),
        }
    }

    /// A function or constant used as a value: a constant's value, or a
    /// closure of a named function that captures nothing.
    fn item_value(&mut self, e: ExprId, item: &ItemRef<'db>) -> R<Val> {
        let u = match item {
            // An operation as a function is its shim, which performs it.
            ItemRef::Op(op) => self.lcx.op_fn_use(*op, &self.mono(e)?)?,
            _ => self.use_of(item, &self.mir[e].ty)?,
        };
        let Use::Instance { instance, mutabilities, rows, dicts } = u else {
            return Err(ErrorKind::unsupported("a builtin or a host function as a value"));
        };
        if !dicts.is_empty() {
            return Err(ErrorKind::unsupported("dictionaries"));
        }
        let f = self.cx.funcs[&instance];
        let margs: Vec<Mutability> = call_margs(&mutabilities)?;
        let rows = self.rows(&rows);
        if let Some(DefWithBody::Const(_)) = instance.body(self.cx.db).map(|b| b.def(self.cx.db)) {
            let results = self.b.call_with(f, margs, rows, Vec::<Atom>::new());
            return Ok(first(&results));
        }
        let MonoTy::Fn(_, _, _, m, _) = self.mono(e)? else {
            return Err(ErrorKind::unsupported("a function whose type isn't a function"));
        };
        let none = Vec::<Atom>::new();
        Ok(Some(Atom::Var(self.b.closure_with(f, margs, rows, mutability(m), none))))
    }

    /// `for x in xs { .. }`: `iter(xs)` once, then `next` of it until it
    /// gives `None`.
    fn for_loop(
        &mut self,
        pat: &Pattern<'db>,
        iterable: ExprId,
        inner: ExprId,
        label: LabelId,
        iter: &Callee<'db>,
        next_fn: &Callee<'db>,
    ) -> R<Val> {
        let xs = self.expr(iterable)?;
        if self.ended() {
            return Ok(None);
        }
        let it = self.call_callee(iter, vec![xs])?;
        let exit = self.b.join(&[]);
        let next = self.b.join_rec(&[]);
        self.b.jump(&next, ());
        self.b.switch_to(next.block());
        let (item, item_ty) = self.call_callee(next_fn, vec![it.0])?;
        // `next` gives an `Option` of the item: `Some(x)` or `None`.
        let MonoTy::Adt(AdtId::Enum(option), ..) = &item_ty else {
            return Err(ErrorKind::unsupported(
                "an iterator whose `next` doesn't give an `Option`",
            ));
        };
        let variants = &boltlang_hir_def::signatures::enum_signature(self.cx.db, *option).variants;
        let some = variants
            .iter()
            .position(|v| v.name.as_str() == "Some")
            .ok_or_else(|| ErrorKind::unsupported("an `Option` without `Some`"))?
            as u32;
        let item = item.ok_or_else(|| ErrorKind::unsupported("this `for` loop"))?;
        let variant = boltlang_hir_def::ids::EnumVariantId { parent: *option, index: some };
        let field_ty = field_ty(self, &item_ty, Ctor::Variant(variant), 0)?;
        let (case, x) = if self.nullable_host(&item_ty) {
            // A host value, until a null.
            let null = self.host_is_null(item.clone());
            let case = self.b.case(null);
            self.b.arm(case, 0);
            (case, Some(self.host_unwrap(item)))
        } else {
            let Some(Ty::Data(id, ..)) = self.cty(&item_ty)? else {
                return Err(ErrorKind::unsupported("this `for` loop"));
            };
            let case = self.b.case(item.clone());
            let ctor = CtorId { data: id, index: some };
            self.b.arm(case, ctor);
            let x = match self.cty(&field_ty)? {
                Some(_) => Some(Atom::Var(self.b.get(ctor, 0, item))),
                None => None,
            };
            (case, x)
        };
        match pat {
            Pattern::Local(Some(local)) => self.bind(*local, x)?,
            Pattern::Local(None) => {}
            Pattern::Decision(d) => self.let_decision(d, x, field_ty, None)?,
        }
        self.targets.push(Target { label, exit, next: Some(next) });
        self.expr(inner)?;
        let target = self.targets.pop().unwrap();
        if !self.ended() {
            self.b.jump(target.next.as_ref().unwrap(), ());
        }
        self.b.default(case);
        self.b.jump(&target.exit, ());
        self.b.switch_to(target.exit.block());
        Ok(None)
    }

    /// Call a function with a body that something other than a call calls,
    /// like the `next` of a `for`: the result, and its type, which is that
    /// of the instance it calls.
    fn call_callee(&mut self, callee: &Callee<'db>, args: Vec<Val>) -> R<(Val, MonoTy<'db>)> {
        let u = self.use_of(&callee.item, &callee.ty)?;
        let Use::Instance { instance, mutabilities, rows, dicts } = u else {
            return Err(ErrorKind::unsupported("this `for` loop"));
        };
        if !dicts.is_empty() {
            return Err(ErrorKind::unsupported("dictionaries"));
        }
        let callee = Cx::new(self.cx.db, instance, &self.cx.erased)?;
        let MonoTy::Fn(_, _, ret, ..) = callee.instance_ty()? else {
            return Err(ErrorKind::unsupported("this `for` loop"));
        };
        // The callee's own mutability and row parameters are the ones this
        // use gives, which has one for each.
        if rows.len() < instance.row_params(self.cx.db) {
            return Err(ErrorKind::unsupported("a call without a row for each row parameter"));
        }
        let ret = subst_muts(&ret, &mutabilities).map_rows(&mut |i| rows[i as usize].clone());
        let f = self.cx.funcs[&instance];
        let margs = call_margs(&mutabilities)?;
        let crows = self.rows(&rows);
        let args = args.into_iter().flatten().collect::<Vec<_>>();
        let results = self.b.call_with(f, margs, crows, args);
        Ok((first(&results), ret))
    }

    /// The index of the loop or block that `break` or `continue` to `label`
    /// goes to.
    fn target(&self, label: LabelId, is_continue: bool) -> R<usize> {
        self.targets
            .iter()
            .rposition(|t| t.label == label && (!is_continue || t.next.is_some()))
            .ok_or_else(|| ErrorKind::unsupported("this `break` or `continue`"))
    }

    fn statement(&mut self, s: &Stmt<'db>) -> R<()> {
        match s {
            Stmt::Let { init: None, .. } => Err(ErrorKind::unsupported("a `let` without a value")),
            Stmt::Let { pat, init: Some(init), else_ } => {
                let v = self.expr(*init)?;
                if self.ended() {
                    return Ok(());
                }
                match pat {
                    Pattern::Local(Some(local)) => self.bind(*local, v),
                    Pattern::Local(None) => Ok(()),
                    Pattern::Decision(d) => {
                        let t = self.mono(*init)?;
                        self.let_decision(d, v, t, *else_)
                    }
                }
            }
            Stmt::Expr(e) => self.expr(*e).map(|_| ()),
        }
    }

    fn literal(&mut self, e: ExprId, lit: &Literal) -> R<Val> {
        let t = self.mono(e)?;
        match lit {
            Literal::Int(n, _) => {
                let s = Scalar::of(&t)?.ok_or_else(|| ErrorKind::unsupported("this literal"))?;
                Ok(Some(int_lit(s, *n as i128)))
            }
            Literal::Float(text, _) => {
                let x = parse_float(text)?;
                match Scalar::of(&t)? {
                    Some(Scalar::F32) => Ok(Some(Atom::Lit(Lit::f32(x as f32)))),
                    Some(Scalar::F64) => Ok(Some(Atom::Lit(Lit::f64(x)))),
                    _ => Err(ErrorKind::unsupported("this literal")),
                }
            }
            Literal::Byte(n) => Ok(Some(Atom::Lit(Lit::I8(*n as i8)))),
            Literal::Char(c) => self.construct(e, 0, vec![Atom::Lit(Lit::I32(*c as u32 as i32))]),
            Literal::String(s) => {
                // A `String` holds its UTF-8 bytes, as `[u8]`.
                let bytes: Vec<Atom> = s.bytes().map(|b| Atom::Lit(Lit::I8(b as i8))).collect();
                let xs = self.b.array_new(Ty::I8, Mutability::Imm, bytes);
                self.construct(e, 0, vec![Atom::Var(xs)])
            }
            Literal::ByteString(_) | Literal::CString(_) => {
                Err(ErrorKind::unsupported("byte strings and C strings"))
            }
        }
    }

    /// A value of the variant `v`, of the type of `e`, with these fields.
    fn variant(
        &mut self,
        e: ExprId,
        v: boltlang_hir_def::ids::EnumVariantId<'db>,
        fields: Vec<Atom>,
    ) -> R<Val> {
        if is_fieldless(self.cx.db, v.parent) {
            return Ok(Some(Atom::Lit(Lit::I32(v.index as i32))));
        }
        // `None` is null, and `Some(x)` is `x`, which fits.
        if self.nullable_host(&self.mono(e)?) {
            return Ok(match fields.into_iter().next() {
                Some(x) => Some(x),
                None => Some(Atom::Var(self.b.op(Op::HostNull, ())[0])),
            });
        }
        self.construct(e, v.index, fields)
    }

    /// Whether `t` is an `Option` of a type of the host, which is a
    /// `nullable host`.
    fn nullable_host(&self, t: &MonoTy<'db>) -> bool {
        is_nullable_host(self.cx.db, self.cx.root, t)
    }

    /// The `host` in the `nullable host` `v`, which isn't null.
    fn host_unwrap(&mut self, v: Atom) -> Atom {
        Atom::Var(self.b.op(Op::HostUnwrap, [v])[0])
    }

    /// Whether the `nullable host` `v` is null, as an `i32`.
    fn host_is_null(&mut self, v: Atom) -> Atom {
        Atom::Var(self.b.op(Op::HostIsNull, [v])[0])
    }

    /// A new object of the type of `e`, made with its constructor `index`.
    fn construct(&mut self, e: ExprId, index: u32, fields: Vec<Atom>) -> R<Val> {
        let t = self.mono(e)?;
        let Some(Ty::Data(id, m, margs, rows)) = self.cty(&t)? else {
            return Err(ErrorKind::unsupported("this value"));
        };
        let ctor = CtorId { data: id, index };
        Ok(Some(Atom::Var(self.b.new_object_full(ctor, m, margs, rows, fields))))
    }

    /// Call `item`, used at the type `use_ty`, with these arguments; the
    /// result has the type `result_ty`.
    fn call_item(
        &mut self,
        item: &ItemRef<'db>,
        use_ty: &boltlang_hir_ty::Ty<'db>,
        args: Vec<Val>,
        result_ty: &MonoTy<'db>,
    ) -> R<Val> {
        let u = self.use_of(item, use_ty)?;
        if let Use::Op { op, effect_args } = u {
            return self.perform(op, &effect_args, use_ty, args, result_ty);
        }
        let args: Vec<Atom> = args.into_iter().flatten().collect();
        let results = match u {
            Use::Instance { instance, mutabilities, rows, dicts } => {
                if !dicts.is_empty() {
                    return Err(ErrorKind::unsupported("dictionaries"));
                }
                let f = self.cx.funcs[&instance];
                let margs = call_margs(&mutabilities)?;
                let rows = self.rows(&rows);
                self.b.call_with(f, margs, rows, args)
            }
            Use::Extern(f) => {
                let results = self.b.call_extern(self.cx.externs[&f], args);
                // A host function can return although it's declared not to,
                // and nothing in Bolt can promise it won't: stop then.
                if matches!(result_ty, MonoTy::Never) {
                    self.b.trap();
                }
                // With `abi = flat`, a tuple is given back as its elements.
                let flat = boltlang_hir_ty::builtin::link(self.cx.db, f)
                    .is_some_and(|l| l.abi == boltlang_hir_def::item_tree::LinkAbi::Flat);
                if flat && matches!(result_ty, MonoTy::Tuple(elems, _) if !elems.is_empty()) {
                    let Some(Ty::Data(id, m, margs, rows)) = self.cty(result_ty)? else {
                        return Err(ErrorKind::unsupported("this tuple"));
                    };
                    let ctor = CtorId { data: id, index: 0 };
                    let fields: Vec<Atom> = results.iter().map(|&v| Atom::Var(v)).collect();
                    return Ok(Some(Atom::Var(
                        self.b.new_object_full(ctor, m, margs, rows, fields),
                    )));
                }
                results
            }
            Use::Builtin(builtin) => return self.builtin(builtin, args, result_ty),
            Use::Method { .. } => return Err(ErrorKind::unsupported("dictionaries")),
            Use::Op { .. } => unreachable!("performed above"),
        };
        Ok(first(&results))
    }

    /// A `perform` of the operation `f`, which the nearest handler of its
    /// effect runs. Its arguments are converted to the types of the
    /// operation's declaration, which has `any` for its own type
    /// parameters, and its result back (see [`crate::conv`]).
    fn perform(
        &mut self,
        f: FunctionId<'db>,
        effect_args: &[MonoTy<'db>],
        use_ty: &boltlang_hir_ty::Ty<'db>,
        args: Vec<Val>,
        result_ty: &MonoTy<'db>,
    ) -> R<Val> {
        let db = self.cx.db;
        let (effect, index) =
            self.cx.op(f, effect_args).ok_or_else(|| ErrorKind::unsupported("this operation"))?;
        let ItemContainerId::Effect(e) = f.container(db) else {
            return Err(ErrorKind::unsupported("this operation"));
        };
        let (declared, ret) = crate::op_params(db, self.cx.root, &(e, effect_args.into()), f)?;
        let MonoTy::Fn(given, ..) = self.mono_of(use_ty)? else {
            return Err(ErrorKind::unsupported("this operation"));
        };
        let mut ccx = ConvCx { db, root: self.cx.root, types: self.types };
        let mut atoms = Vec::new();
        for ((v, d), g) in args.into_iter().zip(&declared).zip(given.iter()) {
            atoms.extend(self.cx.convs.emit(&mut ccx, &mut self.b, v, g, d)?);
        }
        let results = self.b.perform(effect, index, atoms);
        let result = first(&results);
        // A handler can't resume with a value of a type that has none, so
        // the operation doesn't return then.
        if matches!(result_ty, MonoTy::Never) {
            if !matches!(ret, MonoTy::Never) {
                self.b.trap();
            }
            return Ok(None);
        }
        self.cx.convs.emit(&mut ccx, &mut self.b, result, &ret, result_ty)
    }

    /// Call what a [`Callee`] calls, with these arguments.
    fn call_use(&mut self, callee: &Callee<'db>, args: Vec<Val>) -> R<Val> {
        let MonoTy::Fn(_, _, ret, ..) = self.mono_of(&callee.ty)? else {
            return Err(ErrorKind::unsupported("a call whose type isn't a function"));
        };
        self.call_item(&callee.item, &callee.ty, args, &ret)
    }

    fn builtin(&mut self, builtin: Builtin, args: Vec<Atom>, result: &MonoTy<'db>) -> R<Val> {
        let one = |v: conduit_ir::Var| Ok(Some(Atom::Var(v)));
        match builtin {
            Builtin::Prim(op, t) => self.prim(op, Scalar::of(&MonoTy::Builtin(t))?.unwrap(), args),
            Builtin::Resume => {
                let results = self.b.resume(args[0].clone(), args[1..].to_vec());
                Ok(first(&results))
            }
            Builtin::ArrayLen => one(self.b.array_len(args[0].clone())),
            // An element of a type with no values is read for the check of
            // its index, and gives nothing.
            Builtin::ArrayGet => {
                let x = self.b.array_get(args[0].clone(), args[1].clone());
                match self.cty(result)? {
                    Some(_) => one(x),
                    None => Ok(None),
                }
            }
            Builtin::ArraySet => {
                let x = args.get(2).cloned().unwrap_or_else(placeholder);
                self.b.array_set(args[0].clone(), args[1].clone(), x);
                Ok(None)
            }
            // A copy that can't be changed. Conduit's `copies` pass makes it
            // a freeze when nothing else has the array.
            Builtin::ArrayFreeze => one(self.b.array_clone(Mutability::Imm, args[0].clone())),
            Builtin::Abort => {
                self.b.trap();
                Ok(None)
            }
            Builtin::ArrayAlloc => {
                let Some(Ty::Array(elem, _)) = self.cty(result)? else {
                    return Err(ErrorKind::unsupported("an array of a type with no values"));
                };
                one(self.b.array_alloc(*elem, args[0].clone()))
            }
            Builtin::ArrayCopy => {
                let [dst, dst_start, src, src_start, count] = &args[..] else {
                    return Err(ErrorKind::unsupported("this call of `array_copy`"));
                };
                self.b.array_copy(
                    dst.clone(),
                    dst_start.clone(),
                    src.clone(),
                    src_start.clone(),
                    count.clone(),
                );
                Ok(None)
            }
        }
    }

    fn prim(&mut self, op: PrimOp, s: Scalar, args: Vec<Atom>) -> R<Val> {
        let sign = s.sign();
        let float = matches!(s, Scalar::F32 | Scalar::F64);
        let p = match op {
            // In the bytes of an `[i8]`, as Conduit reads and writes them:
            // `u16` and `i16` are both an `i16`.
            PrimOp::Load => {
                let vs = self.b.op_as(Op::ArrayLoad, args, vec![s.ty()]);
                return Ok(first(&vs));
            }
            PrimOp::Store => {
                self.b.op(Op::ArrayStore, args);
                return Ok(None);
            }
            PrimOp::Add => Prim::Add,
            PrimOp::Sub => Prim::Sub,
            PrimOp::Mul => Prim::Mul,
            PrimOp::Div if float => Prim::Div(Sign::Signed),
            PrimOp::Div => Prim::Div(sign),
            PrimOp::Rem => Prim::Rem(sign),
            PrimOp::BitAnd => Prim::And,
            PrimOp::BitOr => Prim::Or,
            PrimOp::BitXor => Prim::Xor,
            PrimOp::Shl => Prim::Shl,
            PrimOp::Shr => Prim::Shr(sign),
            PrimOp::Eq => Prim::Eq,
            PrimOp::Ne => Prim::Ne,
            PrimOp::Lt => Prim::Lt(sign),
            PrimOp::Le => Prim::Le(sign),
            PrimOp::Gt => Prim::Gt(sign),
            PrimOp::Ge => Prim::Ge(sign),
            PrimOp::Neg if float => Prim::Neg,
            PrimOp::Neg => {
                let zero = int_lit(s, 0);
                let v = self.b.prim(Prim::Sub, [zero, args[0].clone()]);
                return Ok(Some(Atom::Var(v)));
            }
            PrimOp::Not => {
                // `!` of a `Bool` flips its bit, and of an integer all of them.
                let mask = if s == Scalar::Bool { int_lit(s, 1) } else { int_lit(s, -1) };
                let v = self.b.prim(Prim::Xor, [args[0].clone(), mask]);
                return Ok(Some(Atom::Var(v)));
            }
        };
        let mut args = args;
        // The amount of a shift is a `u32`, which a shift of another width
        // needs as its own type.
        if matches!(op, PrimOp::Shl | PrimOp::Shr) && s.ty() != Ty::I32 {
            let n = self.b.convert(s.ty(), Sign::Unsigned, args[1].clone());
            args[1] = Atom::Var(n);
        }
        Ok(Some(Atom::Var(self.b.prim(p, args))))
    }

    fn cast(&mut self, inner: ExprId, e: ExprId, v: Val) -> R<Val> {
        let from = Scalar::of(&self.mono(inner)?)?;
        let to = Scalar::of(&self.mono(e)?)?;
        let (Some(from), Some(to), Some(v)) = (from, to, v) else {
            return Err(ErrorKind::unsupported("this cast"));
        };
        if from == to {
            return Ok(Some(v));
        }
        // Narrower keeps the low bits, and wider extends by the sign of what's
        // cast. Of the same width, only how it's read changes.
        let sign = from.sign();
        let v = if from.ty() == to.ty() { v } else { Atom::Var(self.b.convert(to.ty(), sign, v)) };
        Ok(Some(v))
    }

    fn assign(&mut self, place: &MirPlace<'db>, op: Option<&Callee<'db>>, value: ExprId) -> R<()> {
        // What's assigned to, evaluated once, and its old value for `+=`.
        enum Target {
            Local(conduit_ir::Var),
            Field(CtorId, u32, Atom),
            Index(Val, Val),
        }
        let target = match place {
            MirPlace::Local(local) => match self.locals.get(local) {
                Some(LocalVal::Ref(r)) => Target::Local(*r),
                Some(LocalVal::Value(None)) => return self.expr(value).map(|_| ()),
                _ => return Err(ErrorKind::unsupported("assigning to this local")),
            },
            MirPlace::Field { base, ctor, index } => {
                let t = self.mono(*base)?;
                let (_, index) = &tuple_field(self.cx.db, &t, *ctor, *index)?;
                let obj = self.expr(*base)?;
                if self.ended() {
                    return Ok(());
                }
                let ctor = self.data_ctor(&t)?;
                Target::Field(ctor, *index, obj.ok_or_else(|| ErrorKind::unsupported("this"))?)
            }
            MirPlace::Index { base, index, .. } => {
                let xs = self.expr(*base)?;
                if self.ended() {
                    return Ok(());
                }
                let i = self.expr(*index)?;
                if self.ended() {
                    return Ok(());
                }
                Target::Index(xs, i)
            }
        };
        let mut new = self.expr(value)?;
        if self.ended() {
            return Ok(());
        }
        if let Some(op) = op {
            let old = match (&target, place) {
                (Target::Local(r), _) => Some(Atom::Var(self.b.ref_get(*r))),
                (Target::Field(c, i, obj), _) => Some(Atom::Var(self.b.get(*c, *i, obj.clone()))),
                (Target::Index(xs, i), MirPlace::Index { get: Some(get), .. }) => {
                    self.call_use(get, vec![xs.clone(), i.clone()])?
                }
                (Target::Index(..), _) => {
                    return Err(ErrorKind::unsupported("this compound assignment"));
                }
            };
            new = self.call_use(op, vec![old, new])?;
        }
        match (target, place) {
            (Target::Local(r), _) => {
                if let Some(v) = new {
                    self.b.ref_set(r, v);
                }
            }
            (Target::Field(c, i, obj), _) => {
                if let Some(v) = new {
                    self.b.set(c, i, obj, v);
                }
            }
            (Target::Index(xs, i), MirPlace::Index { set, .. }) => {
                self.call_use(set, vec![xs, i, new])?;
            }
            (Target::Index(..), _) => unreachable!("an index is only a place of an index"),
        }
        Ok(())
    }

    fn if_expr(&mut self, e: ExprId, c: ExprId, then: ExprId, other: Option<ExprId>) -> R<Val> {
        let tys: Vec<Ty> = self.cty(&self.mono(e)?)?.into_iter().collect();
        let k = self.b.join(&tys);
        let mut branch = |l: &mut Self, x: Option<ExprId>| -> R<()> {
            let v = match x {
                Some(x) => l.expr(x)?,
                None => None,
            };
            if !l.ended() {
                l.b.jump(&k, Vec::from_iter(v));
            }
            Ok(())
        };
        let branch = std::cell::RefCell::new(&mut branch);
        self.branch_on(c, &mut |l| (branch.borrow_mut())(l, Some(then)), &mut |l| {
            (branch.borrow_mut())(l, other)
        })?;
        self.b.switch_to(k.block());
        Ok(first(k.params()))
    }

    fn match_expr(
        &mut self,
        e: ExprId,
        scrutinee: ExprId,
        d: &Decision<'db>,
        guards: &[Option<ExprId>],
        bodies: &[ExprId],
    ) -> R<Val> {
        let t = self.mono(scrutinee)?;
        let v = self.expr(scrutinee)?;
        if self.ended() {
            return Ok(None);
        }
        let tys: Vec<Ty> = self.cty(&self.mono(e)?)?.into_iter().collect();
        let k = self.b.join(&tys);
        self.decide(d, v, t, false, guards, &mut |l, arm| {
            let v = l.expr(bodies[arm as usize])?;
            if !l.ended() {
                l.b.jump(&k, Vec::from_iter(v));
            }
            Ok(())
        })?;
        self.b.switch_to(k.block());
        Ok(first(k.params()))
    }

    /// A `let` with a pattern: its bindings go on after it, and a value it
    /// doesn't match runs `other`, which never ends.
    fn let_decision(
        &mut self,
        d: &Decision<'db>,
        v: Val,
        t: MonoTy<'db>,
        other: Option<ExprId>,
    ) -> R<()> {
        // The bindings' values go on to the code after the `let`.
        let mut locals: Vec<Local> = Vec::new();
        for node in &d.nodes {
            if let Node::Arm { bindings, .. } = node {
                for &(b, _) in bindings {
                    if !locals.contains(&Local::Binding(b)) {
                        locals.push(Local::Binding(b));
                    }
                }
            }
        }
        let mut tys = Vec::new();
        let mut has_value = Vec::new();
        for &local in &locals {
            let t = self.cty(&self.local_ty(local)?)?;
            has_value.push(t.is_some());
            tys.extend(t);
        }
        let k = self.b.join(&tys);
        self.decide(d, v, t, other.is_some(), &[], &mut |l, arm| {
            if arm == FAIL {
                // The `else`, which never ends.
                if let Some(other) = other {
                    l.expr(other)?;
                }
                if !l.ended() {
                    l.b.unreachable();
                }
                return Ok(());
            }
            let mut args = Vec::new();
            for local in &locals {
                match l.locals.get(local).cloned() {
                    Some(LocalVal::Value(Some(v))) => args.push(v),
                    Some(LocalVal::Ref(r)) => args.push(Atom::Var(l.b.ref_get(r))),
                    _ => {}
                }
            }
            l.b.jump(&k, args);
            Ok(())
        })?;
        self.b.switch_to(k.block());
        let mut params = k.params().iter();
        for (&local, has) in locals.iter().zip(has_value) {
            let v = if has { params.next().map(|&p| Atom::Var(p)) } else { None };
            self.bind(local, v)?;
        }
        Ok(())
    }

    /// Lower a decision on `value` of type `t`: `arm` lowers the body of
    /// an arm, with its bindings bound, and ends its block. `guards` are
    /// those of the arms of a `match`.
    #[allow(clippy::too_many_arguments)]
    fn decide(
        &mut self,
        d: &Decision<'db>,
        value: Val,
        t: MonoTy<'db>,
        fail_arm: bool,
        guards: &[Option<ExprId>],
        arm: &mut dyn FnMut(&mut Self, u32) -> R<()>,
    ) -> R<()> {
        let mut dl = DecisionLower::new(self, d, value, t)?;
        dl.fail_arm = fail_arm;
        dl.guards = guards.to_vec();
        dl.run(self, arm)
    }

    /// Branch on a condition: `then` runs if it holds, with the bindings of
    /// its `let` if it's one, and `other` if not. Each ends its block.
    fn branch_on(
        &mut self,
        c: ExprId,
        then: &mut dyn FnMut(&mut Self) -> R<()>,
        other: &mut dyn FnMut(&mut Self) -> R<()>,
    ) -> R<()> {
        let mir = self.mir;
        if let ExprKind::Let { scrutinee, decision } = &mir[c].kind {
            let t = self.mono(*scrutinee)?;
            let v = self.expr(*scrutinee)?;
            if self.ended() {
                return Ok(());
            }
            return self.decide(decision, v, t, true, &[], &mut |l, arm| {
                if arm == FAIL { other(l) } else { then(l) }
            });
        }
        let Some(cond) = self.expr(c)? else { return Ok(()) };
        let case = self.b.case(cond);
        self.b.arm(case, 0);
        other(self)?;
        self.b.default(case);
        then(self)
    }
}

/// `t` with the `i`th mutability parameter replaced by `args[i]`, also in
/// the arguments of the effects in its rows.
fn subst_muts<'db>(t: &MonoTy<'db>, args: &[boltlang_mir_mono::MonoMut]) -> MonoTy<'db> {
    use boltlang_mir_mono::MonoMut;
    t.map_muts(&mut |m| match m {
        MonoMut::Param(i) => args.get(i as usize).copied().unwrap_or(m),
        other => other,
    })
}

/// The first of `vars`, which is the value of a call or a join point
/// that gives one.
fn first(vars: &[conduit_ir::Var]) -> Val {
    vars.first().map(|&v| Atom::Var(v))
}

/// An integer or `Bool` literal of a scalar type, wrapped to its range.
fn int_lit(s: Scalar, n: i128) -> Atom {
    match s {
        Scalar::Int { bits: 8, .. } => Atom::Lit(Lit::I8(n as i8)),
        Scalar::Int { bits: 16, .. } => Atom::Lit(Lit::I16(n as i16)),
        Scalar::Int { bits: 64, .. } => Atom::Lit(Lit::I64(n as i64)),
        _ => Atom::Lit(Lit::I32(n as i32)),
    }
}

fn parse_float(text: &str) -> R<f64> {
    text.replace('_', "")
        .trim_end_matches("f32")
        .trim_end_matches("f64")
        .parse()
        .map_err(|_| ErrorKind::unsupported("this literal"))
}

/// The field that `ctor` and `index` name in a value of type `t`, as a
/// field at an index: one of a tuple named by its name, or by where it is
/// from the end of the positional fields, which only monomorphization
/// knows, is at the place the tuple has it.
fn tuple_field<'db>(
    db: &'db dyn boltlang_base_db::Db,
    t: &MonoTy<'db>,
    ctor: Ctor<'db>,
    index: u32,
) -> R<(Ctor<'db>, u32)> {
    if !matches!(ctor, Ctor::TupleEnd | Ctor::TupleNamed(_)) {
        return Ok((ctor, index));
    }
    let (elems, names): (&[MonoTy<'db>], &[boltlang_hir_def::Name]) = match t {
        MonoTy::Tuple(elems, _) => (elems, &[]),
        MonoTy::Record(elems, names, _) => (elems, names),
        _ => return Err(ErrorKind::unsupported("a field of this type")),
    };
    let positional = elems.len() - names.len();
    let missing = || ErrorKind::unsupported("a field the tuple doesn't have");
    let at = match ctor {
        Ctor::TupleEnd => positional.checked_sub(1 + index as usize).ok_or_else(missing)?,
        Ctor::TupleNamed(name) => {
            positional + names.iter().position(|n| n == name.name(db)).ok_or_else(missing)?
        }
        _ => unreachable!(),
    };
    Ok((Ctor::Tuple(elems.len() as u32), at as u32))
}

/// The type of field `index` of a value of type `t` made by `ctor`.
fn field_ty<'db>(
    l: &Lower<'_, '_, 'db>,
    t: &MonoTy<'db>,
    ctor: Ctor<'db>,
    index: u32,
) -> R<MonoTy<'db>> {
    let unknown = |_| ErrorKind::unsupported("a field whose type isn't known");
    let (ctor, index) = tuple_field(l.cx.db, t, ctor, index)?;
    let fields = match (ctor, t) {
        (Ctor::Tuple(_), MonoTy::Tuple(elems, _) | MonoTy::Record(elems, ..)) => elems.to_vec(),
        (Ctor::Struct(s), MonoTy::Adt(_, args, muts, own)) => {
            field_types(l.cx.db, l.cx.root, VariantDef::Struct(s), args, muts, *own)
                .map_err(unknown)?
        }
        (Ctor::Variant(v), MonoTy::Adt(_, args, muts, own)) => {
            field_types(l.cx.db, l.cx.root, VariantDef::Variant(v), args, muts, *own)
                .map_err(unknown)?
        }
        _ => return Err(ErrorKind::unsupported("this pattern")),
    };
    fields.get(index as usize).cloned().ok_or_else(|| ErrorKind::unsupported("this pattern"))
}

/// The mutability arguments of a call. Conduit takes `mut`, `imm` or a
/// parameter of the caller, never `ro`: a function called at a `ro`
/// mutability would need an instance of its own (#320).
fn call_margs(mutabilities: &[boltlang_mir_mono::MonoMut]) -> Result<Vec<Mutability>, ErrorKind> {
    if mutabilities.contains(&boltlang_mir_mono::MonoMut::ReadOnly) {
        return Err(ErrorKind::unsupported("a function called at a `ro` mutability"));
    }
    Ok(mutabilities.iter().map(|&m| mutability(m)).collect())
}
