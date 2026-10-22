
use std::{marker::PhantomData};

use rowan::SyntaxElement;

use boltlang_common::IterExt;
use crate::{SyntaxKind::{self, *}, SyntaxNode, SyntaxNodeChildren, SyntaxToken};

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

    fn find_token(&self, kind: SyntaxKind) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|x| x.kind() == kind)
    }

    fn find_node<N: Node>(&self) -> Option<N> {
        self.syntax()
            .children()
            .find_map(N::cast)
    }

    fn rfind_node<N: Node>(&self) -> Option<N> {
        // FIXME this could be optimised in Rowan
        self.syntax()
            .children()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
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

pub enum TypeExpr {
    Named(NamedTypeExpr),
    Arrow(ArrowTypeExpr),
}

impl Node for TypeExpr {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind, NAMED_TYPE_EXPR | ARROW_TYPE_EXPR)
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        match syntax.kind() {
            NAMED_TYPE_EXPR => TypeExpr::Named(NamedTypeExpr(syntax)),
            ARROW_TYPE_EXPR => TypeExpr::Arrow(ArrowTypeExpr(syntax)),
            _ => unreachable!(),
        }
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            TypeExpr::Named(node) => node.syntax(),
            TypeExpr::Arrow(node) => node.syntax(),
        }
    }
}

pub struct ArrowTypeExpr(SyntaxNode);

impl ArrowTypeExpr {

    pub fn params(&self) -> impl Iterator<Item = TypeExpr> {
        ChildrenIter::new(&self.0).skip_last(1)
    }

    pub fn return_ty(&self) -> Option<TypeExpr> {
        self.rfind_node()
    }

}

impl Node for ArrowTypeExpr {
    fn kind() -> SyntaxKind {
        ARROW_TYPE_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        ArrowTypeExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct NamedTypeExpr(SyntaxNode);

impl NamedTypeExpr {

    pub fn name(&self) -> Option<SyntaxToken> {
        self.find_token(IDENT)
    }

}

impl Node for NamedTypeExpr {
    fn kind() -> SyntaxKind {
        NAMED_TYPE_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        NamedTypeExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub enum Pattern {
    Named(NamedPattern),
    Typed(TypedPattern),
}

impl Node for Pattern {

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind,  NAMED_PATT | TYPED_PATT)
    }

    fn wrap(syntax: SyntaxNode) -> Self {
        match syntax.kind() {
            NAMED_PATT => Self::Named(NamedPattern(syntax)),
            TYPED_PATT => Self::Typed(TypedPattern(syntax)),
            _ => unreachable!(),
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Named(node) => node.syntax(),
            Self::Typed(node) => node.syntax(),
        }
    }

}

pub struct TypedPattern(SyntaxNode);

impl TypedPattern {

    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }

    pub fn type_expression(&self) -> Option<TypeExpr> {
        self.find_node()
    }

}

impl Node for TypedPattern {
    fn kind() -> SyntaxKind {
        TYPED_PATT
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        TypedPattern(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct NamedPattern(SyntaxNode);

impl NamedPattern {

    pub fn mut_keyword(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }

    pub fn name(&self) -> Option<SyntaxToken> {
        self.find_token(IDENT)
    }

}


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
    Call(CallExpr),
    Fun(FunExpr),
    Block(BlockExpr),
    Bin(BinExpr),
    Prefix(PrefixExpr),
    Nest(NestExpr),
    Tuple(TupleExpr),
    Index(IndexExpr),
    Cast(CastExpr),
    Try(TryExpr),
    Ret(RetExpr),
}

impl Node for Expr {

    fn wrap(syntax: SyntaxNode) -> Self {
        match syntax.kind() {
            PATH_EXPR => Expr::Named(NamedExpr(syntax)),
            LIT_EXPR => Expr::Lit(LitExpr(syntax)),
            CALL_EXPR => Expr::Call(CallExpr(syntax)),
            FUN_EXPR => Expr::Fun(FunExpr(syntax)),
            BLOCK_EXPR => Expr::Block(BlockExpr(syntax)),
            BIN_EXPR => Expr::Bin(BinExpr(syntax)),
            PREFIX_EXPR => Expr::Prefix(PrefixExpr(syntax)),
            NEST_EXPR => Expr::Nest(NestExpr(syntax)),
            TUPLE_EXPR => Expr::Tuple(TupleExpr(syntax)),
            INDEX_EXPR => Expr::Index(IndexExpr(syntax)),
            CAST_EXPR => Expr::Cast(CastExpr(syntax)),
            TRY_EXPR => Expr::Try(TryExpr(syntax)),
            RET_EXPR => Expr::Ret(RetExpr(syntax)),
            _ => unreachable!(),
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Expr::Lit(node) => node.syntax(),
            Expr::Call(node) => node.syntax(),
            Expr::Named(node) => node.syntax(),
            Expr::Fun(node) => node.syntax(),
            Expr::Block(node) => node.syntax(),
            Expr::Bin(node) => node.syntax(),
            Expr::Prefix(node) => node.syntax(),
            Expr::Nest(node) => node.syntax(),
            Expr::Tuple(node) => node.syntax(),
            Expr::Index(node) => node.syntax(),
            Expr::Cast(node) => node.syntax(),
            Expr::Try(node) => node.syntax(),
            Expr::Ret(node) => node.syntax(),
        }
    }

    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            PATH_EXPR | LIT_EXPR | CALL_EXPR | FUN_EXPR | BLOCK_EXPR | BIN_EXPR | PREFIX_EXPR
                | NEST_EXPR | TUPLE_EXPR | INDEX_EXPR | CAST_EXPR | TRY_EXPR | RET_EXPR
        )
    }

}

/// Implements [`Node`] for a newtype around a single [`SyntaxKind`].
macro_rules! simple_node {
    ($name:ident, $kind:ident) => {
        impl Node for $name {
            fn kind() -> SyntaxKind {
                $kind
            }
            fn wrap(syntax: SyntaxNode) -> Self {
                $name(syntax)
            }
            fn syntax(&self) -> &SyntaxNode {
                &self.0
            }
        }
    };
}

/// `a + b`
pub struct BinExpr(SyntaxNode);

simple_node!(BinExpr, BIN_EXPR);

impl BinExpr {
    pub fn lhs(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn operator(&self) -> Option<SyntaxToken> {
        self.find_token(OPERATOR)
    }
    pub fn rhs(&self) -> Option<Expr> {
        self.0.children().filter_map(Expr::cast).nth(1)
    }
}

/// `-a`, `+a`, `!a`
pub struct PrefixExpr(SyntaxNode);

simple_node!(PrefixExpr, PREFIX_EXPR);

impl PrefixExpr {
    pub fn operator(&self) -> Option<SyntaxToken> {
        self.find_token(OPERATOR)
    }
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `(a)`
pub struct NestExpr(SyntaxNode);

simple_node!(NestExpr, NEST_EXPR);

impl NestExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `(a, b)`
pub struct TupleExpr(SyntaxNode);

simple_node!(TupleExpr, TUPLE_EXPR);

impl TupleExpr {
    pub fn elements(&self) -> ChildrenIter<Expr> {
        ChildrenIter::new(&self.0)
    }
}

/// `a[i]`
pub struct IndexExpr(SyntaxNode);

simple_node!(IndexExpr, INDEX_EXPR);

impl IndexExpr {
    pub fn base(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn index(&self) -> Option<Expr> {
        self.0.children().filter_map(Expr::cast).nth(1)
    }
}

/// `a as T`
pub struct CastExpr(SyntaxNode);

simple_node!(CastExpr, CAST_EXPR);

impl CastExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn type_expr(&self) -> Option<TypeExpr> {
        self.find_node()
    }
}

/// `a?`
pub struct TryExpr(SyntaxNode);

simple_node!(TryExpr, TRY_EXPR);

impl TryExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `return a`
pub struct RetExpr(SyntaxNode);

simple_node!(RetExpr, RET_EXPR);

impl RetExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

pub struct BlockExpr(SyntaxNode);

impl Node for BlockExpr {
    fn kind() -> SyntaxKind {
        BLOCK_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        BlockExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl BlockExpr {
    pub fn block(&self) -> Option<Block> {
        self.find_node()
    }
}

pub struct Block(SyntaxNode);

impl Node for Block {
    fn kind() -> SyntaxKind {
        SyntaxKind::BLOCK
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        Block(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl Block {

    pub fn elements(&self) -> ChildrenIter<SourceElement> {
        ChildrenIter::new(&self.0)
    }

}

pub struct FunExpr(SyntaxNode);

impl FunExpr {
    pub fn params(&self) -> ChildrenIter<Pattern> {
        ChildrenIter::new(&self.0)
    }
    pub fn body(&self) -> Option<Expr> {
        self.find_node()
    }
}

impl Node for FunExpr {
    fn kind() -> SyntaxKind {
        FUN_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        FunExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct LitExpr(SyntaxNode);

impl LitExpr {
    pub fn value(&self) -> Option<SyntaxToken> {
        self.0.children_with_tokens().find_map(|x| match x {
            SyntaxElement::Token(token) if matches!(
                token.kind(),
                BIN_INT | OCT_INT | DEC_INT | HEX_INT | STRING
            ) => Some(token),
            _ => None,
        })
    }
}

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

pub struct CallExpr(SyntaxNode);

impl Node for CallExpr {
    fn kind() -> SyntaxKind {
        CALL_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        CallExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

pub struct NamedExpr(SyntaxNode);

impl Node for NamedExpr {
    fn kind() -> SyntaxKind {
        PATH_EXPR
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        NamedExpr(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl CallExpr {
    pub fn operator(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn args(&self) -> impl Iterator<Item = Expr> {
        self.0.children().filter_map(Expr::cast).skip(1)
    }
}

impl NamedExpr {
    pub fn name(&self) -> Option<SyntaxToken> {
        self.find_token(IDENT)
    }
}

pub struct VarDecl(SyntaxNode);

impl Node for VarDecl {
    fn kind() -> SyntaxKind {
        LET_STMT
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        VarDecl(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl VarDecl {

    pub fn pub_keyword(&self) -> Option<SyntaxToken> {
        self.find_token(PUB_KEYWORD)
    }

    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }

    pub fn type_expr(&self) -> Option<TypeExpr> {
        self.find_node()
    }

    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
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

pub struct Param(SyntaxNode);

impl Node for Param {
    fn kind() -> SyntaxKind {
        PARAM
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        Param(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl Param {

    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }

    pub fn type_expr(&self) -> Option<TypeExpr> {
        self.find_node()
    }

    pub fn default(&self) -> Option<Expr> {
        self.find_node()
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

    pub fn pub_keyword(&self) -> Option<SyntaxToken> {
        self.find_token(PUB_KEYWORD)
    }

    pub fn name(&self) -> Option<SyntaxToken> {
        self.find_token(IDENT)
    }

    pub fn params(&self) -> ChildrenIter<Param> {
        ChildrenIter::new(&self.0)
    }

    pub fn type_signature(&self ) -> Option<TypeExpr> {
        self.find_node()
    }

    pub fn body(&self) -> Option<Expr> {
        self.find_node()
    }

}

/// All-rounder for elements in blocks, function bodies, modules and source files.
///
/// In some contexts, certain element types should not be processed.
pub enum SourceElement {
    VarDecl(VarDecl),
    FuncDecl(FuncDecl),
    Expr(Expr),
}

impl Node for SourceElement {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(kind, LET_STMT | FUNC_DECL) || Expr::can_cast(kind)
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        match syntax.kind() {
            LET_STMT => Self::VarDecl(VarDecl(syntax)),
            FUNC_DECL => Self::FuncDecl(FuncDecl(syntax)),
            _ => Self::Expr(Expr::wrap(syntax)),
        }
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            SourceElement::VarDecl(node) => node.syntax(),
            SourceElement::FuncDecl(node) => node.syntax(),
            SourceElement::Expr(node) => node.syntax(),
        }
    }
}

pub struct SourceFile(SyntaxNode);

impl Node for SourceFile {
    fn kind() -> SyntaxKind {
        SOURCE_FILE
    }
    fn wrap(syntax: SyntaxNode) -> Self {
        SourceFile(syntax)
    }
    fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}

impl SourceFile {
    pub fn elements(&self) -> ChildrenIter<SourceElement> {
        ChildrenIter::new(&self.0)
    }
}
