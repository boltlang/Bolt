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

use std::collections::{HashMap, HashSet, VecDeque};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    body::{Body, DefWithBody, DefWithBodyId, body},
    hir::{BinaryOp, Expr, ExprId, Pat, PatId, Path, PathSegment},
    ids::{FunctionId, ImplId},
    nameres::test_functions,
    resolver::{Resolver, ValueNs, ValueResolution},
};
use boltlang_hir_ty::{Ty, lang::LangClass};

use crate::{
    instance::{Instance, generics},
    ty::MonoTy,
    uses::{ClassItem, Cx, Dict, Use, UseError, dict_entries},
};

/// Where in a body a use is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Site {
    /// A path that names a function or constant, the method of a method
    /// call (the callee), an operator, or an index `xs[i]` that's read.
    Expr(ExprId),
    /// A pattern that names a constant.
    Pat(PatId),
    /// The `set_index` of an assignment to `xs[i]`, at the assignment.
    SetIndex(ExprId),
    /// The `iter` of a `for`, at the `for`.
    Iter(ExprId),
    /// The `next` of a `for`, at the `for`.
    Next(ExprId),
}

/// An instance and what each of its uses is.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct InstanceUses<'db> {
    pub instance: Instance<'db>,
    pub uses: Vec<(Site, Use<'db>)>,
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
    pub errors: Vec<(Instance<'db>, Option<Site>, UseError)>,
}

/// A dictionary of an impl at its type parameters.
pub type DictKey<'db> = (ImplId<'db>, Box<[MonoTy<'db>]>);

/// The uses in a body, and those that can't be compiled.
pub type Scanned<'db> = (Vec<(Site, Use<'db>)>, Vec<(Site, UseError)>);

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
        if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(root) {
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

/// The instance of a function that isn't generic, like a test.
fn plain_instance<'db>(db: &'db dyn Db, f: FunctionId<'db>, root: File) -> Instance<'db> {
    let body = DefWithBodyId::new(db, DefWithBody::Function(f));
    let count = generics(db, body)
        .as_ref()
        .map_or(0, |g| g.kinds.iter().filter(|k| **k == crate::instance::Kind::Ty).count());
    Instance::new(db, body, vec![MonoTy::unit(); count].into_boxed_slice(), root)
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
        Use::Builtin(_) | Use::Extern(_) => {}
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
        MonoTy::Adt(_, args, ..) | MonoTy::Tuple(args) => args.iter().any(|t| occurs(a, t)),
        MonoTy::Array(_, elem) => occurs(a, elem),
        MonoTy::Fn(params, ret, _) => params.iter().any(|t| occurs(a, t)) || occurs(a, ret),
        MonoTy::ErasedAssoc(_, args) => args.iter().any(|t| occurs(a, t)),
        MonoTy::Builtin(_) | MonoTy::Never | MonoTy::Erased(_) => false,
    }
}

/// The uses in an instance's body, and those that can't be compiled.
pub fn scan<'db>(cx: &Cx<'_, 'db>) -> Scanned<'db> {
    let db = cx.db;
    let def = cx.instance.body(db);
    let body = body(db, def);
    let mut scanner = Scanner { cx, def, body, uses: Vec::new(), errors: Vec::new() };
    scanner.run();
    (scanner.uses, scanner.errors)
}

struct Scanner<'a, 'b, 'db> {
    cx: &'a Cx<'b, 'db>,
    def: DefWithBodyId<'db>,
    body: &'a Body,
    uses: Vec<(Site, Use<'db>)>,
    errors: Vec<(Site, UseError)>,
}

impl<'db> Scanner<'_, '_, 'db> {
    fn push(&mut self, site: Site, found: Result<Use<'db>, UseError>) {
        match found {
            Ok(u) => self.uses.push((site, u)),
            Err(e) => self.errors.push((site, e)),
        }
    }

    fn run(&mut self) {
        let body = self.body;
        // The `xs[i]` that are assigned to with `=`, which aren't read.
        let mut written = Vec::new();
        for (_, expr) in body.exprs.iter() {
            if let Expr::Binary { op: BinaryOp::Assign(None), lhs, .. } = expr
                && let Expr::Index { .. } = &body[*lhs]
            {
                written.push(*lhs);
            }
        }
        for (id, expr) in body.exprs.iter() {
            match expr {
                Expr::Path(path) => self.path(id, path),
                Expr::Field { .. } => {
                    if let Some(path) = self.extended_path(id) {
                        self.path(id, &path);
                    }
                }
                Expr::Call { callee, args } => {
                    if let Some(&f) = self.cx.infer.method_calls.get(id) {
                        let Expr::Field { expr: receiver, .. } = &body[*callee] else { continue };
                        let all: Vec<ExprId> =
                            std::iter::once(*receiver).chain(args.iter().copied()).collect();
                        let found = self
                            .fn_ty(&all, self.cx.infer.expr_ty(id))
                            .and_then(|t| self.cx.function_use(*callee, f, &t));
                        self.push(Site::Expr(*callee), found);
                    }
                }
                Expr::Binary { op: BinaryOp::Operator(name), lhs, rhs } => {
                    self.operator(id, name, &[*lhs, *rhs], self.cx.infer.expr_ty(id));
                }
                Expr::Binary { op: BinaryOp::Assign(op), lhs, rhs } => {
                    if let Some(name) = op {
                        let ty = self.cx.infer.expr_ty(*lhs);
                        self.operator(id, name, &[*lhs, *rhs], ty);
                    }
                    if let Expr::Index { base, .. } = &body[*lhs] {
                        self.set_index(id, *base, *rhs);
                    }
                }
                Expr::Prefix { op, expr: inner } => {
                    self.operator(id, op, &[*inner], self.cx.infer.expr_ty(id));
                }
                Expr::Index { base, .. } if !written.contains(&id) => self.index(id, *base),
                Expr::For { iterable, .. } => self.for_loop(id, *iterable),
                _ => {}
            }
        }
        for (id, pat) in body.pats.iter() {
            if let Pat::Path(path) = pat {
                let resolution = Resolver::for_body(self.cx.db, self.def)
                    .resolve_path_in_value_ns(self.cx.db, path);
                if let Some(ValueResolution::Value(ValueNs::Const(c))) = resolution {
                    let found = self.const_use_at_pat(c);
                    self.push(Site::Pat(id), found);
                }
            }
        }
    }

    /// A constant in a pattern is a file's or an impl's: a class constant
    /// can't be matched on, since its value isn't known.
    fn const_use_at_pat(
        &self,
        c: boltlang_hir_def::ids::ConstId<'db>,
    ) -> Result<Use<'db>, UseError> {
        let body = DefWithBodyId::new(self.cx.db, DefWithBody::Const(c));
        Ok(Use::Instance {
            instance: Instance::new(
                self.cx.db,
                body,
                Box::default(),
                self.cx.instance.root(self.cx.db),
            ),
            mutabilities: Box::default(),
            dicts: Box::default(),
        })
    }

    fn path(&mut self, id: ExprId, path: &Path) {
        let db = self.cx.db;
        let resolution = Resolver::for_expr(db, self.def, id).resolve_path_in_value_ns(db, path);
        let value = match resolution {
            Some(ValueResolution::Value(ValueNs::Overloaded(_))) => {
                match self.cx.infer.overload(id) {
                    Some(f) => ValueNs::Function(f),
                    None => return self.push(Site::Expr(id), Err(UseError::UnknownType)),
                }
            }
            Some(ValueResolution::Value(value)) => value,
            _ => return,
        };
        let found = match value {
            ValueNs::Function(f) => {
                self.cx.expr_ty(id).and_then(|t| self.cx.function_use(id, f, &t))
            }
            ValueNs::Const(c) => self.cx.expr_ty(id).and_then(|t| self.cx.const_use(id, c, &t)),
            _ => return,
        };
        self.push(Site::Expr(id), found);
    }

    /// `m.f` or `Vec.new`: a field of something that isn't a value, which
    /// is a longer path, as the type checker reads it.
    fn extended_path(&self, id: ExprId) -> Option<Path> {
        let db = self.cx.db;
        let (base, name) = match &self.body[id] {
            Expr::Field { expr, name } => (*expr, name),
            _ => return None,
        };
        let path = match &self.body[base] {
            Expr::Path(path) => path.clone(),
            Expr::Field { .. } => self.extended_path(base)?,
            _ => return None,
        };
        let resolver = Resolver::for_expr(db, self.def, base);
        if let Some(ValueResolution::Value(_)) = resolver.resolve_path_in_value_ns(db, &path) {
            return None;
        }
        let mut segments = path.segments.to_vec();
        segments.push(PathSegment { name: name.clone(), generic_args: None });
        Some(Path { segments: segments.into() })
    }

    /// The type of a use called with `args` that gives `ret`.
    fn fn_ty(&self, args: &[ExprId], ret: &Ty<'db>) -> Result<MonoTy<'db>, UseError> {
        let params: Box<[MonoTy<'db>]> =
            args.iter().map(|&a| self.cx.expr_ty(a)).collect::<Result<_, _>>()?;
        Ok(MonoTy::Fn(params, Box::new(self.cx.ty(ret)?), crate::ty::MonoMut::Imm))
    }

    /// An operator, a call of the function with its name in scope.
    fn operator(
        &mut self,
        id: ExprId,
        name: &boltlang_hir_def::Name,
        args: &[ExprId],
        ret: &Ty<'db>,
    ) {
        let db = self.cx.db;
        let path = Path::from_name(name.clone());
        let f = match self.cx.infer.overload(id) {
            Some(f) => Some(f),
            None => {
                match Resolver::for_expr(db, self.def, id).resolve_path_in_value_ns(db, &path) {
                    Some(ValueResolution::Value(ValueNs::Function(f))) => Some(f),
                    _ => None,
                }
            }
        };
        let Some(f) = f else { return self.push(Site::Expr(id), Err(UseError::UnknownType)) };
        let found = self.fn_ty(args, ret).and_then(|t| self.cx.function_use(id, f, &t));
        self.push(Site::Expr(id), found);
    }

    /// The method that syntax calls of a class of the prelude, for the
    /// class arguments `args`.
    fn lang_use(
        &mut self,
        site: Site,
        at: ExprId,
        lang: LangClass,
        arg: MonoTy<'db>,
        use_ty: MonoTy<'db>,
    ) {
        let db = self.cx.db;
        let file = self.def.def(db).file(db);
        let Some((class, method)) =
            lang.class(db, file).and_then(|c| Some((c, lang.method(db, c)?)))
        else {
            return self.push(site, Err(UseError::NoImpl(lang.name().to_string())));
        };
        let found = self.cx.class_item_use(at, class, &[arg], ClassItem::Function(method), &use_ty);
        self.push(site, found);
    }

    fn index(&mut self, id: ExprId, base: ExprId) {
        let found = (|| {
            let base_ty = self.cx.expr_ty(base)?;
            let use_ty = MonoTy::Fn(
                Box::new([base_ty.clone(), MonoTy::Builtin(i32_ty())]),
                Box::new(self.cx.expr_ty(id)?),
                crate::ty::MonoMut::Imm,
            );
            Ok((base_ty, use_ty))
        })();
        match found {
            Ok((arg, use_ty)) => self.lang_use(Site::Expr(id), id, LangClass::Index, arg, use_ty),
            Err(e) => self.push(Site::Expr(id), Err(e)),
        }
    }

    fn set_index(&mut self, id: ExprId, base: ExprId, value: ExprId) {
        let found = (|| {
            let base_ty = self.cx.expr_ty(base)?;
            let use_ty = MonoTy::Fn(
                Box::new([base_ty.clone(), MonoTy::Builtin(i32_ty()), self.cx.expr_ty(value)?]),
                Box::new(MonoTy::unit()),
                crate::ty::MonoMut::Imm,
            );
            Ok((base_ty, use_ty))
        })();
        match found {
            Ok((arg, use_ty)) => {
                self.lang_use(Site::SetIndex(id), id, LangClass::IndexMut, arg, use_ty)
            }
            Err(e) => self.push(Site::SetIndex(id), Err(e)),
        }
    }

    fn for_loop(&mut self, id: ExprId, iterable: ExprId) {
        let db = self.cx.db;
        let file = self.def.def(db).file(db);
        let found = (|| {
            let class = LangClass::Iterable.class(db, file).ok_or(UseError::UnknownType)?;
            let alias = LangClass::Iterable.assoc(db, class).ok_or(UseError::UnknownType)?;
            let iterable_hir = self.cx.infer.expr_ty(iterable).clone();
            let iter = self.cx.ty(&Ty::Assoc(alias, Box::new([iterable_hir])))?;
            Ok((self.cx.expr_ty(iterable)?, iter))
        })();
        let (iterable_ty, iter) = match found {
            Ok(found) => found,
            Err(e) => return self.push(Site::Iter(id), Err(e)),
        };
        let mi = crate::ty::MonoMut::Imm;
        let iter_ty = MonoTy::Fn(Box::new([iterable_ty.clone()]), Box::new(iter.clone()), mi);
        self.lang_use(Site::Iter(id), id, LangClass::Iterable, iterable_ty, iter_ty);
        let next_ty = MonoTy::Fn(Box::new([iter.clone()]), Box::new(MonoTy::Never), mi);
        self.lang_use(Site::Next(id), id, LangClass::Iterator, iter, next_ty);
    }
}

fn i32_ty() -> boltlang_hir_def::builtin_type::BuiltinType {
    boltlang_hir_def::builtin_type::BuiltinType::Int(
        boltlang_hir_def::builtin_type::BuiltinInt::I32,
    )
}
