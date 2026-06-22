use std::collections::{HashMap, HashSet};

use lazy_static::lazy_static;

use crate::{Diagnostic, SyntaxKind::*, SyntaxToken, Type, diagnostic::{BindingNotFoundDiagnostic, Span}, tc::{TVSub, solve::{Constraint, Solver}, types::TVar}, util::DropBomb};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SymbolKind {
    Var,
    Type,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Scheme {
    pub unbound: TVSet,
    pub ty: Type,
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

pub type TVSet = HashSet<TVar>;

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

pub struct ForkedEnv {
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

    pub fn id(&self) -> TypeEnvId {
        self.id
    }

}

/// A unique 'pointer' to a typing environment
///
/// Only valid within an InferContext
pub type TypeEnvId = usize;

pub struct InferContext {
    pub solver: Solver,
    envs: Vec<TypeEnvData>,
}

pub type Constraints = Vec<Constraint>;

lazy_static! {
    pub static ref INT_TYPE: Type = Type::Con("Int".to_string(), vec![]);
    pub static ref STRING_TYPE: Type = Type::Con("String".to_string(), vec![]);
    pub static ref BOOL_TYPE: Type = Type::Con("Bool".to_string(), vec![]);
    pub static ref UNIT_TYPE: Type = Type::Con("Unit".to_string(), vec![]);
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

    pub fn fresh_type_var(&mut self) -> TVar {
        self.solver.unifier.fresh_type_var()
    }

    pub fn instantiate(&mut self, scm: &Scheme) -> Type {
        let mut sub = TVSub::new();
        for tv in &scm.unbound {
            sub.insert(*tv, self.fresh_type_var().into());
        }
        scm.ty.clone().substitute(&sub)
    }

    pub fn global_env(&self) -> TypeEnvId {
        0
    }

    pub fn generalize(&self, ty: Type, env: TypeEnvId) -> Scheme {
        let mut unbound = TVSet::new();
        for tv in ty.uni_vars() {
            if !self.env_has_uni_var(env, &tv) {
                unbound.insert(tv);
            }
        }
        Scheme::new(unbound, ty)
    }

    pub fn env_has_uni_var(&self, env: TypeEnvId, tv: &TVar) -> bool {
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

    pub fn env_add<S: Into<String>>(&mut self, env: TypeEnvId, name: S, kind: SymbolKind, scm: Scheme) {
        self.envs[env].add(name, kind, scm);
    }

    pub fn lookup(&mut self, name: &str, span: Span, kind: SymbolKind, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
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

    pub fn fork_env(&mut self, _env: TypeEnvId) -> ForkedEnv {
        let i = self.envs.len();
        self.envs.push(TypeEnvData::new());
        ForkedEnv::new(i)
    }

    pub fn drop_env(&mut self, mut env: ForkedEnv) {
        env.bomb.defuse();
        debug_assert!(self.envs.len()-1 == env.id);
        self.envs.pop();
    }

    pub fn infer_literal(&mut self, token: SyntaxToken) -> Type {
        let ty = match token.kind() {
            BIN_INT | OCT_INT | DEC_INT | HEX_INT => INT_TYPE.clone(),
            STRING => STRING_TYPE.clone(),
            _ => unreachable!(),
        };
        ty
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
