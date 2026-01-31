use std::{collections::{HashMap, HashSet}, vec};

use lazy_static::lazy_static;
use notify_debouncer_mini::new_debouncer;

use crate::{ast::*, diagnostic::{BindingNotFoundDiagnostic, Diagnostics, Source}, tc::{solve::Solver, TVSub, TVar, Type}, util::IterExt, File, SyntaxKind::*, SyntaxToken};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SymbolKind {
    Var,
    Type,
}

pub type TVSet = HashSet<TVar>;

#[derive(Clone, Eq, PartialEq)]
pub struct Scheme {
    unbound: TVSet,
    ty: Type,
}

impl Scheme {

    pub fn new(unbound: TVSet, ty: Type) -> Self {
        Self { unbound, ty }
    }

    pub fn mono(ty: Type) -> Self {
        Self { ty, unbound: TVSet::new() }
    }

    pub fn has_uni_var(&self, tv: &TVar) -> bool {
        !self.unbound.contains(tv) && self.ty.has_uni_var(tv)
    }

}

pub enum Constraint {
    TypesEqual {
        provenance: Provenance,
        left: Type,
        right: Type,
    },
}

struct TypeEnvData {
    mapping: HashMap<(SymbolKind, String), Scheme>,
}

impl TypeEnvData {

    fn new() -> Self {
        Self { mapping: HashMap::new() }
    }

    fn get(&self, name: &str, kind: SymbolKind) -> Option<&Scheme> {
        self.mapping.get(&(kind, name.to_owned()))
    }

    fn add<S: Into<String>>(&mut self, name: S, kind: SymbolKind, scheme: Scheme) {
        let name = name.into();
        debug_assert!(!self.mapping.contains_key(&(kind, name.clone())));
        self.mapping.insert((kind, name), scheme);
    }

}

type TypeEnvId = usize;

pub type Constraints = Vec<Constraint>;

lazy_static! {

    static ref INT_TYPE: Type = Type::Con("Int".to_string(), vec![]);
    static ref STRING_TYPE: Type = Type::Con("String".to_string(), vec![]);
    static ref BOOL_TYPE: Type = Type::Con("Bool".to_string(), vec![]);
    static ref UNIT_TYPE: Type = Type::Con("Unit".to_string(), vec![]);

}

pub struct InferContext<'d> {
    diagnostics: &'d mut dyn Diagnostics,
    pub solver: Solver,
    envs: Vec<TypeEnvData>,
}

pub enum Provenance {
    TypeSignature(Source),
    AppExpectedFun(Source),
    UnexpectedFun(Source),
    ExpectedUnify(Source),
}

impl Provenance {

    pub fn source(&self) -> &Source {
        match self {
            Provenance::TypeSignature(source) => source,
            Provenance::AppExpectedFun(source) => source,
            Provenance::UnexpectedFun(source) => source,
            Provenance::ExpectedUnify(source) => source,
        }
    }

}

impl <'d> InferContext<'d> {

    pub fn new(diagnostics: &'d mut dyn Diagnostics) -> Self {
        let mut global_env = TypeEnvData::new();
        global_env.add("True", SymbolKind::Var, Scheme::mono(BOOL_TYPE.clone()));
        global_env.add("False", SymbolKind::Var, Scheme::mono(BOOL_TYPE.clone()));
        global_env.add("Bool", SymbolKind::Type, Scheme::mono(BOOL_TYPE.clone()));
        global_env.add("Int", SymbolKind::Type, Scheme::mono(INT_TYPE.clone()));
        global_env.add("String", SymbolKind::Type, Scheme::mono(STRING_TYPE.clone()));
        Self {
            diagnostics,
            envs: vec![ global_env ],
            solver: Solver::new(),
        }
    }

    fn fresh_type_var(&mut self) -> TVar {
        self.solver.unifier.fresh_type_var()
    }

    fn instantiate(&mut self, scm: &Scheme) -> Type {
        let mut sub = TVSub::new();
        for tv in &scm.unbound {
            sub.insert(*tv, self.fresh_type_var().into());
        }
        scm.ty.clone().substitute(&sub)
    }

    fn global_env(&self) -> TypeEnvId {
        0
    }

    fn generalize(&self, ty: Type, env: TypeEnvId) -> Scheme {
        let mut unbound = TVSet::new();
        for tv in ty.uni_vars() {
            if !self.env_has_uni_var(env, &tv) {
                unbound.insert(tv);
            }
        }
        Scheme::new(unbound, ty)
    }

    fn env_has_uni_var(&self, env: TypeEnvId, tv: &TVar) -> bool {
        for i in (0..env).rev() {
            let data = &self.envs[i];
            for scm in data.mapping.values() {
                if scm.has_uni_var(tv) {
                    return true;
                }
            }
        }
        false
    }

    fn env_add<S: Into<String>>(&mut self, env: TypeEnvId, name: S, kind: SymbolKind, scm: Scheme) {
        self.envs[env].add(name, kind, scm);
    }

    fn lookup(&mut self, name: &str, source: Source, kind: SymbolKind, env: TypeEnvId) -> (Type, Constraints) {
        for i in (0..env).rev() {
            let scm = self.envs[i].get(name, kind).cloned();
            if let Some(scm) = scm {
                 return (self.instantiate(&scm), vec![]);
            }
        }
        self.diagnostics.add(BindingNotFoundDiagnostic::new(name.to_owned(), kind, source).into());
        (self.fresh_type_var().into(), vec![])
    }

    fn fork_env(&mut self, _env: TypeEnvId) -> TypeEnvId {
        let i = self.envs.len();
        self.envs.push(TypeEnvData::new());
        i
    }

    fn infer_literal(&mut self, token: SyntaxToken) -> (Type, Constraints) {
        match token.kind() {
            BIN_INT | OCT_INT | DEC_INT | HEX_INT => (INT_TYPE.clone(), vec![]),
            STRING => (STRING_TYPE.clone(), vec![]),
            _ => unreachable!(),
        }
    }

    fn infer_pattern(&mut self, pattern: &Pattern, to_insert: TypeEnvId, env: TypeEnvId, file: File) -> (Type, Constraints) {
        match pattern {
            Pattern::Named(named) => {
                let ty: Type = self.fresh_type_var().into();
                if let Some(name) = named.name() {
                    self.env_add(to_insert, name.text().to_string(), SymbolKind::Var, Scheme::mono(ty.clone()));
                }
                (ty, vec![])
            }
        }
    }

    pub fn infer_expr(&mut self, expr: &Expr, env: TypeEnvId, file: File) -> (Type, Constraints) {
        match expr {
            Expr::Block(block) => {
                let elements: Vec<_> = block.elements().collect();
                if elements.is_empty() {
                    return (UNIT_TYPE.clone(), vec![]);
                }
                let mut out = Constraints::new();
                for element in elements.iter().skip_last(1) {
                    out.extend(self.infer_element(element, env, file));
                }
                let last = elements.last().unwrap();
                let ty = if let SourceElement::Expr(expr) = last {
                    let (ty, ty_out) = self.infer_expr(&expr, env, file);
                    out.extend(ty_out);
                    ty
                } else {
                    UNIT_TYPE.clone()
                };
                (ty, out)
            }
            Expr::Named(named) => match named.name() {
                Some(name) => self.lookup(
                    name.text(),
                    Source::new(file, name.text_range().into()),
                    SymbolKind::Var,
                    env
                ),
                None => (self.fresh_type_var().into(), vec![]),
            }
            Expr::Lit(lit) => match lit.value() {
                Some(lit) => self.infer_literal(lit),
                None => (self.fresh_type_var().into(), vec![]),
            }
            Expr::Fun(fun) => {
                let mut out = Constraints::new();
                let new_env = self.fork_env(env);
                let (mut ty, ty_out) = match fun.body() {
                    Some(expr) => self.infer_expr(&expr, env, file),
                    None => (self.fresh_type_var().into(), vec![]),
                };
                out.extend(ty_out);
                for pattern in fun.params().collect::<Vec<_>>().into_iter().rev() {
                    let (param_ty, param_out) = self.infer_pattern(&pattern, new_env, env, file);
                    out.extend(param_out);
                    ty = Type::fun(param_ty, ty);
                }
                (ty, out)
            }
            Expr::Call(call) => {
                // FIXME check this is correct
                let (mut fun_ty, fun_out) = match call.operator() {
                    Some(e) => self.infer_expr(&e, env, file),
                    None => (self.fresh_type_var().into(), vec![]),
                };
                let mut out = fun_out;
                for arg in call.args() {
                    let (arg_ty, ret_ty_2) = match &fun_ty {
                        Type::Fun(left, right) => (left.as_ref().clone(), right.as_ref().clone()),
                        ty => {
                            let arg_ty: Type = self.fresh_type_var().into();
                            let ret_ty: Type = self.fresh_type_var().into();
                            out.push(Constraint::TypesEqual {
                                provenance: Provenance::AppExpectedFun(Source::new(file, call.syntax().text_range().into())),
                                left: Type::fun(arg_ty.clone(), ret_ty.clone()),
                                right: ty.clone(),
                            });
                            (arg_ty, ret_ty)
                        }
                    };
                    out.extend(self.check_expr(&arg, &arg_ty, env, file));
                    fun_ty = ret_ty_2;
                }
                (fun_ty, out)
            }
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, ty: Type, to_insert: TypeEnvId, env: TypeEnvId) -> Constraints {
        match pattern {
            Pattern::Named(named) => {
                if let Some(name) = named.name() {
                    self.env_add(to_insert, name.text(), SymbolKind::Var, Scheme::mono(ty));
                }
                vec![]
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, ty: &Type, env: TypeEnvId, file: File) -> Constraints {
        match (expr, ty) {
            (Expr::Fun(fun), ty) => {
                let mut out = Constraints::new();
                let new_env = self.fork_env(env);
                let mut ty = ty.clone();
                for pattern in fun.params() {
                    let (arg_ty, ret_ty) = match ty {
                        Type::Fun(arg_ty, ret_ty) => (arg_ty.as_ref().clone(), ret_ty.as_ref().clone()),
                        ty => {
                            let arg_ty: Type = self.fresh_type_var().into();
                            let ret_ty: Type = self.fresh_type_var().into();
                            out.push(Constraint::TypesEqual {
                                provenance: Provenance::UnexpectedFun(Source::new(file, fun.syntax().text_range().into())),
                                left: ty.clone(),
                                right: Type::fun(arg_ty.clone(), ret_ty.clone()),
                            });
                            (arg_ty, ret_ty)
                        }
                    };
                    let param_out = self.check_pattern(&pattern, arg_ty, new_env, env);
                    out.extend(param_out);
                    ty = ret_ty;
                }
                let body_out = self.check_expr(expr, &ty, new_env, file);
                out.extend(body_out);
                out
            }
            _ => {
                let (actual_ty, mut out) = self.infer_expr(expr, env, file);
                out.push(Constraint::TypesEqual {
                    provenance: Provenance::ExpectedUnify(Source::new(file, expr.syntax().text_range().into())),
                    left: actual_ty,
                    right: ty.clone(),
                });
                out
            }
        }
    }

    fn infer_type_expr(&mut self, te: &TypeExpr, env: TypeEnvId, file: File) -> (Type, Constraints) {
        match te {
            TypeExpr::Arrow(arrow) => {
                let mut out = Constraints::new();
                let ret_ty = match arrow.return_ty() {
                    None => self.fresh_type_var().into(),
                    Some(te) => {
                        let (ret_ty, ret_out) = self.infer_type_expr(&te, env, file);
                        out.extend(ret_out);
                        ret_ty
                    }
                };
                let ty = Type::signature(
                    arrow.params()
                        .map(|te| {
                            let (param_ty, param_out) = self.infer_type_expr(&te, env, file);
                            out.extend(param_out);
                            param_ty
                        })
                        // FIXME params should return a DoubleEndedIterator
                        .collect::<Vec<_>>(),
                    ret_ty
                );
                (ty, out)
            }
            TypeExpr::Named(named) => match named.name() {
                None => (self.fresh_type_var().into(), vec![]),
                Some(name) => self.lookup(
                    name.text(),
                    Source::new(file, name.text_range().into()),
                    SymbolKind::Type,
                    env
                ),
            }
        }
    }

    fn infer_var_decl_like(
        &mut self,
        pattern: &Option<Pattern>,
        te: &Option<TypeExpr>,
        expr: &Option<Expr>,
        to_insert: TypeEnvId,
        env: TypeEnvId,
        file: File
    ) -> (Type, Constraints) {
        let mut out = Constraints::new();
        let mut ty = None;
        if let Some(te) = te {
            let (te_ty, te_ty_out) = self.infer_type_expr(&te, env, file);
            out.extend(te_ty_out);
            ty = Some(te_ty);
        }
        if let Some(pattern) = pattern {
            match &ty {
                Some(ty) => {
                    out.extend(self.check_pattern(&pattern, ty.clone(), to_insert, env));
                }
                None => {
                    let (patt_ty, patt_out) = self.infer_pattern(&pattern, to_insert, env, file);
                    out.extend(patt_out);
                    ty = Some(patt_ty);
                }
            }
        }
        if let Some(expr) = expr {
            match &ty {
                Some(ty) => {
                    out.extend(self.check_expr(&expr, &ty, env, file));
                }
                None => {
                    let (_expr_ty, expr_out) = self.infer_expr(&expr, env, file);
                    out.extend(expr_out);
                }
            }
        }
        (ty.unwrap_or_else(|| self.fresh_type_var().into()), out)
    }

    fn infer_var_decl(&mut self, decl: &VarDecl, env: TypeEnvId, file: File) -> (Type, Constraints) {
        self.infer_var_decl_like(&decl.pattern(), &decl.type_expr(), &decl.expr(), env, env, file)
    }

    fn infer_func_decl(&mut self, node: &FuncDecl, env: TypeEnvId, file: File) -> Constraints {
        let mut out = Constraints::new();
        let new_env = self.fork_env(env);
        let ret_ty = match node.body() {
            None => self.fresh_type_var().into(),
            Some(expr) => {
                let (body_ty, body_out) = self.infer_expr(&expr, env, file);
                out.extend(body_out);
                body_ty
            }
        };
        let actual_ty = Type::signature(
            node.params()
                .map(|param| {
                    let (param_ty, param_out) = self.infer_var_decl_like(
                        &param.pattern(),
                        &param.type_expr(),
                        &param.default(),
                        new_env,
                        env,
                        file
                    );
                    out.extend(param_out);
                    param_ty
                })
                .collect::<Vec<_>>(),
            ret_ty
        );
        if let Some(te) = node.type_signature() {
            let (sig_ty, sig_ty_out) = self.infer_type_expr(&te, env, file);
            out.extend(sig_ty_out);
            out.push(Constraint::TypesEqual {
                provenance: Provenance::TypeSignature(Source::new(file, te.syntax().text_range().into())),
                left: sig_ty,
                right: actual_ty.clone(),
            });
        }
        if let Some(name) = node.name() {
            self.env_add(env, name.text(), SymbolKind::Var, self.generalize(actual_ty, env));
        }
        out
    }

    fn infer_element(&mut self, element: &SourceElement, env: TypeEnvId, file: File) -> Constraints {
        match element {
            SourceElement::VarDecl(decl) => self.infer_var_decl(decl, env, file).1,
            SourceElement::FuncDecl(decl) => self.infer_func_decl(decl, env, file),
            SourceElement::Expr(expr) => self.infer_expr(expr, env, file).1,
        }
    }

    pub fn infer_source_file(&mut self, node: &SourceFile, file: File) -> Constraints {
        let env = self.fork_env(self.global_env());
        node.elements().flat_map(|x| self.infer_element(&x, env, file)).collect()
    }

    pub fn solve(&mut self, constraints: &[Constraint]) {
        for constraint in constraints {
            self.solver.add(constraint, self.diagnostics);
        }
        self.solver.solve();
    }

}

