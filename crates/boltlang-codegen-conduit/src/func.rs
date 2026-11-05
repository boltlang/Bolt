//! Lowering the body of an instance to a Conduit function.

use std::collections::{BTreeSet, HashMap};

use boltlang_base_db::source_text;
use boltlang_hir_def::{
    Name,
    body::{Body, DefWithBodyId, body_with_source_map},
    builtin_type::BuiltinValue,
    expr_store::ExpressionStoreSourceMap,
    hir::{
        BinaryOp, BindingId, Expr, ExprId, LabelId, Literal, LogicOp, Pat, PatId, Path, Statement,
    },
    ids::EnumVariantId,
    resolver::{Resolver, ValueNs, ValueResolution},
};
use boltlang_hir_ty::{
    AdtId,
    builtin::{Builtin, PrimOp},
    lower::{VariantDef, field_types as hir_field_types},
};
use boltlang_lower::{
    Cx, InstanceUses, MonoTy, Site, Use,
    decision::{Bound, Case, Ctor, Decision, Node, NodeId, Place, PlaceId, Value, body_decisions},
    field_types,
    lambda::{Lambda, body_lambdas},
};
use conduit_ir::{
    Atom, CtorId, Func, Lit, Module, Mutability, Prim, Sign, Ty, build::FuncBuilder,
    build::JoinPoint,
};

use crate::{
    Ctx, Error, ErrorKind, ExternKey,
    ty::{Scalar, Types, is_fieldless, mutability},
};

type R<T> = Result<T, ErrorKind>;

/// The arm a decision's callback gets for a value that no arm matches,
/// when the decision is a `let`'s, whose `else` runs then.
const FAIL: u32 = u32::MAX;

/// A value: none for `()`, `!` and code that's never reached.
type Val = Option<Atom>;

/// Where a local is.
#[derive(Clone)]
enum Local {
    Value(Val),
    /// A local that's assigned.
    Ref(conduit_ir::Var),
}

/// A loop or a labeled block that `break` can leave.
struct Target {
    label: Option<LabelId>,
    /// Where `break` goes, with the value if it gives one.
    exit: JoinPoint,
    /// Where `continue` goes, for a loop.
    next: Option<JoinPoint>,
}

/// The function of the instance `iu`.
/// The function of the instance `iu`, or of one of its lambdas, lifted.
pub(crate) fn lower_body<'db>(
    cx: &Ctx<'db>,
    module: &Module,
    types: &mut Types<'db>,
    iu: &InstanceUses<'db>,
    lambda: Option<&Lambda>,
) -> Result<Func, Error> {
    let db = cx.db;
    let def = iu.instance.body(db);
    let (body, map) = body_with_source_map(db, def);
    let lcx = Cx::new(db, iu.instance, &cx.erased).map_err(|e| Error {
        instance: iu.instance.display(db).to_string(),
        at: None,
        kind: e.into(),
    })?;
    let id = match lambda {
        Some(lambda) => cx.lambdas[&(iu.instance, lambda.expr)],
        None => cx.funcs[&iu.instance],
    };
    let sig = &module.funcs[&id];
    let mut b = FuncBuilder::new(module, sig.sig().params, sig.results.clone());
    b.set_mut_params(sig.mut_params.clone());
    let mut l = Lower {
        cx,
        lcx: &lcx,
        types,
        def,
        body,
        map,
        b,
        lambda,
        uses: iu.uses.iter().cloned().collect(),
        locals: HashMap::new(),
        targets: Vec::new(),
        at: None,
        mut_literals: lcx.infer.mut_literals.iter().map(|&(outer, inner)| (inner, outer)).collect(),
        var_names: Default::default(),
    };
    let error = |l: &Lower<'_, '_, 'db>, kind: ErrorKind| Error {
        instance: iu.instance.display(db).to_string(),
        at: l.at.map(|e| l.snippet(e)),
        kind,
    };
    if let Err(kind) = l.run() {
        return Err(error(&l, kind));
    }
    let instance = iu.instance.display(db).to_string();
    let var_names = std::mem::take(&mut l.var_names);
    let mut f = l.b.finish().map_err(|es| Error {
        instance,
        at: None,
        kind: ErrorKind::Invalid(
            es.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "),
        ),
    })?;
    f.debug.vars.extend(var_names.into_iter().map(|(v, n)| (v, n.into())));
    Ok(f)
}

struct Lower<'a, 'm, 'db> {
    cx: &'a Ctx<'db>,
    lcx: &'a Cx<'a, 'db>,
    types: &'a mut Types<'db>,
    def: DefWithBodyId<'db>,
    body: &'db Body,
    map: &'db ExpressionStoreSourceMap,
    b: FuncBuilder<'m, Module>,
    /// The lambda this is the function of, if it's one.
    lambda: Option<&'a Lambda>,
    uses: HashMap<Site, Use<'db>>,
    locals: HashMap<BindingId, Local>,
    targets: Vec<Target>,
    /// The expression being lowered, for errors.
    at: Option<ExprId>,
    /// The `e` of each `mut e` that makes a literal `mut`, with the `mut e`.
    mut_literals: HashMap<ExprId, ExprId>,
    /// The names of the variables of locals, for the text format.
    var_names: std::collections::BTreeMap<conduit_ir::Var, String>,
}

impl<'a, 'm, 'db> Lower<'a, 'm, 'db> {
    fn snippet(&self, e: ExprId) -> String {
        let text = source_text(self.cx.db, self.def.def(self.cx.db).file(self.cx.db));
        self.map
            .expr_syntax(e)
            .map_or("?".to_string(), |p| text[p.start as usize..p.end as usize].to_string())
    }

    fn ended(&self) -> bool {
        self.b.is_ended(self.b.current())
    }

    fn mono(&self, e: ExprId) -> R<MonoTy<'db>> {
        Ok(self.lcx.expr_ty(e)?)
    }

    fn cty(&mut self, t: &MonoTy<'db>) -> R<Option<Ty>> {
        self.types.value(self.cx.db, self.cx.root, t)
    }

    fn run(&mut self) -> R<()> {
        if let Some(lambda) = self.lambda {
            return self.run_lambda(lambda);
        }
        let db = self.cx.db;
        // The parameters: each of a type with values is one of Conduit's.
        let mut next = 0;
        for &pat in self.body.params.iter() {
            let t = self.lcx.pat_ty(pat)?;
            let value = match self.cty(&t)? {
                Some(_) => {
                    next += 1;
                    Some(Atom::Var(self.b.param(next - 1)))
                }
                None => None,
            };
            self.bind_pat(pat, value)?;
        }
        let result = self.expr(self.body.body_expr)?;
        if !self.ended() {
            // A constant, a function or a file: its value, if it has one.
            match self.lcx.instance.body(db).def(db) {
                boltlang_hir_def::body::DefWithBody::File(_) => self.b.ret(()),
                _ => self.b.ret(result.into_iter().collect::<Vec<_>>()),
            }
        }
        Ok(())
    }

    /// The function of a lambda: its captures, then its parameters.
    fn run_lambda(&mut self, lambda: &Lambda) -> R<()> {
        let mut next = 0;
        for &b in lambda.captures.iter() {
            let local = match self.cty(&self.lcx.binding_ty(b)?)? {
                Some(_) => {
                    next += 1;
                    let p = self.b.param(next - 1);
                    let name = crate::ident(self.body.bindings[b].name.as_str());
                    self.var_names.entry(p).or_insert(name);
                    if self.body.bindings[b].mutable {
                        Local::Ref(p)
                    } else {
                        Local::Value(Some(Atom::Var(p)))
                    }
                }
                None => Local::Value(None),
            };
            self.locals.insert(b, local);
        }
        let Expr::Lambda { params, body: inner, .. } = &self.body[lambda.expr] else {
            return Err(ErrorKind::unsupported("this lambda"));
        };
        for &pat in params.iter() {
            let t = self.lcx.pat_ty(pat)?;
            let value = match self.cty(&t)? {
                Some(_) => {
                    next += 1;
                    Some(Atom::Var(self.b.param(next - 1)))
                }
                None => None,
            };
            self.bind_pat(pat, value)?;
        }
        let result = self.expr(*inner)?;
        if !self.ended() {
            self.b.ret(result.into_iter().collect::<Vec<_>>());
        }
        Ok(())
    }

    /// A closure of the lifted lambda `e`, with the values it captures.
    fn closure(&mut self, e: ExprId) -> R<Val> {
        let f = self.cx.lambdas[&(self.lcx.instance, e)];
        let lambda = body_lambdas(self.cx.db, self.def)
            .iter()
            .find(|l| l.expr == e)
            .ok_or_else(|| ErrorKind::unsupported("this lambda"))?;
        let mut captures = Vec::new();
        for b in lambda.captures.iter() {
            match self.locals.get(b) {
                Some(Local::Value(Some(v))) => captures.push(v.clone()),
                Some(Local::Ref(r)) => captures.push(Atom::Var(*r)),
                Some(Local::Value(None)) => {}
                None => return Err(ErrorKind::unsupported("a capture that isn't in scope")),
            }
        }
        let MonoTy::Fn(_, _, m) = self.mono(e)? else {
            return Err(ErrorKind::unsupported("a lambda whose type isn't a function"));
        };
        Ok(Some(Atom::Var(self.b.closure_at(f, self.own_margs(), mutability(m), captures))))
    }

    /// The instance's own mutability parameters, to give a lifted lambda,
    /// which has them too.
    fn own_margs(&self) -> Vec<Mutability> {
        (1..=self.lcx.instance.mut_params(self.cx.db) as u32).map(Mutability::Param).collect()
    }

    /// Bind the pattern of a parameter or a `let` to `value`, which it
    /// can't fail to match.
    fn bind_pat(&mut self, pat: PatId, value: Val) -> R<()> {
        match &self.body[pat] {
            Pat::Wild => Ok(()),
            Pat::Bind { id, subpat: None } => self.bind(*id, value),
            _ => Err(ErrorKind::unsupported("this pattern here")),
        }
    }

    fn bind(&mut self, id: BindingId, value: Val) -> R<()> {
        let local = if self.body.bindings[id].mutable {
            match value {
                Some(v) => Local::Ref(self.b.ref_new(v)),
                None => Local::Value(None),
            }
        } else {
            Local::Value(value)
        };
        // The variable is shown by the local's name, unless it has one.
        let var = match &local {
            Local::Ref(v) | Local::Value(Some(Atom::Var(v))) => Some(*v),
            Local::Value(_) => None,
        };
        if let Some(v) = var {
            let name = crate::ident(self.body.bindings[id].name.as_str());
            self.var_names.entry(v).or_insert(name);
        }
        self.locals.insert(id, local);
        Ok(())
    }

    fn resolve(&self, e: ExprId, path: &Path) -> Option<ValueNs<'db>> {
        let db = self.cx.db;
        match Resolver::for_expr(db, self.def, e).resolve_path_in_value_ns(db, path)? {
            ValueResolution::Value(ValueNs::Overloaded(_)) => {
                self.lcx.infer.overload(e).map(ValueNs::Function)
            }
            ValueResolution::Value(v) => Some(v),
            _ => None,
        }
    }

    /// `e` as a path, for one like `Shape.Circle`, which is a field of a
    /// name that isn't a value.
    fn expr_path(&self, e: ExprId) -> Option<Path> {
        match &self.body[e] {
            Expr::Path(path) => Some(path.clone()),
            Expr::Field { expr, name } => {
                let mut segments = self.expr_path(*expr)?.segments.into_vec();
                segments.push(boltlang_hir_def::hir::PathSegment {
                    name: name.clone(),
                    generic_args: None,
                });
                Some(Path { segments: segments.into() })
            }
            _ => None,
        }
    }

    /// What `e` names, if it's a path that isn't a local's field.
    fn named(&self, e: ExprId) -> Option<ValueNs<'db>> {
        let path = self.expr_path(e)?;
        match self.resolve(e, &path)? {
            ValueNs::Local(_) if !matches!(self.body[e], Expr::Path(_)) => None,
            v => Some(v),
        }
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
        let body = self.body;
        match &body[e] {
            Expr::Literal(lit) => self.literal(e, lit),
            Expr::Path(path) => self.path(e, path),
            Expr::Block { statements, tail, label, .. } => {
                let exit = match label {
                    Some(label) => {
                        let tys: Vec<Ty> = self.cty(&self.mono(e)?)?.into_iter().collect();
                        let exit = self.b.join(&tys);
                        self.targets.push(Target { label: Some(*label), exit, next: None });
                        true
                    }
                    None => false,
                };
                for s in statements.iter() {
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
                    self.b.jump(&target.exit, v.into_iter().collect::<Vec<_>>());
                }
                self.b.switch_to(target.exit.block());
                Ok(target.exit.params().first().map(|&p| Atom::Var(p)))
            }
            Expr::Call { callee, args } => self.call(e, *callee, args),
            Expr::Field { expr: base, name } => {
                if self.uses.contains_key(&Site::Expr(e)) || self.named(e).is_some() {
                    let path = self.expr_path(e).unwrap();
                    return self.path(e, &path);
                }
                let t = self.mono(*base)?;
                let obj = val!(self.expr(*base));
                let (ctor, index) = self.field_of(&t, name)?;
                let obj = obj.ok_or_else(|| ErrorKind::unsupported("a field of `()`"))?;
                Ok(Some(Atom::Var(self.b.get(ctor, index, obj))))
            }
            Expr::Binary { op: BinaryOp::Logic(op), lhs, rhs } => {
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
            Expr::Binary { op: BinaryOp::Assign(op), lhs, rhs } => {
                self.assign(e, op.is_some(), *lhs, *rhs)?;
                Ok(None)
            }
            Expr::Binary { op: BinaryOp::Operator(_), lhs, rhs } => {
                let l = val!(self.expr(*lhs));
                let r = val!(self.expr(*rhs));
                self.call_use(Site::Expr(e), vec![l, r], e)
            }
            Expr::Prefix { expr: inner, .. } => {
                let v = val!(self.expr(*inner));
                self.call_use(Site::Expr(e), vec![v], e)
            }
            Expr::Index { base, index } => {
                let xs = val!(self.expr(*base));
                let i = val!(self.expr(*index));
                self.call_use(Site::Expr(e), vec![xs, i], e)
            }
            Expr::Cast { expr: inner, .. } => {
                let v = val!(self.expr(*inner));
                self.cast(*inner, e, v)
            }
            Expr::Tuple { exprs } => {
                let mut fields = Vec::new();
                for &x in exprs.iter() {
                    fields.extend(val!(self.expr(x)));
                }
                if exprs.is_empty() {
                    return Ok(None);
                }
                self.construct(e, 0, fields)
            }
            Expr::Array { exprs } => {
                let mut elems = Vec::new();
                for &x in exprs.iter() {
                    elems.push(val!(self.expr(x)).ok_or_else(|| {
                        ErrorKind::unsupported("an array of a type with no values")
                    })?);
                }
                let Some(Ty::Array(elem, m)) = self.cty(&self.literal_ty(e)?)? else {
                    return Err(ErrorKind::unsupported("this array"));
                };
                Ok(Some(Atom::Var(self.b.array_new(*elem, m, elems))))
            }
            Expr::ArrayRepeat { value, count } => {
                let v = val!(self.expr(*value))
                    .ok_or_else(|| ErrorKind::unsupported("an array of a type with no values"))?;
                let n = val!(self.expr(*count)).ok_or_else(|| ErrorKind::unsupported("this"))?;
                let Some(Ty::Array(elem, m)) = self.cty(&self.literal_ty(e)?)? else {
                    return Err(ErrorKind::unsupported("this array"));
                };
                Ok(Some(Atom::Var(self.b.array_fill(*elem, m, n, v))))
            }
            Expr::Record { path, fields, spread } => {
                if spread.is_some() {
                    return Err(ErrorKind::unsupported("`..` in a struct literal"));
                }
                let def = self.record_def(e, path.as_ref())?;
                let field_tys = hir_field_types(self.cx.db, def);
                let mut values: Vec<Option<Val>> = vec![None; field_tys.fields.len()];
                for f in fields.iter() {
                    let v = val!(self.expr(f.expr));
                    let (i, _) = field_tys
                        .field(&f.name)
                        .ok_or_else(|| ErrorKind::unsupported("this field"))?;
                    values[i] = Some(v);
                }
                let values: Vec<Atom> = values.into_iter().flatten().flatten().collect();
                let index = match def {
                    VariantDef::Struct(_) => 0,
                    VariantDef::Variant(v) => v.index,
                };
                self.construct(e, index, values)
            }
            Expr::If { condition, then_branch, else_branch } => {
                self.if_expr(e, *condition, *then_branch, *else_branch)
            }
            Expr::Match { effect_arms, .. } if !effect_arms.is_empty() => {
                Err(ErrorKind::unsupported("a match with effect arms"))
            }
            Expr::Match { expr: scrut, arms, .. } => self.match_expr(e, *scrut, arms),
            Expr::While { condition, body: inner, label } => {
                let exit = self.b.join(&[]);
                let next = self.b.join_rec(&[]);
                self.b.jump(&next, ());
                self.b.switch_to(next.block());
                self.targets.push(Target { label: *label, exit, next: Some(next) });
                let (exit_label, inner) = (self.targets.len() - 1, *inner);
                self.branch_on(
                    *condition,
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
            Expr::For { pat, iterable, body: inner, label } => {
                self.for_loop(e, *pat, *iterable, *inner, *label)
            }
            Expr::Loop { body: inner, label } => {
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
                Ok(target.exit.params().first().map(|&p| Atom::Var(p)))
            }
            Expr::Break { expr: value, label } => {
                let v = match value {
                    Some(x) => val!(self.expr(*x)),
                    None => None,
                };
                let i = self.target(label.as_ref(), false)?;
                let exit = &self.targets[i].exit;
                self.b.jump(exit, v.into_iter().collect::<Vec<_>>());
                Ok(None)
            }
            Expr::Continue { label } => {
                let i = self.target(label.as_ref(), true)?;
                let next = self.targets[i].next.as_ref().unwrap();
                self.b.jump(next, ());
                Ok(None)
            }
            Expr::Return { expr: value } => {
                let v = match value {
                    Some(x) => val!(self.expr(*x)),
                    None => None,
                };
                self.b.ret(v.into_iter().collect::<Vec<_>>());
                Ok(None)
            }
            Expr::Mut { expr: inner } if self.lcx.infer.is_mut_literal(e) => self.expr(*inner),
            Expr::Mut { .. } => Err(ErrorKind::unsupported("`mut e` that isn't a literal")),
            Expr::Imm { .. } => Err(ErrorKind::missing(
                "a deep copy that keeps what's shared and cycles, for `imm e`: it has to tell \
                 objects apart",
            )),
            Expr::Lambda { .. } => self.closure(e),
            Expr::Let { .. } => Err(ErrorKind::unsupported("`let` here")),
            Expr::Missing => Err(ErrorKind::unsupported("code with syntax errors")),
            Expr::Try { .. } | Expr::Range { .. } | Expr::MacroCall { .. } => {
                Err(ErrorKind::unsupported("this expression"))
            }
        }
    }

    /// `for x in xs { .. }`: `iter(xs)` once, then `next` of it until it
    /// gives `None`.
    fn for_loop(
        &mut self,
        e: ExprId,
        pat: PatId,
        iterable: ExprId,
        inner: ExprId,
        label: Option<LabelId>,
    ) -> R<Val> {
        let xs = self.expr(iterable)?;
        if self.ended() {
            return Ok(None);
        }
        let it = self.call_use_ty(Site::Iter(e), vec![xs])?;
        let exit = self.b.join(&[]);
        let next = self.b.join_rec(&[]);
        self.b.jump(&next, ());
        self.b.switch_to(next.block());
        let (item, item_ty) = self.call_use_ty(Site::Next(e), vec![it.0])?;
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
        let Some(Ty::Data(id, ..)) = self.cty(&item_ty)? else {
            return Err(ErrorKind::unsupported("this `for` loop"));
        };
        let item = item.ok_or_else(|| ErrorKind::unsupported("this `for` loop"))?;
        let case = self.b.case(item.clone());
        let ctor = CtorId { data: id, index: some };
        self.b.arm(case, ctor);
        let field_ty = field_ty(
            self,
            &item_ty,
            Ctor::Variant(EnumVariantId { parent: *option, index: some }),
            0,
        )?;
        let x = match self.cty(&field_ty)? {
            Some(_) => Some(Atom::Var(self.b.get(ctor, 0, item))),
            None => None,
        };
        self.bind_pat(pat, x)?;
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

    /// Call the use at `site`, which `boltlang-lower` knows the type of
    /// from what it calls: the result, and its type.
    fn call_use_ty(&mut self, site: Site, args: Vec<Val>) -> R<(Val, MonoTy<'db>)> {
        let u = self
            .uses
            .get(&site)
            .cloned()
            .ok_or_else(|| ErrorKind::unsupported("a call this backend found no use for"))?;
        let Use::Instance { instance, mutabilities, dicts } = u else {
            return Err(ErrorKind::unsupported("this `for` loop"));
        };
        if !dicts.is_empty() {
            return Err(ErrorKind::unsupported("dictionaries"));
        }
        let callee = Cx::new(self.cx.db, instance, &self.cx.erased)?;
        let MonoTy::Fn(_, ret, _) = callee.instance_ty()? else {
            return Err(ErrorKind::unsupported("this `for` loop"));
        };
        // The callee's own mutability parameters are the ones this use gives.
        let ret = subst_muts(&ret, &mutabilities);
        let f = self.cx.funcs[&instance];
        let margs = mutabilities.iter().map(|&m| mutability(m)).collect();
        let results = self.b.call_at(f, margs, args.into_iter().flatten().collect::<Vec<_>>());
        Ok((results.first().map(|&v| Atom::Var(v)), ret))
    }

    /// The type a literal makes: that of the `mut e` it's in, if any.
    fn literal_ty(&self, e: ExprId) -> R<MonoTy<'db>> {
        self.mono(self.mut_literals.get(&e).copied().unwrap_or(e))
    }

    /// The index of the loop or block that `break` or `continue` with
    /// `label` goes to.
    fn target(&self, label: Option<&Name>, is_continue: bool) -> R<usize> {
        self.targets
            .iter()
            .rposition(|t| {
                (!is_continue || t.next.is_some())
                    && match label {
                        Some(name) => t.label.is_some_and(|l| self.body.labels[l].name == *name),
                        None => t.next.is_some(),
                    }
            })
            .ok_or_else(|| ErrorKind::unsupported("this `break` or `continue`"))
    }

    fn statement(&mut self, s: &Statement) -> R<()> {
        match s {
            Statement::Let { pat, initializer: Some(init), else_branch: None, .. } => {
                let v = self.expr(*init)?;
                if self.ended() {
                    return Ok(());
                }
                match &self.body[*pat] {
                    Pat::Wild | Pat::Bind { subpat: None, .. } => self.bind_pat(*pat, v),
                    _ => {
                        let t = self.lcx.pat_ty(*pat)?;
                        self.let_decision(*pat, v, t, None)
                    }
                }
            }
            Statement::Let { pat, initializer: Some(init), else_branch: Some(other), .. } => {
                let v = self.expr(*init)?;
                if self.ended() {
                    return Ok(());
                }
                let t = self.lcx.pat_ty(*pat)?;
                self.let_decision(*pat, v, t, Some(*other))
            }
            Statement::Let { initializer: None, .. } => {
                Err(ErrorKind::unsupported("a `let` without a value"))
            }
            Statement::Expr { expr, .. } => self.expr(*expr).map(|_| ()),
            Statement::Item => Ok(()),
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
                let x: f64 = text
                    .replace('_', "")
                    .trim_end_matches("f32")
                    .trim_end_matches("f64")
                    .parse()
                    .map_err(|_| ErrorKind::unsupported("this literal"))?;
                match Scalar::of(&t)? {
                    Some(Scalar::F32) => Ok(Some(Atom::Lit(Lit::f32(x as f32)))),
                    Some(Scalar::F64) => Ok(Some(Atom::Lit(Lit::f64(x)))),
                    _ => Err(ErrorKind::unsupported("this literal")),
                }
            }
            Literal::Byte(n) => Ok(Some(Atom::Lit(Lit::I32(*n as i32)))),
            Literal::Char(c) => self.construct(e, 0, vec![Atom::Lit(Lit::I32(*c as u32 as i32))]),
            Literal::String(s) => {
                // A `String` holds its UTF-8 bytes, as `[u8]`.
                let bytes: Vec<Atom> = s.bytes().map(|b| Atom::Lit(Lit::I32(b as i32))).collect();
                let xs = self.b.array_new(Ty::I32, Mutability::Imm, bytes);
                self.construct(e, 0, vec![Atom::Var(xs)])
            }
            Literal::ByteString(_) | Literal::CString(_) => {
                Err(ErrorKind::unsupported("byte strings and C strings"))
            }
        }
    }

    fn path(&mut self, e: ExprId, path: &Path) -> R<Val> {
        match self.resolve(e, path) {
            Some(ValueNs::Local(id)) => match self.locals.get(&id).cloned() {
                Some(Local::Value(v)) => Ok(v),
                Some(Local::Ref(r)) => Ok(Some(Atom::Var(self.b.ref_get(r)))),
                None => Err(ErrorKind::unsupported("a local that isn't in scope")),
            },
            Some(ValueNs::Builtin(BuiltinValue::True)) => Ok(Some(Atom::Lit(Lit::I32(1)))),
            Some(ValueNs::Builtin(BuiltinValue::False)) => Ok(Some(Atom::Lit(Lit::I32(0)))),
            Some(ValueNs::Variant(v)) => {
                if !matches!(self.mono(e)?, MonoTy::Adt(..)) {
                    return Err(ErrorKind::unsupported("a variant as a function"));
                }
                self.variant(e, v, Vec::new())
            }
            Some(ValueNs::Const(_)) => self.call_use(Site::Expr(e), Vec::new(), e),
            Some(ValueNs::Function(_)) => self.function_value(e),
            _ => Err(ErrorKind::unsupported("this name")),
        }
    }

    /// A named function as a value: a closure of its instance that
    /// captures nothing.
    fn function_value(&mut self, e: ExprId) -> R<Val> {
        let Some(Use::Instance { instance, mutabilities, dicts }) =
            self.uses.get(&Site::Expr(e)).cloned()
        else {
            return Err(ErrorKind::unsupported("a builtin or a host function as a value"));
        };
        if !dicts.is_empty() {
            return Err(ErrorKind::unsupported("dictionaries"));
        }
        let MonoTy::Fn(_, _, m) = self.mono(e)? else {
            return Err(ErrorKind::unsupported("a function whose type isn't a function"));
        };
        let f = self.cx.funcs[&instance];
        let margs = mutabilities.iter().map(|&m| mutability(m)).collect();
        Ok(Some(Atom::Var(self.b.closure_at(f, margs, mutability(m), Vec::<Atom>::new()))))
    }

    /// A value of the variant `v`, of the type of `e`, with these fields.
    fn variant(&mut self, e: ExprId, v: EnumVariantId<'db>, fields: Vec<Atom>) -> R<Val> {
        if is_fieldless(self.cx.db, v.parent) {
            return Ok(Some(Atom::Lit(Lit::I32(v.index as i32))));
        }
        self.construct(e, v.index, fields)
    }

    /// A new object of the type of `e`, made with its constructor `index`.
    fn construct(&mut self, e: ExprId, index: u32, fields: Vec<Atom>) -> R<Val> {
        let t = self.literal_ty(e)?;
        let Some(Ty::Data(id, m, margs, _)) = self.cty(&t)? else {
            return Err(ErrorKind::unsupported("this value"));
        };
        Ok(Some(Atom::Var(self.b.new_object_with(CtorId { data: id, index }, m, margs, fields))))
    }

    /// The struct or variant a struct literal makes.
    fn record_def(&self, e: ExprId, path: Option<&Path>) -> R<VariantDef<'db>> {
        if let Some(path) = path
            && let Some(ValueNs::Variant(v)) = self.resolve(e, path)
        {
            return Ok(VariantDef::Variant(v));
        }
        match self.mono(e)? {
            MonoTy::Adt(AdtId::Struct(s), ..) => Ok(VariantDef::Struct(s)),
            _ => Err(ErrorKind::unsupported("this struct literal")),
        }
    }

    /// The constructor and index of the field `name` of a value of type `t`.
    fn field_of(&mut self, t: &MonoTy<'db>, name: &Name) -> R<(CtorId, u32)> {
        let Some(Ty::Data(id, ..)) = self.cty(t)? else {
            return Err(ErrorKind::unsupported("a field of this type"));
        };
        let index = match t {
            MonoTy::Tuple(_) => name.as_str().parse::<u32>().ok(),
            MonoTy::Adt(AdtId::Struct(s), ..) => {
                hir_field_types(self.cx.db, VariantDef::Struct(*s))
                    .field(name)
                    .map(|(i, _)| i as u32)
            }
            _ => None,
        };
        let index = index.ok_or_else(|| ErrorKind::unsupported("this field"))?;
        Ok((CtorId { data: id, index: 0 }, index))
    }

    fn call(&mut self, e: ExprId, callee: ExprId, args: &[ExprId]) -> R<Val> {
        macro_rules! val {
            ($x:expr) => {{
                let v = $x?;
                if self.ended() {
                    return Ok(None);
                }
                v
            }};
        }
        let mut values = Vec::new();
        if self.lcx.infer.method_calls.get(e).is_some() {
            let Expr::Field { expr: receiver, .. } = &self.body[callee] else {
                return Err(ErrorKind::unsupported("this method call"));
            };
            values.push(val!(self.expr(*receiver)));
        }
        if self.uses.contains_key(&Site::Expr(callee)) {
            for &a in args {
                values.push(val!(self.expr(a)));
            }
            return self.call_use(Site::Expr(callee), values, e);
        }
        if let Some(ValueNs::Variant(v)) = self.named(callee) {
            for &a in args {
                values.push(val!(self.expr(a)));
            }
            return self.variant(e, v, values.into_iter().flatten().collect());
        }
        // A value that's a function, like a closure: `apply` it.
        let f = val!(self.expr(callee)).ok_or_else(|| ErrorKind::unsupported("calling this"))?;
        for &a in args {
            values.push(val!(self.expr(a)));
        }
        let args: Vec<Atom> = values.into_iter().flatten().collect();
        let results = self.b.apply(f, args);
        if matches!(self.mono(e)?, MonoTy::Never) {
            self.b.unreachable();
        }
        Ok(results.first().map(|&v| Atom::Var(v)))
    }

    /// Call what the use at `site` is, with these arguments, for the
    /// expression `e`, whose type is the result.
    fn call_use(&mut self, site: Site, args: Vec<Val>, e: ExprId) -> R<Val> {
        let u = self
            .uses
            .get(&site)
            .cloned()
            .ok_or_else(|| ErrorKind::unsupported("a call this backend found no use for"))?;
        let args: Vec<Atom> = args.into_iter().flatten().collect();
        let result_ty = self.mono(e)?;
        let results = match u {
            Use::Instance { instance, mutabilities, dicts } => {
                if !dicts.is_empty() {
                    return Err(ErrorKind::unsupported("dictionaries"));
                }
                let f = self.cx.funcs[&instance];
                let margs = mutabilities.iter().map(|&m| mutability(m)).collect();
                self.b.call_at(f, margs, args)
            }
            Use::Extern(f) => {
                let results = self.b.call_extern(self.cx.externs[&ExternKey::Function(f)], args);
                // A host function can return although it's declared not to,
                // and nothing in Bolt can promise it won't: stop then.
                if matches!(result_ty, MonoTy::Never) {
                    self.b.trap();
                }
                results
            }
            Use::Builtin(builtin) => return self.builtin(builtin, args, &result_ty),
            Use::Method { .. } => return Err(ErrorKind::unsupported("dictionaries")),
        };
        Ok(results.first().map(|&v| Atom::Var(v)))
    }

    fn builtin(&mut self, builtin: Builtin, args: Vec<Atom>, result: &MonoTy<'db>) -> R<Val> {
        let one = |v: conduit_ir::Var| Ok(Some(Atom::Var(v)));
        match builtin {
            Builtin::Prim(op, t) => self.prim(op, Scalar::of(&MonoTy::Builtin(t))?.unwrap(), args),
            // Only a handler has a continuation, and handlers aren't lowered.
            Builtin::Resume => Err(ErrorKind::unsupported("resuming a continuation")),
            Builtin::ArrayLen => one(self.b.array_len(args[0].clone())),
            Builtin::ArrayGet => one(self.b.array_get(args[0].clone(), args[1].clone())),
            Builtin::ArraySet => {
                self.b.array_set(args[0].clone(), args[1].clone(), args[2].clone());
                Ok(None)
            }
            // A copy that can't be changed. Conduit's `copies` pass makes it
            // a freeze when nothing else has the array.
            Builtin::ArrayFreeze => one(self.b.array_clone(Mutability::Imm, args[0].clone())),
            Builtin::Print => {
                let Some(print) = self.cx.helpers.print else {
                    return Err(ErrorKind::unsupported("this `print`"));
                };
                self.b.call(print, args);
                Ok(None)
            }
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
        let sign = if s.signed() { Sign::Signed } else { Sign::Unsigned };
        let float = matches!(s, Scalar::F32 | Scalar::F64);
        let p = match op {
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
                return Ok(Some(self.wrap(s, Atom::Var(v))));
            }
            PrimOp::Not => {
                // `!` of a `Bool` flips its bit, and of an integer all of them.
                let mask = if s == Scalar::Bool { int_lit(s, 1) } else { int_lit(s, -1) };
                let v = self.b.prim(Prim::Xor, [args[0].clone(), mask]);
                return Ok(Some(self.wrap(s, Atom::Var(v))));
            }
        };
        let mut args = args;
        // The amount of a shift is a `u32`, which a 64-bit shift needs as an
        // `i64`.
        if matches!(op, PrimOp::Shl | PrimOp::Shr) && s.ty() == Ty::I64 {
            let n = self.b.convert(Ty::I64, Sign::Unsigned, args[1].clone());
            args[1] = Atom::Var(n);
        }
        let v = Atom::Var(self.b.prim(p, args));
        let arith =
            matches!(op, PrimOp::Add | PrimOp::Sub | PrimOp::Mul | PrimOp::Div | PrimOp::Shl);
        Ok(Some(if arith { self.wrap(s, v) } else { v }))
    }

    /// An 8- or 16-bit integer in an `i32`, back in range after arithmetic.
    fn wrap(&mut self, s: Scalar, v: Atom) -> Atom {
        let Scalar::Int { bits: bits @ (8 | 16), signed } = s else { return v };
        if signed {
            let n = 32 - bits as i32;
            let up = self.b.prim(Prim::Shl, [v, Atom::Lit(Lit::I32(n))]);
            Atom::Var(self.b.prim(Prim::Shr(Sign::Signed), [Atom::Var(up), Atom::Lit(Lit::I32(n))]))
        } else {
            let mask = (1i32 << bits) - 1;
            Atom::Var(self.b.prim(Prim::And, [v, Atom::Lit(Lit::I32(mask))]))
        }
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
        let sign = if from.signed() { Sign::Signed } else { Sign::Unsigned };
        let v = if from.ty() == to.ty() { v } else { Atom::Var(self.b.convert(to.ty(), sign, v)) };
        Ok(Some(self.wrap(to, v)))
    }

    fn assign(&mut self, e: ExprId, compound: bool, lhs: ExprId, rhs: ExprId) -> R<()> {
        // What `lhs` is, evaluated once, and its old value for `+=`.
        enum Place {
            Local(conduit_ir::Var),
            Field(CtorId, u32, Atom),
            Index(Val, Val),
        }
        let body = self.body;
        let place = match &body[lhs] {
            Expr::Path(path) => match self.resolve(lhs, path) {
                Some(ValueNs::Local(id)) => match self.locals.get(&id) {
                    Some(Local::Ref(r)) => Place::Local(*r),
                    Some(Local::Value(None)) => return self.expr(rhs).map(|_| ()),
                    _ => return Err(ErrorKind::unsupported("assigning to this local")),
                },
                _ => return Err(ErrorKind::unsupported("assigning to this")),
            },
            Expr::Field { expr: base, name } => {
                let t = self.mono(*base)?;
                let obj = self.expr(*base)?;
                if self.ended() {
                    return Ok(());
                }
                let (ctor, i) = self.field_of(&t, name)?;
                Place::Field(ctor, i, obj.ok_or_else(|| ErrorKind::unsupported("this"))?)
            }
            Expr::Index { base, index } => {
                let xs = self.expr(*base)?;
                if self.ended() {
                    return Ok(());
                }
                let i = self.expr(*index)?;
                if self.ended() {
                    return Ok(());
                }
                Place::Index(xs, i)
            }
            _ => return Err(ErrorKind::unsupported("assigning to this")),
        };
        let mut value = self.expr(rhs)?;
        if self.ended() {
            return Ok(());
        }
        if compound {
            let old = match &place {
                Place::Local(r) => Some(Atom::Var(self.b.ref_get(*r))),
                Place::Field(c, i, obj) => Some(Atom::Var(self.b.get(*c, *i, obj.clone()))),
                Place::Index(xs, i) => {
                    self.call_use(Site::Expr(lhs), vec![xs.clone(), i.clone()], lhs)?
                }
            };
            value = self.call_use(Site::Expr(e), vec![old, value], lhs)?;
        }
        match place {
            Place::Local(r) => {
                if let Some(v) = value {
                    self.b.ref_set(r, v);
                }
            }
            Place::Field(c, i, obj) => {
                if let Some(v) = value {
                    self.b.set(c, i, obj, v);
                }
            }
            Place::Index(xs, i) => {
                self.call_use(Site::SetIndex(e), vec![xs, i, value], e)?;
            }
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
                l.b.jump(&k, v.into_iter().collect::<Vec<_>>());
            }
            Ok(())
        };
        let branch = std::cell::RefCell::new(&mut branch);
        self.branch_on(c, &mut |l| (branch.borrow_mut())(l, Some(then)), &mut |l| {
            (branch.borrow_mut())(l, other)
        })?;
        self.b.switch_to(k.block());
        Ok(k.params().first().map(|&p| Atom::Var(p)))
    }

    fn match_expr(
        &mut self,
        e: ExprId,
        scrut: ExprId,
        arms: &[boltlang_hir_def::hir::MatchArm],
    ) -> R<Val> {
        let t = self.mono(scrut)?;
        let v = self.expr(scrut)?;
        if self.ended() {
            return Ok(None);
        }
        let decisions = body_decisions(self.cx.db, self.def);
        let d = decisions
            .matches
            .iter()
            .find(|(m, _)| *m == e)
            .and_then(|(_, d)| d.as_ref().ok())
            .ok_or_else(|| ErrorKind::unsupported("this match"))?
            .clone();
        let tys: Vec<Ty> = self.cty(&self.mono(e)?)?.into_iter().collect();
        let k = self.b.join(&tys);
        let bodies: Vec<ExprId> = arms.iter().map(|a| a.expr).collect();
        self.decide(&d, v, t, false, &mut |l, arm| {
            let v = l.expr(bodies[arm as usize])?;
            if !l.ended() {
                l.b.jump(&k, v.into_iter().collect::<Vec<_>>());
            }
            Ok(())
        })?;
        self.b.switch_to(k.block());
        Ok(k.params().first().map(|&p| Atom::Var(p)))
    }

    /// A `let` with a pattern: its bindings go on after it, and a value it
    /// doesn't match runs `other`, which never ends.
    fn let_decision(&mut self, pat: PatId, v: Val, t: MonoTy<'db>, other: Option<ExprId>) -> R<()> {
        let d = self.let_decision_of(pat)?;
        // The bindings' values go on to the code after the `let`.
        let mut bindings: Vec<BindingId> = Vec::new();
        collect_bindings(self.body, pat, &mut bindings);
        let mut tys = Vec::new();
        let mut has_value = Vec::new();
        for &b in &bindings {
            let t = self.cty(&self.lcx.binding_ty(b)?)?;
            has_value.push(t.is_some());
            tys.extend(t);
        }
        let k = self.b.join(&tys);
        self.decide(&d, v, t, other.is_some(), &mut |l, arm| {
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
            for &b in &bindings {
                match l.locals.get(&b).cloned() {
                    Some(Local::Value(Some(v))) => args.push(v),
                    Some(Local::Ref(r)) => args.push(Atom::Var(l.b.ref_get(r))),
                    _ => {}
                }
            }
            l.b.jump(&k, args);
            Ok(())
        })?;
        self.b.switch_to(k.block());
        let mut params = k.params().iter();
        for (&b, has) in bindings.iter().zip(has_value) {
            let v = if has { params.next().map(|&p| Atom::Var(p)) } else { None };
            self.bind(b, v)?;
        }
        Ok(())
    }

    /// Lower a decision on `value` of type `t`: `arm` lowers the body of
    /// an arm, with its bindings bound, and ends its block.
    fn decide(
        &mut self,
        d: &Decision<'db>,
        value: Val,
        t: MonoTy<'db>,
        fail_arm: bool,
        arm: &mut dyn FnMut(&mut Self, u32) -> R<()>,
    ) -> R<()> {
        let mut dl = DecisionLower::new(self, d, value, t)?;
        dl.fail_arm = fail_arm;
        dl.run(self, arm)
    }

    /// The decision of the `let` pattern `pat`.
    fn let_decision_of(&self, pat: PatId) -> R<Decision<'db>> {
        body_decisions(self.cx.db, self.def)
            .lets
            .iter()
            .find(|(p, _)| *p == pat)
            .and_then(|(_, d)| d.as_ref().ok())
            .cloned()
            .ok_or_else(|| ErrorKind::unsupported("this pattern"))
    }

    /// Branch on a condition: `then` runs if it holds, with the bindings of
    /// its `let` if it's one, and `other` if not. Each ends its block.
    fn branch_on(
        &mut self,
        c: ExprId,
        then: &mut dyn FnMut(&mut Self) -> R<()>,
        other: &mut dyn FnMut(&mut Self) -> R<()>,
    ) -> R<()> {
        if let Expr::Let { pat, expr } = &self.body[c] {
            let (pat, expr) = (*pat, *expr);
            let t = self.mono(expr)?;
            let v = self.expr(expr)?;
            if self.ended() {
                return Ok(());
            }
            let d = self.let_decision_of(pat)?;
            return self.decide(&d, v, t, true, &mut |l, arm| {
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

/// `t` with the `i`th mutability parameter replaced by `args[i]`.
fn subst_muts<'db>(t: &MonoTy<'db>, args: &[boltlang_lower::MonoMut]) -> MonoTy<'db> {
    use boltlang_lower::MonoMut;
    let m = |m: &MonoMut| match m {
        MonoMut::Param(i) => args.get(*i as usize).copied().unwrap_or(*m),
        other => *other,
    };
    match t {
        MonoTy::Adt(adt, tys, muts, own) => MonoTy::Adt(
            *adt,
            tys.iter().map(|t| subst_muts(t, args)).collect(),
            muts.iter().map(m).collect(),
            m(own),
        ),
        MonoTy::Tuple(tys) => MonoTy::Tuple(tys.iter().map(|t| subst_muts(t, args)).collect()),
        MonoTy::Array(own, elem) => MonoTy::Array(m(own), Box::new(subst_muts(elem, args))),
        MonoTy::Fn(ps, r, own) => MonoTy::Fn(
            ps.iter().map(|t| subst_muts(t, args)).collect(),
            Box::new(subst_muts(r, args)),
            m(own),
        ),
        other => other.clone(),
    }
}

/// The bindings of a pattern, in order.
fn collect_bindings(body: &Body, pat: PatId, out: &mut Vec<BindingId>) {
    match &body[pat] {
        Pat::Bind { id, subpat } => {
            out.push(*id);
            if let Some(p) = subpat {
                collect_bindings(body, *p, out);
            }
        }
        other => {
            for p in other.children() {
                collect_bindings(body, p, out);
            }
        }
    }
}

/// An integer or `Bool` literal of a scalar type, wrapped to its range.
fn int_lit(s: Scalar, n: i128) -> Atom {
    match s {
        Scalar::Int { bits: 64, .. } => Atom::Lit(Lit::I64(n as i64)),
        Scalar::Int { bits, signed: true } if bits < 32 => {
            let shift = 32 - bits as u32;
            Atom::Lit(Lit::I32(((n as i32) << shift) >> shift))
        }
        Scalar::Int { bits, signed: false } if bits < 32 => {
            Atom::Lit(Lit::I32((n as i32) & ((1 << bits) - 1)))
        }
        _ => Atom::Lit(Lit::I32(n as i32)),
    }
}

/// Lowering one decision. A node that more than one test leads to is a
/// join point, so that the code after it is made once. Its parameters are
/// the fields of variants it reads that it can't read itself, since a
/// field of a variant can only be read where a `case` found the variant.
struct DecisionLower<'d, 'db> {
    d: &'d Decision<'db>,
    /// Whether a value no arm takes goes to the callback, as [`FAIL`].
    fail_arm: bool,
    scrut: Val,
    place_tys: Vec<MonoTy<'db>>,
    joins: HashMap<NodeId, (JoinPoint, Vec<PlaceId>)>,
}

impl<'d, 'db> DecisionLower<'d, 'db> {
    fn new<'a, 'm>(
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
                    let of_ty = place_tys[*of as usize].clone();
                    field_ty(l, &of_ty, *ctor, *index)?
                }
            };
            place_tys.push(ty);
        }
        Ok(DecisionLower { d, fail_arm: false, scrut, place_tys, joins: HashMap::new() })
    }

    fn run<'a, 'm>(
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
                if let Some(t) = l.cty(&self.place_tys[p as usize].clone())? {
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
                let of_ty = self.place_tys[of as usize].clone();
                let Some(Ty::Data(id, ..)) = l.cty(&of_ty)? else {
                    return Err(ErrorKind::unsupported("a field of this type"));
                };
                let c = match ctor {
                    Ctor::Variant(v) => v.index,
                    Ctor::Tuple(_) | Ctor::Struct(_) => 0,
                };
                // A field of a type with no values is none.
                match (obj, l.cty(&self.place_tys[p as usize].clone())?) {
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
                let t = self.place_tys[*place as usize].clone();
                let v = self
                    .place(l, *place, env)?
                    .ok_or_else(|| ErrorKind::unsupported("this match"))?;
                // A `Char` is tested by its code point.
                let v = match (&t, cases.first()) {
                    (MonoTy::Adt(..), Some((Case::Char(_), _))) => {
                        let Some(Ty::Data(id, ..)) = l.cty(&t)? else {
                            return Err(ErrorKind::unsupported("this match"));
                        };
                        Atom::Var(l.b.get(CtorId { data: id, index: 0 }, 0, v))
                    }
                    _ => v,
                };
                let data = match l.cty(&t)? {
                    Some(Ty::Data(id, ..))
                        if !matches!(cases.first(), Some((Case::Char(_), _))) =>
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
                let t = self.place_tys[*place as usize].clone();
                let mut v = self
                    .place(l, *place, env)?
                    .ok_or_else(|| ErrorKind::unsupported("this match"))?;
                let s = match Scalar::of(&t)? {
                    Some(s) => s,
                    None => {
                        // A `Char`, by its code point.
                        let Some(Ty::Data(id, ..)) = l.cty(&t)? else {
                            return Err(ErrorKind::unsupported("this pattern"));
                        };
                        v = Atom::Var(l.b.get(CtorId { data: id, index: 0 }, 0, v));
                        Scalar::Int { bits: 32, signed: false }
                    }
                };
                let sign = if s.signed() { Sign::Signed } else { Sign::Unsigned };
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
            Node::Guard { guard, bindings, then, otherwise, .. } => {
                self.bind(l, bindings, env)?;
                let Some(c) = l.expr(*guard)? else { return Ok(()) };
                self.branch(l, c, *then, *otherwise, env, arm)
            }
            Node::Arm { arm: index, bindings } => {
                self.bind(l, bindings, env)?;
                arm(l, *index)
            }
            Node::Fail if self.fail_arm => arm(l, FAIL),
            Node::Fail => {
                // Say so on stderr, then stop: reaching this is defined, so
                // it's a `trap`, not an `unreachable`.
                if let Some(unmatched) = l.cx.helpers.unmatched {
                    l.b.call(unmatched, ());
                }
                l.b.trap();
                Ok(())
            }
        }
    }

    fn bind<'a, 'm>(
        &self,
        l: &mut Lower<'a, 'm, 'db>,
        bindings: &[(BindingId, PlaceId)],
        env: &mut HashMap<PlaceId, Val>,
    ) -> R<()> {
        for &(b, p) in bindings {
            let v = self.place(l, p, env)?;
            l.bind(b, v)?;
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

fn parse_float(text: &str) -> R<f64> {
    text.replace('_', "")
        .trim_end_matches("f32")
        .trim_end_matches("f64")
        .parse()
        .map_err(|_| ErrorKind::unsupported("this literal"))
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

/// The type of field `index` of a value of type `t` made by `ctor`.
fn field_ty<'db>(
    l: &Lower<'_, '_, 'db>,
    t: &MonoTy<'db>,
    ctor: Ctor<'db>,
    index: u32,
) -> R<MonoTy<'db>> {
    let unknown = |_| ErrorKind::unsupported("a field whose type isn't known");
    let fields = match (ctor, t) {
        (Ctor::Tuple(_), MonoTy::Tuple(elems)) => elems.to_vec(),
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
