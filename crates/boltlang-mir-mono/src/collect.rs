//! Finding every instance a program needs, from its entry file and tests.
//!
//! The collector follows each use from the roots, as rustc's collector does.
//! A function that recurses polymorphically, like `fn depth[A](n:
//! Nested[A])` calling `depth::<(A, A)>`, would need instances without
//! end. When the collector sees a function's instance under one of its own
//! whose types are inside the new one's, like `(i32, i32)` under `i32`, it
//! erases every function on that path: each is compiled once, with its
//! types [`MonoTy::Erased`], and gets dictionaries for its classes (see
//! [`crate::uses`]). It then starts over, until nothing grows.
//!
//! That can erase a group that would have stopped growing, through
//! associated types; the program is still correct, only slower.

use std::collections::{HashMap, HashSet, VecDeque, hash_map::Entry};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId},
    hir::ExprId as HirExprId,
    ids::{ConstId, FunctionId, ImplId},
    nameres::test_functions,
};
use boltlang_hir_ty::Ty;
use boltlang_mir::{Body as MirBody, ExprId, ExprKind, ItemRef, Pattern, Place, Stmt};
use boltlang_mir_build::{MirBodies, mir_bodies};
use boltlang_pattern::decision::{Bound, Decision, Node as DecisionNode, Value, body_decisions};

use boltlang_mir_transform::shim_body;

use crate::{
    instance::{Instance, InstanceDef, Kind, generics},
    ty::MonoTy,
    uses::{ClassItem, Cx, Dict, Use, UseError, dict_entries},
};

/// Where a use is: a node of the MIR of a body, of one of its lambdas or of
/// one of its defaults, and which of the node's uses it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct Site<'db> {
    /// The number of the MIR body, as in [`MirBodies::all`]: 0 for the
    /// body, then its lambdas, then its defaults.
    pub body: u32,
    pub expr: ExprId,
    pub role: Role<'db>,
}

/// Which use of a MIR node a [`Site`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Role<'db> {
    /// The function or constant of an `Item` node.
    Item,
    /// The `iter` of a `for`.
    Iter,
    /// The `next` of a `for`.
    Next,
    /// The operator of a compound assignment, like the `+` of `+=`.
    Op,
    /// The `index` that a compound assignment to `xs[i]` reads with.
    Get,
    /// The `set_index` of an assignment to `xs[i]`.
    Set,
    /// A constant that a pattern of the node's decision compares with.
    Const(ConstId<'db>),
    /// The deep copy of `imm e`.
    Imm,
}

/// An instance and what each of its uses is.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct InstanceUses<'db> {
    pub instance: Instance<'db>,
    pub uses: Vec<(Site<'db>, Use<'db>)>,
}

/// The instances of a program.
#[derive(Debug, Clone, Default, PartialEq, Eq, salsa::SalsaValue)]
pub struct Program<'db> {
    /// The entry file's top-level statements, then its tests.
    pub roots: Vec<Instance<'db>>,
    /// Every instance the roots need, in the order they were found.
    pub instances: Vec<InstanceUses<'db>>,
    /// The dictionaries erased code uses, with their entries.
    pub dicts: Vec<(DictKey<'db>, Vec<(ClassItem<'db>, Instance<'db>)>)>,
    /// The functions compiled once with their types erased.
    pub erased: Vec<FunctionId<'db>>,
    /// Uses that can't be compiled.
    pub errors: Vec<(Instance<'db>, Option<Site<'db>>, UseError)>,
}

/// A dictionary of an impl at its type parameters.
pub type DictKey<'db> = (ImplId<'db>, Box<[MonoTy<'db>]>);

/// The uses in a body, and those that can't be compiled.
pub type Scanned<'db> = (Vec<(Site<'db>, Use<'db>)>, Vec<(Site<'db>, UseError)>);

/// How many instances a program can have before the collector gives up.
const MAX_INSTANCES: usize = 100_000;

/// The instances that the program whose entry file is `entry` needs.
#[salsa::tracked(returns(ref))]
pub fn program<'db>(db: &'db dyn Db, entry: File) -> Program<'db> {
    let mut erased = Vec::new();
    loop {
        match collect(db, entry, &erased) {
            Ok(program) => return program,
            Err(Grows(group)) => {
                let before = erased.len();
                for f in group {
                    if !erased.contains(&f) {
                        erased.push(f);
                    }
                }
                // Already erased and still growing can't happen, but
                // shouldn't loop forever either.
                if erased.len() == before {
                    let mut program = collect(db, entry, &[]).unwrap_or_default();
                    program.errors.push((
                        Instance::entry(db, entry),
                        None,
                        UseError::NoImpl("instances that grow without end".into()),
                    ));
                    return program;
                }
            }
        }
    }
}

/// The functions to erase, since their instances grow.
struct Grows<'db>(Vec<FunctionId<'db>>);

struct Node<'db> {
    instance: Instance<'db>,
    parent: Option<usize>,
}

fn collect<'db>(
    db: &'db dyn Db,
    entry: File,
    erased: &[FunctionId<'db>],
) -> Result<Program<'db>, Grows<'db>> {
    let mut program = Program { erased: erased.to_vec(), ..Program::default() };
    program.roots.push(Instance::entry(db, entry));
    for f in test_functions(db, entry) {
        program.roots.push(plain_instance(db, *f, entry));
    }
    let mut nodes: Vec<Node<'db>> = Vec::new();
    let mut seen: HashMap<Instance<'db>, usize> = HashMap::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    for &root in &program.roots {
        if let Entry::Vacant(e) = seen.entry(root) {
            e.insert(nodes.len());
            queue.push_back(nodes.len());
            nodes.push(Node { instance: root, parent: None });
        }
    }
    let mut seen_dicts: HashSet<DictKey<'db>> = HashSet::new();
    while let Some(index) = queue.pop_front() {
        let instance = nodes[index].instance;
        let cx = match Cx::new(db, instance, erased) {
            Ok(cx) => cx,
            Err(e) => {
                program.errors.push((instance, None, e));
                continue;
            }
        };
        let (uses, errors) = scan(&cx);
        program.errors.extend(errors.into_iter().map(|(site, e)| (instance, Some(site), e)));
        let mut found = Vec::new();
        for (_, u) in &uses {
            instances_of_use(db, u, instance.root(db), &mut found, &mut seen_dicts, &mut program)?;
        }
        // A `match` that no arm may take calls the prelude's `panic`.
        if can_fail(db, instance) {
            found.extend(panic_instance(db, instance.root(db)));
        }
        for next in found {
            if seen.contains_key(&next) {
                continue;
            }
            if let Some(group) = grows(db, &nodes, index, next) {
                return Err(Grows(group));
            }
            if nodes.len() >= MAX_INSTANCES {
                program.errors.push((
                    instance,
                    None,
                    UseError::NoImpl("more instances than the collector allows".into()),
                ));
                return Ok(program);
            }
            seen.insert(next, nodes.len());
            queue.push_back(nodes.len());
            nodes.push(Node { instance: next, parent: Some(index) });
        }
        program.instances.push(InstanceUses { instance, uses });
    }
    Ok(program)
}

/// Whether a `match` of `instance` can be taken by no arm.
fn can_fail<'db>(db: &'db dyn Db, instance: Instance<'db>) -> bool {
    instance.body(db).is_some_and(|body| {
        body_decisions(db, body)
            .matches
            .iter()
            .any(|(_, d)| d.as_ref().is_ok_and(|d| d.nodes.contains(&DecisionNode::Fail)))
    })
}

/// The instance of the prelude's `panic`, as `root` sees it, which a
/// `match` that no arm takes calls, if there's a prelude with one.
pub fn panic_instance<'db>(db: &'db dyn Db, root: File) -> Option<Instance<'db>> {
    let f = boltlang_hir_ty::lang::panic_fn(db, root)?;
    Some(plain_instance(db, f, root))
}

/// The instance of a function that isn't generic, like a test.
fn plain_instance<'db>(db: &'db dyn Db, f: FunctionId<'db>, root: File) -> Instance<'db> {
    let body = DefWithBodyId::new(db, DefWithBody::Function(f));
    let count = generics(db, body)
        .as_ref()
        .map_or(0, |g| g.kinds.iter().filter(|k| **k == Kind::Ty).count());
    Instance::of_body(db, body, vec![MonoTy::unit(); count].into_boxed_slice(), root)
}

/// The instances a use needs, with those of the dictionaries it gives.
fn instances_of_use<'db>(
    db: &'db dyn Db,
    u: &Use<'db>,
    root: File,
    out: &mut Vec<Instance<'db>>,
    seen_dicts: &mut HashSet<DictKey<'db>>,
    program: &mut Program<'db>,
) -> Result<(), Grows<'db>> {
    let mut dicts: Vec<&Dict<'db>> = Vec::new();
    match u {
        Use::Instance { instance, dicts: given, .. } => {
            out.push(*instance);
            dicts.extend(given.iter());
        }
        Use::Method { dict, dicts: given, .. } => {
            dicts.push(dict);
            dicts.extend(given.iter());
        }
        Use::Builtin(_) | Use::Extern(_) | Use::Op { .. } => {}
    }
    while let Some(dict) = dicts.pop() {
        match dict {
            Dict::Param(_) => {}
            Dict::Super { of, .. } => dicts.push(of),
            Dict::Impl { imp, args, subs } => {
                dicts.extend(subs.iter());
                let key = (*imp, args.clone());
                if !seen_dicts.insert(key.clone()) {
                    continue;
                }
                match dict_entries(db, *imp, args, root) {
                    Ok(entries) => {
                        out.extend(entries.iter().map(|(_, i)| *i));
                        program.dicts.push((key, entries));
                    }
                    Err(e) => program.errors.push((Instance::entry(db, root), None, e)),
                }
            }
        }
    }
    Ok(())
}

/// The functions to erase if `next`, needed by the instance `index`, is a
/// function's instance whose types grow on one of its own on the path to
/// it: those of the path from that one to `next`.
fn grows<'db>(
    db: &'db dyn Db,
    nodes: &[Node<'db>],
    index: usize,
    next: Instance<'db>,
) -> Option<Vec<FunctionId<'db>>> {
    let f = next.function(db)?;
    let mut at = Some(index);
    let mut path = Vec::new();
    while let Some(i) = at {
        let node = &nodes[i];
        if let Some(g) = node.instance.function(db) {
            path.push(g);
        }
        if node.instance.function(db) == Some(f) && inside(node.instance.args(db), next.args(db)) {
            path.push(f);
            return Some(path);
        }
        at = node.parent;
    }
    None
}

/// Whether each of `old` is in what's in the same place of `new`, and one
/// is strictly inside it.
fn inside(old: &[MonoTy<'_>], new: &[MonoTy<'_>]) -> bool {
    old.len() == new.len()
        && old.iter().zip(new).all(|(a, b)| occurs(a, b))
        && old.iter().zip(new).any(|(a, b)| a != b)
}

fn occurs<'db>(a: &MonoTy<'db>, b: &MonoTy<'db>) -> bool {
    if a == b {
        return true;
    }
    match b {
        MonoTy::Adt(_, args, ..) | MonoTy::Tuple(args, _) | MonoTy::Record(args, ..) => {
            args.iter().any(|t| occurs(a, t))
        }
        MonoTy::Array(_, elem) => occurs(a, elem),
        MonoTy::Fn(params, _, ret, _, row) => {
            params.iter().any(|t| occurs(a, t))
                || occurs(a, ret)
                || row.effects.iter().flat_map(|e| e.args.iter()).any(|t| occurs(a, t))
        }
        MonoTy::Row(row) => row.effects.iter().flat_map(|e| e.args.iter()).any(|t| occurs(a, t)),
        MonoTy::ErasedAssoc(_, args) => args.iter().any(|t| occurs(a, t)),
        MonoTy::Builtin(_)
        | MonoTy::Never
        | MonoTy::Erased(_)
        | MonoTy::Any(_)
        | MonoTy::Host(_) => false,
    }
}

/// The uses in an instance's MIR, of its body and its lambdas, and those
/// that can't be compiled.
pub fn scan<'db>(cx: &Cx<'_, 'db>) -> Scanned<'db> {
    let db = cx.db;
    let shim;
    // Each body, with its number in `MirBodies::all`.
    let all: Vec<(u32, &MirBody<'db>)> = match cx.instance.def(db) {
        InstanceDef::Body(body) => {
            let bodies = mir_bodies(db, *body);
            std::iter::once(&bodies.body)
                .chain(&bodies.lambdas)
                .enumerate()
                .map(|(i, b)| (i as u32, b))
                .collect()
        }
        InstanceDef::Default(body, param) => {
            mir_bodies(db, *body).default(*param).into_iter().collect()
        }
        InstanceDef::Shim(s) => {
            shim = shim_body(db, s, cx.instance.root(db));
            vec![(0, &shim)]
        }
    };
    let mut uses = Vec::new();
    let mut errors = Vec::new();
    let mut push = |site: Site<'db>, found: Result<Use<'db>, UseError>| match found {
        Ok(u) => uses.push((site, u)),
        Err(e) => errors.push((site, e)),
    };
    for (body, mir) in all {
        // The nodes that are called, which an operation as anything else
        // isn't.
        let callees: HashSet<ExprId> = mir
            .exprs()
            .filter_map(|(_, n)| match &n.kind {
                ExprKind::Call { callee, .. } => Some(*callee),
                _ => None,
            })
            .collect();
        for (expr, node) in mir.exprs() {
            let at = |role| Site { body, expr, role };
            let item_use =
                |item: &ItemRef<'db>, ty: &Ty<'db>| cx.ty(ty).and_then(|t| cx.item_use(item, &t));
            match &node.kind {
                // An operation that isn't called is a function: its shim.
                ExprKind::Item(ItemRef::Op(f)) if !callees.contains(&expr) => {
                    let found = cx.ty(&node.ty).and_then(|t| cx.op_fn_use(*f, &t));
                    push(at(Role::Item), found)
                }
                ExprKind::Item(item) => push(at(Role::Item), item_use(item, &node.ty)),
                ExprKind::Imm { expr: inner } => match cx.imm_use(&mir[*inner].ty) {
                    Ok(Some(u)) => push(at(Role::Imm), Ok(u)),
                    Ok(None) => {}
                    Err(e) => push(at(Role::Imm), Err(e)),
                },
                ExprKind::For { pat, iter, next, .. } => {
                    push(at(Role::Iter), item_use(&iter.item, &iter.ty));
                    push(at(Role::Next), item_use(&next.item, &next.ty));
                    if let Pattern::Decision(d) = pat {
                        consts_in(db, cx, d, at, &mut push);
                    }
                }
                ExprKind::Assign { place, op, .. } => {
                    if let Some(op) = op {
                        push(at(Role::Op), item_use(&op.item, &op.ty));
                    }
                    if let Place::Index { get, set, .. } = place {
                        if let Some(get) = get {
                            push(at(Role::Get), item_use(&get.item, &get.ty));
                        }
                        push(at(Role::Set), item_use(&set.item, &set.ty));
                    }
                }
                ExprKind::Match { decision, .. } | ExprKind::Let { decision, .. } => {
                    consts_in(db, cx, decision, at, &mut push);
                }
                ExprKind::Block { stmts, .. } => {
                    for s in stmts.iter() {
                        if let Stmt::Let { pat: Pattern::Decision(d), .. } = s {
                            consts_in(db, cx, d, at, &mut push);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    (uses, errors)
}

/// The constants that a decision compares with, as uses. A constant in a
/// pattern is a file's or an impl's: a class constant can't be matched on,
/// since its value isn't known.
fn consts_in<'db>(
    db: &'db dyn Db,
    cx: &Cx<'_, 'db>,
    d: &Decision<'db>,
    at: impl Fn(Role<'db>) -> Site<'db>,
    push: &mut impl FnMut(Site<'db>, Result<Use<'db>, UseError>),
) {
    for node in &d.nodes {
        let consts: Vec<ConstId<'db>> = match node {
            DecisionNode::Equals { value: Value::Const(c), .. } => vec![*c],
            DecisionNode::Range { start, end, .. } => [start, end]
                .into_iter()
                .filter_map(|b| match b {
                    Some(Bound::Const(c)) => Some(*c),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        for c in consts {
            let body = DefWithBodyId::new(db, DefWithBody::Const(c));
            let instance = Instance::of_body(db, body, Box::default(), cx.instance.root(db));
            let u = Use::Instance {
                instance,
                mutabilities: Box::default(),
                rows: Box::default(),
                dicts: Box::default(),
            };
            push(at(Role::Const(c)), Ok(u));
        }
    }
}

impl<'db> Site<'db> {
    /// The MIR body the use is in, of `def`, of one of its lambdas, or of
    /// one of its defaults.
    pub fn mir<'a>(&self, bodies: &'a MirBodies<'db>) -> Option<&'a MirBody<'db>> {
        bodies.nth(self.body)
    }

    /// The HIR expression the use's node came from, for errors.
    pub fn origin(&self, db: &'db dyn Db, def: DefWithBodyId<'db>) -> Option<HirExprId> {
        self.mir(mir_bodies(db, def))?[self.expr].origin
    }

    /// The type the use is used at: that of its node or its callee.
    pub fn use_ty(&self, db: &'db dyn Db, def: DefWithBodyId<'db>) -> Option<Ty<'db>> {
        let node = &self.mir(mir_bodies(db, def))?[self.expr];
        match (&node.kind, self.role) {
            (_, Role::Item) => Some(node.ty.clone()),
            (ExprKind::For { iter, .. }, Role::Iter) => Some(iter.ty.clone()),
            (ExprKind::For { next, .. }, Role::Next) => Some(next.ty.clone()),
            (ExprKind::Assign { op: Some(op), .. }, Role::Op) => Some(op.ty.clone()),
            (ExprKind::Assign { place: Place::Index { get: Some(get), .. }, .. }, Role::Get) => {
                Some(get.ty.clone())
            }
            (ExprKind::Assign { place: Place::Index { set, .. }, .. }, Role::Set) => {
                Some(set.ty.clone())
            }
            _ => None,
        }
    }
}
