//! Building the MIR of a body from its HIR and its inference.
//!
//! Each node of HIR becomes one of MIR, with what the checker found for it
//! written in: its type, what its names refer to, and the function that an
//! operator, a method call, an index or a `for` calls. Nothing is
//! desugared: that's for passes. What isn't supported yet becomes an
//! [`ExprKind::Unsupported`] node, and the rest of the body is built.

use std::collections::HashMap;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::{Body as HirBody, DefWithBodyId, body},
    builtin_type::{BuiltinInt, BuiltinType, BuiltinValue},
    hir::{
        self, BinaryOp, EffectArm, Expr, MatchArm, Pat, PatId, Path, PathSegment, Spread,
        Statement, TupleItem,
    },
    ids::{ConstId, FunctionId, ItemContainerId},
    resolver::{GenericDefId, Resolver, TypeNs, ValueNs, ValueResolution},
};
use boltlang_hir_ty::{
    AdtId, InferenceResult, Ty,
    builtin::builtin,
    function_scheme, infer_body,
    instances::class_args,
    lang::LangClass,
    lower::{VariantDef, field_types},
    scheme::generic_param_ids,
};
use boltlang_mir::{
    Arm, Body, BodyOwner, Callee, Capture, ClassItem, ExprId, ExprKind, ItemRef, LabelId, Local,
    LocalDecl, OpArms, Pattern, Place, Stmt,
};
use boltlang_pattern::decision::{
    BodyDecisions, Ctor, Decision, FieldName, OperationDecision, body_decisions,
};

use crate::lambda::body_lambdas;

/// The MIR of a body, and of each lambda in it, in the order they're
/// written.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct MirBodies<'db> {
    pub body: Body<'db>,
    pub lambdas: Vec<Body<'db>>,
}

impl<'db> MirBodies<'db> {
    /// The body of the lambda `expr`.
    pub fn lambda(&self, expr: hir::ExprId) -> Option<&Body<'db>> {
        self.lambdas
            .iter()
            .find(|b| matches!(b.owner, BodyOwner::Lambda { expr: e, .. } if e == expr))
    }
}

/// The MIR of the body `def` and of its lambdas.
#[salsa::tracked(returns(ref))]
pub fn mir_bodies<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> MirBodies<'db> {
    let hir = body(db, def);
    let cx = Cx {
        db,
        def,
        file: def.def(db).file(db),
        hir,
        infer: infer_body(db, def),
        decisions: body_decisions(db, def),
        mut_literals: infer_body(db, def).mut_literals.iter().map(|&(o, i)| (i, o)).collect(),
    };
    let mut lambdas = Vec::new();
    let mut b = Builder::new(&cx, BodyOwner::Def(def), &mut lambdas);
    let root = if b.params(&hir.params) {
        b.expr(hir.body_expr)
    } else {
        b.unsupported(hir.body_expr, "patterns in parameters")
    };
    let mut body = b.finish();
    body.root = root;
    if let boltlang_hir_def::body::DefWithBody::Function(f) = def.def(db) {
        let params = &boltlang_hir_def::signatures::function_signature(db, f).params;
        if params.iter().any(|p| p.kind != hir::ParamKind::Positional) {
            body.param_kinds = params.iter().map(|p| (p.kind, p.name.clone())).collect();
        }
    }
    MirBodies { body, lambdas }
}

/// What the builders of a body and its lambdas share.
struct Cx<'a, 'db> {
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    file: File,
    hir: &'a HirBody,
    infer: &'db InferenceResult<'db>,
    decisions: &'a BodyDecisions<'db>,
    /// The `e` of each `mut e` that makes a literal `mut`, with the `mut e`.
    mut_literals: HashMap<hir::ExprId, hir::ExprId>,
}

/// A block or loop that `break` can leave.
struct Scope {
    /// Its name in the source, if it has one.
    name: Option<Name>,
    label: LabelId,
    is_loop: bool,
}

struct Builder<'a, 'c, 'db> {
    cx: &'c Cx<'a, 'db>,
    db: &'db dyn Db,
    mir: Body<'db>,
    scopes: Vec<Scope>,
    /// The bodies of the lambdas found so far.
    lambdas: &'c mut Vec<Body<'db>>,
}

impl<'a, 'c, 'db> Builder<'a, 'c, 'db> {
    fn new(cx: &'c Cx<'a, 'db>, owner: BodyOwner<'db>, lambdas: &'c mut Vec<Body<'db>>) -> Self {
        Builder { cx, db: cx.db, mir: Body::new(owner), scopes: Vec::new(), lambdas }
    }

    fn finish(self) -> Body<'db> {
        self.mir
    }

    fn ty(&self, e: hir::ExprId) -> Ty<'db> {
        self.cx.infer.expr_ty(e).clone()
    }

    fn node(&mut self, e: hir::ExprId, kind: ExprKind<'db>) -> ExprId {
        let ty = self.ty(e);
        self.mir.alloc(kind, ty, Some(e))
    }

    fn unsupported(&mut self, e: hir::ExprId, what: &'static str) -> ExprId {
        self.node(e, ExprKind::Unsupported(what))
    }

    /// Declare the local of a binding, once.
    fn declare(&mut self, b: hir::BindingId) -> Local {
        if self.mir.bindings.get(b).is_none() {
            let binding = &self.cx.hir.bindings[b];
            let decl = LocalDecl {
                name: Some(binding.name.clone()),
                ty: self.cx.infer.binding_ty(b).clone(),
                mutable: binding.mutable,
            };
            self.mir.bindings.insert(b, decl);
        }
        Local::Binding(b)
    }

    /// Declare the locals that the patterns `pats` bind.
    fn declare_in(&mut self, pats: &[PatId]) {
        for (_, b) in self.cx.hir.binds_in(pats) {
            self.declare(b);
        }
    }

    /// The local of a pattern that can't fail: `x`, or `_` for none. Other
    /// patterns give `None`.
    fn simple_pattern(&mut self, pat: PatId) -> Option<Option<Local>> {
        match &self.cx.hir[pat] {
            Pat::Wild => Some(None),
            Pat::Bind { id, subpat: None } => Some(Some(self.declare(*id))),
            _ => None,
        }
    }

    /// The parameters of a function or a lambda, which must be simple.
    /// Returns whether they are.
    fn params(&mut self, pats: &[PatId]) -> bool {
        for &pat in pats {
            match self.simple_pattern(pat) {
                Some(Some(local)) => self.mir.params.push(local),
                Some(None) => {
                    let ty = self.cx.infer.pat_ty(pat).clone();
                    let local = self.mir.new_temp(None, ty);
                    self.mir.params.push(local);
                }
                None => return false,
            }
        }
        true
    }

    fn let_decision(&self, pat: PatId) -> Option<Decision<'db>> {
        let (_, found) = self.cx.decisions.lets.iter().find(|(p, _)| *p == pat)?;
        found.as_ref().ok().cloned()
    }

    // Names.

    fn resolve(&self, e: hir::ExprId, path: &Path) -> Option<ValueResolution<'db>> {
        let db = self.db;
        match Resolver::for_expr(db, self.cx.def, e).resolve_path_in_value_ns(db, path)? {
            ValueResolution::Value(ValueNs::Overloaded(_)) => {
                self.cx.infer.overload(e).map(|f| ValueResolution::Value(ValueNs::Function(f)))
            }
            other => Some(other),
        }
    }

    /// `e` as a path, for one like `Shape.Circle`, which is a field of a
    /// name that isn't a value.
    fn expr_path(&self, e: hir::ExprId) -> Option<Path> {
        match &self.cx.hir[e] {
            Expr::Path(path) => Some(path.clone()),
            Expr::Field { expr, name } => {
                let mut segments = self.expr_path(*expr)?.segments.into_vec();
                segments.push(PathSegment { name: name.clone(), generic_args: None });
                Some(Path { segments: segments.into() })
            }
            _ => None,
        }
    }

    /// What `e` names, if it's a path that isn't a field of a local.
    fn named(&self, e: hir::ExprId) -> Option<ValueResolution<'db>> {
        let path = self.expr_path(e)?;
        match self.resolve(e, &path)? {
            ValueResolution::Value(ValueNs::Local(_))
                if !matches!(self.cx.hir[e], Expr::Path(_)) =>
            {
                None
            }
            v => Some(v),
        }
    }

    /// What a use of the function `f` at `at` refers to.
    fn fn_ref(&self, at: hir::ExprId, f: FunctionId<'db>) -> ItemRef<'db> {
        let db = self.db;
        if let Some(b) = builtin(db, f) {
            return ItemRef::Builtin(b);
        }
        if boltlang_hir_ty::builtin::link(db, f).is_some() {
            return ItemRef::Extern(f);
        }
        match f.container(db) {
            ItemContainerId::Effect(_) => ItemRef::Op(f),
            ItemContainerId::Class(class) => {
                let given = self.cx.infer.type_args(at).and_then(|given| {
                    let scheme = function_scheme(db, f).ok()?;
                    Some(class_args(db, class, &scheme.params, given))
                });
                ItemRef::Method {
                    item: ClassItem::Function(f),
                    class_args: given,
                    imp: self.cx.infer.instance(at, class),
                }
            }
            _ => ItemRef::Fn(f),
        }
    }

    fn const_ref(&self, at: hir::ExprId, c: ConstId<'db>) -> ItemRef<'db> {
        let db = self.db;
        match c.container(db) {
            ItemContainerId::Class(class) => {
                let given = self.cx.infer.type_args(at).map(|given| {
                    let params = generic_param_ids(db, [GenericDefId::Class(class)]);
                    class_args(db, class, &params, given)
                });
                ItemRef::Method {
                    item: ClassItem::Const(c),
                    class_args: given,
                    imp: self.cx.infer.instance(at, class),
                }
            }
            _ => ItemRef::Const(c),
        }
    }

    /// The function an operator at `at` calls: the one with its name in
    /// scope, or the overload the checker chose.
    fn operator_ref(&self, at: hir::ExprId, name: &Name) -> Option<ItemRef<'db>> {
        let f = match self.cx.infer.overload(at) {
            Some(f) => f,
            None => match self.resolve(at, &Path::from_name(name.clone()))? {
                ValueResolution::Value(ValueNs::Function(f)) => f,
                _ => return None,
            },
        };
        Some(self.fn_ref(at, f))
    }

    /// The method of a class of the prelude that syntax calls, like `index`
    /// for `xs[i]`, for the class argument `arg`.
    fn lang_ref(&self, at: hir::ExprId, lang: LangClass, arg: Ty<'db>) -> Option<ItemRef<'db>> {
        let class = lang.class(self.db, self.cx.file)?;
        let method = lang.method(self.db, class)?;
        Some(ItemRef::Method {
            item: ClassItem::Function(method),
            class_args: Some(Box::new([arg])),
            imp: self.cx.infer.instance(at, class),
        })
    }

    /// A call of `item` used at the type `ty`, with `args`, for `e`.
    fn call(
        &mut self,
        e: hir::ExprId,
        callee_at: hir::ExprId,
        item: ItemRef<'db>,
        args: Vec<ExprId>,
    ) -> ExprId {
        let params: Vec<Ty<'db>> = args.iter().map(|&a| self.mir[a].ty.clone()).collect();
        let use_ty = Ty::func(params, self.ty(e));
        let callee = self.mir.alloc(ExprKind::Item(item), use_ty, Some(callee_at));
        self.node(e, ExprKind::Call { callee, args: args.into() })
    }

    // Expressions.

    fn exprs(&mut self, es: &[hir::ExprId]) -> Box<[ExprId]> {
        es.iter().map(|&e| self.expr(e)).collect()
    }

    fn expr(&mut self, e: hir::ExprId) -> ExprId {
        let hir = self.cx.hir;
        match &hir[e] {
            Expr::Missing => self.unsupported(e, "code with syntax errors"),
            Expr::Literal(lit) => self.node(e, ExprKind::Literal(lit.clone())),
            Expr::Path(path) => match self.resolve(e, path) {
                Some(r) => self.named_value(e, r),
                None => self.unsupported(e, "this name"),
            },
            Expr::Block { statements, tail, label, .. } => {
                let label = label.map(|l| {
                    let name = hir.labels[l].name.clone();
                    let label = self.mir.new_label(Some(name.clone()));
                    self.scopes.push(Scope { name: Some(name), label, is_loop: false });
                    label
                });
                let stmts = statements.iter().filter_map(|s| self.stmt(s)).collect();
                let tail = tail.map(|t| self.expr(t));
                if label.is_some() {
                    self.scopes.pop();
                }
                self.node(e, ExprKind::Block { stmts, tail, label })
            }
            Expr::Call { callee, args } => self.call_expr(e, *callee, args),
            Expr::Field { expr: base, name } => {
                if let Some(r) = self.named(e) {
                    return self.named_value(e, r);
                }
                let base = self.expr(*base);
                match self.field_index(&self.mir[base].ty.clone(), name) {
                    Some((ctor, index)) => self.node(e, ExprKind::Field { base, ctor, index }),
                    None => self.unsupported(e, "a field of this type"),
                }
            }
            Expr::Binary { op: BinaryOp::Logic(op), lhs, rhs } => {
                let (lhs, rhs) = (self.expr(*lhs), self.expr(*rhs));
                self.node(e, ExprKind::Logic { op: *op, lhs, rhs })
            }
            Expr::Binary { op: BinaryOp::Assign(op), lhs, rhs } => {
                self.assign(e, op.as_ref(), *lhs, *rhs)
            }
            Expr::Binary { op: BinaryOp::Operator(name), lhs, rhs } => {
                let Some(item) = self.operator_ref(e, name) else {
                    return self.unsupported(e, "an operator the checker didn't resolve");
                };
                let args = vec![self.expr(*lhs), self.expr(*rhs)];
                self.call(e, e, item, args)
            }
            Expr::Prefix { op, expr: inner } => {
                let Some(item) = self.operator_ref(e, op) else {
                    return self.unsupported(e, "an operator the checker didn't resolve");
                };
                let args = vec![self.expr(*inner)];
                self.call(e, e, item, args)
            }
            Expr::Index { base, index } => {
                let Some(item) = self.lang_ref(e, LangClass::Index, self.ty(*base)) else {
                    return self.unsupported(e, "an index without the prelude's `Index`");
                };
                let args = vec![self.expr(*base), self.expr(*index)];
                self.call(e, e, item, args)
            }
            Expr::Cast { expr: inner, .. } => {
                let inner = self.expr(*inner);
                self.node(e, ExprKind::Cast { expr: inner })
            }
            Expr::Imm { expr: inner } => {
                let inner = self.expr(*inner);
                self.node(e, ExprKind::Imm { expr: inner })
            }
            // `mut` before a literal makes it `mut`, which the literal's type
            // says: the type of the `mut e`.
            Expr::Mut { expr: inner } if self.cx.infer.is_mut_literal(e) => self.expr(*inner),
            Expr::Mut { .. } => self.unsupported(e, "`mut e` that isn't a literal"),
            Expr::Range { lhs: Some(start), rhs: Some(end), .. } => {
                let fields = Box::new([self.expr(*start), self.expr(*end)]);
                match self.ty(e) {
                    Ty::Adt(AdtId::Struct(s), ..) => {
                        self.node(e, ExprKind::Construct { ctor: Ctor::Struct(s), fields })
                    }
                    _ => self.unsupported(e, "a range without the prelude's `Range`"),
                }
            }
            Expr::Range { .. } => self.unsupported(e, "a range without a start or an end"),
            Expr::NamedTuple { items } => self.named_tuple(e, items),
            Expr::CallWith { callee, args } => self.call_with(e, *callee, args),
            Expr::Tuple { exprs } => {
                let fields = self.exprs(exprs);
                self.literal(
                    e,
                    ExprKind::Construct { ctor: Ctor::Tuple(fields.len() as u32), fields },
                )
            }
            Expr::Array { exprs } => {
                let elems = self.exprs(exprs);
                self.literal(e, ExprKind::Array { elems })
            }
            Expr::ArrayRepeat { value, count } => {
                let (value, count) = (self.expr(*value), self.expr(*count));
                self.literal(e, ExprKind::ArrayRepeat { value, count })
            }
            Expr::Record { path, fields, spread } => {
                if spread.is_some() {
                    return self.unsupported(e, "`..` in a struct literal");
                }
                let def = match path.as_ref().and_then(|p| self.resolve(e, p)) {
                    Some(ValueResolution::Value(ValueNs::Variant(v))) => VariantDef::Variant(v),
                    Some(ValueResolution::Type(r)) if matches!(r.ty, TypeNs::Variant(_)) => {
                        let TypeNs::Variant(v) = r.ty else { unreachable!() };
                        VariantDef::Variant(v)
                    }
                    _ => match self.literal_ty(e) {
                        Ty::Adt(AdtId::Struct(s), ..) => VariantDef::Struct(s),
                        _ => return self.unsupported(e, "this struct literal"),
                    },
                };
                let declared = field_types(self.db, def);
                let mut values: Vec<Option<ExprId>> = vec![None; declared.fields.len()];
                for f in fields.iter() {
                    let v = self.expr(f.expr);
                    match declared.field(&f.name) {
                        Some((i, _)) => values[i] = Some(v),
                        None => return self.unsupported(e, "this field"),
                    }
                }
                let Some(fields) = values.into_iter().collect::<Option<Box<[ExprId]>>>() else {
                    return self.unsupported(e, "a struct literal without all its fields");
                };
                let ctor = match def {
                    VariantDef::Struct(s) => Ctor::Struct(s),
                    VariantDef::Variant(v) => Ctor::Variant(v),
                };
                self.literal(e, ExprKind::Construct { ctor, fields })
            }
            Expr::If { condition, then_branch, else_branch } => {
                let cond = self.cond(*condition);
                let then = self.expr(*then_branch);
                let else_ = else_branch.map(|x| self.expr(x));
                self.node(e, ExprKind::If { cond, then, else_ })
            }
            Expr::Let { .. } => self.unsupported(e, "`let` here"),
            Expr::Match { expr: scrutinee, arms, effect_arms } if !effect_arms.is_empty() => {
                self.handle(e, *scrutinee, arms, effect_arms)
            }
            Expr::Match { expr: scrutinee, arms, .. } => self.match_expr(e, *scrutinee, arms),
            Expr::While { condition, body: inner, label } => {
                let label = self.loop_label(*label);
                let cond = self.cond(*condition);
                let body = self.expr(*inner);
                self.scopes.pop();
                self.node(e, ExprKind::While { cond, body, label })
            }
            Expr::Loop { body: inner, label } => {
                let label = self.loop_label(*label);
                let body = self.expr(*inner);
                self.scopes.pop();
                self.node(e, ExprKind::Loop { body, label })
            }
            Expr::For { pat, iterable, body: inner, label } => {
                self.for_loop(e, *pat, *iterable, *inner, *label)
            }
            Expr::Break { expr: value, label } => {
                let value = value.map(|v| self.expr(v));
                match self.target(label.as_ref(), false) {
                    Some(label) => self.node(e, ExprKind::Break { label, value }),
                    None => self.unsupported(e, "this `break`"),
                }
            }
            Expr::Continue { label } => match self.target(label.as_ref(), true) {
                Some(label) => self.node(e, ExprKind::Continue { label }),
                None => self.unsupported(e, "this `continue`"),
            },
            Expr::Return { expr: value } => {
                let value = value.map(|v| self.expr(v));
                self.node(e, ExprKind::Return { value })
            }
            Expr::Lambda { .. } => self.closure(e),
            Expr::Try { expr: inner, value, error } => self.try_expr(e, *inner, *value, *error),
            Expr::MacroCall { .. } => self.unsupported(e, "a macro"),
        }
    }

    /// The type a literal makes: that of the `mut e` it's in, if any.
    fn literal_ty(&self, e: hir::ExprId) -> Ty<'db> {
        self.ty(self.cx.mut_literals.get(&e).copied().unwrap_or(e))
    }

    /// A node for an array, a struct or a tuple, which `mut` can make `mut`.
    fn literal(&mut self, e: hir::ExprId, kind: ExprKind<'db>) -> ExprId {
        let ty = self.literal_ty(e);
        self.mir.alloc(kind, ty, Some(e))
    }

    /// What a path names, as a value.
    fn named_value(&mut self, e: hir::ExprId, r: ValueResolution<'db>) -> ExprId {
        match r {
            ValueResolution::Value(ValueNs::Local(b)) => {
                let local = self.declare(b);
                self.node(e, ExprKind::Local(local))
            }
            ValueResolution::Value(ValueNs::Builtin(BuiltinValue::True)) => {
                self.node(e, ExprKind::Bool(true))
            }
            ValueResolution::Value(ValueNs::Builtin(BuiltinValue::False)) => {
                self.node(e, ExprKind::Bool(false))
            }
            ValueResolution::Value(ValueNs::Function(f)) => {
                let item = self.fn_ref(e, f);
                self.node(e, ExprKind::Item(item))
            }
            ValueResolution::Value(ValueNs::Const(c)) => {
                let item = self.const_ref(e, c);
                self.node(e, ExprKind::Item(item))
            }
            // A variant or struct with fields, used as a function, is its
            // constructor; without fields, it's a value.
            ValueResolution::Value(ValueNs::Variant(v)) => self.ctor_value(e, Ctor::Variant(v)),
            ValueResolution::Type(r) if r.unresolved.is_none() => match r.ty {
                TypeNs::Struct(s) => self.ctor_value(e, Ctor::Struct(s)),
                _ => self.unsupported(e, "this name"),
            },
            _ => self.unsupported(e, "this name"),
        }
    }

    /// A variant or a struct named as a value: the value, if it has no
    /// fields, and its constructor as a function if it has some.
    fn ctor_value(&mut self, e: hir::ExprId, ctor: Ctor<'db>) -> ExprId {
        match self.ty(e) {
            Ty::Adt(..) => self.literal(e, ExprKind::Construct { ctor, fields: Box::new([]) }),
            Ty::Fn(..) => self.node(e, ExprKind::Item(ItemRef::Ctor(ctor))),
            _ => self.unsupported(e, "this constructor"),
        }
    }

    /// A tuple with named fields or spreads, like `(x: 5, **origin)`. Its
    /// parts are evaluated in the order they're written, each into a
    /// temporary, and then the tuple is made of them in the order of its
    /// fields: the positional ones, then the named ones by name. A spread
    /// gives the fields of its temporary.
    fn named_tuple(&mut self, e: hir::ExprId, items: &[TupleItem<hir::ExprId>]) -> ExprId {
        let mut stmts = Vec::new();
        let Some((mut positional, named)) = self.parts_in_temps(items, &mut stmts) else {
            return self.unsupported(e, "a spread of a tuple with a pack in it");
        };
        positional.extend(named.into_iter().map(|(_, v)| v));
        let ctor = Ctor::Tuple(positional.len() as u32);
        let tuple = self.literal(e, ExprKind::Construct { ctor, fields: positional.into() });
        let ty = self.literal_ty(e);
        let block = ExprKind::Block { stmts: stmts.into(), tail: Some(tuple), label: None };
        self.mir.alloc(block, ty, Some(e))
    }

    /// The parts of a tuple or of the arguments of a call, each evaluated
    /// into a temporary in the order they're written, with a `let` for it
    /// in `stmts`: the positional values, and the named ones in the order
    /// of their names. A spread gives the fields of its temporary. `None`
    /// for a spread of a tuple with a pack in it, whose fields aren't known
    /// before monomorphization.
    #[allow(clippy::type_complexity)]
    fn parts_in_temps(
        &mut self,
        items: &[TupleItem<hir::ExprId>],
        stmts: &mut Vec<Stmt<'db>>,
    ) -> Option<(Vec<ExprId>, Vec<(Name, ExprId)>)> {
        let mut positional = Vec::new();
        let mut named: Vec<(Name, ExprId)> = Vec::new();
        for item in items {
            let x = item.value();
            let ty = self.ty(x);
            if matches!(item, TupleItem::Spread(..)) && ty.has_spread() {
                return None;
            }
            let value = self.expr(x);
            let temp = self.mir.new_temp(None, ty.clone());
            stmts.push(Stmt::Let {
                pat: Pattern::Local(Some(temp)),
                init: Some(value),
                else_: None,
            });
            let local = |b: &mut Self| b.mir.alloc(ExprKind::Local(temp), ty.clone(), Some(x));
            match item {
                TupleItem::Positional(_) => positional.push(local(self)),
                TupleItem::Named(name, _) => named.push((name.clone(), local(self))),
                TupleItem::Spread(spread, _) => {
                    let (tys, fields): (&[Ty<'db>], &[(Name, Ty<'db>)]) = match &ty {
                        Ty::Tuple(tys, _) => (tys, &[]),
                        Ty::Record(tys, fields, _) => (tys, fields),
                        _ => (&[], &[]),
                    };
                    let ctor = Ctor::Tuple((tys.len() + fields.len()) as u32);
                    let field = |b: &mut Self, index: usize, t: &Ty<'db>| {
                        let base = local(b);
                        let kind = ExprKind::Field { base, ctor, index: index as u32 };
                        b.mir.alloc(kind, t.clone(), Some(x))
                    };
                    match spread {
                        Spread::Positional => {
                            for (i, t) in tys.iter().enumerate() {
                                positional.push(field(self, i, t));
                            }
                        }
                        Spread::Named => {
                            for (j, (name, t)) in fields.iter().enumerate() {
                                named.push((name.clone(), field(self, tys.len() + j, t)));
                            }
                        }
                    }
                }
            }
        }
        named.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
        Some((positional, named))
    }

    /// A call with named arguments or spreads, like `f(1, sep: ",")`: its
    /// arguments are evaluated in the order they're written, and passed as
    /// the function's type lays them out: the positional ones, then the
    /// keyword ones in the order of their names. The function's packs must
    /// be known here.
    fn call_with(
        &mut self,
        e: hir::ExprId,
        callee: hir::ExprId,
        items: &[TupleItem<hir::ExprId>],
    ) -> ExprId {
        let hir = self.cx.hir;
        let Ty::Fn(params, ..) = self.ty(callee) else {
            return self.unsupported(e, "a call of this");
        };
        if params.rest.is_some() || params.keyword_rest.is_some() {
            return self.unsupported(e, "a call that passes on a pack");
        }
        let method = self.cx.infer.method_calls.get(e).copied();
        let mut stmts = Vec::new();
        // `x.f(y)`: `x` first.
        let mut all = Vec::with_capacity(items.len() + 1);
        if method.is_some() {
            let Expr::Field { expr: receiver, .. } = &hir[callee] else {
                return self.unsupported(e, "this method call");
            };
            all.push(TupleItem::Positional(*receiver));
        }
        all.extend_from_slice(items);
        let Some((mut values, named)) = self.parts_in_temps(&all, &mut stmts) else {
            return self.unsupported(e, "a spread of a tuple with a pack in it");
        };
        values.extend(named.into_iter().map(|(_, v)| v));
        let call = match method {
            Some(f) => {
                let item = self.fn_ref(callee, f);
                self.call(e, callee, item, values)
            }
            None => {
                let callee = self.expr(callee);
                self.node(e, ExprKind::Call { callee, args: values.into() })
            }
        };
        let block = ExprKind::Block { stmts: stmts.into(), tail: Some(call), label: None };
        let ty = self.ty(e);
        self.mir.alloc(block, ty, Some(e))
    }

    /// The constructor and index of the field `name` of a value of type `t`.
    fn field_index(&self, t: &Ty<'db>, name: &Name) -> Option<(Ctor<'db>, u32)> {
        match t {
            Ty::Tuple(tys, _) => {
                let i = name.as_str().parse::<u32>().ok()?;
                Some((Ctor::Tuple(tys.len() as u32), i))
            }
            // With a pack spread in it, a positional field is only known
            // before the first spread, and a named one is found by its name
            // once monomorphization knows the pack.
            t if t.has_spread() => {
                let parts = t.tuple_parts()?;
                match name.as_str().parse::<usize>() {
                    Ok(i) => {
                        let fixed = parts.positional.iter().take_while(|t| !t.is_spread());
                        (i < fixed.count()).then_some((Ctor::Tuple(0), i as u32))
                    }
                    // After the positional fields, whose number isn't known,
                    // or among those of a named pack.
                    Err(_) => {
                        let field = FieldName::new(self.db, name.clone());
                        Some((Ctor::TupleNamed(field), 0))
                    }
                }
            }
            // `t.0` or `t.x`, where the named fields come after the
            // positional ones, by name.
            Ty::Record(tys, named, _) => {
                let i = match name.as_str().parse::<usize>() {
                    Ok(i) if i < tys.len() => i,
                    Ok(_) => return None,
                    Err(_) => tys.len() + named.iter().position(|(n, _)| n == name)?,
                };
                Some((Ctor::Tuple((tys.len() + named.len()) as u32), i as u32))
            }
            Ty::Adt(AdtId::Struct(s), ..) => {
                let (i, _) = field_types(self.db, VariantDef::Struct(*s)).field(name)?;
                Some((Ctor::Struct(*s), i as u32))
            }
            _ => None,
        }
    }

    fn call_expr(&mut self, e: hir::ExprId, callee: hir::ExprId, args: &[hir::ExprId]) -> ExprId {
        let hir = self.cx.hir;
        // `x.f(y)`: a call of the method `f`, with `x` first.
        if let Some(&f) = self.cx.infer.method_calls.get(e) {
            let Expr::Field { expr: receiver, .. } = &hir[callee] else {
                return self.unsupported(e, "this method call");
            };
            let mut values = vec![self.expr(*receiver)];
            values.extend(args.iter().map(|&a| self.expr(a)));
            let item = self.fn_ref(callee, f);
            return self.call(e, callee, item, values);
        }
        // `Some(x)` and `Wrapper(x)` make a value.
        let ctor = match self.named(callee) {
            Some(ValueResolution::Value(ValueNs::Variant(v))) => Some(Ctor::Variant(v)),
            Some(ValueResolution::Type(r)) if r.unresolved.is_none() => match r.ty {
                TypeNs::Struct(s) => Some(Ctor::Struct(s)),
                _ => None,
            },
            _ => None,
        };
        if let Some(ctor) = ctor {
            let fields = self.exprs(args);
            return self.literal(e, ExprKind::Construct { ctor, fields });
        }
        let callee = self.expr(callee);
        let args = self.exprs(args);
        self.node(e, ExprKind::Call { callee, args })
    }

    fn assign(
        &mut self,
        e: hir::ExprId,
        op: Option<&Name>,
        lhs: hir::ExprId,
        rhs: hir::ExprId,
    ) -> ExprId {
        let hir = self.cx.hir;
        let place = match &hir[lhs] {
            Expr::Path(path) => match self.resolve(lhs, path) {
                Some(ValueResolution::Value(ValueNs::Local(b))) => Place::Local(self.declare(b)),
                _ => return self.unsupported(e, "assigning to this"),
            },
            Expr::Field { expr: base, name } if self.named(lhs).is_none() => {
                let base = self.expr(*base);
                match self.field_index(&self.mir[base].ty.clone(), name) {
                    Some((ctor, index)) => Place::Field { base, ctor, index },
                    None => return self.unsupported(e, "assigning to a field of this type"),
                }
            }
            Expr::Index { base, index } => {
                let base_ty = self.ty(*base);
                let i32_ty = Ty::Builtin(BuiltinType::Int(BuiltinInt::I32));
                let get = match op {
                    Some(_) => match self.lang_ref(lhs, LangClass::Index, base_ty.clone()) {
                        Some(item) => Some(Callee {
                            item,
                            ty: Ty::func([base_ty.clone(), i32_ty.clone()], self.ty(lhs)),
                        }),
                        None => return self.unsupported(e, "an index without `Index`"),
                    },
                    None => None,
                };
                let Some(set) = self.lang_ref(e, LangClass::IndexMut, base_ty.clone()) else {
                    return self.unsupported(e, "an index without the prelude's `IndexMut`");
                };
                let set_ty = Ty::func([base_ty, i32_ty, self.ty(rhs)], Ty::unit());
                let (base, index) = (self.expr(*base), self.expr(*index));
                Place::Index { base, index, get, set: Callee { item: set, ty: set_ty } }
            }
            _ => return self.unsupported(e, "assigning to this"),
        };
        let op = match op {
            Some(name) => match self.operator_ref(e, name) {
                Some(item) => {
                    Some(Callee { item, ty: Ty::func([self.ty(lhs), self.ty(rhs)], self.ty(lhs)) })
                }
                None => return self.unsupported(e, "an operator the checker didn't resolve"),
            },
            None => None,
        };
        let value = self.expr(rhs);
        self.node(e, ExprKind::Assign { place, op, value })
    }

    /// A condition, which can be a `let`.
    fn cond(&mut self, c: hir::ExprId) -> ExprId {
        if let Expr::Let { pat, expr } = &self.cx.hir[c] {
            let (pat, expr) = (*pat, *expr);
            let Some(decision) = self.let_decision(pat) else {
                return self.unsupported(c, "this pattern");
            };
            self.declare_in(&[pat]);
            let scrutinee = self.expr(expr);
            return self.node(c, ExprKind::Let { scrutinee, decision: Box::new(decision) });
        }
        self.expr(c)
    }

    /// `e?`, as `match e { Some(value) => value, None => return None }`, or
    /// for a `Result`, `match e { Ok(value) => value, Err(error) => return
    /// Err(error) }`, with the hidden bindings of the `?`.
    fn try_expr(
        &mut self,
        e: hir::ExprId,
        inner: hir::ExprId,
        value: hir::BindingId,
        error: hir::BindingId,
    ) -> ExprId {
        use boltlang_hir_ty::infer::TryKind;
        use boltlang_pattern::decision::{Case, Node, Place as DecisionPlace};
        let Some(t) = self.cx.infer.tries.get(e).cloned() else {
            return self.unsupported(e, "`e?`");
        };
        let (parent, good, bad, is_result) = match t.kind {
            TryKind::Option(id) => (id, "Some", "None", false),
            TryKind::Result(id) => (id, "Ok", "Err", true),
        };
        let variants = &boltlang_hir_def::signatures::enum_signature(self.db, parent).variants;
        let variant = |name: &str| {
            let index = variants.iter().position(|v| v.name.as_str() == name)?;
            Some(boltlang_hir_def::ids::EnumVariantId { parent, index: index as u32 })
        };
        let (Some(good), Some(bad)) = (variant(good), variant(bad)) else {
            return self.unsupported(e, "`e?`");
        };
        // The value, the field of `Some` or `Ok`, at place 1, and the error,
        // the field of `Err`, at place 2.
        let mut places = vec![
            DecisionPlace::Scrutinee,
            DecisionPlace::Field { of: 0, ctor: Ctor::Variant(good), index: 0 },
        ];
        if is_result {
            places.push(DecisionPlace::Field { of: 0, ctor: Ctor::Variant(bad), index: 0 });
        }
        let nodes = vec![
            Node::Switch {
                place: 0,
                cases: vec![(Case::Variant(good), 1), (Case::Variant(bad), 2)],
                default: None,
            },
            Node::Arm { arm: 0, bindings: vec![(value, 1)] },
            Node::Arm { arm: 1, bindings: if is_result { vec![(error, 2)] } else { Vec::new() } },
        ];
        let decision = Box::new(Decision { places, nodes, root: 0, arms: 2 });
        let scrutinee = self.expr(inner);
        let value_ty = self.cx.infer.binding_ty(value).clone();
        let value = self.declare(value);
        let ok = self.mir.alloc(ExprKind::Local(value), value_ty, Some(e));
        let fields: Box<[ExprId]> = if is_result {
            let error_ty = self.cx.infer.binding_ty(error).clone();
            let error = self.declare(error);
            Box::new([self.mir.alloc(ExprKind::Local(error), error_ty, Some(e))])
        } else {
            Box::new([])
        };
        let back = ExprKind::Construct { ctor: Ctor::Variant(bad), fields };
        let back = self.mir.alloc(back, t.ret.clone(), Some(e));
        let ret = self.mir.alloc(ExprKind::Return { value: Some(back) }, Ty::Never, Some(e));
        let arms = Box::new([Arm { guard: None, body: ok }, Arm { guard: None, body: ret }]);
        self.node(e, ExprKind::Match { scrutinee, decision, arms })
    }

    fn match_expr(&mut self, e: hir::ExprId, scrutinee: hir::ExprId, arms: &[MatchArm]) -> ExprId {
        let found = self.cx.decisions.matches.iter().find(|(m, _)| *m == e);
        let Some((_, Ok(decision))) = found else {
            return self.unsupported(e, "this pattern");
        };
        let decision = Box::new(decision.clone());
        let scrutinee = self.expr(scrutinee);
        let pats: Vec<PatId> = arms.iter().map(|a| a.pat).collect();
        self.declare_in(&pats);
        let arms = arms
            .iter()
            .map(|a| Arm { guard: a.guard.map(|g| self.expr(g)), body: self.expr(a.expr) })
            .collect();
        self.node(e, ExprKind::Match { scrutinee, decision, arms })
    }

    /// A `match` with `effect` arms: a handler of the one effect its arms
    /// are for. The arms of each operation come in the order the decision
    /// has them, and an arm for several operations is built once.
    fn handle(
        &mut self,
        e: hir::ExprId,
        scrutinee: hir::ExprId,
        arms: &[MatchArm],
        effect_arms: &[EffectArm],
    ) -> ExprId {
        let found = self.cx.decisions.matches.iter().find(|(m, _)| *m == e);
        let Some((_, Ok(value))) = found else { return self.unsupported(e, "this pattern") };
        let value = Box::new(value.clone());
        let decisions: Vec<&OperationDecision<'db>> =
            self.cx.decisions.operations.iter().filter(|o| o.expr == e).collect();
        if decisions.iter().any(|d| d.decision.is_err()) {
            return self.unsupported(e, "this pattern");
        }
        let body = self.expr(scrutinee);
        let mut pats: Vec<PatId> = arms.iter().map(|a| a.pat).collect();
        for arm in effect_arms {
            pats.extend(arm.pats());
        }
        self.declare_in(&pats);
        let arms: Box<[Arm]> = arms
            .iter()
            .map(|a| Arm { guard: a.guard.map(|g| self.expr(g)), body: self.expr(a.expr) })
            .collect();
        // Each effect arm, once, with its continuation.
        let mut built: HashMap<usize, (Arm, Option<Local>)> = HashMap::new();
        let mut ops = Vec::new();
        for d in decisions {
            let Ok(decision) = &d.decision else { unreachable!("checked above") };
            let mut op_arms = Vec::new();
            let mut conts = Vec::new();
            for pat in &d.pats {
                let Some(i) = effect_arms.iter().position(|a| a.ops.contains(pat)) else {
                    return self.unsupported(e, "this effect arm");
                };
                if let std::collections::hash_map::Entry::Vacant(slot) = built.entry(i) {
                    let arm = &effect_arms[i];
                    let Some(k) = self.simple_pattern(arm.cont) else {
                        return self.unsupported(e, "this pattern of a continuation");
                    };
                    let guard = arm.guard.map(|g| self.expr(g));
                    slot.insert((Arm { guard, body: self.expr(arm.expr) }, k));
                }
                let (arm, k) = &built[&i];
                op_arms.push(arm.clone());
                conts.push(*k);
            }
            let ty = d.pats.first().map_or(Ty::Error, |&p| self.cx.infer.pat_ty(p).clone());
            ops.push(OpArms {
                op: d.op,
                ty,
                decision: Box::new(decision.clone()),
                arms: op_arms.into(),
                conts: conts.into(),
            });
        }
        self.node(e, ExprKind::Handle { body, value, arms, ops: ops.into() })
    }

    fn stmt(&mut self, s: &Statement) -> Option<Stmt<'db>> {
        Some(match s {
            Statement::Let { pat, initializer, else_branch, .. } => {
                let pattern = match self.simple_pattern(*pat) {
                    Some(local) => Pattern::Local(local),
                    None => match self.let_decision(*pat) {
                        Some(d) => {
                            self.declare_in(&[*pat]);
                            Pattern::Decision(Box::new(d))
                        }
                        None => {
                            let at = initializer.unwrap_or(self.cx.hir.body_expr);
                            return Some(Stmt::Expr(self.unsupported(at, "this pattern")));
                        }
                    },
                };
                let init = initializer.map(|i| self.expr(i));
                let else_ = else_branch.map(|x| self.expr(x));
                Stmt::Let { pat: pattern, init, else_ }
            }
            Statement::Expr { expr, .. } => Stmt::Expr(self.expr(*expr)),
            Statement::Item => return None,
        })
    }

    /// A new label for a loop, which `break` and `continue` can go to.
    fn loop_label(&mut self, label: Option<hir::LabelId>) -> LabelId {
        let name = label.map(|l| self.cx.hir.labels[l].name.clone());
        let mir = self.mir.new_label(name.clone());
        self.scopes.push(Scope { name, label: mir, is_loop: true });
        mir
    }

    /// The label that `break` or `continue` with `name` goes to: the
    /// innermost loop without a name.
    fn target(&self, name: Option<&Name>, is_continue: bool) -> Option<LabelId> {
        let scope = self.scopes.iter().rev().find(|s| {
            (!is_continue || s.is_loop)
                && match name {
                    Some(name) => s.name.as_ref() == Some(name),
                    None => s.is_loop,
                }
        })?;
        Some(scope.label)
    }

    fn for_loop(
        &mut self,
        e: hir::ExprId,
        pat: PatId,
        iterable: hir::ExprId,
        inner: hir::ExprId,
        label: Option<hir::LabelId>,
    ) -> ExprId {
        let db = self.db;
        // `for x in ..`, or a pattern like `(x, y)`, which matches each item
        // as a `let` does.
        let pattern = match self.simple_pattern(pat) {
            Some(local) => Pattern::Local(local),
            None => match self.let_decision(pat) {
                Some(d) => {
                    self.declare_in(&[pat]);
                    Pattern::Decision(Box::new(d))
                }
                None => return self.unsupported(e, "this pattern here"),
            },
        };
        let iterable_ty = self.ty(iterable);
        let found = (|| {
            let iterable_class = LangClass::Iterable.class(db, self.cx.file)?;
            let alias = LangClass::Iterable.assoc(db, iterable_class)?;
            let iter_ty = Ty::Assoc(alias, Box::new([iterable_ty.clone()]));
            let iter = Callee {
                item: self.lang_ref(e, LangClass::Iterable, iterable_ty.clone())?,
                ty: Ty::func([iterable_ty.clone()], iter_ty.clone()),
            };
            let next = Callee {
                item: self.lang_ref(e, LangClass::Iterator, iter_ty.clone())?,
                ty: Ty::func([iter_ty], Ty::Never),
            };
            Some((iter, next))
        })();
        let Some((iter, next)) = found else {
            return self.unsupported(e, "a `for` without the prelude's `Iterable`");
        };
        let iterable = self.expr(iterable);
        let label = self.loop_label(label);
        let body = self.expr(inner);
        self.scopes.pop();
        self.node(e, ExprKind::For { pat: pattern, iterable, body, label, iter, next })
    }

    /// A closure of the lambda `e`, whose body is built too.
    fn closure(&mut self, e: hir::ExprId) -> ExprId {
        let Some(lambda) = body_lambdas(self.db, self.cx.def).iter().find(|l| l.expr == e) else {
            return self.unsupported(e, "this lambda");
        };
        let captures: Box<[Capture]> = lambda
            .captures
            .iter()
            .map(|&b| Capture { local: self.declare(b), by_ref: self.cx.hir.bindings[b].mutable })
            .collect();
        let Expr::Lambda { params, body: inner, .. } = &self.cx.hir[e] else {
            return self.unsupported(e, "this lambda");
        };
        let owner = BodyOwner::Lambda { parent: self.cx.def, expr: e };
        let mut found = Vec::new();
        let mut b = Builder::new(self.cx, owner, &mut found);
        for c in captures.iter() {
            let Local::Binding(id) = c.local else { continue };
            b.declare(id);
        }
        b.mir.captures = captures.to_vec();
        let root = if b.params(params) {
            b.expr(*inner)
        } else {
            b.unsupported(*inner, "patterns in a lambda's parameters")
        };
        let mut body = b.finish();
        body.root = root;
        self.lambdas.push(body);
        self.lambdas.extend(found);
        self.node(e, ExprKind::Closure { lambda: e, captures })
    }
}
