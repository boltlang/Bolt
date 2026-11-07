//! Builtins: functions that the compiler provides, rather than Bolt code.
//! The standard library declares them as functions without a body, with
//! `#[builtin]` before them, like
//!
//! ```text
//! #[builtin]
//! pub fn print(text: String);
//! ```
//!
//! so that the type checker sees ordinary signatures, and each backend
//! implements them its own way: the evaluator in Rust, Wasm with imports
//! from the host. A declaration must have a name and a signature from
//! [`Builtin`], which the diagnostics check. A generic builtin, like
//! `array_len`, can name its type parameters as it likes.
//!
//! Each operation of a primitive type is a builtin too, named after the
//! type and the operation, like `i32_add`, `f64_lt` or `bool_not`
//! ([`PrimOp`]): what the operators do on that type, as a function.

use std::sync::OnceLock;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    builtin_type::{BuiltinInt, BuiltinType, BuiltinUint},
    hir::Mutability,
    ids::{FunctionId, ItemContainerId},
    item_tree::{FunctionName, Link},
};

use crate::{
    AdtId,
    lang::{cont_struct, string_struct},
    ty::Ty,
};

/// What a builtin of a primitive type for a prefix operator does, like
/// `i32_neg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum PrefixOp {
    /// `-x`
    Neg,
    /// `!x`
    Not,
}

/// What a builtin of a primitive type for an arithmetic or bitwise operator
/// does, like `i32_add`.
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

/// What a builtin of a primitive type for a comparison does, like `i32_lt`.
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
pub enum Builtin {
    /// `fn print(text: String)`: write the text and a newline.
    Print,
    /// `fn abort() -> !`: stop the program with an error, at once. std's
    /// `panic` prints its message, then aborts.
    Abort,
    /// `fn array_len[T](xs: [T]) -> i32`: the number of elements of an array
    /// of either mutability.
    ArrayLen,
    /// `fn array_alloc[T](len: i32) -> mut [T]`: a new array of `len`
    /// elements that aren't set yet. Reading one before it's set is an error
    /// at run time, so it's meant for types in `std` that keep track of
    /// which elements are set, like a growable array.
    ArrayAlloc,
    /// `fn array_get[T](xs: [T], i: i32) -> T`: the element at `i`, which
    /// `xs[i]` gives through the prelude's `impl Index[[T]]`.
    ArrayGet,
    /// `fn array_set[T](xs: mut [T], i: i32, x: T)`: replace the element at
    /// `i`, for `xs[i] = x` through the prelude's `impl IndexMut[mut [T]]`.
    ArraySet,
    /// `fn array_copy[T](dst: mut [T], dst_start: i32, src: [T], src_start:
    /// i32, count: i32)`: copy `count` elements of `src` from `src_start` to
    /// `dst` from `dst_start`. The two can be the same array, and the ranges
    /// can overlap. Unset elements are copied as they are.
    ArrayCopy,
    /// `fn array_freeze[T](xs: [T]) -> imm [T]`: a copy of `xs` that can't
    /// be changed. Code can't make an `imm` array with computed elements
    /// otherwise, since one that's built is `mut`.
    ArrayFreeze,
    /// `fn resume[B, U, R](k: Cont[B, U, R], value: B) -> U does R`: go on
    /// with the computation a handler stopped, with `value` as the result of
    /// the operation. It gives what the handler's `match` gives for the
    /// rest of the computation, and does what the computation does besides
    /// the effects the `match` handles. `Cont` is the prelude's struct
    /// ([`cont_struct`]).
    Resume,
    /// An operation of a primitive type, like `fn i32_add(a: i32, b: i32) ->
    /// i32`: what the operator does on that type.
    Prim(PrimOp, BuiltinType),
}

/// An operation of a primitive type, as the operators do it: the name of a
/// builtin is the type's and the operation's, like `i32_add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum PrimOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    /// Shift by a `u32`.
    Shl,
    Shr,
    Neg,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl PrimOp {
    pub const ALL: &[PrimOp] = &[
        PrimOp::Add,
        PrimOp::Sub,
        PrimOp::Mul,
        PrimOp::Div,
        PrimOp::Rem,
        PrimOp::BitAnd,
        PrimOp::BitOr,
        PrimOp::BitXor,
        PrimOp::Shl,
        PrimOp::Shr,
        PrimOp::Neg,
        PrimOp::Not,
        PrimOp::Eq,
        PrimOp::Ne,
        PrimOp::Lt,
        PrimOp::Le,
        PrimOp::Gt,
        PrimOp::Ge,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PrimOp::Add => "add",
            PrimOp::Sub => "sub",
            PrimOp::Mul => "mul",
            PrimOp::Div => "div",
            PrimOp::Rem => "rem",
            PrimOp::BitAnd => "bitand",
            PrimOp::BitOr => "bitor",
            PrimOp::BitXor => "bitxor",
            PrimOp::Shl => "shl",
            PrimOp::Shr => "shr",
            PrimOp::Neg => "neg",
            PrimOp::Not => "not",
            PrimOp::Eq => "eq",
            PrimOp::Ne => "ne",
            PrimOp::Lt => "lt",
            PrimOp::Le => "le",
            PrimOp::Gt => "gt",
            PrimOp::Ge => "ge",
        }
    }

    /// Whether `ty` has the operation, as it has the operator: integers
    /// all but `!`, and `-` only when they're signed; floats arithmetic,
    /// `-` and comparisons; and `Bool` the bitwise ones, `!` and
    /// comparisons.
    pub fn applies_to(self, ty: BuiltinType) -> bool {
        use PrimOp::*;
        let cmp = matches!(self, Eq | Ne | Lt | Le | Gt | Ge);
        match ty {
            BuiltinType::Int(_) => self != Not,
            BuiltinType::Uint(_) => !matches!(self, Neg | Not),
            BuiltinType::Float(_) => cmp || matches!(self, Add | Sub | Mul | Div | Rem | Neg),
            BuiltinType::Bool => cmp || matches!(self, BitAnd | BitOr | BitXor | Not),
        }
    }

    /// The arithmetic or bitwise operator it is, if it's one.
    pub fn arith(self) -> Option<ArithOp> {
        Some(match self {
            PrimOp::Add => ArithOp::Add,
            PrimOp::Sub => ArithOp::Sub,
            PrimOp::Mul => ArithOp::Mul,
            PrimOp::Div => ArithOp::Div,
            PrimOp::Rem => ArithOp::Rem,
            PrimOp::BitAnd => ArithOp::BitAnd,
            PrimOp::BitOr => ArithOp::BitOr,
            PrimOp::BitXor => ArithOp::BitXor,
            PrimOp::Shl => ArithOp::Shl,
            PrimOp::Shr => ArithOp::Shr,
            _ => return None,
        })
    }

    /// The comparison it is, if it's one.
    pub fn cmp(self) -> Option<CmpOp> {
        Some(match self {
            PrimOp::Eq => CmpOp::Eq,
            PrimOp::Ne => CmpOp::NotEq,
            PrimOp::Lt => CmpOp::Lt,
            PrimOp::Le => CmpOp::LtEq,
            PrimOp::Gt => CmpOp::Gt,
            PrimOp::Ge => CmpOp::GtEq,
            _ => return None,
        })
    }
}

/// Every builtin, with its name: those of [`Builtin::BASE`], then the
/// operations of each primitive type.
fn all_builtins() -> &'static [(Builtin, &'static str)] {
    static ALL: OnceLock<Vec<(Builtin, &'static str)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all: Vec<(Builtin, &'static str)> =
            Builtin::BASE.iter().map(|&b| (b, b.base_name())).collect();
        for &(name, ty) in BuiltinType::ALL {
            for &op in PrimOp::ALL {
                if op.applies_to(ty) {
                    let name: &'static str =
                        format!("{}_{}", name.to_lowercase(), op.name()).leak();
                    all.push((Builtin::Prim(op, ty), name));
                }
            }
        }
        all
    })
}

impl Builtin {
    /// The builtins other than the operations of primitive types.
    pub const BASE: &[Builtin] = &[
        Builtin::Print,
        Builtin::Abort,
        Builtin::ArrayLen,
        Builtin::ArrayAlloc,
        Builtin::ArrayGet,
        Builtin::ArraySet,
        Builtin::ArrayCopy,
        Builtin::ArrayFreeze,
        Builtin::Resume,
    ];

    /// The builtins that compiled code imports from the host. The others
    /// work on arrays, which the code generator doesn't compile yet.
    pub const HOST: &[Builtin] = &[Builtin::Print];

    /// Every builtin: those of [`Builtin::BASE`], then the operations of
    /// each primitive type.
    pub fn all() -> impl Iterator<Item = Builtin> {
        all_builtins().iter().map(|&(b, _)| b)
    }

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Prim(..) => {
                all_builtins().iter().find(|&&(b, _)| b == self).map_or("?", |&(_, n)| n)
            }
            _ => self.base_name(),
        }
    }

    fn base_name(self) -> &'static str {
        match self {
            Builtin::Prim(..) => unreachable!("named in `all_builtins`"),
            Builtin::Print => "print",
            Builtin::Abort => "abort",
            Builtin::ArrayLen => "array_len",
            Builtin::ArrayAlloc => "array_alloc",
            Builtin::ArrayGet => "array_get",
            Builtin::ArraySet => "array_set",
            Builtin::ArrayCopy => "array_copy",
            Builtin::ArrayFreeze => "array_freeze",
            Builtin::Resume => "resume",
        }
    }

    pub fn by_name(name: &str) -> Option<Builtin> {
        all_builtins().iter().find(|&&(_, n)| n == name).map(|&(b, _)| b)
    }

    /// Whether it has an effect on the world, which evaluation at compile
    /// time, like that of a const, doesn't allow.
    pub fn is_effect(self) -> bool {
        match self {
            Builtin::Print => true,
            // Stopping with an error is fine anywhere.
            Builtin::Abort
            | Builtin::ArrayLen
            | Builtin::ArrayAlloc
            | Builtin::ArrayGet
            | Builtin::ArraySet
            | Builtin::ArrayCopy
            | Builtin::ArrayFreeze
            // What the computation does is in its effects.
            | Builtin::Resume
            | Builtin::Prim(..) => false,
        }
    }

    /// The type a declaration of it in `file` must have. Its type
    /// parameters, and the mutabilities that can be either, are
    /// [`Ty::Bound`]. A string is the prelude's `String`, as `file` sees it
    /// ([`string_struct`]): without one, `print` and `panic` can't be
    /// declared. It's always `imm` when it's an `imm struct`, and can be of
    /// either mutability otherwise.
    pub fn signature<'db>(self, db: &'db dyn Db, file: File) -> Ty<'db> {
        let str_ty = match string_struct(db, file) {
            Some(id) if id.item(db).is_imm => {
                Ty::adt(AdtId::Struct(id), [], [], Ty::Mutability(Mutability::Imm))
            }
            Some(id) => Ty::adt(AdtId::Struct(id), [], [], Ty::Bound(0)),
            None => Ty::Error,
        };
        let i32_ty = Ty::Builtin(BuiltinType::Int(BuiltinInt::I32));
        match self {
            Builtin::Print => Ty::func([str_ty], Ty::unit()),
            Builtin::Abort => Ty::func([], Ty::Never),
            Builtin::ArrayLen => Ty::func([Ty::array(Ty::Bound(0), Ty::Bound(1))], i32_ty),
            Builtin::ArrayAlloc => {
                Ty::func([i32_ty], Ty::array(Ty::Mutability(Mutability::Mut), Ty::Bound(0)))
            }
            Builtin::ArrayGet => {
                Ty::func([Ty::array(Ty::Bound(0), Ty::Bound(1)), i32_ty], Ty::Bound(1))
            }
            Builtin::ArraySet => {
                let xs = Ty::array(Ty::Mutability(Mutability::Mut), Ty::Bound(0));
                Ty::func([xs, i32_ty, Ty::Bound(0)], Ty::unit())
            }
            Builtin::ArrayCopy => {
                let dst = Ty::array(Ty::Mutability(Mutability::Mut), Ty::Bound(0));
                let src = Ty::array(Ty::Bound(1), Ty::Bound(0));
                Ty::func([dst, i32_ty.clone(), src, i32_ty.clone(), i32_ty], Ty::unit())
            }
            Builtin::ArrayFreeze => Ty::func(
                [Ty::array(Ty::Bound(0), Ty::Bound(1))],
                Ty::array(Ty::Mutability(Mutability::Imm), Ty::Bound(1)),
            ),
            Builtin::Resume => {
                let (b, u, r) = (Ty::Bound(0), Ty::Bound(1), Ty::Bound(2));
                let cont = match cont_struct(db, file) {
                    Some(id) => Ty::adt(
                        AdtId::Struct(id),
                        [b.clone(), u.clone(), r.clone()],
                        [],
                        Ty::Mutability(Mutability::Imm),
                    ),
                    None => Ty::Error,
                };
                Ty::func_does([cont, b], u, r)
            }
            Builtin::Prim(op, ty) => {
                let t = Ty::Builtin(ty);
                match op {
                    PrimOp::Neg | PrimOp::Not => Ty::func([t.clone()], t),
                    PrimOp::Shl | PrimOp::Shr => {
                        Ty::func([t.clone(), Ty::Builtin(BuiltinType::Uint(BuiltinUint::U32))], t)
                    }
                    PrimOp::Eq | PrimOp::Ne | PrimOp::Lt | PrimOp::Le | PrimOp::Gt | PrimOp::Ge => {
                        Ty::func([t.clone(), t], Ty::bool())
                    }
                    _ => Ty::func([t.clone(), t.clone()], t),
                }
            }
        }
    }

    /// Whether `declared`, the type of a declaration in `file`, is its
    /// signature, with any names for the type parameters.
    pub fn has_signature<'db>(self, db: &'db dyn Db, file: File, declared: &Ty<'db>) -> bool {
        canonical(declared) == canonical(&self.signature(db, file))
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

/// Whether `f` is declared as a builtin: a function at the top level of a
/// file, without a body, with `#[builtin]`.
pub fn is_builtin_decl(db: &dyn Db, f: FunctionId<'_>) -> bool {
    let item = f.item(db);
    item.is_builtin
        && !item.has_body
        && item.owner.is_none()
        && matches!(f.container(db), ItemContainerId::File(_))
}

/// Where the host provides `f`, if it's declared as a function the host
/// provides: at the top level of a file, without a body, with a
/// `#[link(..)]` that's written right and no `#[builtin]`.
pub fn link<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Option<&'db Link> {
    let item = f.item(db);
    let link = item.link.as_ref()?;
    let ok = link.error.is_none()
        && !item.is_builtin
        && !item.has_body
        && item.owner.is_none()
        && matches!(f.container(db), ItemContainerId::File(_));
    ok.then_some(link)
}

/// The builtin that `f` is, if it's declared as one with the name of one.
pub fn builtin(db: &dyn Db, f: FunctionId<'_>) -> Option<Builtin> {
    if !is_builtin_decl(db, f) {
        return None;
    }
    match &f.item(db).name {
        Some(FunctionName::Name(name)) => Builtin::by_name(name.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_has_its_own_name() {
        let all: Vec<Builtin> = Builtin::all().collect();
        for &b in &all {
            assert_eq!(Builtin::by_name(b.name()), Some(b), "{}", b.name());
        }
        let mut names: Vec<&str> = all.iter().map(|b| b.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len());
    }

    #[test]
    fn primitive_operations_are_named_after_their_type() {
        let i32_ty = BuiltinType::Int(BuiltinInt::I32);
        assert_eq!(Builtin::by_name("i32_add"), Some(Builtin::Prim(PrimOp::Add, i32_ty)));
        assert_eq!(
            Builtin::by_name("bool_not"),
            Some(Builtin::Prim(PrimOp::Not, BuiltinType::Bool))
        );
        assert_eq!(Builtin::by_name("string_add"), None);
        // Only what the operators do: unsigned integers have no `-`, and
        // integers no `!`.
        assert_eq!(Builtin::by_name("u8_neg"), None);
        assert_eq!(Builtin::by_name("i32_not"), None);
        assert_eq!(Builtin::by_name("char_add"), None);
        let db = boltlang_base_db::TestDb::new();
        let file = db.file("/main.bolt", "");
        let shl = Builtin::by_name("i64_shl").unwrap().signature(&db, file);
        let i64_ty = Ty::Builtin(BuiltinType::Int(BuiltinInt::I64));
        assert_eq!(
            shl,
            Ty::func([i64_ty.clone(), Ty::Builtin(BuiltinType::Uint(BuiltinUint::U32))], i64_ty)
        );
    }
}
