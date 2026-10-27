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
//! [`Builtin`], which the diagnostics check. A generic builtin, like
//! `array_len`, can name its type parameters as it likes.

use boltlang_base_db::Db;
use boltlang_hir_def::{
    builtin_type::{BuiltinInt, BuiltinType},
    hir::Mutability,
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
    /// `fn array_len[T](xs: [T]) -> i32`: the number of elements of an array
    /// of either mutability.
    ArrayLen,
    /// `fn array_alloc[T](len: i32) -> mut [T]`: a new array of `len`
    /// elements that aren't set yet. Reading one before it's set is an error
    /// at run time, so it's meant for types in `std` that keep track of
    /// which elements are set, like a growable array.
    ArrayAlloc,
}

impl Builtin {
    pub const ALL: &[Builtin] = &[Builtin::Print, Builtin::Panic, Builtin::ArrayLen, Builtin::ArrayAlloc];

    /// The builtins that compiled code imports from the host. The others
    /// work on arrays, which the code generator doesn't compile yet.
    pub const HOST: &[Builtin] = &[Builtin::Print, Builtin::Panic];

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Print => "print",
            Builtin::Panic => "panic",
            Builtin::ArrayLen => "array_len",
            Builtin::ArrayAlloc => "array_alloc",
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
            Builtin::Panic | Builtin::ArrayLen | Builtin::ArrayAlloc => false,
        }
    }

    /// The type a declaration of it must have. Its type parameters, and
    /// the mutabilities that can be either, are [`Ty::Bound`].
    pub fn signature<'db>(self) -> Ty<'db> {
        let str_ty = Ty::Builtin(BuiltinType::String);
        let i32_ty = Ty::Builtin(BuiltinType::Int(BuiltinInt::I32));
        match self {
            Builtin::Print => Ty::Fn(Box::new([str_ty]), Box::new(Ty::unit())),
            Builtin::Panic => Ty::Fn(Box::new([str_ty]), Box::new(Ty::Never)),
            Builtin::ArrayLen => Ty::Fn(Box::new([Ty::array(Ty::Bound(0), Ty::Bound(1))]), Box::new(i32_ty)),
            Builtin::ArrayAlloc => {
                Ty::Fn(Box::new([i32_ty]), Box::new(Ty::array(Ty::Mutability(Mutability::Mut), Ty::Bound(0))))
            }
        }
    }

    /// Whether `declared`, the type of a declaration, is its signature,
    /// with any names for the type parameters.
    pub fn has_signature<'db>(self, declared: &Ty<'db>) -> bool {
        canonical(declared) == canonical(&self.signature())
    }
}

/// `ty` with its type parameters and bound variables numbered in the order
/// they first appear, so that types that differ only in those names are
/// equal.
fn canonical<'db>(ty: &Ty<'db>) -> Ty<'db> {
    let mut seen = Vec::new();
    ty.fold(&mut |t| {
        if !matches!(t, Ty::Param(_) | Ty::Bound(_)) {
            return None;
        }
        let index = seen.iter().position(|s| s == t).unwrap_or_else(|| {
            seen.push(t.clone());
            seen.len() - 1
        });
        Some(Ty::Bound(index as u32))
    })
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
