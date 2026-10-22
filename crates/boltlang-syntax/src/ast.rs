
use std::{marker::PhantomData};

use rowan::SyntaxElement;

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

/// Defines an enum over several node types, like [`Expr`], with one variant
/// per node kind.
macro_rules! ast_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident($node:ident) = $kind:ident),* $(,)? }) => {
        $(#[$meta])*
        pub enum $name {
            $($variant($node)),*
        }

        impl Node for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, $($kind)|*)
            }
            fn wrap(syntax: SyntaxNode) -> Self {
                match syntax.kind() {
                    $($kind => $name::$variant($node(syntax)),)*
                    _ => unreachable!(),
                }
            }
            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $($name::$variant(node) => node.syntax()),*
                }
            }
        }
    };
}

ast_enum!(
    /// A type, as in the grammar's `_type`.
    Type {
        Path(PathType) = PATH_TYPE,
        Tuple(TupleType) = TUPLE_TYPE,
        Array(ArrayType) = ARRAY_TYPE,
        Never(NeverType) = NEVER_TYPE,
        Bounded(BoundedType) = BOUNDED_TYPE,
        Fn(FnType) = FN_TYPE,
        ImplTrait(ImplTraitType) = IMPL_TRAIT_TYPE,
    }
);

/// `Vec[T]`, `Shape.Circle`
pub struct PathType(SyntaxNode);

simple_node!(PathType, PATH_TYPE);

impl PathType {
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
}

/// `(A, B)`, or `()` for the unit type.
pub struct TupleType(SyntaxNode);

simple_node!(TupleType, TUPLE_TYPE);

impl TupleType {
    pub fn fields(&self) -> ChildrenIter<Type> {
        ChildrenIter::new(&self.0)
    }
    pub fn is_unit(&self) -> bool {
        self.fields().next().is_none()
    }
}

/// `[T]` or `[T; N]`
pub struct ArrayType(SyntaxNode);

simple_node!(ArrayType, ARRAY_TYPE);

impl ArrayType {
    pub fn element(&self) -> Option<Type> {
        self.find_node()
    }
    pub fn length(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `!`
pub struct NeverType(SyntaxNode);

simple_node!(NeverType, NEVER_TYPE);

/// `A + B`
pub struct BoundedType(SyntaxNode);

simple_node!(BoundedType, BOUNDED_TYPE);

impl BoundedType {
    pub fn lhs(&self) -> Option<Type> {
        self.find_node()
    }
    pub fn rhs(&self) -> Option<Type> {
        self.0.children().filter_map(Type::cast).nth(1)
    }
}

/// `fn(A) -> B`, `extern "C" fn()`, or `FnOnce(A) -> B` with a trait.
pub struct FnType(SyntaxNode);

simple_node!(FnType, FN_TYPE);

impl FnType {
    /// The trait, in `FnOnce(A) -> B`.
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
    pub fn abi(&self) -> Option<Abi> {
        self.find_node()
    }
    pub fn param_list(&self) -> Option<ParamList> {
        self.find_node()
    }
    pub fn ret_type(&self) -> Option<RetType> {
        self.find_node()
    }
}

/// `impl Trait` or `impl for[T] Trait`
pub struct ImplTraitType(SyntaxNode);

simple_node!(ImplTraitType, IMPL_TRAIT_TYPE);

impl ImplTraitType {
    /// The parameters after `for`, if there are any.
    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
}

/// `[T, U: Bound = Default, const N: usize]`
pub struct GenericParamList(SyntaxNode);

simple_node!(GenericParamList, GENERIC_PARAM_LIST);

impl GenericParamList {
    pub fn params(&self) -> ChildrenIter<GenericParam> {
        ChildrenIter::new(&self.0)
    }
}

ast_enum!(GenericParam {
    Type(TypeParam) = TYPE_PARAM,
    Const(ConstParam) = CONST_PARAM,
});

/// `T`, `T: Bound` or `T = Default`. Can also be a metavariable in a macro.
pub struct TypeParam(SyntaxNode);

simple_node!(TypeParam, TYPE_PARAM);

impl TypeParam {
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn bounds(&self) -> Option<TraitBounds> {
        self.find_node()
    }
    pub fn default_type(&self) -> Option<Type> {
        self.find_node()
    }
}

/// `const N: usize = 3`
pub struct ConstParam(SyntaxNode);

simple_node!(ConstParam, CONST_PARAM);

impl ConstParam {
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
    pub fn default_value(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `-> T`
pub struct RetType(SyntaxNode);

simple_node!(RetType, RET_TYPE);

impl RetType {
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
}

/// `extern` or `extern "C"`
pub struct Abi(SyntaxNode);

simple_node!(Abi, ABI);

impl Abi {
    pub fn string(&self) -> Option<SyntaxToken> {
        self.find_token(STRING)
    }
}

/// `[A, B]` after a type
pub struct GenericArgList(SyntaxNode);

simple_node!(GenericArgList, GENERIC_ARG_LIST);

impl GenericArgList {
    pub fn args(&self) -> ChildrenIter<GenericArg> {
        ChildrenIter::new(&self.0)
    }
}

ast_enum!(GenericArg {
    Type(TypeArg) = TYPE_ARG,
    AssocType(AssocTypeArg) = ASSOC_TYPE_ARG,
    Const(ConstArg) = CONST_ARG,
});

pub struct TypeArg(SyntaxNode);

simple_node!(TypeArg, TYPE_ARG);

impl TypeArg {
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
    pub fn bounds(&self) -> Option<TraitBounds> {
        self.find_node()
    }
}

/// `Item = T`
pub struct AssocTypeArg(SyntaxNode);

simple_node!(AssocTypeArg, ASSOC_TYPE_ARG);

impl AssocTypeArg {
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
    pub fn generic_arg_list(&self) -> Option<GenericArgList> {
        self.find_node()
    }
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
}

/// A literal or block as a generic argument.
pub struct ConstArg(SyntaxNode);

simple_node!(ConstArg, CONST_ARG);

impl ConstArg {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `: A + B`
pub struct TraitBounds(SyntaxNode);

simple_node!(TraitBounds, TRAIT_BOUNDS);

impl TraitBounds {
    pub fn bounds(&self) -> ChildrenIter<Type> {
        ChildrenIter::new(&self.0)
    }
}

ast_enum!(
    /// A pattern, as in the grammar's `_pattern`.
    Pattern {
        Named(NamedPattern) = NAMED_PATT,
        Lit(LitPattern) = LIT_PATT,
        Wildcard(WildcardPattern) = WILDCARD_PATT,
        Path(PathPattern) = PATH_PATT,
        Tuple(TuplePattern) = TUPLE_PATT,
        TupleStruct(TupleStructPattern) = TUPLE_STRUCT_PATT,
        Struct(StructPattern) = STRUCT_PATT,
        Rest(RestPattern) = REST_PATT,
        Slice(SlicePattern) = SLICE_PATT,
        Captured(CapturedPattern) = CAPTURED_PATT,
        Mut(MutPattern) = MUT_PATT,
        Range(RangePattern) = RANGE_PATT,
        Or(OrPattern) = OR_PATT,
        ConstBlock(ConstBlockPattern) = CONST_BLOCK_PATT,
    }
);

/// A binding like `x`
pub struct NamedPattern(SyntaxNode);

simple_node!(NamedPattern, NAMED_PATT);

impl NamedPattern {
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
}

/// `1`, `-1` or `"s"`
pub struct LitPattern(SyntaxNode);

simple_node!(LitPattern, LIT_PATT);

impl LitPattern {
    pub fn minus_token(&self) -> Option<SyntaxToken> {
        self.find_token(MINUS)
    }
    /// The literal token, without the `-`.
    pub fn literal(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|token| !token.kind().is_trivia() && token.kind() != MINUS)
    }
}

/// `_`
pub struct WildcardPattern(SyntaxNode);

simple_node!(WildcardPattern, WILDCARD_PATT);

/// A constant or variant like `Color.Red`
pub struct PathPattern(SyntaxNode);

simple_node!(PathPattern, PATH_PATT);

impl PathPattern {
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
}

/// `(a, b)`
pub struct TuplePattern(SyntaxNode);

simple_node!(TuplePattern, TUPLE_PATT);

impl TuplePattern {
    pub fn fields(&self) -> ChildrenIter<Pattern> {
        ChildrenIter::new(&self.0)
    }
}

/// `Some(x)`
pub struct TupleStructPattern(SyntaxNode);

simple_node!(TupleStructPattern, TUPLE_STRUCT_PATT);

impl TupleStructPattern {
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
    pub fn fields(&self) -> ChildrenIter<Pattern> {
        ChildrenIter::new(&self.0)
    }
}

/// `Point { x, y: 0, .. }`
pub struct StructPattern(SyntaxNode);

simple_node!(StructPattern, STRUCT_PATT);

impl StructPattern {
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
    pub fn fields(&self) -> ChildrenIter<FieldPattern> {
        ChildrenIter::new(&self.0)
    }
    /// The `..` at the end, if there is one.
    pub fn rest(&self) -> Option<RestPattern> {
        self.find_node()
    }
}

/// `x`, `mut x` or `x: pattern` in a struct pattern
pub struct FieldPattern(SyntaxNode);

simple_node!(FieldPattern, FIELD_PATT);

impl FieldPattern {
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
    /// The field name in `x: pattern`.
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
    /// The name bound by the shorthand form `x`.
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
}

/// `..` in a tuple, slice or struct pattern
pub struct RestPattern(SyntaxNode);

simple_node!(RestPattern, REST_PATT);

/// `[a, .., b]`
pub struct SlicePattern(SyntaxNode);

simple_node!(SlicePattern, SLICE_PATT);

impl SlicePattern {
    pub fn elements(&self) -> ChildrenIter<Pattern> {
        ChildrenIter::new(&self.0)
    }
}

/// `x @ pattern`
pub struct CapturedPattern(SyntaxNode);

simple_node!(CapturedPattern, CAPTURED_PATT);

impl CapturedPattern {
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
}

/// `mut x`
pub struct MutPattern(SyntaxNode);

simple_node!(MutPattern, MUT_PATT);

impl MutPattern {
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
}

/// `1..=5`, `a..` or `..=b`
pub struct RangePattern(SyntaxNode);

simple_node!(RangePattern, RANGE_PATT);

impl RangePattern {
    /// The operator: `..` or `..=`.
    pub fn op_token(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|token| matches!(token.kind(), DOTDOT | DOTDOTEQUALS))
    }
    pub fn start(&self) -> Option<Pattern> {
        let op = self.op_token()?;
        self.0.children().filter(|n| n.text_range().end() <= op.text_range().start()).find_map(Pattern::cast)
    }
    pub fn end(&self) -> Option<Pattern> {
        let op = self.op_token()?;
        self.0.children().filter(|n| n.text_range().start() >= op.text_range().end()).find_map(Pattern::cast)
    }
}

/// `a | b`
pub struct OrPattern(SyntaxNode);

simple_node!(OrPattern, OR_PATT);

impl OrPattern {
    pub fn patterns(&self) -> ChildrenIter<Pattern> {
        ChildrenIter::new(&self.0)
    }
}

/// `const { ... }`
pub struct ConstBlockPattern(SyntaxNode);

simple_node!(ConstBlockPattern, CONST_BLOCK_PATT);

impl ConstBlockPattern {
    pub fn block(&self) -> Option<BlockExpr> {
        self.find_node()
    }
}

pub enum Expr {
    Path(PathExpr),
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
            PATH_EXPR => Expr::Path(PathExpr(syntax)),
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
            Expr::Path(node) => node.syntax(),
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


/// A reference to a variable or function, like `x` or `self`.
pub struct PathExpr(SyntaxNode);

simple_node!(PathExpr, PATH_EXPR);

impl PathExpr {
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
}

/// `a.b.c`: a qualifier (`a.b`) and a segment (`c`).
pub struct Path(SyntaxNode);

simple_node!(Path, PATH);

impl Path {
    /// The path before the last `.`, if there is one.
    pub fn qualifier(&self) -> Option<Path> {
        self.find_node()
    }
    /// The last segment.
    pub fn segment(&self) -> Option<PathSegment> {
        self.find_node()
    }
    /// All segments, from first to last.
    pub fn segments(&self) -> Vec<PathSegment> {
        let mut segments = self.qualifier().map(|q| q.segments()).unwrap_or_default();
        segments.extend(self.segment());
        segments
    }
}

pub struct PathSegment(SyntaxNode);

simple_node!(PathSegment, PATH_SEGMENT);

impl PathSegment {
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
    pub fn generic_arg_list(&self) -> Option<GenericArgList> {
        self.find_node()
    }
}

/// A reference to a name: an identifier, `self` or a metavariable.
pub struct NameRef(SyntaxNode);

simple_node!(NameRef, NAME_REF);

impl NameRef {
    pub fn token(&self) -> Option<SyntaxToken> {
        first_non_trivia_token(&self.0)
    }
    pub fn text(&self) -> String {
        self.token().map(|t| t.text().to_string()).unwrap_or_default()
    }
}

/// The name of something being declared, like the `f` in `fn f()`.
pub struct Name(SyntaxNode);

simple_node!(Name, NAME);

impl Name {
    pub fn ident(&self) -> Option<SyntaxToken> {
        self.find_token(IDENT)
    }
    pub fn text(&self) -> String {
        self.ident().map(|t| t.text().to_string()).unwrap_or_default()
    }
}

/// `a + b`
pub struct BinExpr(SyntaxNode);

simple_node!(BinExpr, BIN_EXPR);

/// The first token directly inside `node` that isn't whitespace or a comment.
fn first_non_trivia_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|x| x.into_token())
        .find(|token| !token.kind().is_trivia())
}

impl BinExpr {
    pub fn lhs(&self) -> Option<Expr> {
        self.find_node()
    }
    /// The operator, e.g. `+` or a user-defined `<$>`.
    pub fn operator(&self) -> Option<SyntaxToken> {
        first_non_trivia_token(&self.0)
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
        first_non_trivia_token(&self.0)
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
    pub fn type_expr(&self) -> Option<Type> {
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
    pub fn param_list(&self) -> Option<ParamList> {
        self.find_node()
    }
    pub fn params(&self) -> Vec<Param> {
        self.param_list().map(|list| list.params().collect()).unwrap_or_default()
    }
    pub fn ret_type(&self) -> Option<RetType> {
        self.find_node()
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
                INT_NUMBER | FLOAT_NUMBER | STRING | BYTE_STRING | C_STRING | CHAR | BYTE
                    | TRUE_KEYWORD | FALSE_KEYWORD
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


impl CallExpr {
    pub fn operator(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn args(&self) -> impl Iterator<Item = Expr> {
        self.0
            .children()
            .find(|node| node.kind() == ARG_LIST)
            .into_iter()
            .flat_map(|list| list.children().filter_map(Expr::cast))
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

    pub fn type_expr(&self) -> Option<Type> {
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

/// `(a, b: T = 1)`
pub struct ParamList(SyntaxNode);

simple_node!(ParamList, PARAM_LIST);

impl ParamList {
    /// The ordinary parameters, leaving out `self` and variadic ones.
    pub fn params(&self) -> ChildrenIter<Param> {
        ChildrenIter::new(&self.0)
    }
    pub fn self_param(&self) -> Option<SelfParam> {
        self.find_node()
    }
    pub fn variadic_param(&self) -> Option<VariadicParam> {
        self.find_node()
    }
}

/// `self` or `mut self`
pub struct SelfParam(SyntaxNode);

simple_node!(SelfParam, SELF_PARAM);

impl SelfParam {
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
}

/// `...` or `args: ...`
pub struct VariadicParam(SyntaxNode);

simple_node!(VariadicParam, VARIADIC_PARAM);

impl VariadicParam {
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
}

/// A parameter like `x`, `x: T` or `x: T = 1`.
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

    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }

    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }

    pub fn type_expr(&self) -> Option<Type> {
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

    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }

    pub fn param_list(&self) -> Option<ParamList> {
        self.find_node()
    }

    pub fn params(&self) -> Vec<Param> {
        self.param_list().map(|list| list.params().collect()).unwrap_or_default()
    }

    pub fn ret_type(&self) -> Option<RetType> {
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
