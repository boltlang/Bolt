use std::{collections::{HashMap, HashSet}, vec};

use ena::unify::{EqUnifyValue, InPlaceUnificationTable, UnifyKey};
use lazy_static::lazy_static;

use crate::{Diagnostic, SyntaxKind::*, SyntaxNode, SyntaxToken, ast::*};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SymbolKind {
    Var,
    Type,
}

/// A unique name for the type constructor.
type ConId = String;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct TVar(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Type {

    /// A temporary hole used during type inference.
    UniVar(TVar),

    /// The type of type constructors.
    Con(ConId, Vec<Type>),

    /// The type of a function accepting one argument.
    ///
    /// Could be a [Con], but we define it here for performance and ease of use.
    Fun(Box<Type>, Box<Type>),
}

impl Type {

    fn uni_vars_helper(&self, out: &mut Vec<TVar>) {
        match self {
            Self::UniVar(tv) => {
                out.push(*tv);
            },
            Self::Con(_, args) => {
                for arg in args {
                    arg.uni_vars_helper(out);
                }
            },
            Self::Fun(left, right) => {
                left.uni_vars_helper(out);
                right.uni_vars_helper(out);
            },
        }
    }

    pub fn uni_vars(&self) -> Vec<TVar> {
        let mut out = Vec::new();
        self.uni_vars_helper(&mut out);
        out
    }

    pub fn has_uni_var(&self, tv: &TVar) -> bool {
        match self {
            Self::UniVar(tv_2) => tv == tv_2,
            Self::Con(_, args) => args.iter().any(|t| t.has_uni_var(tv)),
            Self::Fun(left, right) => left.has_uni_var(tv) || right.has_uni_var(tv),
        }
    }

    pub fn substitute(self, sub: &TVSub) -> Type {
        match self {
            Type::UniVar(tv) => sub.get(&tv).cloned().unwrap_or(self),
            Type::Con(id, args) => Type::Con(
                id,
                args.into_iter()
                    .map(|t| t.substitute(sub))
                    .collect()
            ),
            Type::Fun(left, right) => Type::Fun(
                Box::new(left.substitute(sub)),
                Box::new(right.substitute(sub))
            ),
        }
    }

}

impl From<TVar> for Type {
    fn from(tv: TVar) -> Self {
        Type::UniVar(tv)
    }
}

type TVSet = HashSet<TVar>;

struct Scheme {
    unbound: TVSet,
    ty: Type,
}

impl Scheme {

    pub fn new(unbound: TVSet, ty: Type) -> Self {
        Self { unbound, ty }
    }

    pub fn has_uni_var(&self, tv: &TVar) -> bool {
        !self.unbound.contains(tv) && self.ty.has_uni_var(tv)
    }

}

pub enum Constraint {
    TypesEqual {
        left: Type,
        right: Type,
    },
}

struct TypeEnv {
    mapping: HashMap<(SymbolKind, String), Scheme>,
}

impl TypeEnv {

    fn get(&self, name: &str, kind: SymbolKind) -> Option<&Scheme> {
        self.mapping.get(&(kind, name.to_owned()))
    }

    fn add(&mut self, name: String, kind: SymbolKind, scheme: Scheme) {
        debug_assert!(self.mapping.contains_key(&(kind, name.clone())));
        self.mapping.insert((kind, name), scheme);
    }

    fn has_uni_var(&self, tv: &TVar) -> bool {
        for scm in self.mapping.values() {
            if scm.has_uni_var(tv) {
                return true;
            }
        }
        false
    }

}

type Constraints = Vec<Constraint>;

lazy_static! {

    static ref INT_TYPE: Type = Type::Con("Int".to_string(), vec![]);
    static ref STRING_TYPE: Type = Type::Con("String".to_string(), vec![]);
    static ref BOOL_TYPE: Type = Type::Con("Bool".to_string(), vec![]);

}

type TVSub = HashMap<TVar, Type>;

impl EqUnifyValue for Type {}

impl UnifyKey for TVar {

    type Value = Option<Type>;

    fn index(&self) -> u32 {
        self.0
    }

    fn from_index(u: u32) -> Self {
        TVar(u)
    }

    fn tag() -> &'static str {
        "TypeVar"
    }

}

struct InferContext {
    table: InPlaceUnificationTable<TVar>,
}

impl InferContext {

    fn new() -> Self {
        Self { table: InPlaceUnificationTable::new() }
    }

    fn fresh(&mut self) -> TVar {
        self.table.new_key(None)
    }

    fn instantiate(&mut self, scm: &Scheme) -> Type {
        let mut sub = TVSub::new();
        for tv in &scm.unbound {
            sub.insert(*tv, self.fresh().into());
        }
        scm.ty.clone().substitute(&sub)
    }

    fn generalize(&self, ty: Type, env: &TypeEnv) -> Scheme {
        let mut unbound = TVSet::new();
        for tv in ty.uni_vars() {
            if !env.has_uni_var(&tv) {
                unbound.insert(tv);
            }
        }
        Scheme::new(unbound, ty)
    }

    fn lookup(&mut self, name: &str, kind: SymbolKind, env: &TypeEnv) -> (Type, Constraints) {
        match env.get(name, kind) {
            None => {
                // TODO
                // Diagnostic::binding_not_found(name.to_owned(), );
                (self.fresh().into(), vec![])
            },
            Some(scm) => (self.instantiate(scm), vec![]),
        }
    }

    fn infer_literal(&mut self, token: SyntaxToken) -> (Type, Constraints) {
        match token.kind() {
            BIN_INT | OCT_INT | DEC_INT | HEX_INT => (INT_TYPE.clone(), vec![]),
            STRING => (STRING_TYPE.clone(), vec![]),
            _ => unreachable!(),
        }
    }

    fn infer_expr(&mut self, expr: &Expr, env: &mut TypeEnv) -> (Type, Constraints) {
        match expr {
            Expr::Named(named) => match named.name() {
                Some(name) => self.lookup(name.text(),  SymbolKind::Var, env),
                None => (self.fresh().into(), vec![]),
            },
            Expr::Lit(lit) => match lit.value() {
                Some(lit) => self.infer_literal(lit),
                None => (self.fresh().into(), vec![]),
            },
        }
    }

    fn check_expr(&mut self, expr: &Expr, ty: &Type, env: &mut TypeEnv) -> Constraints {
        todo!()
    }

    fn infer_source_file(&mut self, sf: &SourceFile) -> Constraints {
        todo!()
    }

    fn solve(&mut self, constraints: &[Constraint]) -> Self {
        todo!()
    }

}

struct CheckResult {
    mapping: HashMap<SyntaxNode, Scheme>,
}

pub fn check_source_file(node: &SourceFile) -> CheckResult {
    let mapping = HashMap::new();
    let mut infer = InferContext::new();
    let mut constraints = Constraints::new();
    constraints.extend(infer.infer_source_file(node));
    infer.solve(&constraints);
    CheckResult {
        mapping,
    }
}
