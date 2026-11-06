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
//! - The host functions of an `extern` block are Conduit externs, in the
//!   module the block names. The module `bolt` has `write_file` and
//!   `flush`, which `print` and a `match` that no arm takes write with
//!   (see [`host`]). `abort` is a `trap`. An `unreachable` is only where Bolt can promise
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
//! What's left out: erased code and dictionaries, patterns in parameters,
//! handlers, and what needs something Conduit doesn't have yet: 128-bit
//! integers. Using one is an
//! [`ErrorKind::Unsupported`] or an [`ErrorKind::MissingInConduit`] error.

mod func;
mod host;
pub mod run;
mod ty;

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
use boltlang_hir_ty::builtin::Builtin;
use boltlang_mir_build::{
    lambda::{Lambda, body_lambdas},
    mir_bodies,
};
use boltlang_mir_mono::{
    Cx, Instance, InstanceDef, MonoTy, Site, Use, UseError, closed_ty, program,
};
use boltlang_mir_transform::shim_body;
use boltlang_pattern::decision::{Node, body_decisions};
use conduit_ir::{Extern, ExternId, Func, FuncId, Module, MutParams, Row, Sig};

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
/// of an `extern` block. The module `bolt`'s are in [`host`].
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
    /// The host's `write_file` and `flush`, once something needs them.
    pub io: Option<host::Io>,
    /// The functions that call them: `bolt_print` and `bolt_unmatched`.
    pub helpers: host::Helpers,
    /// The effects of the program, with their operations in order.
    pub effects: HashMap<EffectId<'db>, (conduit_ir::EffectId, Vec<FunctionId<'db>>)>,
    /// The row of every function: `Any` in a program with effects, since
    /// Bolt doesn't give Conduit its rows yet, and none otherwise.
    pub row: Row,
}

impl<'db> Ctx<'db> {
    /// The effect of the operation `f`, and its index among the effect's.
    pub(crate) fn op(&self, f: FunctionId<'db>) -> Option<(conduit_ir::EffectId, u32)> {
        let ItemContainerId::Effect(e) = f.container(self.db) else { return None };
        let (id, ops) = self.effects.get(&e)?;
        Some((*id, ops.iter().position(|&g| g == f)? as u32))
    }
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
    // Operations have no body: what they do is up to a handler, and there are
    // no handlers yet.
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
        io: None,
        helpers: host::Helpers::default(),
        effects: HashMap::new(),
        row: Row::NOTHING,
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
    // The effects that the program performs or handles, from their
    // operations.
    let mut ops = Vec::new();
    for iu in &p.instances {
        for (_, u) in &iu.uses {
            if let Use::Op(f) = u {
                ops.push(*f);
            }
        }
        let Some(body) = iu.instance.body(db) else { continue };
        let bodies = mir_bodies(db, body);
        for mir in std::iter::once(&bodies.body).chain(&bodies.lambdas) {
            for (_, node) in mir.exprs() {
                if let boltlang_mir::ExprKind::Handle { ops: handled, .. } = &node.kind {
                    ops.extend(handled.iter().map(|o| o.op));
                }
            }
        }
    }
    let mut effects = Vec::new();
    for f in ops {
        if let ItemContainerId::Effect(e) = f.container(db)
            && !effects.contains(&e)
        {
            effects.push(e);
        }
    }
    if !effects.is_empty() {
        cx.row = Row::ANY;
        types.row = Row::ANY;
    }
    for e in effects {
        if let Err(kind) = declare_effect(&mut cx, &mut module, &mut types, e) {
            let name = e.item(db).name.as_ref().map_or("?", |n| n.as_str()).to_string();
            errors.push(Error { instance: name, at: None, kind });
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
        for (site, u) in &iu.uses {
            if let Err(kind) = declare_extern(&mut cx, &mut module, &mut types, &lcx, site, u) {
                errors.push(error(kind));
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    // The functions that call the host: `bolt_print`, if something prints,
    // and `bolt_unmatched`, if a `match` can be taken by no arm.
    if let Some(string) = &cx.helpers.string {
        let id = FuncId(names.len() as u64);
        names.insert(id, "bolt_print".into());
        sigs.push((id, (Sig::new(vec![string.clone()], Vec::new()), MutParams::default())));
        cx.helpers.print = Some(id);
    }
    let can_fail = p.instances.iter().filter_map(|iu| iu.instance.body(db)).any(|body| {
        body_decisions(db, body)
            .matches
            .iter()
            .any(|(_, d)| d.as_ref().is_ok_and(|d| d.nodes.contains(&Node::Fail)))
    });
    if can_fail {
        host::declare_io(&mut module, &mut cx.io);
        let id = FuncId(names.len() as u64);
        names.insert(id, "bolt_unmatched".into());
        sigs.push((id, (Sig::new(Vec::new(), Vec::new()), MutParams::default())));
        cx.helpers.unmatched = Some(id);
    }
    module.data = types.decls.clone();
    let helpers = [cx.helpers.print, cx.helpers.unmatched];
    for (id, (sig, mut_params)) in &sigs {
        let mut f = Func::new(sig.params.clone(), sig.results.clone());
        f.mut_params = mut_params.clone();
        // The helpers only call the host, which performs nothing.
        if !helpers.contains(&Some(*id)) {
            f.row = cx.row.clone();
        }
        module.funcs.insert(*id, f);
    }
    let mut built = Vec::new();
    for iu in &p.instances {
        match func::lower_body(&cx, &module, &mut types, iu, None) {
            Ok(f) => built.push((cx.funcs[&iu.instance], f)),
            Err(e) => errors.push(e),
        }
        let lambdas = iu.instance.body(db).map_or(&[][..], |b| body_lambdas(db, b));
        for lambda in lambdas {
            match func::lower_body(&cx, &module, &mut types, iu, Some(lambda)) {
                Ok(f) => built.push((cx.lambdas[&(iu.instance, lambda.expr)], f)),
                Err(e) => errors.push(e),
            }
        }
    }
    let helpers = [
        cx.helpers
            .print
            .zip(cx.helpers.string.as_ref())
            .map(|(id, string)| (id, host::print_func(&module, cx.io.unwrap(), string))),
        cx.helpers.unmatched.map(|id| (id, host::unmatched_func(&module, cx.io.unwrap()))),
    ];
    for (id, f) in helpers.into_iter().flatten() {
        match f {
            Ok(f) => built.push((id, f)),
            Err(kind) => errors.push(Error { instance: names[&id].clone(), at: None, kind }),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    for (id, mut f) in built {
        f.debug.name = Some(ident(&names[&id]).into());
        module.funcs.insert(id, f);
    }
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

/// The Conduit signature of an instance, and its mutability parameters.
fn instance_sig<'db>(
    types: &mut Types<'db>,
    lcx: &Cx<'_, 'db>,
    root: File,
) -> Result<(Sig, MutParams), ErrorKind> {
    let db = lcx.db;
    let mut sig = Sig::new(Vec::new(), Vec::new());
    let def = match lcx.instance.def(db) {
        InstanceDef::Body(body) => Some(body.def(db)),
        InstanceDef::Shim(_) => None,
    };
    match def {
        // A shim is a function too.
        Some(DefWithBody::Function(_)) | None => {
            let MonoTy::Fn(params, ret, _) = lcx.instance_ty()? else {
                return Err(ErrorKind::unsupported("a function whose type isn't a function"));
            };
            for t in params.iter() {
                sig.params.extend(types.value(db, root, t)?);
            }
            sig.results.extend(types.value(db, root, &ret)?);
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
    Ok((sig, mut_params))
}

/// The signature of a lifted lambda: its captures, then its parameters.
/// A local that's assigned is captured as its ref.
fn lambda_sig<'db>(
    types: &mut Types<'db>,
    lcx: &Cx<'_, 'db>,
    root: File,
    lambda: &Lambda,
) -> Result<(Sig, MutParams), ErrorKind> {
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
    let MonoTy::Fn(params, ret, _) = lcx.expr_ty(lambda.expr)? else {
        return Err(ErrorKind::unsupported("a lambda whose type isn't a function"));
    };
    for t in params.iter() {
        sig.params.extend(types.value(db, root, t)?);
    }
    sig.results.extend(types.value(db, root, &ret)?);
    let mut_params = MutParams { count: lcx.instance.mut_params(db) as u32, assumes: Vec::new() };
    Ok((sig, mut_params))
}

/// Declare the effect `e`, with the signature of each of its operations.
fn declare_effect<'db>(
    cx: &mut Ctx<'db>,
    module: &mut Module,
    types: &mut Types<'db>,
    e: EffectId<'db>,
) -> Result<(), ErrorKind> {
    let db = cx.db;
    if !boltlang_hir_def::signatures::effect_signature(db, e).generic_params.type_params.is_empty()
    {
        return Err(ErrorKind::unsupported("an effect with type parameters"));
    }
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
        let sig = boltlang_hir_ty::lower::fn_sig(db, op);
        let closed = |t: &boltlang_hir_ty::Ty<'db>| {
            closed_ty(db, cx.root, t)
                .map_err(|_| ErrorKind::unsupported("an operation with type parameters"))
        };
        let mut params = Vec::new();
        for p in sig.params.iter() {
            let p = p.as_ref().ok_or_else(|| ErrorKind::unsupported("this operation"))?;
            params.extend(types.value(db, cx.root, &closed(p)?)?);
        }
        let ret = match &sig.ret {
            Some(t) => closed(t)?,
            None => MonoTy::unit(),
        };
        let results = types.value(db, cx.root, &ret)?.into_iter().collect();
        // An operation's signature has no row.
        sigs.push(Sig::new(params, results));
    }
    let id = conduit_ir::EffectId(module.effects.len() as u64);
    let debug_name = e.item(db).name.as_ref().map(|n| ident(n.as_str()).into());
    module.effects.insert(id, conduit_ir::EffectDecl { ops: sigs, debug_name });
    cx.effects.insert(e, (id, ops));
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
    let (key, module_name, name) = match u {
        Use::Extern(f) => {
            let abi = match f.container(db) {
                boltlang_hir_def::ids::ItemContainerId::ExternBlock(b) => {
                    b.item(db).abi.clone().unwrap_or_else(|| "env".to_string())
                }
                _ => "env".to_string(),
            };
            let name = f.item(db).display_name().map(|n| n.to_string()).unwrap_or_default();
            (ExternKey::Function(*f), abi, name)
        }
        // `print` is a function of the module, `bolt_print`, which calls the
        // host's `write_file` and `flush`. Its id is given once every
        // instance has one.
        Use::Builtin(Builtin::Print) => {
            if cx.helpers.string.is_some() {
                return Ok(());
            }
            let MonoTy::Fn(params, ..) = use_ty(site)? else {
                return Err(ErrorKind::unsupported("a `print` whose type isn't a function"));
            };
            let Some(string) = types.value(db, cx.root, &params[0])? else {
                return Err(ErrorKind::unsupported("a `String` with no values"));
            };
            host::declare_io(module, &mut cx.io);
            cx.helpers.string = Some(string);
            return Ok(());
        }
        _ => return Ok(()),
    };
    if cx.externs.contains_key(&key) {
        return Ok(());
    }
    let MonoTy::Fn(params, ret, _) = use_ty(site)? else {
        return Err(ErrorKind::unsupported("a host function whose type isn't a function"));
    };
    let mut sig = Sig::new(Vec::new(), Vec::new());
    for t in params.iter() {
        sig.params.extend(types.value(db, cx.root, t)?);
    }
    sig.results.extend(types.value(db, cx.root, &ret)?);
    let id = ExternId(module.externs.len() as u64);
    let debug_name = Some(name.as_str().into());
    module
        .externs
        .insert(id, Extern { module: module_name.into(), name: name.into(), sig, debug_name });
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
