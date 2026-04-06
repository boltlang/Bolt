use std::{collections::{HashMap, HashSet}, vec};

use lazy_static::lazy_static;

use crate::{
    Diagnostic, SyntaxKind::*, SyntaxToken, ast::*, diagnostic::{
        BindingNotFoundDiagnostic,
        Span
    }, tc::{TVSub, TVar, Type, solve::Solver}, util::{DropBomb, IterExt}
};

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

struct ForkedEnv {
    id: TypeEnvId,
    bomb: DropBomb,
}

impl ForkedEnv {

    fn new(id: TypeEnvId) -> Self {
        Self {
            id,
            bomb: DropBomb::new("typing environment must explicitly be dropped"),
        }
    }

    fn id(&self) -> TypeEnvId {
        self.id
    }

}

pub type Constraints = Vec<Constraint>;

lazy_static! {

    static ref INT_TYPE: Type = Type::Con("Int".to_string(), vec![]);
    static ref STRING_TYPE: Type = Type::Con("String".to_string(), vec![]);
    static ref BOOL_TYPE: Type = Type::Con("Bool".to_string(), vec![]);
    static ref UNIT_TYPE: Type = Type::Con("Unit".to_string(), vec![]);

}

pub struct InferContext {
    pub solver: Solver,
    envs: Vec<TypeEnvData>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Provenance {
    TypeSignature(Span),
    AppExpectedFun(Span),
    UnexpectedFun(Span),
    ExpectedUnify(Span),
}

impl Provenance {

    pub fn span(&self) -> &Span {
        match self {
            Provenance::TypeSignature(span) => span,
            Provenance::AppExpectedFun(span) => span,
            Provenance::UnexpectedFun(span) => span,
            Provenance::ExpectedUnify(span) => span,
        }
    }

}

impl InferContext {

    pub fn new() -> Self {
        let mut global_env = TypeEnvData::new();
        global_env.add("True", SymbolKind::Var, Scheme::mono(BOOL_TYPE.clone()));
        global_env.add("False", SymbolKind::Var, Scheme::mono(BOOL_TYPE.clone()));
        global_env.add("Bool", SymbolKind::Type, Scheme::mono(BOOL_TYPE.clone()));
        global_env.add("Int", SymbolKind::Type, Scheme::mono(INT_TYPE.clone()));
        global_env.add("String", SymbolKind::Type, Scheme::mono(STRING_TYPE.clone()));
        Self {
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

    fn lookup(&mut self, name: &str, span: Span, kind: SymbolKind, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        // Search in this environment and all parent environments
        for i in (0..env).rev() {
            let scm = self.envs[i].get(name, kind).cloned();
            if let Some(scm) = scm {
                 return (self.instantiate(&scm), vec![], vec![]);
            }
        }
        // Binding waSources not found
        (
            self.fresh_type_var().into(),
            vec![],
            vec![
                BindingNotFoundDiagnostic::new(name.to_owned(), kind, span).into()
            ]
        )
    }

    fn fork_env(&mut self, _env: TypeEnvId) -> ForkedEnv {
        let i = self.envs.len();
        self.envs.push(TypeEnvData::new());
        ForkedEnv::new(i)
    }

    fn drop_env(&mut self, mut env: ForkedEnv) {
        env.bomb.defuse();
        debug_assert!(self.envs.len()-1 == env.id);
        self.envs.pop();
    }

    fn infer_literal(&mut self, token: SyntaxToken) -> Type {
        let ty = match token.kind() {
            BIN_INT | OCT_INT | DEC_INT | HEX_INT => INT_TYPE.clone(),
            STRING => STRING_TYPE.clone(),
            _ => unreachable!(),
        };
        ty
    }

    fn infer_pattern(&mut self, pattern: &Pattern, to_insert: TypeEnvId, _env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match pattern {
            Pattern::Named(named) => {
                let ty: Type = self.fresh_type_var().into();
                if let Some(name) = named.name() {
                    self.env_add(to_insert, name.text().to_string(), SymbolKind::Var, Scheme::mono(ty.clone()));
                }
                (ty, vec![], vec![])
            }
        }
    }

    pub fn infer_expr(&mut self, expr: &Expr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match expr {
            Expr::Block(expr) => {
                let elements = match expr.block() {
                    Some(block) => block.elements().collect(),
                    None => vec![],
                };
                if elements.is_empty() {
                    return (UNIT_TYPE.clone(), vec![], vec![]);
                }
                let mut cs = Constraints::new();
                let mut ds = Vec::new();
                for element in elements.iter().skip_last(1) {
                    let (el_cs, el_ds) = self.infer_element(element, false, env);
                    cs.extend(el_cs);
                    ds.extend(el_ds);
                }
                let last = elements.last().unwrap();
                let ty = if let SourceElement::Expr(expr) = last {
                    let (ty, ty_out, ty_ds) = self.infer_expr(&expr, env);
                    cs.extend(ty_out);
                    ds.extend(ty_ds);
                    ty
                } else {
                    UNIT_TYPE.clone()
                };
                (ty, cs, ds)
            }
            Expr::Named(named) => match named.name() {
                Some(name) => self.lookup(
                    name.text(),
                    name.text_range().into(),
                    SymbolKind::Var,
                    env
                ),
                None => (self.fresh_type_var().into(), vec![], vec![]),
            }
            Expr::Lit(lit) => match lit.value() {
                Some(lit) => (self.infer_literal(lit), vec![], vec![]),
                None => (self.fresh_type_var().into(), vec![], vec![]),
            }
            Expr::Fun(fun) => {
                let mut cs = Constraints::new();
                let mut ds = Vec::new();
                let new_env = self.fork_env(env);
                let (mut ty, ty_cs, ty_ds) = match fun.body() {
                    Some(expr) => self.infer_expr(&expr, env),
                    None => (self.fresh_type_var().into(), vec![], vec![]),
                };
                cs.extend(ty_cs);
                ds.extend(ty_ds);
                for pattern in fun.params().collect::<Vec<_>>().into_iter().rev() {
                    let (param_ty, param_cs, param_ds) = self.infer_pattern(&pattern, new_env.id(), env);
                    cs.extend(param_cs);
                    ds.extend(param_ds);
                    ty = Type::fun(param_ty, ty);
                }
                self.drop_env(new_env);
                (ty, cs, ds)
            }
            Expr::Call(call) => {
                let (mut fun_ty, mut cs, mut ds) = match call.operator() {
                    Some(e) => self.infer_expr(&e, env),
                    None => (self.fresh_type_var().into(), vec![], vec![]),
                };
                for arg in call.args() {
                    let (arg_ty, ret_ty_2) = match &fun_ty {
                        Type::Fun(left, right) => (left.as_ref().clone(), right.as_ref().clone()),
                        ty => {
                            let arg_ty: Type = self.fresh_type_var().into();
                            let ret_ty: Type = self.fresh_type_var().into();
                            cs.push(Constraint::TypesEqual {
                                provenance: Provenance::AppExpectedFun(call.syntax().text_range().into()),
                                left: Type::fun(arg_ty.clone(), ret_ty.clone()),
                                right: ty.clone(),
                            });
                            (arg_ty, ret_ty)
                        }
                    };
                    let (check_cs, check_ds) = self.check_expr(&arg, &arg_ty, env);
                    cs.extend(check_cs);
                    ds.extend(check_ds);
                    fun_ty = ret_ty_2;
                }
                (fun_ty, cs, ds)
            }
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, ty: Type, to_insert: TypeEnvId, _env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        match pattern {
            Pattern::Named(named) => {
                if let Some(name) = named.name() {
                    self.env_add(to_insert, name.text(), SymbolKind::Var, Scheme::mono(ty));
                }
                (vec![], vec![])
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, ty: &Type, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        // Attempt to handle some special cases
        match (expr, ty) {
            // Case where a literal expression is matched with a literal type
            (Expr::Lit(lit), ty) => if let Some(value) = lit.value() {
                if self.infer_literal(value) == *ty {
                    return (vec![], vec![]);
                }
            }
            // Case where a lambda expression is being compared with the type
            (Expr::Fun(fun), ty) => {
                let mut ds = Vec::new();
                let mut cs = Constraints::new();
                let new_env = self.fork_env(env);
                let mut ty = ty.clone();
                for pattern in fun.params() {
                    let (arg_ty, ret_ty) = match ty {
                        Type::Fun(arg_ty, ret_ty) => (arg_ty.as_ref().clone(), ret_ty.as_ref().clone()),
                        ty => {
                            let arg_ty: Type = self.fresh_type_var().into();
                            let ret_ty: Type = self.fresh_type_var().into();
                            cs.push(Constraint::TypesEqual {
                                provenance: Provenance::UnexpectedFun(fun.syntax().text_range().into()),
                                left: ty.clone(),
                                right: Type::fun(arg_ty.clone(), ret_ty.clone()),
                            });
                            (arg_ty, ret_ty)
                        }
                    };
                    let (param_cs, param_ds) = self.check_pattern(&pattern, arg_ty, new_env.id(), env);
                    cs.extend(param_cs);
                    ds.extend(param_ds);
                    ty = ret_ty;
                }
                let (body_cs, body_ds) = self.check_expr(expr, &ty, new_env.id());
                cs.extend(body_cs);
                ds.extend(body_ds);
                self.drop_env(new_env);
                return (cs, ds)
            },
            // No special case, so the logic below will run
            _ => {},
        }
        // Fallback logic that just performs further downward inference
        let (actual_ty, mut cs, ds) = self.infer_expr(expr, env);
        cs.push(Constraint::TypesEqual {
            provenance: Provenance::ExpectedUnify(expr.syntax().text_range().into()),
            left: actual_ty,
            right: ty.clone(),
        });
        (cs, ds)
    }

    fn infer_type_expr(&mut self, te: &TypeExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match te {
            TypeExpr::Arrow(arrow) => {
                let mut cs = Constraints::new();
                let mut ds = Vec::new();
                let ret_ty = match arrow.return_ty() {
                    None => self.fresh_type_var().into(),
                    Some(te) => {
                        let (ret_ty, ret_cs, ret_ds) = self.infer_type_expr(&te, env);
                        cs.extend(ret_cs);
                        ds.extend(ret_ds);
                        ret_ty
                    }
                };
                let ty = Type::signature(
                    arrow.params()
                        .map(|te| {
                            let (param_ty, param_cs, param_ds) = self.infer_type_expr(&te, env);
                            cs.extend(param_cs);
                            ds.extend(param_ds);
                            param_ty
                        })
                        // FIXME params should return a DoubleEndedIterator
                        .collect::<Vec<_>>(),
                    ret_ty
                );
                (ty, cs, ds)
            }
            TypeExpr::Named(named) => match named.name() {
                None => (self.fresh_type_var().into(), vec![], vec![]),
                Some(name) => self.lookup(
                    name.text(),
                    name.text_range().into(),
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
        generalize: bool,
        env: TypeEnvId
    ) -> (Type, Constraints, Vec<Diagnostic>) {
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        let mut ty = None;
        if let Some(te) = te {
            let (te_ty, te_ty_cs, te_ty_ds) = self.infer_type_expr(&te, env);
            cs.extend(te_ty_cs);
            ds.extend(te_ty_ds);
            ty = Some(te_ty);
        }
        if let Some(pattern) = pattern {
            match &ty {
                Some(ty) => {
                    let (patt_cs, patt_ds) = self.check_pattern(&pattern, ty.clone(), to_insert, env);
                    cs.extend(patt_cs);
                    ds.extend(patt_ds);
                }
                None => {
                    let (patt_ty, patt_cs, patt_ds) = self.infer_pattern(&pattern, to_insert, env);
                    cs.extend(patt_cs);
                    ds.extend(patt_ds);
                    ty = Some(patt_ty);
                }
            }
        }
        if let Some(expr) = expr {
            match &ty {
                Some(ty) => {
                    let (expr_cs, expr_ds) = self.check_expr(&expr, &ty, env);
                    cs.extend(expr_cs);
                    ds.extend(expr_ds);
                }
                None => {
                    let (_expr_ty, expr_cs, expr_ds) = self.infer_expr(&expr, env);
                    cs.extend(expr_cs);
                    ds.extend(expr_ds);
                }
            }
        }
        (ty.unwrap_or_else(|| self.fresh_type_var().into()), cs, ds)
    }

    fn infer_var_decl(&mut self, decl: &VarDecl, is_toplevel: bool, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        let (_ty, cs, ds) = self.infer_var_decl_like(
            &decl.pattern(),
            &decl.type_expr(),
            &decl.expr(),
            env,
            is_toplevel,
            env
        );
        (cs, ds)
    }

    fn infer_func_decl(&mut self, node: &FuncDecl, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        let new_env = self.fork_env(env);
        let ret_ty = match node.body() {
            None => self.fresh_type_var().into(),
            Some(expr) => {
                let (body_ty, body_cs, body_ds) = self.infer_expr(&expr, env);
                cs.extend(body_cs);
                ds.extend(body_ds);
                body_ty
            }
        };
        let actual_ty = Type::signature(
            node.params()
                .map(|param| {
                    let (param_ty, param_cs, param_ds) = self.infer_var_decl_like(
                        &param.pattern(),
                        &param.type_expr(),
                        &param.default(),
                        new_env.id(),
                        false,
                        env
                    );
                    cs.extend(param_cs);
                    ds.extend(param_ds);
                    param_ty
                })
                .collect::<Vec<_>>(),
            ret_ty
        );
        if let Some(te) = node.type_signature() {
            let (sig_ty, sig_cs, sig_ds) = self.infer_type_expr(&te, env);
            cs.extend(sig_cs);
            ds.extend(sig_ds);
            cs.push(Constraint::TypesEqual {
                provenance: Provenance::TypeSignature(te.syntax().text_range().into()),
                left: actual_ty.clone(),
                right: sig_ty,
            });
        }
        if let Some(name) = node.name() {
            self.env_add(env, name.text(), SymbolKind::Var, self.generalize(actual_ty, env));
        }
        self.drop_env(new_env);
        (cs, ds)
    }

    fn infer_element(&mut self, element: &SourceElement, is_toplevel: bool, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        match element {
            SourceElement::VarDecl(decl) => self.infer_var_decl(decl, is_toplevel, env),
            SourceElement::FuncDecl(decl) => self.infer_func_decl(decl, env),
            SourceElement::Expr(expr) => {
                let (_ty, cs, ds) = self.infer_expr(expr, env);
                (cs, ds)
            }
        }
    }

    pub fn infer_source_file(&mut self, node: &SourceFile) -> (Constraints, Vec<Diagnostic>) {
        let env = self.fork_env(self.global_env());
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        for element in node.elements() {
            let (el_cs, el_ds) = self.infer_element(&element, true, env.id());
            cs.extend(el_cs);
            ds.extend(el_ds);
        }
        self.drop_env(env);
        (cs, ds)
    }

    pub fn solve(&mut self, constraints: &[Constraint]) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for constraint in constraints {
            out.extend(self.solver.add(constraint));
        }
        self.solver.solve();
        out
    }

}

