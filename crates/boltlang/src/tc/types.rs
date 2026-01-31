use std::{collections::HashMap, env::remove_var, fmt::Display};

use ena::unify::{EqUnifyValue, UnifyKey};

/// A unique name for the type constructor.
pub type ConId = String;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TVar(u32);

impl Display for TVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a{}", self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {

    /// A temporary hole used during type inference.
    UniVar(TVar),

    /// A type constructor with some arguments.
    Con(ConId, Vec<Type>),

    /// The type of a function accepting one argument.
    ///
    /// Could be a [Con], but we define it here for performance and ease of use.
    Fun(Box<Type>, Box<Type>),
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UniVar(var) => write!(f, "{}", var),
            Self::Fun(left, right) => write!(f, "{} -> {}", left, right),
            Self::Con(id, args) => {
                write!(f, "{}", id)?;
                for arg in args {
                    write!(f, " {}", arg)?;
                }
                Ok(())
            }
        }
    }
}

pub type TVSub = HashMap<TVar, Type>;

impl Type {

    pub fn fun(left: Type, right: Type) -> Type {
        Type::Fun(Box::new(left), Box::new(right))
    }

    pub fn signature(params: impl IntoIterator<Item = Type, IntoIter: DoubleEndedIterator>, ret_ty: Type) -> Type {
        let mut out = ret_ty;
        for param in params.into_iter().rev() {
            out = Type::fun(param, out);
        }
        out
    }

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
