//! The code of a compiled function, before linking. It's Wasm's
//! instructions, except that calls name what they call instead of giving a
//! function index: indices depend on the whole module, and only
//! [`link`](crate::link) knows them. So a function's code only changes when
//! the function does.

use boltlang_hir_def::ids::FunctionId;
use boltlang_hir_ty::builtin::Builtin;

use crate::Instance;

/// A Wasm value type. Bolt's unit type and `!` have no value, so they have
/// no `ValTy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ValTy {
    I32,
    I64,
    F32,
    F64,
}

/// The type of a `block`, `loop` or `if`: what it leaves on the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BlockTy {
    Empty,
    Value(ValTy),
}

impl From<Option<ValTy>> for BlockTy {
    fn from(ty: Option<ValTy>) -> Self {
        ty.map_or(BlockTy::Empty, BlockTy::Value)
    }
}

/// What a call calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Callee<'db> {
    /// A Bolt function, with its type arguments.
    Instance(Instance<'db>),
    /// A function of an `extern` block, which the host provides in the
    /// module `env`.
    Import(FunctionId<'db>),
    /// A builtin, which the host provides in the module `bolt`.
    Builtin(Builtin),
}

/// A Wasm instruction. Floats are kept as bits, so that code can be
/// compared.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Inst<'db> {
    I32Const(i32),
    I64Const(i64),
    F32Const(u32),
    F64Const(u64),
    /// The address of a string literal, which linking decides: a `str` is
    /// the address of its length, as a `u32`, followed by its UTF-8 bytes.
    Str(Box<str>),
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    Call(Callee<'db>),
    Block(BlockTy),
    Loop(BlockTy),
    If(BlockTy),
    Else,
    End,
    /// Branch to the `n`th enclosing block, loop or if.
    Br(u32),
    BrIf(u32),
    Return,
    Unreachable,
    Drop,
    /// An instruction without immediates, like `i32.add`.
    Op(Op),
}

/// The instructions without immediates that the code generator uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Op {
    I32Add, I32Sub, I32Mul, I32DivS, I32DivU, I32RemS, I32RemU,
    I32And, I32Or, I32Xor, I32Shl, I32ShrS, I32ShrU,
    I32Eq, I32Ne, I32LtS, I32LtU, I32LeS, I32LeU, I32GtS, I32GtU, I32GeS, I32GeU, I32Eqz,
    I32Extend8S, I32Extend16S,
    I64Add, I64Sub, I64Mul, I64DivS, I64DivU, I64RemS, I64RemU,
    I64And, I64Or, I64Xor, I64Shl, I64ShrS, I64ShrU,
    I64Eq, I64Ne, I64LtS, I64LtU, I64LeS, I64LeU, I64GtS, I64GtU, I64GeS, I64GeU,
    F32Add, F32Sub, F32Mul, F32Div, F32Neg,
    F32Eq, F32Ne, F32Lt, F32Le, F32Gt, F32Ge,
    F64Add, F64Sub, F64Mul, F64Div, F64Neg,
    F64Eq, F64Ne, F64Lt, F64Le, F64Gt, F64Ge,
    I32WrapI64, I64ExtendI32S, I64ExtendI32U,
    F32ConvertI32S, F32ConvertI32U, F32ConvertI64S, F32ConvertI64U,
    F64ConvertI32S, F64ConvertI32U, F64ConvertI64S, F64ConvertI64U,
    I32TruncSatF32S, I32TruncSatF32U, I32TruncSatF64S, I32TruncSatF64U,
    I64TruncSatF32S, I64TruncSatF32U, I64TruncSatF64S, I64TruncSatF64U,
    F32DemoteF64, F64PromoteF32,
}

/// A compiled function: its signature, the types of its other locals, and
/// its code.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct CompiledFn<'db> {
    pub params: Vec<ValTy>,
    pub result: Option<ValTy>,
    /// The locals after the parameters.
    pub locals: Vec<ValTy>,
    pub code: Vec<Inst<'db>>,
    /// What the code calls, in order of first call.
    pub callees: Vec<Callee<'db>>,
}
