//! A code generator from Bolt to WebAssembly.
//!
//! It's minimal for now: it compiles the scalar types (integers, floats,
//! `bool`, `char` and `()`), functions, `let`, `if`, loops, `break`,
//! `continue`, `return`, and `match` on scalars. Calls to functions of
//! `extern` blocks are imports from the host, in the module `env`. Anything
//! else is an [`ErrorKind::Unsupported`] error.
//!
//! The code generator works on the bodies of hir-def and the types that
//! hir-ty infers for them, through Salsa queries:
//!
//! - [`compile_instance`] compiles one function for concrete type
//!   arguments: generic functions are monomorphized. The code it gives refers
//!   to other functions by [`Callee`], not by index, so that it only changes
//!   when the function does.
//! - [`reachable_instances`] finds the instances the entry of a program
//!   calls, directly or not.
//! - [`link`] numbers them, and emits and validates the module.
//!
//! The statements at the top level of the entry file are the exported
//! function `main`. It returns the value of the last statement, if it's an
//! expression with a value, like `bolt run` prints. Top-level functions that
//! aren't generic are exported too, under their names.
//!
//! Integers wrap around when they overflow, unlike in the evaluator, and
//! dividing by zero traps. `isize` and `usize` are 32 bits, and `i128` and
//! `u128` aren't supported.

mod emit;
mod ir;
mod lower;

#[cfg(test)]
mod tests;

use std::{fmt, ops::Range};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    hir::ExprId,
};
use boltlang_hir_ty::Ty;

pub use emit::link;
pub use ir::{BlockTy, Callee, CompiledFn, Inst, Op, ValTy};

/// A body compiled for concrete type arguments: a function, or the
/// top-level statements of the entry file.
#[salsa::interned(debug)]
pub struct Instance<'db> {
    #[returns(copy)]
    pub body: DefWithBodyId<'db>,
    /// The types of the parameters of the function's scheme, then of its
    /// bound variables: see [`boltlang_hir_ty::Scheme`].
    #[returns(ref)]
    pub type_args: Box<[Ty<'db>]>,
}

/// The instance for the top-level statements of `file`.
pub fn entry_instance<'db>(db: &'db dyn Db, file: File) -> Instance<'db> {
    Instance::new(db, DefWithBodyId::new(db, DefWithBody::File(file)), Box::<[Ty<'db>]>::default())
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct CodegenError<'db> {
    pub kind: ErrorKind,
    /// The expression that couldn't be compiled, in its body.
    pub location: Option<(DefWithBodyId<'db>, ExprId)>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum ErrorKind {
    /// Something the code generator can't compile yet.
    Unsupported(String),
    /// A type that inference didn't decide, which happens after a type
    /// error.
    UnknownType,
    /// The module isn't valid Wasm, which is a bug in the code generator.
    Invalid(String),
}

impl fmt::Display for CodegenError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Unsupported(what) => write!(f, "{what} is not supported by the code generator yet"),
            ErrorKind::UnknownType => write!(f, "the type of this expression is not known"),
            ErrorKind::Invalid(message) => write!(f, "the code generator made an invalid module: {message}"),
        }
    }
}

impl<'db> CodegenError<'db> {
    /// The file and byte range of the expression of the error.
    pub fn source_range(&self, db: &'db dyn Db) -> Option<(File, Range<u32>)> {
        let (def, expr) = self.location?;
        let file = match def.def(db) {
            DefWithBody::Function(id) => id.file(db),
            DefWithBody::Const(id) => id.file(db),
            DefWithBody::Variant(id) => id.parent.file(db),
            DefWithBody::File(file) => file,
        };
        let ptr = body_with_source_map(db, def).1.expr_syntax(expr)?;
        Some((file, ptr.start..ptr.end))
    }
}

/// The code of an instance, or why it can't be compiled.
#[salsa::tracked(returns(ref))]
pub fn compile_instance<'db>(db: &'db dyn Db, instance: Instance<'db>) -> Result<CompiledFn<'db>, CodegenError<'db>> {
    lower::compile(db, instance)
}

/// The instances that the entry of `file` calls, directly or not, with the
/// entry first. They're found with a worklist rather than by recursion, so
/// that recursive functions aren't cycles of queries.
#[salsa::tracked(returns(ref))]
pub fn reachable_instances<'db>(db: &'db dyn Db, file: File) -> Vec<Instance<'db>> {
    let mut found = vec![entry_instance(db, file)];
    let mut next = 0;
    while next < found.len() {
        if let Ok(compiled) = compile_instance(db, found[next]) {
            for callee in &compiled.callees {
                if let Callee::Instance(instance) = callee
                    && !found.contains(instance)
                {
                    found.push(*instance);
                }
            }
        }
        next += 1;
    }
    found
}
