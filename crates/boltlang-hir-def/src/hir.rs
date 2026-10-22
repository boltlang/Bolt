//! The lowered forms of expressions, patterns and types, as stored in an
//! [`ExpressionStore`](crate::expr_store::ExpressionStore).
//!
//! Like rust-analyzer's `hir` module, these are simpler than the syntax:
//!
//! - Parentheses are gone.
//! - Closures and anonymous functions (`fn (x) = e`) are both [`Expr::Lambda`].
//! - Custom operators like `<$>` are [`BinaryOp::Custom`], and assignments
//!   are binary operators too.
//! - `mut x` is a [`Binding`] with `mutable` set.
//! - Macro invocations are opaque, since macros aren't expanded yet.
//!
//! Names in paths aren't resolved here; that's up to the type checker.

use crate::{Name, arena::Idx};

pub type ExprId = Idx<Expr>;
pub type PatId = Idx<Pat>;
pub type TypeRefId = Idx<TypeRef>;
pub type BindingId = Idx<Binding>;
pub type LabelId = Idx<Label>;

/// A literal like `1`, `"s"` or `true`. Floats are kept as text, so
/// literals compare exactly.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Literal {
    Bool(bool),
    /// The value and the type suffix, like `u8` in `10u8`.
    Int(u128, Option<Name>),
    /// The text without `_` separators or suffix, and the suffix.
    Float(String, Option<Name>),
    Char(char),
    Byte(u8),
    String(String),
    ByteString(Vec<u8>),
    CString(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum PrefixOp {
    /// `-x`
    Neg,
    /// `!x`
    Not,
}

/// A built-in arithmetic or bitwise operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum CmpOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BinaryOp {
    Arith(ArithOp),
    Cmp(CmpOp),
    Logic(LogicOp),
    /// `=`, or a compound assignment like `+=` with its operator.
    Assign(Option<ArithOp>),
    /// An operator the grammar doesn't define, like `<$>`.
    Custom(Name),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BlockKind {
    Plain,
    /// `async { }`
    Async,
    /// `const { }`
    Const,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum LambdaKind {
    /// `|x| body`
    Closure,
    /// `async |x| body`
    AsyncClosure,
    /// `fn (x) = body`
    Function,
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Expr {
    /// Where an expression should be but couldn't be parsed.
    Missing,
    Path(Path),
    Literal(Literal),
    Block { statements: Box<[Statement]>, tail: Option<ExprId>, label: Option<LabelId>, kind: BlockKind },
    Call { callee: ExprId, args: Box<[ExprId]> },
    /// `x.name` or `t.0`; a tuple index is a name like `0`.
    Field { expr: ExprId, name: Name },
    Await { expr: ExprId },
    Try { expr: ExprId },
    Cast { expr: ExprId, type_ref: TypeRefId },
    Index { base: ExprId, index: ExprId },
    Prefix { op: PrefixOp, expr: ExprId },
    Binary { op: BinaryOp, lhs: ExprId, rhs: ExprId },
    /// `a..b`, `a..=b`, `a..`, `..b` or `..`
    Range { lhs: Option<ExprId>, rhs: Option<ExprId>, inclusive: bool },
    Tuple { exprs: Box<[ExprId]> },
    /// `[a, b]`
    Array { exprs: Box<[ExprId]> },
    /// `[value; count]`
    ArrayRepeat { value: ExprId, count: ExprId },
    /// `Point { x, y: 1, ..base }`
    Record { path: Option<Path>, fields: Box<[RecordField]>, spread: Option<ExprId> },
    If { condition: ExprId, then_branch: ExprId, else_branch: Option<ExprId> },
    /// `let pattern = value` in a condition
    Let { pat: PatId, expr: ExprId },
    Match { expr: ExprId, arms: Box<[MatchArm]> },
    While { condition: ExprId, body: ExprId, label: Option<LabelId> },
    Loop { body: ExprId, label: Option<LabelId> },
    For { pat: PatId, iterable: ExprId, body: ExprId, label: Option<LabelId> },
    /// The label is resolved by the type checker.
    Break { expr: Option<ExprId>, label: Option<Name> },
    Continue { label: Option<Name> },
    Return { expr: Option<ExprId> },
    Yield { expr: Option<ExprId> },
    /// A closure or an anonymous function.
    Lambda {
        params: Box<[PatId]>,
        param_types: Box<[Option<TypeRefId>]>,
        ret_type: Option<TypeRefId>,
        body: ExprId,
        kind: LambdaKind,
    },
    /// A macro invocation, which isn't expanded yet.
    MacroCall { name: Option<Name> },
}

/// `x`, `x: value` or `0: value` in a struct literal.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct RecordField {
    pub name: Name,
    pub expr: ExprId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct MatchArm {
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub expr: ExprId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Statement {
    /// `let pattern: Type = value else { ... };`
    Let { pat: PatId, type_ref: Option<TypeRefId>, initializer: Option<ExprId>, else_branch: Option<ExprId> },
    /// An expression followed by `;`, or one that ends in a block.
    Expr { expr: ExprId, has_semi: bool },
    /// An item inside a body, like a nested function. Items in bodies
    /// aren't lowered yet.
    Item,
}

/// A variable introduced by a pattern.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Binding {
    pub name: Name,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Label {
    pub name: Name,
}

/// A pattern.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Pat {
    Missing,
    /// `_`
    Wild,
    /// `x`, or `x @ pattern`
    Bind { id: BindingId, subpat: Option<PatId> },
    /// `1`, `-1` or `"s"`
    Lit { literal: Literal, negative: bool },
    /// A constant or variant like `Color.Red`
    Path(Path),
    /// `(a, b)`. `rest` is the position of a `..`, if there is one.
    Tuple { args: Box<[PatId]>, rest: Option<u32> },
    /// `Some(x)`
    TupleStruct { path: Path, args: Box<[PatId]>, rest: Option<u32> },
    /// `Point { x, y: 0, .. }`
    Record { path: Path, fields: Box<[RecordFieldPat]>, rest: bool },
    /// `[a, .., b]`
    Slice { args: Box<[PatId]>, rest: Option<u32> },
    /// `1..=5`, `a..` or `..=b`. The ends are literal or path patterns.
    Range { start: Option<PatId>, end: Option<PatId>, inclusive: bool },
    /// `a | b`
    Or(Box<[PatId]>),
    /// `const { ... }`
    ConstBlock(ExprId),
    MacroCall { name: Option<Name> },
}

/// `x`, `mut x` or `x: pattern` in a struct pattern.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct RecordFieldPat {
    pub name: Name,
    pub pat: PatId,
}

/// A path like `Shape.Circle` or `Vec[T]`, not yet resolved.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Path {
    pub segments: Box<[PathSegment]>,
}

impl Path {
    pub fn from_name(name: Name) -> Path {
        Path { segments: Box::new([PathSegment { name, generic_args: None }]) }
    }

    /// The name if the path has a single segment without generic arguments.
    pub fn as_name(&self) -> Option<&Name> {
        match &*self.segments {
            [PathSegment { name, generic_args: None }] => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct PathSegment {
    pub name: Name,
    /// The `[A, B]` of `Vec[A, B]`.
    pub generic_args: Option<Box<[GenericArg]>>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum GenericArg {
    /// A type, with the bounds after it in `T: Ord + Hash`.
    Type { type_ref: TypeRefId, bounds: Box<[TypeRefId]> },
    /// A binding like `Item = T`.
    AssocType { name: Name, generic_args: Option<Box<[GenericArg]>>, type_ref: TypeRefId },
    /// A literal or block, like `3` or `{ N + 1 }`.
    Const(ExprId),
}

/// A type as written, before it's resolved.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum TypeRef {
    /// Where a type should be but couldn't be parsed.
    Error,
    Path(Path),
    /// `(A, B)`, or `()` for the unit type.
    Tuple(Box<[TypeRefId]>),
    /// `[T]` or `[T; N]`
    Array { element: TypeRefId, len: Option<ExprId> },
    /// `!`
    Never,
    /// `fn(A, B) -> C`, or `FnOnce(A) -> C` with a trait.
    Fn {
        trait_path: Option<Path>,
        params: Box<[(Option<Name>, TypeRefId)]>,
        is_variadic: bool,
        ret_type: Option<TypeRefId>,
        is_async: bool,
        is_const: bool,
        /// `Some` for `extern "C" fn`, with the ABI if one is given.
        abi: Option<Option<String>>,
    },
    /// `impl Trait`, with the generic parameters of `impl for[T] Trait`.
    ImplTrait { generic_params: Box<[Name]>, bounds: Box<[TypeRefId]> },
    /// `A + B`
    Bounded(Box<[TypeRefId]>),
    MacroCall { name: Option<Name> },
}
