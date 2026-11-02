//! The mid-level IR (#324): a typed, structured tree that a body is lowered
//! to after type checking, for the evaluator and the code generator to run.
//!
//! Only the representation is here yet: lowering from the HIR and
//! monomorphization come later, and nothing uses it so far.
//!
//! - **Structured control flow.** Blocks, `loop`, `if` and `switch`, with
//!   labels, `break`, `continue` and `return`, as in Wasm. There's no
//!   control-flow graph. A labeled block is also a join point: several
//!   paths can `break` to it, so the code after it is written once.
//! - **Atoms as operands.** An [`Operand`] is a local or a constant, never a
//!   nested expression, so the order things run in is in the IR. A
//!   [`StmtKind::Let`] gives each intermediate result a local.
//! - **Control flow has values.** Blocks, `if`, `loop` and `switch` are
//!   [`Rvalue`]s, so a `let` can take the value of one, and `break 'l v`
//!   gives the block `'l` its value.
//! - **Resolved calls.** A [`Callee`] names its function with its type
//!   arguments, or, in generic code, a class method with its class's
//!   arguments, whose impl is chosen once the types are known.
//! - **Places.** A tuple is a value: assigning to one of its fields, like
//!   `t.0 = x`, changes only the local that holds it ([`StmtKind::Assign`]
//!   to a [`Place`]). A struct is an object that every name for it shares:
//!   changing one of its fields is [`StmtKind::SetField`].
//! - **Closures and cells.** A [`Rvalue::Closure`] lists the locals it
//!   captures. A local that a closure captures and that's assigned is a
//!   cell ([`Local::cell`]), shared between the closure and its body (#181).
//! - **Types.** Every local has a type, which can mention the function's
//!   type parameters: one IR per body, generic like the body.
//! - **Sources.** Every statement keeps the expression it comes from, for
//!   errors and for the evaluator's fuel.
//!
//! A body's locals and labels are numbered in tables of the [`Body`];
//! blocks, statements and operands are a tree that its blocks own. Every
//! type derives [`salsa::SalsaValue`] and holds Salsa IDs rather than
//! references into the database, so that a lowering query can return a
//! [`Body`].
//!
//! [`Body::display`] prints a body in the notation of #324:
//!
//! ```text
//! body(xs: mut [i32], i: i32) {
//!   let %0: i32 = call Index[mut [i32]].index(xs, i)
//!   let %1: i32 = call +[i32](%0, 1i32)
//!   call IndexMut[mut [i32]].set_index(xs, i, %1)
//! }
//! ```

mod print;

#[cfg(test)]
mod tests;

use boltlang_hir_def::{
    Name,
    arena::{Arena, Idx},
    hir::ExprId,
    ids::{ClassId, EnumVariantId, FunctionId, StructId},
};
use boltlang_hir_ty::{Ty, builtin::Builtin};

pub use print::DisplayBody;

pub type LocalId<'db> = Idx<Local<'db>>;
pub type LabelId = Idx<Label>;

/// The IR of a body: a function, or the top-level statements of a file.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Body<'db> {
    /// Every variable and temporary of the body, its closures' too.
    pub locals: Arena<Local<'db>>,
    pub labels: Arena<Label>,
    /// The locals of the parameters, in order.
    pub params: Vec<LocalId<'db>>,
    /// What the body runs; its value is the body's result.
    pub block: Block<'db>,
}

/// A variable or a temporary.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Local<'db> {
    /// The variable's name, or `None` for a temporary, printed `%0`.
    pub name: Option<Name>,
    pub ty: Ty<'db>,
    /// It can be assigned again: `let mut`.
    pub mutable: bool,
    /// A closure captures it and something assigns it, so the closure and
    /// the code around it share it: backends store it in a cell. Reading
    /// and writing it are the same statements as for any local.
    pub cell: bool,
}

/// A label of a block or a loop, for `break` and `continue`.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Label {
    /// The label as written, like `'outer`, or `None` for one that
    /// lowering makes, like a join point of a `match`.
    pub name: Option<Name>,
}

/// Statements, then a value: what the block gives when it ends normally.
/// `break` to its label gives it a value too.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Block<'db> {
    pub label: Option<LabelId>,
    pub stmts: Vec<Stmt<'db>>,
    pub value: Operand<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Stmt<'db> {
    pub kind: StmtKind<'db>,
    /// The expression it comes from, if there is one.
    pub source: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum StmtKind<'db> {
    /// `let %0: i32 = call +[i32](a, b)`: a new local with the value.
    Let {
        local: LocalId<'db>,
        value: Rvalue<'db>,
    },
    /// `x = a` or `t.0 = a`: change the value of a local, or a field of a
    /// tuple in one, which no other name sees.
    Assign {
        place: Place<'db>,
        value: Operand<'db>,
    },
    /// `p.x = a` for a struct `p`: change a field of the object, which
    /// every name for it sees. `field` is the index of the field.
    SetField {
        object: Operand<'db>,
        field: u32,
        value: Operand<'db>,
    },
    /// An rvalue run for what it does, like a call whose result isn't used.
    Eval(Rvalue<'db>),
    /// Leave the block or loop `label`, giving a block a value.
    Break {
        label: LabelId,
        value: Option<Operand<'db>>,
    },
    /// Go back to the start of the loop `label`.
    Continue {
        label: LabelId,
    },
    Return(Operand<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Rvalue<'db> {
    Use(Operand<'db>),
    Call {
        callee: Callee<'db>,
        args: Vec<Operand<'db>>,
    },
    /// A new tuple, struct, variant or array, from its parts. Whether a
    /// struct or an array is `mut` is in the type of the local it's put in.
    Construct {
        kind: ConstructKind<'db>,
        args: Vec<Operand<'db>>,
    },
    /// `t.0` or `p.x`: a field, by its index.
    Project {
        base: Operand<'db>,
        field: u32,
    },
    Block(Block<'db>),
    If {
        cond: Operand<'db>,
        then_block: Block<'db>,
        else_block: Block<'db>,
    },
    /// Runs its body again until a `break` leaves it.
    Loop {
        label: LabelId,
        body: Block<'db>,
    },
    Switch(Switch<'db>),
    /// A closure, with the locals it captures. Its parameters and the
    /// locals of its body are in the body's table too.
    Closure {
        captures: Vec<LocalId<'db>>,
        params: Vec<LocalId<'db>>,
        body: Block<'db>,
    },
}

/// An atom: a local or a constant.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Operand<'db> {
    Local(LocalId<'db>),
    Const(Constant<'db>),
}

impl Operand<'_> {
    pub fn unit() -> Self {
        Operand::Const(Constant::Unit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Constant<'db> {
    Unit,
    Bool(bool),
    /// An integer of the type `ty`, like `-1i32`.
    Int {
        value: u128,
        negative: bool,
        ty: Ty<'db>,
    },
    /// A float, as written without `_` separators, so that it's exact.
    Float {
        text: String,
        ty: Ty<'db>,
    },
    Char(char),
    String(String),
    /// A function as a value, with its type arguments.
    Function {
        id: FunctionId<'db>,
        type_args: Vec<Ty<'db>>,
    },
}

/// A local, or a field of a tuple in a local, like `t.0.1`.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Place<'db> {
    pub local: LocalId<'db>,
    /// The indices of the tuple fields, outermost first.
    pub fields: Vec<u32>,
}

impl<'db> Place<'db> {
    pub fn local(local: LocalId<'db>) -> Self {
        Place { local, fields: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Callee<'db> {
    /// A function, with the types of its scheme's parameters.
    Function { id: FunctionId<'db>, type_args: Vec<Ty<'db>> },
    /// A class method in generic code, with the class's arguments: the
    /// impl it calls is chosen once the types are known.
    ClassMethod { class: ClassId<'db>, class_args: Vec<Ty<'db>>, method: FunctionId<'db> },
    /// A function the compiler provides, like `i32_add`.
    Builtin(Builtin),
    /// A closure or a function held in a local.
    Value(Operand<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum ConstructKind<'db> {
    Tuple,
    /// A struct, from its fields in order.
    Struct(StructId<'db>),
    /// A variant of an enum, from its fields in order.
    Variant(EnumVariantId<'db>),
    /// An array of its arguments.
    Array,
    /// `[value; count]`, from the value and the count.
    ArrayRepeat,
}

/// Runs the arm that matches the operand, or the default.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Switch<'db> {
    pub scrutinee: Operand<'db>,
    pub arms: Vec<SwitchArm<'db>>,
    /// What runs when no arm matches. Without one, the arms must cover
    /// every value.
    pub default: Option<Block<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct SwitchArm<'db> {
    pub test: SwitchTest<'db>,
    pub body: Block<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum SwitchTest<'db> {
    /// The operand is this variant: its fields, in order, go in the locals,
    /// or nowhere for `None`.
    Variant { variant: EnumVariantId<'db>, fields: Vec<Option<LocalId<'db>>> },
    /// The operand is this value of a scalar: an integer, a `char` or a
    /// `Bool`.
    Value(Constant<'db>),
}
