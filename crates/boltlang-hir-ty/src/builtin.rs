//! Builtins: functions that the compiler provides, rather than Bolt code.
//! The standard library declares them in `extern "bolt"` blocks, like
//!
//! ```text
//! extern "bolt" {
//!     pub fn print(text: String);
//! }
//! ```
//!
//! so that the type checker sees ordinary signatures, and each backend
//! implements them its own way: the evaluator in Rust, Wasm with imports
//! from the host. A declaration must have a name and a signature from
//! [`Builtin`], which the diagnostics check.

use boltlang_base_db::Db;
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    ids::{ExternBlockId, FunctionId, ItemContainerId},
    item_tree::FunctionName,
};

use crate::ty::Ty;

/// The ABI of the `extern` blocks that declare builtins.
pub const ABI: &str = "bolt";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Builtin {
    /// `fn print(text: String)`: write the text and a newline.
    Print,
    /// `fn panic(message: String) -> !`: stop the program with an error.
    Panic,
}

impl Builtin {
    pub const ALL: &[Builtin] = &[Builtin::Print, Builtin::Panic];

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Print => "print",
            Builtin::Panic => "panic",
        }
    }

    pub fn by_name(name: &str) -> Option<Builtin> {
        Self::ALL.iter().copied().find(|b| b.name() == name)
    }

    /// Whether it has an effect on the world, which evaluation at compile
    /// time, like that of a const, doesn't allow.
    pub fn is_effect(self) -> bool {
        match self {
            Builtin::Print => true,
            // Stopping with an error is fine anywhere.
            Builtin::Panic => false,
        }
    }

    /// The type a declaration of it must have.
    pub fn signature<'db>(self) -> Ty<'db> {
        let str_ty = Ty::Builtin(BuiltinType::String);
        match self {
            Builtin::Print => Ty::Fn(Box::new([str_ty]), Box::new(Ty::unit())),
            Builtin::Panic => Ty::Fn(Box::new([str_ty]), Box::new(Ty::Never)),
        }
    }
}

/// Whether an `extern` block declares builtins.
pub fn is_builtin_block(db: &dyn Db, block: ExternBlockId<'_>) -> bool {
    block.item(db).abi.as_deref() == Some(ABI)
}

/// The builtin that `f` is, if it's declared in an `extern "bolt"` block
/// with the name of one.
pub fn builtin(db: &dyn Db, f: FunctionId<'_>) -> Option<Builtin> {
    let ItemContainerId::ExternBlock(block) = f.container(db) else { return None };
    if !is_builtin_block(db, block) {
        return None;
    }
    match &f.item(db).name {
        Some(FunctionName::Name(name)) => Builtin::by_name(name.as_str()),
        _ => None,
    }
}
