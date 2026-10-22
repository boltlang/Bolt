//! The primitive types, like rust-analyzer's `BuiltinType`. Their names
//! aren't keywords: they're found when nothing in scope has the name, so a
//! program can define its own `i32`.
//!
//! Library types like `String` aren't built in; they're defined in Bolt.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BuiltinInt {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BuiltinUint {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BuiltinFloat {
    F32,
    F64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BuiltinType {
    Bool,
    Char,
    Str,
    Int(BuiltinInt),
    Uint(BuiltinUint),
    Float(BuiltinFloat),
}

impl BuiltinType {
    pub const ALL: &[(&str, BuiltinType)] = &[
        ("bool", BuiltinType::Bool),
        ("char", BuiltinType::Char),
        ("str", BuiltinType::Str),
        ("i8", BuiltinType::Int(BuiltinInt::I8)),
        ("i16", BuiltinType::Int(BuiltinInt::I16)),
        ("i32", BuiltinType::Int(BuiltinInt::I32)),
        ("i64", BuiltinType::Int(BuiltinInt::I64)),
        ("i128", BuiltinType::Int(BuiltinInt::I128)),
        ("isize", BuiltinType::Int(BuiltinInt::Isize)),
        ("u8", BuiltinType::Uint(BuiltinUint::U8)),
        ("u16", BuiltinType::Uint(BuiltinUint::U16)),
        ("u32", BuiltinType::Uint(BuiltinUint::U32)),
        ("u64", BuiltinType::Uint(BuiltinUint::U64)),
        ("u128", BuiltinType::Uint(BuiltinUint::U128)),
        ("usize", BuiltinType::Uint(BuiltinUint::Usize)),
        ("f32", BuiltinType::Float(BuiltinFloat::F32)),
        ("f64", BuiltinType::Float(BuiltinFloat::F64)),
    ];

    pub fn by_name(name: &str) -> Option<BuiltinType> {
        Self::ALL.iter().find(|(n, _)| *n == name).map(|(_, ty)| *ty)
    }

    pub fn name(self) -> &'static str {
        Self::ALL.iter().find(|(_, ty)| *ty == self).map(|(n, _)| *n).unwrap()
    }
}

impl fmt::Display for BuiltinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
