//! Lowering Bolt to the Conduit IR.
//!
//! [`lower`] turns a program, its entry file and its tests, into a Conduit
//! [`Module`]: one function per instance that `boltlang-mir-mono` finds, with
//! the data types their types need. Conduit's functions are generic in
//! mutabilities but not in types, as instances are. Each body is lowered
//! from its MIR, which `boltlang-mir-build` makes.
//!
//! - A function's parameters and results are its values: a parameter or a
//!   result of type `()` or `!` has none.
//! - A local that's assigned is a ref, as Conduit has it, and a promotion
//!   pass in Conduit makes it a variable again.
//! - A `match`, and a `let` with a pattern, in a statement or a condition,
//!   follow the decision that `boltlang-pattern` makes for them.
//! - The host functions, with `#[link(wasm, ..)]`, are Conduit externs, in the
//!   module the block names, like std's `print` calls WASI's `fd_write`. A
//!   `match` that no arm takes calls the prelude's `panic`, if there is
//!   one, and traps. `abort` is a `trap`. An `unreachable` is only where Bolt can promise
//!   that nothing gets, like after a call of a function that never
//!   returns.
//!
//! - A lambda is lifted to a function of its own, which takes what it
//!   captures first, as `boltlang-mir-mono` finds it, then its parameters. A
//!   local that's assigned is captured as its ref. Making the lambda is a
//!   `closure` of that function, and calling a value that isn't a named
//!   function is an `apply`.
//!
//! - A tuple struct or a variant used as a function, like `Some` in
//!   `xs.map(Some)`, is a closure of its constructor's shim, a function
//!   that `boltlang-mir-transform` makes, once for each instance.
//!
//! - `imm e` is a call of the deep copy of its type, a shim that
//!   `boltlang-mir-transform` makes, unless `imm` doesn't change the type.
//!
//! - A call of an operation of an effect is a `perform`, and a `match` with
//!   effect arms is a `handle`: its scrutinee becomes a closure, its value
//!   arms the return clause, and the arms of each operation its clause,
//!   each a function of its own. The clauses share the locals of the code
//!   around the `match` that they use. A `match` with arms for several
//!   effects is one handler of all of them. Rows are Bolt's: see `ty`. An
//!   effect with type parameters is an effect for each of its arguments,
//!   and an operation used as a function is a closure of a shim that
//!   performs it.
//!
//! What's left out: erased code and dictionaries, patterns in parameters,
//! operations with type parameters of their own, and what needs something
//! Conduit doesn't have yet: 128-bit integers and handler clauses with
//! mutability parameters. Using one is an
//! [`ErrorKind::Unsupported`] or an [`ErrorKind::MissingInConduit`] error.

mod func;
pub mod node;
pub mod run;
mod ty;
mod wasi;
pub mod wasm;

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};

use boltlang_base_db::{Db, File, source_text};
use boltlang_hir_def::{
    body::{DefWithBody, body_with_source_map},
    hir::ExprId,
    ids::{EffectId, FunctionId, ItemContainerId},
    item_tree::AssocItem,
};
use boltlang_mir_build::{
    lambda::{Lambda, body_lambdas},
    mir_bodies,
};
use boltlang_mir_mono::{
    Cx, Instance, InstanceDef, MonoRow, MonoTy, Site, Use, UseError, closed_ty, program,
};
use boltlang_mir_transform::shim_body;
use conduit_ir::{Extern, ExternId, Func, FuncId, HandlerId, Module, MutParams, Row, Sig};

use crate::func::Part;

use crate::ty::Types;

/// A program in Conduit.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub module: Module,
    /// The function of the entry file's top-level statements.
    pub entry: FuncId,
    /// The tests, with their names.
    pub tests: Vec<(String, FuncId)>,
    /// What each function is an instance of, like `id[i32]`.
    pub names: BTreeMap<FuncId, String>,
}

/// Why a program couldn't be lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The instance, like `id[i32]`.
    pub instance: String,
    /// The code it's about, if any.
    pub at: Option<String>,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// A use that can't be compiled, which `boltlang-mir-mono` found.
    Use(UseError),
    /// Something this backend doesn't lower yet.
    Unsupported(String),
    /// Something Conduit has no way to say yet.
    MissingInConduit(String),
    /// Conduit's builder or verifier rejected what was made: a bug here.
    Invalid(String),
}

impl ErrorKind {
    pub(crate) fn unsupported(what: impl Into<String>) -> Self {
        ErrorKind::Unsupported(what.into())
    }

    pub(crate) fn missing(what: impl Into<String>) -> Self {
        ErrorKind::MissingInConduit(what.into())
    }
}

impl From<UseError> for ErrorKind {
    fn from(e: UseError) -> Self {
        ErrorKind::Use(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "in {}", self.instance)?;
        if let Some(at) = &self.at {
            write!(f, " at `{at}`")?;
        }
        match &self.kind {
            ErrorKind::Use(UseError::UnknownType) => write!(f, ": a type isn't known"),
            ErrorKind::Use(UseError::Cycle) => {
                write!(f, ": its type depends on itself across files")
            }
            ErrorKind::Use(UseError::NoImpl(c)) => write!(f, ": nothing gives `{c}`"),
            ErrorKind::Unsupported(what) => write!(f, ": {what} isn't supported yet"),
            ErrorKind::MissingInConduit(what) => {
                write!(f, ": needs what Conduit doesn't have yet: {what}")
            }
            ErrorKind::Invalid(what) => write!(f, ": the IR made is invalid: {what}"),
        }
    }
}

/// What a host function is, so that each one is declared once: a function
/// with `#[link]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ExternKey<'db> {
    Function(FunctionId<'db>),
}

/// What every body is lowered with.
pub(crate) struct Ctx<'db> {
    pub db: &'db dyn Db,
    pub root: File,
    pub funcs: HashMap<Instance<'db>, FuncId>,
    /// The function each lambda of an instance is lifted to.
    pub lambdas: HashMap<(Instance<'db>, ExprId), FuncId>,
    pub externs: HashMap<ExternKey<'db>, ExternId>,
    pub erased: Vec<FunctionId<'db>>,
    /// The prelude's `panic`, with the type of the `String` it takes, if
    /// a `match` that no arm takes calls it.
    pub panic: Option<(FuncId, conduit_ir::Ty)>,
    /// The effects of the program, one for each effect and its arguments,
    /// like `State[i32]`, with its operations in order.
    pub effects: HashMap<EffectKey<'db>, (conduit_ir::EffectId, Vec<FunctionId<'db>>)>,
    /// What each `match` with effect arms is lowered to, by its instance,
    /// the MIR body it's in (0 for the instance's, `i + 1` for its `i`th
    /// lambda's) and its node.
    pub handles: HashMap<HandleKey<'db>, HandleFns<'db>>,
}

pub(crate) type HandleKey<'db> = (Instance<'db>, u32, boltlang_mir::ExprId);

/// The handler and functions a `match` with effect arms is lowered to: its
/// scrutinee as a closure, which captures `captures`, and its value arms
/// and the arms of each operation as the handler's clauses, which share
/// `env`.
pub(crate) struct HandleFns<'db> {
    pub handler: HandlerId,
    pub body: FuncId,
    pub ret: FuncId,
    /// Each effect it handles, with its arguments, in the order the arms
    /// name them, and the clause of each of its operations, in their order.
    pub effects: Vec<(EffectKey<'db>, Vec<FuncId>)>,
    pub env: Vec<boltlang_mir::Local>,
    pub captures: Vec<boltlang_mir::Local>,
}

/// An effect and its arguments, like `State[i32]`.
pub(crate) type EffectKey<'db> = (EffectId<'db>, Box<[MonoTy<'db>]>);

impl<'db> Ctx<'db> {
    /// The effect of the operation `f`, with the arguments `args`, and its
    /// index among the effect's operations.
    pub(crate) fn op(
        &self,
        f: FunctionId<'db>,
        args: &[MonoTy<'db>],
    ) -> Option<(conduit_ir::EffectId, u32)> {
        let ItemContainerId::Effect(e) = f.container(self.db) else { return None };
        let (id, ops) = self.effects.get(&(e, args.into()))?;
        Some((*id, ops.iter().position(|&g| g == f)? as u32))
    }
}

/// The effects, with their arguments, of the operations that a handler
/// handles, each once, in the order the arms name them.
fn handled_effects<'db>(
    lcx: &Cx<'_, 'db>,
    ops: &[boltlang_mir::OpArms<'db>],
) -> Result<Vec<EffectKey<'db>>, ErrorKind> {
    let db = lcx.db;
    if ops.is_empty() {
        return Err(ErrorKind::unsupported("a handler of no operation"));
    }
    let mut out: Vec<EffectKey<'db>> = Vec::new();
    for o in ops {
        let ItemContainerId::Effect(e) = o.op.container(db) else {
            return Err(ErrorKind::unsupported("this handler"));
        };
        let key = match lcx.item_use(&boltlang_mir::ItemRef::Op(o.op), &lcx.ty(&o.ty)?)? {
            Use::Op { effect_args, .. } => (e, effect_args),
            _ => return Err(ErrorKind::unsupported("this handler")),
        };
        if !out.contains(&key) {
            out.push(key);
        }
    }
    Ok(out)
}

/// The tuple of an operation's arguments, which the arms of a handler
/// match. It's `mut`, so that it can hold arguments of any mutability.
pub(crate) fn op_args_tuple<'db>(tys: &[MonoTy<'db>]) -> MonoTy<'db> {
    MonoTy::Tuple(tys.into(), boltlang_mir_mono::MonoMut::Mut)
}

/// The types of the parameters and result of the operation `op` of the
/// effect `key`, with the effect's arguments in place.
pub(crate) fn op_params<'db>(
    db: &'db dyn Db,
    root: File,
    key: &EffectKey<'db>,
    op: FunctionId<'db>,
) -> Result<(Vec<MonoTy<'db>>, MonoTy<'db>), ErrorKind> {
    let params = boltlang_hir_ty::scheme::generic_param_ids(
        db,
        [boltlang_hir_def::resolver::GenericDefId::Effect(key.0)],
    );
    let closed = |t: &boltlang_hir_ty::Ty<'db>| {
        let t = t.fill_unwritten(&mut || {
            boltlang_hir_ty::Ty::Mutability(boltlang_hir_ty::ty::Mutability::Imm)
        });
        closed_ty(db, root, &params, &key.1, &t)
            .map_err(|_| ErrorKind::unsupported("an operation with type parameters"))
    };
    let sig = boltlang_hir_ty::lower::fn_sig(db, op);
    let mut args = Vec::new();
    for p in sig.params.iter() {
        args.push(closed(p.as_ref().ok_or_else(|| ErrorKind::unsupported("this operation"))?)?);
    }
    let ret = match &sig.ret {
        Some(t) => closed(t)?,
        None => MonoTy::unit(),
    };
    Ok((args, ret))
}

/// The program of `entry` and its tests, in Conduit.
pub fn lower(db: &dyn Db, entry: File) -> Result<Lowered, Vec<Error>> {
    let p = program(db, entry);
    let snippet = |instance: Instance<'_>, site: Option<&Site<'_>>| -> Option<String> {
        let def = instance.body(db)?;
        let (_, map) = body_with_source_map(db, def);
        let ptr = map.expr_syntax(site?.origin(db, def)?)?;
        let text = source_text(db, def.def(db).file(db));
        Some(text[ptr.start as usize..ptr.end as usize].to_string())
    };
    let mut errors: Vec<Error> = p
        .errors
        .iter()
        .map(|(i, site, e)| Error {
            instance: i.display(db).to_string(),
            at: snippet(*i, site.as_ref()),
            kind: ErrorKind::Use(e.clone()),
        })
        .collect();
    for f in &p.erased {
        let name = f.item(db).display_name().map(|n| n.to_string()).unwrap_or_default();
        errors.push(Error {
            instance: name,
            at: None,
            kind: ErrorKind::unsupported("a function whose types are erased"),
        });
    }
    // Operations have no body: a call of one is a `perform`, so none should be
    // an instance.
    for iu in &p.instances {
        if let Some(DefWithBody::Function(f)) = iu.instance.body(db).map(|b| b.def(db))
            && let ItemContainerId::Effect(_) = f.container(db)
        {
            errors.push(Error {
                instance: iu.instance.display(db).to_string(),
                at: None,
                kind: ErrorKind::unsupported("calling an operation of an effect"),
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut cx = Ctx {
        db,
        root: entry,
        funcs: HashMap::new(),
        lambdas: HashMap::new(),
        externs: HashMap::new(),
        erased: p.erased.clone(),
        panic: None,
        effects: HashMap::new(),
        handles: HashMap::new(),
    };
    // From 0, in the order they're defined, so that the text format can
    // write each by its name alone.
    let mut names = BTreeMap::new();
    for (i, iu) in p.instances.iter().enumerate() {
        let id = FuncId(i as u64);
        cx.funcs.insert(iu.instance, id);
        names.insert(id, iu.instance.display(db).to_string());
    }
    // The lambdas, lifted, after the instances.
    for iu in &p.instances {
        let Some(body) = iu.instance.body(db) else { continue };
        for (k, lambda) in body_lambdas(db, body).iter().enumerate() {
            let id = FuncId(names.len() as u64);
            cx.lambdas.insert((iu.instance, lambda.expr), id);
            names.insert(id, format!("{}::lambda{k}", iu.instance.display(db)));
        }
    }
    let mut module = Module::default();
    let mut types = Types::default();
    // The effects that the program performs or handles, with their
    // arguments, from their operations.
    let mut effects: Vec<EffectKey<'_>> = Vec::new();
    for iu in &p.instances {
        for (_, u) in &iu.uses {
            if let Use::Op { op, effect_args } = u
                && let ItemContainerId::Effect(e) = op.container(db)
                && !effects.contains(&(e, effect_args.clone()))
            {
                effects.push((e, effect_args.clone()));
            }
        }
        let Some(body) = iu.instance.body(db) else { continue };
        let Ok(lcx) = Cx::new(db, iu.instance, &p.erased) else { continue };
        let bodies = mir_bodies(db, body);
        for mir in std::iter::once(&bodies.body).chain(&bodies.lambdas) {
            for (_, node) in mir.exprs() {
                if let boltlang_mir::ExprKind::Handle { ops: handled, .. } = &node.kind {
                    match handled_effects(&lcx, handled) {
                        Ok(keys) => {
                            for key in keys {
                                if !effects.contains(&key) {
                                    effects.push(key);
                                }
                            }
                        }
                        Err(kind) => errors.push(Error {
                            instance: iu.instance.display(db).to_string(),
                            at: None,
                            kind,
                        }),
                    }
                }
            }
        }
    }
    for key in effects {
        let name = key.0.item(db).name.as_ref().map_or("?", |n| n.as_str()).to_string();
        if let Err(kind) = declare_effect(&mut cx, &mut module, &mut types, key) {
            errors.push(Error { instance: name, at: None, kind });
        }
    }
    // The functions of each handler, after the lambdas.
    for iu in &p.instances {
        let Some(body) = iu.instance.body(db) else { continue };
        let Ok(lcx) = Cx::new(db, iu.instance, &p.erased) else { continue };
        let bodies = mir_bodies(db, body);
        let instance = iu.instance.display(db).to_string();
        let mut n = 0;
        for (index, mir) in std::iter::once(&bodies.body).chain(&bodies.lambdas).enumerate() {
            for (e, node) in mir.exprs() {
                let boltlang_mir::ExprKind::Handle { body: scrutinee, arms, ops, .. } = &node.kind
                else {
                    continue;
                };
                let mut fresh = |name: String| {
                    let id = FuncId(names.len() as u64);
                    names.insert(id, name);
                    id
                };
                let Ok(handled) = handled_effects(&lcx, ops) else { continue };
                let body_fn = fresh(format!("{instance}::handle{n}"));
                let ret = fresh(format!("{instance}::handle{n}_return"));
                let op_name = |op: FunctionId<'_>| {
                    op.item(db).name.as_ref().map(|n| n.name().to_string()).unwrap_or_default()
                };
                let all_ops: Vec<(EffectKey<'_>, Vec<FunctionId<'_>>)> = handled
                    .into_iter()
                    .map(|effect| {
                        let ops = cx.effects.get(&effect).map(|(_, ops)| ops.clone());
                        (effect, ops.unwrap_or_default())
                    })
                    .collect();
                // A clause is named after its operation, and also its effect
                // when another effect it handles has an operation of that
                // name, like `Reader.get` and `State.get`.
                let shared = |name: &str| {
                    all_ops
                        .iter()
                        .filter(|(_, ops)| ops.iter().any(|&o| op_name(o) == name))
                        .count()
                        > 1
                };
                let effects = all_ops
                    .iter()
                    .map(|(effect, effect_ops)| {
                        let clauses = effect_ops
                            .iter()
                            .map(|&op| {
                                let name = op_name(op);
                                let name = if shared(&name) {
                                    let e = effect.0.item(db).name.as_ref();
                                    format!("{}_{name}", e.map_or("?", |n| n.as_str()))
                                } else {
                                    name
                                };
                                fresh(format!("{instance}::handle{n}_{name}"))
                            })
                            .collect();
                        (effect.clone(), clauses)
                    })
                    .collect();
                let mut code: Vec<boltlang_mir::ExprId> = Vec::new();
                for arm in arms.iter().chain(ops.iter().flat_map(|o| o.arms.iter())) {
                    code.extend(arm.guard);
                    code.push(arm.body);
                }
                let handler = HandlerId(cx.handles.len() as u64);
                cx.handles.insert(
                    (iu.instance, index as u32, e),
                    HandleFns {
                        handler,
                        body: body_fn,
                        ret,
                        effects,
                        env: mir.free_locals(&code, &mir.handle_bindings(e)),
                        captures: mir.free_locals(&[*scrutinee], &[]),
                    },
                );
                n += 1;
            }
        }
    }
    // Every type a body has, so that the data types are all declared before
    // the bodies are built. An error is reported where the type is used.
    let mut sigs = Vec::new();
    for iu in &p.instances {
        let error = |kind: ErrorKind| Error {
            instance: iu.instance.display(db).to_string(),
            at: None,
            kind,
        };
        let lcx = match Cx::new(db, iu.instance, &p.erased) {
            Ok(lcx) => lcx,
            Err(e) => {
                errors.push(error(e.into()));
                continue;
            }
        };
        let shim;
        let mirs: Vec<&boltlang_mir::Body<'_>> = match iu.instance.def(db) {
            InstanceDef::Body(body) => {
                let bodies = mir_bodies(db, *body);
                std::iter::once(&bodies.body).chain(&bodies.lambdas).collect()
            }
            InstanceDef::Shim(s) => {
                shim = shim_body(db, s, iu.instance.root(db));
                vec![&shim]
            }
        };
        for mir in mirs {
            let locals =
                mir.bindings.iter().map(|(_, d)| &d.ty).chain(mir.temps.iter().map(|d| &d.ty));
            for ty in mir.exprs().map(|(_, node)| &node.ty).chain(locals) {
                if let Ok(t) = lcx.ty(ty) {
                    let _ = types.value(db, entry, &t);
                }
            }
        }
        match instance_sig(&mut types, &lcx, entry) {
            Ok(sig) => sigs.push((cx.funcs[&iu.instance], sig)),
            Err(kind) => errors.push(error(kind)),
        }
        let lambdas = iu.instance.body(db).map_or(&[][..], |b| body_lambdas(db, b));
        for lambda in lambdas {
            match lambda_sig(&mut types, &lcx, entry, lambda) {
                Ok(sig) => sigs.push((cx.lambdas[&(iu.instance, lambda.expr)], sig)),
                Err(kind) => errors.push(error(kind)),
            }
        }
        if let Err(kind) = handler_sigs(&cx, &mut module, &mut types, &lcx, &mut sigs) {
            errors.push(error(kind));
        }
        for (site, u) in &iu.uses {
            if let Err(kind) = declare_extern(&mut cx, &mut module, &mut types, &lcx, site, u) {
                errors.push(error(kind));
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    // `panic`, if a `match` calls it: the `String` it takes is its first
    // parameter.
    if let Some(instance) = boltlang_mir_mono::panic_instance(db, entry)
        && let Some(&id) = cx.funcs.get(&instance)
    {
        let string = sigs.iter().find(|(f, _)| *f == id).and_then(|(_, (sig, ..))| {
            let sig: &Sig = sig;
            sig.params.first().cloned()
        });
        cx.panic = string.map(|t| (id, t));
    }
    module.data = types.decls.clone();
    for (id, (sig, mut_params, effect_params)) in &sigs {
        let mut f = Func::new(sig.params.clone(), sig.results.clone());
        f.mut_params = mut_params.clone();
        f.effect_params = *effect_params;
        f.row = sig.row.clone();
        module.funcs.insert(*id, f);
    }
    let mut built = Vec::new();
    for iu in &p.instances {
        match func::lower_body(&cx, &module, &mut types, iu, Part::Body) {
            Ok(f) => built.push((cx.funcs[&iu.instance], f)),
            Err(e) => errors.push(e),
        }
        let lambdas = iu.instance.body(db).map_or(&[][..], |b| body_lambdas(db, b));
        for lambda in lambdas {
            match func::lower_body(&cx, &module, &mut types, iu, Part::Lambda(lambda)) {
                Ok(f) => built.push((cx.lambdas[&(iu.instance, lambda.expr)], f)),
                Err(e) => errors.push(e),
            }
        }
    }
    let mut keys: Vec<&HandleKey<'_>> = cx.handles.keys().collect();
    keys.sort_by_key(|k| cx.handles[*k].handler.0);
    for key in keys {
        let iu = p.instances.iter().find(|iu| iu.instance == key.0).unwrap();
        let fns = &cx.handles[key];
        let mut parts = vec![
            (fns.body, Part::HandleBody(key.1, key.2)),
            (fns.ret, Part::HandleRet(key.1, key.2)),
        ];
        for (effect, (_, clauses)) in fns.effects.iter().enumerate() {
            for (op, &clause) in clauses.iter().enumerate() {
                parts.push((clause, Part::HandleClause(key.1, key.2, effect, op)));
            }
        }
        for (id, part) in parts {
            match func::lower_body(&cx, &module, &mut types, iu, part) {
                Ok(f) => built.push((id, f)),
                Err(e) => errors.push(e),
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    for (id, mut f) in built {
        f.debug.name = Some(ident(&names[&id]).into());
        module.funcs.insert(id, f);
    }
    // The effects that only rows name, which nothing performs or handles.
    while !types.undeclared.is_empty() {
        for key in std::mem::take(&mut types.undeclared) {
            if cx.effects.contains_key(&key) {
                continue;
            }
            let name = key.0.item(db).name.as_ref().map_or("?", |n| n.as_str()).to_string();
            if let Err(kind) = declare_effect(&mut cx, &mut module, &mut types, key) {
                errors.push(Error { instance: name, at: None, kind });
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    module.data = types.decls.clone();
    if let Err(es) = conduit_ir::verify::verify_module(&module) {
        return Err(es
            .into_iter()
            .map(|e| Error {
                instance: "the module".into(),
                at: None,
                kind: ErrorKind::Invalid(e.to_string()),
            })
            .collect());
    }
    let tests = p.roots[1..]
        .iter()
        .map(|i| {
            let name = i
                .function(db)
                .and_then(|f| f.item(db).display_name().map(|n| n.to_string()))
                .unwrap_or_default();
            (name, cx.funcs[i])
        })
        .collect();
    Ok(Lowered { module, entry: cx.funcs[&p.roots[0]], tests, names })
}

/// The Conduit signature of an instance, its mutability parameters and how
/// many effect parameters it has.
fn instance_sig<'db>(
    types: &mut Types<'db>,
    lcx: &Cx<'_, 'db>,
    root: File,
) -> Result<(Sig, MutParams, u32), ErrorKind> {
    let db = lcx.db;
    let mut sig = Sig::new(Vec::new(), Vec::new());
    let def = match lcx.instance.def(db) {
        InstanceDef::Body(body) => Some(body.def(db)),
        InstanceDef::Shim(_) => None,
    };
    match def {
        // A shim is a function too.
        Some(DefWithBody::Function(_)) | None => {
            let MonoTy::Fn(params, ret, _, row) = lcx.instance_ty()? else {
                return Err(ErrorKind::unsupported("a function whose type isn't a function"));
            };
            for t in params.iter() {
                sig.params.extend(types.value(db, root, t)?);
            }
            sig.results.extend(types.value(db, root, &ret)?);
            sig.row = types.row(&row);
        }
        Some(DefWithBody::Const(_)) => {
            sig.results.extend(types.value(db, root, &lcx.instance_ty()?)?)
        }
        Some(DefWithBody::File(_)) => {}
        Some(DefWithBody::Variant(_)) => {
            return Err(ErrorKind::unsupported("the value of a variant"));
        }
    }
    let mut_params = MutParams { count: lcx.instance.mut_params(db) as u32, assumes: Vec::new() };
    Ok((sig, mut_params, lcx.instance.row_params(db) as u32))
}

/// The row of effects of a MIR body of the instance of `lcx`: that of its
/// function or lambda, and none for the code of a file or a constant.
fn body_row<'db>(
    lcx: &Cx<'_, 'db>,
    mir: &boltlang_mir::Body<'db>,
) -> Result<MonoRow<'db>, ErrorKind> {
    let ty = match mir.owner {
        boltlang_mir::BodyOwner::Lambda { expr, .. } => lcx.expr_ty(expr)?,
        _ => lcx.instance_ty()?,
    };
    Ok(match ty {
        MonoTy::Fn(.., row) => row,
        _ => MonoRow::nothing(),
    })
}

/// The signature of a lifted lambda: its captures, then its parameters.
/// A local that's assigned is captured as its ref.
fn lambda_sig<'db>(
    types: &mut Types<'db>,
    lcx: &Cx<'_, 'db>,
    root: File,
    lambda: &Lambda,
) -> Result<(Sig, MutParams, u32), ErrorKind> {
    let db = lcx.db;
    let Some(mir) = lcx.instance.body(db).and_then(|b| mir_bodies(db, b).lambda(lambda.expr))
    else {
        return Err(ErrorKind::unsupported("a lambda without a body"));
    };
    let mut sig = Sig::new(Vec::new(), Vec::new());
    for c in &mir.captures {
        let Some(decl) = mir.local(c.local) else { continue };
        if let Some(t) = types.value(db, root, &lcx.ty(&decl.ty)?)? {
            sig.params.push(if c.by_ref { conduit_ir::Ty::Ref(Box::new(t)) } else { t });
        }
    }
    let MonoTy::Fn(params, ret, _, row) = lcx.expr_ty(lambda.expr)? else {
        return Err(ErrorKind::unsupported("a lambda whose type isn't a function"));
    };
    for t in params.iter() {
        sig.params.extend(types.value(db, root, t)?);
    }
    sig.results.extend(types.value(db, root, &ret)?);
    sig.row = types.row(&row);
    let mut_params = MutParams { count: lcx.instance.mut_params(db) as u32, assumes: Vec::new() };
    Ok((sig, mut_params, lcx.instance.row_params(db) as u32))
}

/// The signatures of the functions of the handlers in the instance of
/// `lcx`, and the handlers themselves.
fn handler_sigs<'db>(
    cx: &Ctx<'db>,
    module: &mut Module,
    types: &mut Types<'db>,
    lcx: &Cx<'_, 'db>,
    sigs: &mut Vec<(FuncId, (Sig, MutParams, u32))>,
) -> Result<(), ErrorKind> {
    let db = cx.db;
    let instance = lcx.instance;
    let Some(body) = instance.body(db) else { return Ok(()) };
    let bodies = mir_bodies(db, body);
    let mut keys: Vec<&HandleKey<'db>> = cx.handles.keys().filter(|k| k.0 == instance).collect();
    keys.sort_by_key(|k| cx.handles[*k].handler.0);
    for key in keys {
        let fns = &cx.handles[key];
        let mir = match key.1 {
            0 => &bodies.body,
            i => &bodies.lambdas[i as usize - 1],
        };
        let boltlang_mir::ExprKind::Handle { body: scrutinee, .. } = &mir[key.2].kind else {
            continue;
        };
        let mut clause_tys = Vec::new();
        let mut local_tys = |locals: &[boltlang_mir::Local],
                             clause_tys: &mut Vec<MonoTy<'db>>|
         -> Result<Vec<conduit_ir::Ty>, ErrorKind> {
            let mut out = Vec::new();
            for &l in locals {
                let Some(decl) = mir.local(l) else { continue };
                let t = lcx.ty(&decl.ty)?;
                clause_tys.push(t.clone());
                if let Some(t) = types.value(db, cx.root, &t)? {
                    out.push(if decl.mutable { conduit_ir::Ty::Ref(Box::new(t)) } else { t });
                }
            }
            Ok(out)
        };
        let env = local_tys(&fns.env, &mut clause_tys)?;
        let captures = local_tys(&fns.captures, &mut Vec::new())?;
        let body_ty = lcx.ty(&mir[*scrutinee].ty)?;
        let ty = lcx.ty(&mir[key.2].ty)?;
        clause_tys.extend([body_ty.clone(), ty.clone()]);
        // The clauses of a handler can't have mutability parameters in
        // Conduit, so their types can't have the instance's.
        if clause_tys.iter().any(MonoTy::has_mut_param) {
            return Err(ErrorKind::missing(
                "handler clauses with mutability parameters, for a handler whose arms have \
                 values of a mutability that's a parameter",
            ));
        }
        let body_results: Vec<conduit_ir::Ty> =
            types.value(db, cx.root, &body_ty)?.into_iter().collect();
        let results: Vec<conduit_ir::Ty> = types.value(db, cx.root, &ty)?.into_iter().collect();
        let none = MutParams::default();
        // Every function of the handler, and the handler, have the
        // instance's row parameters, which `handle` gives them.
        let effect_params = instance.row_params(db) as u32;
        // The handler's row is what its clauses can do, which is what the
        // code around the `match` can do, and also what resuming its
        // continuations does, as the checker has it.
        let row = types.row(&body_row(lcx, mir)?);
        // The Conduit effect of each effect it handles, and its clauses.
        let mut handled = Vec::new();
        for (key, clauses) in &fns.effects {
            let Some((effect, effect_ops)) = cx.effects.get(key) else { continue };
            // The tuple of each operation's arguments, which its arms match.
            for &op in effect_ops {
                let (tys, _) = op_params(db, cx.root, key, op)?;
                if !tys.is_empty() {
                    types.value(db, cx.root, &op_args_tuple(&tys))?;
                }
            }
            for (i, &clause) in clauses.iter().enumerate() {
                let op = &module.effects[effect].ops[i];
                let k = conduit_ir::Ty::Cont(Box::new(Sig {
                    params: op.results.clone(),
                    results: results.clone(),
                    row: row.clone(),
                }));
                let mut params = env.clone();
                params.extend(op.params.iter().cloned());
                params.push(k);
                let mut sig = Sig::new(params, results.clone());
                sig.row = row.clone();
                sigs.push((clause, (sig, none.clone(), effect_params)));
            }
            handled.push((*effect, clauses.clone()));
        }
        // The body is a closure, like a lambda's, which has the instance's
        // mutability parameters. It can perform the handled effects, and
        // what the handler can.
        let own = MutParams { count: instance.mut_params(db) as u32, assumes: Vec::new() };
        let mut body_sig = Sig::new(captures, body_results.clone());
        body_sig.row = handled.iter().fold(row.clone(), |r, (e, _)| r.union(&Row::one(*e)));
        sigs.push((fns.body, (body_sig, own, effect_params)));
        let mut ret_params = env.clone();
        ret_params.extend(body_results);
        let mut ret_sig = Sig::new(ret_params, results.clone());
        ret_sig.row = row.clone();
        sigs.push((fns.ret, (ret_sig, none.clone(), effect_params)));
        let debug_name = Some(
            format!("{}_handler{}", ident(&instance.display(db).to_string()), fns.handler.0).into(),
        );
        module.handlers.insert(
            fns.handler,
            conduit_ir::HandlerDecl {
                effect_params,
                env,
                ops: handled,
                ret: fns.ret,
                row,
                debug_name,
            },
        );
    }
    Ok(())
}

/// Declare the effect `key`, an effect and its arguments, with the
/// signature of each of its operations.
fn declare_effect<'db>(
    cx: &mut Ctx<'db>,
    module: &mut Module,
    types: &mut Types<'db>,
    key: EffectKey<'db>,
) -> Result<(), ErrorKind> {
    let db = cx.db;
    // Before the operations' types, whose rows can name other effects, so
    // that effects are numbered in the order they're declared.
    let id = types.effect(key.clone());
    let e = key.0;
    let ops: Vec<FunctionId<'db>> = e
        .item(db)
        .items
        .iter()
        .filter_map(|item| match *item {
            AssocItem::Function(idx) => Some(FunctionId::new(db, ItemContainerId::Effect(e), idx)),
            _ => None,
        })
        .collect();
    let mut sigs = Vec::new();
    for &op in &ops {
        let (args, ret) = op_params(db, cx.root, &key, op)?;
        let mut params = Vec::new();
        for t in &args {
            params.extend(types.value(db, cx.root, t)?);
        }
        let results = types.value(db, cx.root, &ret)?.into_iter().collect();
        // An operation's signature has no row.
        sigs.push(Sig::new(params, results));
    }
    let name = e.item(db).name.as_ref().map_or("?", |n| n.as_str()).to_string();
    let args: Vec<String> = key.1.iter().map(|t| t.display(db).to_string()).collect();
    let name = if args.is_empty() { name } else { format!("{name}[{}]", args.join(", ")) };
    let debug_name = Some(ident(&name).into());
    module.effects.insert(id, conduit_ir::EffectDecl { ops: sigs, debug_name });
    cx.effects.insert(key, (id, ops));
    Ok(())
}

/// Declare the host function a use calls, if it calls one.
fn declare_extern<'db>(
    cx: &mut Ctx<'db>,
    module: &mut Module,
    types: &mut Types<'db>,
    lcx: &Cx<'_, 'db>,
    site: &Site<'db>,
    u: &Use<'db>,
) -> Result<(), ErrorKind> {
    let db = cx.db;
    let use_ty = |site: &Site<'db>| -> Result<MonoTy<'db>, ErrorKind> {
        let ty = lcx
            .instance
            .body(db)
            .and_then(|body| site.use_ty(db, body))
            .ok_or_else(|| ErrorKind::unsupported("a use without a type"))?;
        Ok(lcx.ty(&ty)?)
    };
    let (key, module_name, name, linked) = match u {
        Use::Extern(f) => {
            let own = f.item(db).display_name().map(|n| n.to_string()).unwrap_or_default();
            // `#[link(wasm, module = "m", name = "n")]`.
            let Some(link) = boltlang_hir_ty::builtin::link(db, *f) else {
                return Err(ErrorKind::unsupported("a host function without `#[link]`"));
            };
            let module = link.module.clone().unwrap_or_default();
            let name = link.name.clone().unwrap_or(own);
            (ExternKey::Function(*f), module, name, *f)
        }
        _ => return Ok(()),
    };
    if cx.externs.contains_key(&key) {
        return Ok(());
    }
    let MonoTy::Fn(params, ret, ..) = use_ty(site)? else {
        return Err(ErrorKind::unsupported("a host function whose type isn't a function"));
    };
    // With `abi = flat`, each `#[nolen]` array is passed without its
    // length, and a tuple result is the extern's results: the first is what
    // the host gives back, and the others it writes to memory.
    let flat = boltlang_hir_ty::builtin::link(db, linked)
        .is_some_and(|l| l.abi == boltlang_hir_def::item_tree::LinkAbi::Flat);
    let declared = boltlang_hir_def::signatures::function_signature(db, linked);
    let mut sig = Sig::new(Vec::new(), Vec::new());
    let mut nolen = Vec::new();
    for (i, t) in params.iter().enumerate() {
        if let Some(t) = types.value(db, cx.root, t)? {
            sig.params.push(t);
            nolen.push(declared.params.get(i).is_some_and(|p| p.nolen));
        }
    }
    match &*ret {
        MonoTy::Tuple(elems, _) if flat => {
            for t in elems.iter() {
                sig.results.extend(types.value(db, cx.root, t)?);
            }
        }
        _ => sig.results.extend(types.value(db, cx.root, &ret)?),
    }
    let id = ExternId(module.externs.len() as u64);
    let debug_name = Some(name.as_str().into());
    let mut ext = Extern { debug_name, ..Extern::new(&module_name, &name, sig) };
    if flat {
        ext.abi = conduit_ir::Abi::Flat;
        if nolen.contains(&true) {
            ext.nolen = nolen;
        }
    }
    module.externs.insert(id, ext);
    cx.externs.insert(key, id);
    Ok(())
}

/// A name of Bolt's, like `Mul[i32].mul` or `*[i32]`, as an identifier,
/// which is what Conduit's text format can name things by: `Mul_i32_mul`
/// and `star_i32`. The code of a file, `<file>`, is `file`, and so its
/// lambdas are `file_lambda0` and so on.
pub(crate) fn ident(name: &str) -> String {
    let name = name.replace("<file>", "file");
    let mut out = String::new();
    let word = |c: char| match c {
        '+' => Some("plus"),
        '-' => Some("minus"),
        '*' => Some("star"),
        '/' => Some("slash"),
        '%' => Some("percent"),
        '<' => Some("lt"),
        '>' => Some("gt"),
        '=' => Some("eq"),
        '!' => Some("not"),
        '&' => Some("and"),
        '|' => Some("or"),
        '^' => Some("xor"),
        '~' => Some("tilde"),
        _ => None,
    };
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            if let Some(w) = word(c) {
                out.push_str(w);
                out.push('_');
            }
        }
    }
    let mut out = out.trim_end_matches('_').to_string();
    if out.is_empty() {
        out.push('_');
    }
    // Words of the text format, which a name can't be.
    const KEYWORDS: &[&str] = &[
        "Any", "Nothing", "and", "any", "cont", "does", "env", "f32", "f32_bits", "f64",
        "f64_bits", "fn", "for", "i16", "i32", "i64", "i8", "imm", "in", "inherit", "labels",
        "mut", "ops", "rec", "ref", "ret", "ro", "vars", "where",
    ];
    if KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}
