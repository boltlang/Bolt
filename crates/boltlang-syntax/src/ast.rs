
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
        Mut(MutType) = MUT_TYPE,
        Never(NeverType) = NEVER_TYPE,
        Bounded(BoundedType) = BOUNDED_TYPE,
        Fn(FnType) = FN_TYPE,
        ImplTrait(ImplTraitType) = IMPL_TRAIT_TYPE,
        Macro(MacroType) = MACRO_TYPE,
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

/// `mut T` or `imm T`, for a type other than an array, like `mut Point`, or
/// a named mutability, like `'m Point`.
pub struct MutType(SyntaxNode);

simple_node!(MutType, MUT_TYPE);

impl MutType {
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
    pub fn imm_token(&self) -> Option<SyntaxToken> {
        self.find_token(IMM_KEYWORD)
    }
    /// The `'m` of `'m Point`.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        self.find_token(LABEL_IDENT)
    }
    pub fn inner(&self) -> Option<Type> {
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

/// `[T]` or `[T; N]`, maybe after `mut` or `imm`
pub struct ArrayType(SyntaxNode);

simple_node!(ArrayType, ARRAY_TYPE);

impl ArrayType {
    /// The `mut` of `mut [T]`.
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
    /// The `imm` of `imm [T]`.
    pub fn imm_token(&self) -> Option<SyntaxToken> {
        self.find_token(IMM_KEYWORD)
    }
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

/// `fn(A) -> B` or `extern "C" fn()`
pub struct FnType(SyntaxNode);

simple_node!(FnType, FN_TYPE);

impl FnType {
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
    Mutability(MutabilityParam) = MUTABILITY_PARAM,
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

/// A named mutability like the `'v` of `struct VecIter[T, 'v]`, with
/// bounds like `'v: imm`.
pub struct MutabilityParam(SyntaxNode);

simple_node!(MutabilityParam, MUTABILITY_PARAM);

impl MutabilityParam {
    /// The `'v`.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        self.find_token(LABEL_IDENT)
    }
    pub fn bounds(&self) -> Option<TraitBounds> {
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
        Macro(MacroPattern) = MACRO_PATT,
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

ast_enum!(
    /// An expression, as in the grammar's `_expression`.
    Expr {
        Path(PathExpr) = PATH_EXPR,
        Lit(LitExpr) = LIT_EXPR,
        Call(CallExpr) = CALL_EXPR,
        Fun(FunExpr) = FUN_EXPR,
        Block(BlockExpr) = BLOCK_EXPR,
        Bin(BinExpr) = BIN_EXPR,
        Prefix(PrefixExpr) = PREFIX_EXPR,
        Nest(NestExpr) = NEST_EXPR,
        Tuple(TupleExpr) = TUPLE_EXPR,
        Index(IndexExpr) = INDEX_EXPR,
        Cast(CastExpr) = CAST_EXPR,
        Try(TryExpr) = TRY_EXPR,
        Ret(RetExpr) = RET_EXPR,
        Field(FieldExpr) = FIELD_EXPR,
        Await(AwaitExpr) = AWAIT_EXPR,
        If(IfExpr) = IF_EXPR,
        Let(LetExpr) = LET_EXPR,
        While(WhileExpr) = WHILE_EXPR,
        Loop(LoopExpr) = LOOP_EXPR,
        For(ForExpr) = FOR_EXPR,
        Break(BreakExpr) = BREAK_EXPR,
        Continue(ContinueExpr) = CONTINUE_EXPR,
        Yield(YieldExpr) = YIELD_EXPR,
        Match(MatchExpr) = MATCH_EXPR,
        Record(RecordExpr) = RECORD_EXPR,
        Array(ArrayExpr) = ARRAY_EXPR,
        Closure(ClosureExpr) = CLOSURE_EXPR,
        Range(RangeExpr) = RANGE_EXPR,
        Macro(MacroExpr) = MACRO_EXPR,
    }
);

/// `name!(tokens)`, `name![tokens]` or `name!{tokens}`
pub struct MacroCall(SyntaxNode);

simple_node!(MacroCall, MACRO_CALL);

impl MacroCall {
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
    pub fn token_tree(&self) -> Option<TokenTree> {
        self.find_node()
    }
}

/// A macro invocation used as an expression
pub struct MacroExpr(SyntaxNode);

simple_node!(MacroExpr, MACRO_EXPR);

impl MacroExpr {
    pub fn macro_call(&self) -> Option<MacroCall> {
        self.find_node()
    }
}

/// A macro invocation used as a pattern
pub struct MacroPattern(SyntaxNode);

simple_node!(MacroPattern, MACRO_PATT);

impl MacroPattern {
    pub fn macro_call(&self) -> Option<MacroCall> {
        self.find_node()
    }
}

/// A macro invocation used as a type
pub struct MacroType(SyntaxNode);

simple_node!(MacroType, MACRO_TYPE);

impl MacroType {
    pub fn macro_call(&self) -> Option<MacroCall> {
        self.find_node()
    }
}


/// A reference to a variable or function, like `x`.
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

/// A reference to a name: an identifier or a metavariable.
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

/// `x.field` or `x.0`
pub struct FieldExpr(SyntaxNode);

simple_node!(FieldExpr, FIELD_EXPR);

impl FieldExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
    /// The field name or tuple index.
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
}

/// `if cond { } else { }`
pub struct IfExpr(SyntaxNode);

simple_node!(IfExpr, IF_EXPR);

impl IfExpr {
    pub fn condition(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn then_branch(&self) -> Option<BlockExpr> {
        self.0.children().filter_map(Expr::cast).nth(1).and_then(|e| match e {
            Expr::Block(block) => Some(block),
            _ => None,
        })
    }
    /// The block or `if` after `else`.
    pub fn else_branch(&self) -> Option<Expr> {
        self.0.children().filter_map(Expr::cast).nth(2)
    }
}

/// `let pattern = value` in a condition
pub struct LetExpr(SyntaxNode);

simple_node!(LetExpr, LET_EXPR);

impl LetExpr {
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `while cond { }`
pub struct WhileExpr(SyntaxNode);

simple_node!(WhileExpr, WHILE_EXPR);

impl WhileExpr {
    pub fn label(&self) -> Option<Label> {
        self.find_node()
    }
    pub fn condition(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn body(&self) -> Option<BlockExpr> {
        self.0.children().filter_map(Expr::cast).nth(1).and_then(|e| match e {
            Expr::Block(block) => Some(block),
            _ => None,
        })
    }
}

/// The first child of `node` that is a block expression.
fn find_block(node: &SyntaxNode) -> Option<BlockExpr> {
    node.children().find_map(BlockExpr::cast)
}

/// `a..b`, `a..=b`, `a..`, `..b` or `..`
pub struct RangeExpr(SyntaxNode);

simple_node!(RangeExpr, RANGE_EXPR);

impl RangeExpr {
    /// The operator: `..` or `..=`.
    pub fn op_token(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|token| matches!(token.kind(), DOTDOT | DOTDOTEQUALS))
    }
    pub fn start(&self) -> Option<Expr> {
        let op = self.op_token()?;
        self.0.children().filter(|n| n.text_range().end() <= op.text_range().start()).find_map(Expr::cast)
    }
    pub fn end(&self) -> Option<Expr> {
        let op = self.op_token()?;
        self.0.children().filter(|n| n.text_range().start() >= op.text_range().end()).find_map(Expr::cast)
    }
}

/// `|a, b| a + b`
pub struct ClosureExpr(SyntaxNode);

simple_node!(ClosureExpr, CLOSURE_EXPR);

impl ClosureExpr {
    pub fn async_token(&self) -> Option<SyntaxToken> {
        self.find_token(ASYNC_KEYWORD)
    }
    pub fn param_list(&self) -> Option<ParamList> {
        self.find_node()
    }
    pub fn ret_type(&self) -> Option<RetType> {
        self.find_node()
    }
    /// The body, or `None` if it is `_`.
    pub fn body(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `[a, b, c]` or `[x; n]`, maybe after `mut`
pub struct ArrayExpr(SyntaxNode);

simple_node!(ArrayExpr, ARRAY_EXPR);

impl ArrayExpr {
    /// The `mut` of `mut [a, b]`.
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
    /// The elements of `[a, b, c]`, or the repeated value of `[x; n]`.
    pub fn elements(&self) -> ChildrenIter<Expr> {
        ChildrenIter::new(&self.0)
    }
    /// Whether this is `[x; n]`.
    pub fn is_repeat(&self) -> bool {
        self.find_token(SEMI).is_some()
    }
    /// The `n` of `[x; n]`.
    pub fn repeat_count(&self) -> Option<Expr> {
        if self.is_repeat() { self.0.children().filter_map(Expr::cast).nth(1) } else { None }
    }
}

/// A struct literal: `Point { x, y: 1, ..base }`
pub struct RecordExpr(SyntaxNode);

simple_node!(RecordExpr, RECORD_EXPR);

impl RecordExpr {
    /// The `mut` of `mut Point { x: 1 }`.
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
    pub fn field_list(&self) -> Option<RecordExprFieldList> {
        self.find_node()
    }
}

/// The `{ x, y: 1, ..base }` of a struct literal
pub struct RecordExprFieldList(SyntaxNode);

simple_node!(RecordExprFieldList, RECORD_EXPR_FIELD_LIST);

impl RecordExprFieldList {
    pub fn fields(&self) -> ChildrenIter<RecordExprField> {
        ChildrenIter::new(&self.0)
    }
    /// The `base` of `..base`.
    pub fn spread(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `x`, `x: value` or `0: value` in a struct literal
pub struct RecordExprField(SyntaxNode);

simple_node!(RecordExprField, RECORD_EXPR_FIELD);

impl RecordExprField {
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
    /// The value, or `None` for the shorthand `x`.
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `match value { pattern => value, ... }`
pub struct MatchExpr(SyntaxNode);

simple_node!(MatchExpr, MATCH_EXPR);

impl MatchExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn match_arm_list(&self) -> Option<MatchArmList> {
        self.find_node()
    }
    pub fn arms(&self) -> Vec<MatchArm> {
        self.match_arm_list().map(|list| list.arms().collect()).unwrap_or_default()
    }
}

/// The `{ ... }` of a `match`
pub struct MatchArmList(SyntaxNode);

simple_node!(MatchArmList, MATCH_ARM_LIST);

impl MatchArmList {
    pub fn arms(&self) -> ChildrenIter<MatchArm> {
        ChildrenIter::new(&self.0)
    }
}

/// `pattern if guard => value`
pub struct MatchArm(SyntaxNode);

simple_node!(MatchArm, MATCH_ARM);

impl MatchArm {
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
    pub fn guard(&self) -> Option<MatchGuard> {
        self.find_node()
    }
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `if condition` in a match arm
pub struct MatchGuard(SyntaxNode);

simple_node!(MatchGuard, MATCH_GUARD);

impl MatchGuard {
    pub fn condition(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `'outer`
pub struct Label(SyntaxNode);

simple_node!(Label, LABEL);

impl Label {
    pub fn ident(&self) -> Option<SyntaxToken> {
        self.find_token(LABEL_IDENT)
    }
}

/// `loop { }`
pub struct LoopExpr(SyntaxNode);

simple_node!(LoopExpr, LOOP_EXPR);

impl LoopExpr {
    pub fn label(&self) -> Option<Label> {
        self.find_node()
    }
    pub fn body(&self) -> Option<BlockExpr> {
        find_block(&self.0)
    }
}

/// `for pattern in iterable { }`
pub struct ForExpr(SyntaxNode);

simple_node!(ForExpr, FOR_EXPR);

impl ForExpr {
    pub fn label(&self) -> Option<Label> {
        self.find_node()
    }
    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }
    pub fn iterable(&self) -> Option<Expr> {
        self.find_node()
    }
    pub fn body(&self) -> Option<BlockExpr> {
        self.0.children().filter_map(Expr::cast).nth(1).and_then(|e| match e {
            Expr::Block(block) => Some(block),
            _ => None,
        })
    }
}

/// `break`, `break 'outer` or `break value`
pub struct BreakExpr(SyntaxNode);

simple_node!(BreakExpr, BREAK_EXPR);

impl BreakExpr {
    pub fn label(&self) -> Option<Label> {
        self.find_node()
    }
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `continue` or `continue 'outer`
pub struct ContinueExpr(SyntaxNode);

simple_node!(ContinueExpr, CONTINUE_EXPR);

impl ContinueExpr {
    pub fn label(&self) -> Option<Label> {
        self.find_node()
    }
}

/// `yield` or `yield value`
pub struct YieldExpr(SyntaxNode);

simple_node!(YieldExpr, YIELD_EXPR);

impl YieldExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `x.await`
pub struct AwaitExpr(SyntaxNode);

simple_node!(AwaitExpr, AWAIT_EXPR);

impl AwaitExpr {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
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
    /// The label of `'a: { }`.
    pub fn label(&self) -> Option<Label> {
        self.find_node()
    }
    /// The `async` of `async { }`.
    pub fn async_token(&self) -> Option<SyntaxToken> {
        self.find_token(ASYNC_KEYWORD)
    }
    /// The `const` of `const { }`.
    pub fn const_token(&self) -> Option<SyntaxToken> {
        self.find_token(CONST_KEYWORD)
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

    /// The statements, leaving out the tail expression.
    pub fn elements(&self) -> ChildrenIter<SourceElement> {
        ChildrenIter::new(&self.0)
    }

    /// The expression at the end without a `;`, which is the block's value.
    pub fn tail_expr(&self) -> Option<Expr> {
        self.find_node()
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
    /// The `mut` of `mut Wrapper(1)`, which makes a tuple struct that can be
    /// changed.
    pub fn mut_token(&self) -> Option<SyntaxToken> {
        self.find_token(MUT_KEYWORD)
    }
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

    pub fn pattern(&self) -> Option<Pattern> {
        self.find_node()
    }

    pub fn type_expr(&self) -> Option<Type> {
        self.find_node()
    }

    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }

    /// The `else` block of `let Some(x) = y else { ... };`
    pub fn else_block(&self) -> Option<BlockExpr> {
        let else_token = self.find_token(ELSE_KEYWORD)?;
        self.0
            .children()
            .filter(|n| n.text_range().start() >= else_token.text_range().end())
            .find_map(BlockExpr::cast)
    }

}


/// `(a, b: T = 1)`
pub struct ParamList(SyntaxNode);

simple_node!(ParamList, PARAM_LIST);

impl ParamList {
    /// The ordinary parameters, leaving out variadic ones.
    pub fn params(&self) -> ChildrenIter<Param> {
        ChildrenIter::new(&self.0)
    }
    pub fn variadic_param(&self) -> Option<VariadicParam> {
        self.find_node()
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

    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }

    pub fn async_token(&self) -> Option<SyntaxToken> {
        self.find_token(ASYNC_KEYWORD)
    }

    pub fn const_token(&self) -> Option<SyntaxToken> {
        self.find_token(CONST_KEYWORD)
    }

    pub fn abi(&self) -> Option<Abi> {
        self.find_node()
    }

    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }

    /// The `+` of an operator definition like `fn a + b`, which is in the
    /// parameter list, between the operands.
    pub fn operator_name(&self) -> Option<OperatorName> {
        self.param_list()?.find_node()
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

    pub fn where_clause(&self) -> Option<WhereClause> {
        self.find_node()
    }

    /// The block, or the expression after `=`. `None` for a signature like
    /// `fn f();`.
    pub fn body(&self) -> Option<Expr> {
        self.find_node()
    }

}

/// `pub`, `pub(super)` or `pub(package)`
pub struct Visibility(SyntaxNode);

simple_node!(Visibility, VISIBILITY);

impl Visibility {
    /// The `super` or `package` between parentheses.
    pub fn restriction(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|token| matches!(token.kind(), SUPER_KEYWORD | PACKAGE_KEYWORD))
    }
}

/// The `+` in `fn a + b`, or the `-` in `fn -x`
pub struct OperatorName(SyntaxNode);

simple_node!(OperatorName, OPERATOR_NAME);

impl OperatorName {
    pub fn operator(&self) -> Option<SyntaxToken> {
        self.0
            .children_with_tokens()
            .filter_map(|x| x.into_token())
            .find(|token| !token.kind().is_trivia())
    }
}

/// `where T: Ord, U: Hash`
pub struct WhereClause(SyntaxNode);

simple_node!(WhereClause, WHERE_CLAUSE);

impl WhereClause {
    pub fn predicates(&self) -> ChildrenIter<WherePred> {
        ChildrenIter::new(&self.0)
    }
}

/// `T: Ord`, or a class constraint like `Convert[A, B]`, in a where clause
pub struct WherePred(SyntaxNode);

simple_node!(WherePred, WHERE_PRED);

impl WherePred {
    /// The constrained type, or the whole class constraint if there are no
    /// bounds.
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
    /// `None` for a class constraint like `Convert[A, B]`.
    pub fn bounds(&self) -> Option<TraitBounds> {
        self.find_node()
    }
}

ast_enum!(
    /// A statement in a block or source file. The tail expression of a block
    /// is not a statement; see [`Block::tail_expr`].
    SourceElement {
        VarDecl(VarDecl) = LET_STMT,
        FuncDecl(FuncDecl) = FUNC_DECL,
        ExprStmt(ExprStmt) = EXPR_STMT,
        Struct(Struct) = STRUCT,
        Enum(Enum) = ENUM,
        Const(Const) = CONST,
        TypeAlias(TypeAlias) = TYPE_ALIAS,
        Class(Class) = CLASS,
        Impl(Impl) = IMPL,
        ExternBlock(ExternBlock) = EXTERN_BLOCK,
        Import(Import) = IMPORT,
        MacroCall(MacroCall) = MACRO_CALL,
        MacroRules(MacroRules) = MACRO_RULES,
    }
);

/// `macro_rules! name { rules }`
pub struct MacroRules(SyntaxNode);

simple_node!(MacroRules, MACRO_RULES);

impl MacroRules {
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn rules(&self) -> ChildrenIter<MacroRule> {
        ChildrenIter::new(&self.0)
    }
}

/// `(pattern) => { body }` in a macro definition
pub struct MacroRule(SyntaxNode);

simple_node!(MacroRule, MACRO_RULE);

impl MacroRule {
    pub fn pattern(&self) -> Option<TokenTree> {
        self.find_node()
    }
    pub fn body(&self) -> Option<TokenTree> {
        self.0.children().filter_map(TokenTree::cast).nth(1)
    }
}

/// `import "./path" (a, b as c);`, `import "./path" as name;` or
/// `import "./path" *;`
pub struct Import(SyntaxNode);

simple_node!(Import, IMPORT);

impl Import {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    /// The string with the path to import.
    pub fn source(&self) -> Option<SyntaxToken> {
        self.find_token(STRING)
    }
    /// The enum of `import Option.*;`, whose variants it imports.
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
    pub fn import_list(&self) -> Option<ImportList> {
        self.find_node()
    }
    /// The `name` of `import "./path" as name;`.
    pub fn alias(&self) -> Option<Name> {
        self.find_node()
    }
    /// The `*` of `import "./path" *;` or `import Option.*;`.
    pub fn star_token(&self) -> Option<SyntaxToken> {
        self.find_token(STAR)
    }
}

/// `(a, b as c)` in an import
pub struct ImportList(SyntaxNode);

simple_node!(ImportList, IMPORT_LIST);

impl ImportList {
    /// The names imported without an alias.
    pub fn names(&self) -> ChildrenIter<NameRef> {
        ChildrenIter::new(&self.0)
    }
    pub fn aliases(&self) -> ChildrenIter<ImportAlias> {
        ChildrenIter::new(&self.0)
    }
}

/// `b as c` in an import list
pub struct ImportAlias(SyntaxNode);

simple_node!(ImportAlias, IMPORT_ALIAS);

impl ImportAlias {
    pub fn name_ref(&self) -> Option<NameRef> {
        self.find_node()
    }
    pub fn alias(&self) -> Option<Name> {
        self.find_node()
    }
}

/// `extern "C" { items }` or `extern "C";`
pub struct ExternBlock(SyntaxNode);

simple_node!(ExternBlock, EXTERN_BLOCK);

impl ExternBlock {
    pub fn abi(&self) -> Option<Abi> {
        self.find_node()
    }
    pub fn item_list(&self) -> Option<ItemList> {
        self.find_node()
    }
}

/// `class Show[T] where T: Eq { items }`
pub struct Class(SyntaxNode);

simple_node!(Class, CLASS);

impl Class {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    /// The class parameters, like the `[T]` of `class Show[T]`.
    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }
    /// Superclasses go here: `class Ord[T] where T: Eq`.
    pub fn where_clause(&self) -> Option<WhereClause> {
        self.find_node()
    }
    pub fn item_list(&self) -> Option<ItemList> {
        self.find_node()
    }
}

/// `impl Show[Dog] { items }`
pub struct Impl(SyntaxNode);

simple_node!(Impl, IMPL);

impl Impl {
    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }
    /// The class instance, like `Show[Dog]`.
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
    pub fn where_clause(&self) -> Option<WhereClause> {
        self.find_node()
    }
    pub fn item_list(&self) -> Option<ItemList> {
        self.find_node()
    }
}

/// The `{ items }` of a class, impl or extern block
pub struct ItemList(SyntaxNode);

simple_node!(ItemList, ITEM_LIST);

impl ItemList {
    pub fn items(&self) -> ChildrenIter<SourceElement> {
        ChildrenIter::new(&self.0)
    }
}

/// `const NAME: Type = value;`
pub struct Const(SyntaxNode);

simple_node!(Const, CONST);

impl Const {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

/// `type A = B;`, or `type Item: Bound;` in a trait
pub struct TypeAlias(SyntaxNode);

simple_node!(TypeAlias, TYPE_ALIAS);

impl TypeAlias {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }
    /// The bounds of an associated type, like `: Iterator`.
    pub fn bounds(&self) -> Option<TraitBounds> {
        self.find_node()
    }
    /// The `where` clauses, before and after the type.
    pub fn where_clauses(&self) -> ChildrenIter<WhereClause> {
        ChildrenIter::new(&self.0)
    }
    /// The type after `=`. `None` for an associated type without a default.
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
}

/// `struct Point { x: i32 }`, `struct Pair(A, B);` or `struct Unit;`
pub struct Struct(SyntaxNode);

simple_node!(Struct, STRUCT);

impl Struct {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }
    pub fn where_clause(&self) -> Option<WhereClause> {
        self.find_node()
    }
    /// `None` for a unit struct like `struct Unit;`.
    pub fn field_list(&self) -> Option<FieldList> {
        self.find_node()
    }
}

/// `enum Option[T] { None, Some(T) }`
pub struct Enum(SyntaxNode);

simple_node!(Enum, ENUM);

impl Enum {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn generic_param_list(&self) -> Option<GenericParamList> {
        self.find_node()
    }
    pub fn where_clause(&self) -> Option<WhereClause> {
        self.find_node()
    }
    pub fn variants(&self) -> Vec<Variant> {
        self.find_node::<VariantList>().map(|list| list.variants().collect()).unwrap_or_default()
    }
}

/// The `{ ... }` of an enum
pub struct VariantList(SyntaxNode);

simple_node!(VariantList, VARIANT_LIST);

impl VariantList {
    pub fn variants(&self) -> ChildrenIter<Variant> {
        ChildrenIter::new(&self.0)
    }
}

/// `None`, `Some(T)`, `Point { x: i32 }` or `A = 1`
pub struct Variant(SyntaxNode);

simple_node!(Variant, VARIANT);

impl Variant {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn field_list(&self) -> Option<FieldList> {
        self.find_node()
    }
    /// The discriminant after `=`.
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
}

ast_enum!(
    /// The fields of a struct or variant.
    FieldList {
        Record(RecordFieldList) = RECORD_FIELD_LIST,
        Tuple(TupleFieldList) = TUPLE_FIELD_LIST,
    }
);

/// `{ x: i32, pub y: i32 }`
pub struct RecordFieldList(SyntaxNode);

simple_node!(RecordFieldList, RECORD_FIELD_LIST);

impl RecordFieldList {
    pub fn fields(&self) -> ChildrenIter<RecordField> {
        ChildrenIter::new(&self.0)
    }
}

/// `x: i32` in a struct or variant
pub struct RecordField(SyntaxNode);

simple_node!(RecordField, RECORD_FIELD);

impl RecordField {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn name(&self) -> Option<Name> {
        self.find_node()
    }
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
}

/// `(i32, pub String)`
pub struct TupleFieldList(SyntaxNode);

simple_node!(TupleFieldList, TUPLE_FIELD_LIST);

impl TupleFieldList {
    pub fn fields(&self) -> ChildrenIter<TupleField> {
        ChildrenIter::new(&self.0)
    }
}

/// `i32` or `pub String` in a tuple struct or variant
pub struct TupleField(SyntaxNode);

simple_node!(TupleField, TUPLE_FIELD);

impl TupleField {
    pub fn visibility(&self) -> Option<Visibility> {
        self.find_node()
    }
    pub fn ty(&self) -> Option<Type> {
        self.find_node()
    }
}

/// An expression used as a statement, like `f(x);` or `if x { }`.
pub struct ExprStmt(SyntaxNode);

simple_node!(ExprStmt, EXPR_STMT);

impl ExprStmt {
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
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

/// `#[meta]` or `#![meta]`
pub struct Attr(SyntaxNode);

simple_node!(Attr, ATTR);

impl Attr {
    /// Whether this is an inner attribute like `#![allow(unused)]`.
    pub fn is_inner(&self) -> bool {
        self.find_token(EXCL).is_some()
    }
    pub fn meta(&self) -> Option<Meta> {
        self.find_node()
    }
}

/// `path`, `path = value` or `path(tokens)` in an attribute
pub struct Meta(SyntaxNode);

simple_node!(Meta, META);

impl Meta {
    pub fn path(&self) -> Option<Path> {
        self.find_node()
    }
    /// The value of `path = value`.
    pub fn expr(&self) -> Option<Expr> {
        self.find_node()
    }
    /// The arguments of `path(tokens)`.
    pub fn token_tree(&self) -> Option<TokenTree> {
        self.find_node()
    }
}

/// Any tokens between balanced `()`, `[]` or `{}`
pub struct TokenTree(SyntaxNode);

simple_node!(TokenTree, TOKEN_TREE);

/// A node that can have attributes like `#[derive(Debug)]`.
pub trait HasAttrs: Node {
    fn attrs(&self) -> ChildrenIter<Attr> {
        ChildrenIter::new(self.syntax())
    }
}

macro_rules! has_attrs {
    ($($name:ident),* $(,)?) => {
        $(impl HasAttrs for $name {})*
    };
}

has_attrs!(
    SourceFile, FuncDecl, VarDecl, ExprStmt, Struct, Enum, Variant, RecordField, TupleField, Const,
    TypeAlias, Class, Impl, ItemList, ExternBlock, Import, Param, MatchArm, MatchArmList,
    RecordExprField, Block,
);

/// What kind of comment a [`Comment`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    /// `// ...` or `/* ... */`
    Plain,
    /// `/// ...` or `/** ... */`, documenting the item after it
    OuterDoc,
    /// `//! ...` or `/*! ... */`, documenting the item it's in
    InnerDoc,
}

/// A line or block comment. Doc comments are ordinary comment tokens; this
/// tells them apart by their text, following the grammar's `line_comment`
/// and `block_comment`.
pub struct Comment(SyntaxToken);

impl Comment {
    pub fn cast(token: SyntaxToken) -> Option<Self> {
        matches!(token.kind(), LINE_COMMENT | BLOCK_COMMENT).then_some(Comment(token))
    }

    pub fn syntax(&self) -> &SyntaxToken {
        &self.0
    }

    pub fn kind(&self) -> CommentKind {
        let text = self.0.text();
        if self.0.kind() == LINE_COMMENT {
            if text.starts_with("///") && !text.starts_with("////") {
                CommentKind::OuterDoc
            } else if text.starts_with("//!") {
                CommentKind::InnerDoc
            } else {
                CommentKind::Plain
            }
        } else if text.starts_with("/**") && !text.starts_with("/***") && text != "/**/" {
            CommentKind::OuterDoc
        } else if text.starts_with("/*!") {
            CommentKind::InnerDoc
        } else {
            CommentKind::Plain
        }
    }

    /// The text of a doc comment without its markers, like ` Hello` for
    /// `/// Hello`. `None` for a plain comment.
    pub fn doc_text(&self) -> Option<&str> {
        if self.kind() == CommentKind::Plain {
            return None;
        }
        let text = self.0.text();
        Some(if self.0.kind() == LINE_COMMENT {
            &text[3..]
        } else {
            text[3..].strip_suffix("*/").unwrap_or(&text[3..])
        })
    }
}

/// A node that can be documented with doc comments.
pub trait HasDocs: Node {
    /// The outer doc comments right before this node, like `/// Hello`
    /// above a function, from first to last.
    fn doc_comments(&self) -> Vec<Comment> {
        let mut comments = Vec::new();
        let mut current = self.syntax().prev_sibling_or_token();
        while let Some(element) = current {
            let Some(token) = element.as_token() else { break };
            if !token.kind().is_trivia() {
                break;
            }
            if let Some(comment) = Comment::cast(token.clone())
                && comment.kind() == CommentKind::OuterDoc
            {
                comments.push(comment);
            }
            current = token.prev_sibling_or_token();
        }
        comments.reverse();
        comments
    }

    /// The inner doc comments at the start of this node, like `//! Hello`
    /// at the top of a file.
    fn inner_doc_comments(&self) -> Vec<Comment> {
        self.syntax()
            .children_with_tokens()
            .map_while(|element| element.into_token().filter(|token| token.kind().is_trivia()))
            .filter_map(Comment::cast)
            .filter(|comment| comment.kind() == CommentKind::InnerDoc)
            .collect()
    }
}

macro_rules! has_docs {
    ($($name:ident),* $(,)?) => {
        $(impl HasDocs for $name {})*
    };
}

has_docs!(
    SourceFile, FuncDecl, Struct, Enum, Variant, RecordField, TupleField, Const, TypeAlias, Class, Impl,
    ExternBlock, Import, MacroRules, Block, ItemList,
);
