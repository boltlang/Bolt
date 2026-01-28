
use std::marker::PhantomData;

use crate::syntax::{SyntaxKind::{self, *}, SyntaxNode, SyntaxNodeChildren, SyntaxToken};

pub trait Node : Sized {

    fn kind() -> SyntaxKind {
        panic!("dynamic Node does not have a single SyntaxKind");
    }

    fn wrap(syntax: SyntaxNode) -> Self;

    fn syntax(&self) -> &SyntaxNode;

    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if Self::can_cast(syntax.kind()) {
            Some(Self::wrap(syntax))
        } else {
            None
        }
    }

    fn can_cast(kind: SyntaxKind) -> bool {
        Self::kind() == kind
    }

    fn token(&self, kind: SyntaxKind) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|x| x.kind() == kind)
    }

    fn node<N: Node>(&self) -> Option<N> {
        self.syntax()
            .children()
            .find_map(N::cast)
    }

}

/// An iterator over `SyntaxNode` children of a particular AST type.
#[derive(Debug, Clone)]
pub struct ChildrenIter<N> {
    inner: SyntaxNodeChildren,
    ph: PhantomData<N>,
}

impl<N> ChildrenIter<N> {
    fn new(parent: &SyntaxNode) -> Self {
        ChildrenIter { inner: parent.children(), ph: PhantomData }
    }
}

impl<N: Node> Iterator for ChildrenIter<N> {
    type Item = N;
    fn next(&mut self) -> Option<N> {
        self.inner.find_map(N::cast)
    }
}

pub enum Pattern {
    Named(NamedPattern),
}

impl Node for Pattern {

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind,  NAMED_PATT)
    }

    fn wrap(syntax: SyntaxNode) -> Self {
        match syntax.kind() {
            NAMED_PATT => Self::Named(NamedPattern::wrap(syntax)),
            _ => unreachable!(),
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Named(node) => node.syntax(),
        }
    }

}

pub struct NamedPattern(SyntaxNode);

impl Node for NamedPattern {
    fn kind() -> SyntaxKind {
        NAMED_PATT
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        NamedPattern(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub enum Expr {
    Named(NamedExpr),
    Lit(LitExpr),
}

impl Node for Expr {

    fn wrap(syntax: SyntaxNode) -> Self {
        match syntax.kind() {
            REF_EXPR => Expr::Named(NamedExpr::wrap(syntax)),
            LIT_EXPR => Expr::Lit(LitExpr::wrap(syntax)),
            _ => unreachable!(),
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Expr::Lit(node) => node.syntax(),
            Expr::Named(node) => node.syntax(),
        }
    }

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind, REF_EXPR | LIT_EXPR)
    }

}

pub struct LitExpr(SyntaxNode);

impl Node for LitExpr {
    fn kind() -> SyntaxKind {
        LIT_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        LitExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct NamedExpr(SyntaxNode);

impl Node for NamedExpr {
    fn kind() -> SyntaxKind {
        REF_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        NamedExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct VarDecl(SyntaxNode);

impl Node for VarDecl {
    fn kind() -> SyntaxKind {
        VAR_DECL
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        VarDecl(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl VarDecl {

    pub fn pattern(&self) -> Option<Pattern> {
        self.node()
    }

}

/// The `: Int` in e.g. `let foo : Int`
pub struct TypeSignature(SyntaxNode);

impl Node for TypeSignature {
    fn kind() -> SyntaxKind {
        TYPE_SIGNATURE
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        TypeSignature(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct FuncDecl(SyntaxNode);

impl Node for FuncDecl {
    fn kind() -> SyntaxKind {
        FUNC_DECL
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        FuncDecl(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl FuncDecl {

    pub fn name(&self) -> Option<SyntaxToken> {
        self.token(IDENTIFIER)
    }

    pub fn params(&self) -> ChildrenIter<Pattern> {
        ChildrenIter::new(&self.0)
    }

    pub fn return_type(&self ) -> Option<TypeSignature> {
        self.node()
    }

    pub fn body(&self) -> Option<Expr> {
        self.node()
    }

}
