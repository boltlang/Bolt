//! Bolt's types in Conduit.
//!
//! - `Bool`, the integers up to 32 bits and fieldless enums are `i32`. An
//!   8- or 16-bit integer is kept in range after arithmetic. `i64` and
//!   `u64` are `i64`, and `isize` and `usize` are 32 bits.
//! - `()` and `!` are no values at all.
//! - A struct, a tuple and an enum with fields are a data type, one per
//!   struct or enum and type arguments. Each mutability in the type
//!   arguments becomes a mutability parameter of the declaration, so that
//!   `Option[mut [i32]]` and `Option[imm [i32]]` share one.
//! - A struct's fields without `mut` or `imm` are `inherit`: they have the
//!   struct's mutability, and the mutabilities a struct declares are its
//!   first parameters.
//! - An enum or a tuple can't be changed in place, but can hold values
//!   that can, which Conduit's deep `imm` doesn't allow. So its mutability
//!   is the most a value it holds can be: `imm` if they're all `imm`, the
//!   parameter if one parameter can be `mut`, and `mut` otherwise.

use std::collections::{BTreeMap, HashMap};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType, BuiltinUint},
    ids::{EnumId, EnumVariantId},
    signatures::{FieldList, enum_signature},
};
use boltlang_hir_ty::{AdtId, lang::cont_struct, lower::VariantDef};
use boltlang_mir_mono::{MonoMut, MonoTy, field_types};
use conduit_ir::{CtorDecl, DataDecl, DataId, Mutability, Row, Sig, Ty};

use crate::ErrorKind;

/// The marker for a struct's own mutability in its field types, which is
/// Conduit's `inherit`.
const OWN: u32 = u32::MAX;

/// How a value of a Bolt type is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scalar {
    /// An integer of this many bits, in an `i32` or an `i64`.
    Int {
        bits: u8,
        signed: bool,
    },
    F32,
    F64,
    Bool,
}

impl Scalar {
    pub(crate) fn of(t: &MonoTy<'_>) -> Result<Option<Scalar>, ErrorKind> {
        let MonoTy::Builtin(b) = t else { return Ok(None) };
        Ok(Some(match b {
            BuiltinType::Bool => Scalar::Bool,
            BuiltinType::Int(int) => match int {
                BuiltinInt::I8 => Scalar::Int { bits: 8, signed: true },
                BuiltinInt::I16 => Scalar::Int { bits: 16, signed: true },
                BuiltinInt::I32 | BuiltinInt::Isize => Scalar::Int { bits: 32, signed: true },
                BuiltinInt::I64 => Scalar::Int { bits: 64, signed: true },
                BuiltinInt::I128 => return Err(ErrorKind::missing("128-bit integers")),
            },
            BuiltinType::Uint(uint) => match uint {
                BuiltinUint::U8 => Scalar::Int { bits: 8, signed: false },
                BuiltinUint::U16 => Scalar::Int { bits: 16, signed: false },
                BuiltinUint::U32 | BuiltinUint::Usize => Scalar::Int { bits: 32, signed: false },
                BuiltinUint::U64 => Scalar::Int { bits: 64, signed: false },
                BuiltinUint::U128 => return Err(ErrorKind::missing("128-bit integers")),
            },
            BuiltinType::Float(BuiltinFloat::F32) => Scalar::F32,
            BuiltinType::Float(BuiltinFloat::F64) => Scalar::F64,
        }))
    }

    pub(crate) fn ty(self) -> Ty {
        match self {
            Scalar::Int { bits: 64, .. } => Ty::I64,
            Scalar::Int { .. } | Scalar::Bool => Ty::I32,
            Scalar::F32 => Ty::F32,
            Scalar::F64 => Ty::F64,
        }
    }

    pub(crate) fn signed(self) -> bool {
        matches!(self, Scalar::Int { signed: true, .. })
    }
}

/// What a data type is of, with every mutability in its type arguments
/// replaced by a parameter of the declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DataKey<'db> {
    Adt(AdtId<'db>, Box<[MonoTy<'db>]>),
    Tuple(Box<[MonoTy<'db>]>),
}

/// The data types of a program, made as their types are first met.
#[derive(Default)]
pub(crate) struct Types<'db> {
    keys: HashMap<DataKey<'db>, DataId>,
    pub decls: BTreeMap<DataId, DataDecl>,
    /// The row of every closure and continuation type: `Any` in a program
    /// with effects, since Bolt doesn't give Conduit its rows yet.
    pub row: Row,
}

impl<'db> Types<'db> {
    /// The Conduit type of a value of type `t`, or `None` for one that is
    /// no values, like `()` and `!`.
    pub(crate) fn value(
        &mut self,
        db: &'db dyn Db,
        root: File,
        t: &MonoTy<'db>,
    ) -> Result<Option<Ty>, ErrorKind> {
        if let Some(s) = Scalar::of(t)? {
            return Ok(Some(s.ty()));
        }
        Ok(Some(match t {
            MonoTy::Builtin(_) => unreachable!("a scalar"),
            MonoTy::Never => return Ok(None),
            MonoTy::Tuple(elems, _) if elems.is_empty() => return Ok(None),
            MonoTy::Tuple(elems, own) => {
                let (args, extracted) = abstract_muts(elems, 0);
                let id = self.data(db, root, DataKey::Tuple(args))?;
                let margs = extracted.iter().map(|&m| mutability(m)).collect();
                Ty::Data(id, mutability(*own), margs, Vec::new())
            }
            MonoTy::Adt(AdtId::Enum(e), ..) if is_fieldless(db, *e) => Ty::I32,
            MonoTy::Adt(adt @ AdtId::Enum(_), args, _, _) => {
                let (args, extracted) = abstract_muts(args, 0);
                let id = self.data(db, root, DataKey::Adt(*adt, args))?;
                let own = held_mutability(&extracted)?;
                Ty::Data(id, own, extracted.iter().map(|&m| mutability(m)).collect(), Vec::new())
            }
            // A continuation, `Cont[B, U, R]`: resuming it with a `B` gives
            // a `U`.
            MonoTy::Adt(AdtId::Struct(s), args, ..) if Some(*s) == cont_struct(db, root) => {
                let (Some(b), Some(u)) = (args.first(), args.get(1)) else {
                    return Err(ErrorKind::unsupported("this continuation"));
                };
                let params = self.value(db, root, b)?.into_iter().collect();
                let results = self.value(db, root, u)?.into_iter().collect();
                Ty::Cont(Box::new(Sig { params, results, row: self.row.clone() }))
            }
            MonoTy::Adt(adt @ AdtId::Struct(_), args, muts, own) => {
                let (args, extracted) = abstract_muts(args, muts.len() as u32);
                let id = self.data(db, root, DataKey::Adt(*adt, args))?;
                let margs = muts.iter().chain(&extracted).map(|&m| mutability(m)).collect();
                Ty::Data(id, mutability(*own), margs, Vec::new())
            }
            MonoTy::Array(m, elem) => match self.value(db, root, elem)? {
                Some(elem) => Ty::Array(Box::new(elem), mutability(*m)),
                None => return Err(ErrorKind::unsupported("an array of a type with no values")),
            },
            MonoTy::Fn(params, ret, m) => {
                let mut sig = Sig::new(Vec::new(), Vec::new());
                for p in params {
                    sig.params.extend(self.value(db, root, p)?);
                }
                sig.results.extend(self.value(db, root, ret)?);
                sig.row = self.row.clone();
                Ty::Fn(Box::new(sig), mutability(*m))
            }
            MonoTy::Erased(_) | MonoTy::ErasedAssoc(..) => {
                return Err(ErrorKind::unsupported("erased types"));
            }
        }))
    }

    /// The data type of `key`, declared if it's new.
    fn data(
        &mut self,
        db: &'db dyn Db,
        root: File,
        key: DataKey<'db>,
    ) -> Result<DataId, ErrorKind> {
        if let Some(&id) = self.keys.get(&key) {
            return Ok(id);
        }
        // From 0, in the order they're met, so that the text format can
        // write each by its name alone.
        let id = DataId(self.keys.len() as u64);
        // Known before its fields are, for a type that holds itself.
        self.keys.insert(key.clone(), id);
        let result = self.declare(db, root, &key);
        match result {
            Ok(decl) => {
                self.decls.insert(id, decl);
                Ok(id)
            }
            Err(e) => {
                self.keys.remove(&key);
                Err(e)
            }
        }
    }

    fn declare(
        &mut self,
        db: &'db dyn Db,
        root: File,
        key: &DataKey<'db>,
    ) -> Result<DataDecl, ErrorKind> {
        let fields_of = |this: &mut Self, tys: Vec<MonoTy<'db>>| -> Result<CtorDecl, ErrorKind> {
            let mut fields = Vec::new();
            for t in &tys {
                match this.value(db, root, t)? {
                    Some(t) => fields.push(t),
                    None => {
                        return Err(ErrorKind::unsupported("a field of a type with no values"));
                    }
                }
            }
            Ok(CtorDecl { fields })
        };
        let unknown = |_| ErrorKind::unsupported("a field whose type isn't known");
        Ok(match key {
            DataKey::Tuple(elems) => {
                let params = count_params(elems);
                DataDecl {
                    // Bolt doesn't give Conduit its effects yet.
                    effect_params: 0,
                    params,
                    ctors: vec![fields_of(self, elems.to_vec())?],
                    debug_name: Some(data_name(db, "Tuple", elems).into()),
                }
            }
            DataKey::Adt(AdtId::Struct(s), args) => {
                let declared = boltlang_hir_ty::lower::named_mutabilities(
                    db,
                    boltlang_hir_def::resolver::GenericDefId::Struct(*s),
                )
                .len() as u32;
                let muts: Vec<MonoMut> = (0..declared).map(MonoMut::Param).collect();
                let tys =
                    field_types(db, root, VariantDef::Struct(*s), args, &muts, MonoMut::Param(OWN))
                        .map_err(unknown)?;
                DataDecl {
                    // Bolt doesn't give Conduit its effects yet.
                    effect_params: 0,
                    params: declared + count_params(args),
                    ctors: vec![fields_of(self, tys)?],
                    debug_name: s
                        .item(db)
                        .name
                        .as_ref()
                        .map(|n| data_name(db, n.as_str(), args).into()),
                }
            }
            DataKey::Adt(AdtId::Enum(e), args) => {
                let variants = enum_signature(db, *e).variants.len() as u32;
                let mut ctors = Vec::new();
                for index in 0..variants {
                    let v = EnumVariantId { parent: *e, index };
                    let tys =
                        field_types(db, root, VariantDef::Variant(v), args, &[], MonoMut::Imm)
                            .map_err(unknown)?;
                    ctors.push(fields_of(self, tys)?);
                }
                DataDecl {
                    // Bolt doesn't give Conduit its effects yet.
                    effect_params: 0,
                    params: count_params(args),
                    ctors,
                    debug_name: e
                        .item(db)
                        .name
                        .as_ref()
                        .map(|n| data_name(db, n.as_str(), args).into()),
                }
            }
        })
    }
}

/// The name of a data type, like `Option_i32` for `Option[i32]`.
fn data_name(db: &dyn Db, name: &str, args: &[MonoTy<'_>]) -> String {
    if args.is_empty() {
        return crate::ident(name);
    }
    let args: Vec<String> = args.iter().map(|a| a.display(db).to_string()).collect();
    crate::ident(&format!("{name}[{}]", args.join(", ")))
}

/// Whether no variant of the enum has fields, so that a value is the index
/// of its variant, an `i32`. An enum without variants is one too.
pub(crate) fn is_fieldless(db: &dyn Db, id: EnumId<'_>) -> bool {
    enum_signature(db, id).variants.iter().all(|v| matches!(v.fields, FieldList::Unit))
}

/// A Bolt mutability as Conduit's: an instance's parameters are numbered
/// from 0 and Conduit's from 1.
pub(crate) fn mutability(m: MonoMut) -> Mutability {
    match m {
        MonoMut::Imm => Mutability::Imm,
        MonoMut::Mut => Mutability::Mut,
        MonoMut::Param(OWN) => Mutability::Inherit,
        MonoMut::Param(i) => Mutability::Param(i + 1),
    }
}

/// `tys` with each mutability in them replaced by a parameter, numbered
/// from `first`, and the mutabilities that were replaced, in order.
fn abstract_muts<'db>(tys: &[MonoTy<'db>], first: u32) -> (Box<[MonoTy<'db>]>, Vec<MonoMut>) {
    let mut extracted = Vec::new();
    let mut next = |m: &MonoMut| {
        extracted.push(*m);
        MonoMut::Param(first + extracted.len() as u32 - 1)
    };
    fn go<'db>(t: &MonoTy<'db>, next: &mut dyn FnMut(&MonoMut) -> MonoMut) -> MonoTy<'db> {
        match t {
            MonoTy::Adt(adt, args, muts, own) => {
                let args = args.iter().map(|a| go(a, next)).collect();
                let muts = muts.iter().map(&mut *next).collect();
                // An enum's own mutability is always `imm`, and what it
                // holds decides its mutability in Conduit.
                let own = if matches!(adt, AdtId::Enum(_)) { *own } else { next(own) };
                MonoTy::Adt(*adt, args, muts, own)
            }
            // `()` has no values, so no mutability either.
            MonoTy::Tuple(elems, _) if elems.is_empty() => t.clone(),
            MonoTy::Tuple(elems, own) => {
                let elems = elems.iter().map(|a| go(a, next)).collect();
                MonoTy::Tuple(elems, next(own))
            }
            MonoTy::Array(m, elem) => {
                let m = next(m);
                MonoTy::Array(m, Box::new(go(elem, next)))
            }
            MonoTy::Fn(params, ret, m) => {
                let params = params.iter().map(|a| go(a, next)).collect();
                let ret = Box::new(go(ret, next));
                MonoTy::Fn(params, ret, next(m))
            }
            MonoTy::ErasedAssoc(alias, args) => {
                MonoTy::ErasedAssoc(*alias, args.iter().map(|a| go(a, next)).collect())
            }
            MonoTy::Builtin(_) | MonoTy::Never | MonoTy::Erased(_) => t.clone(),
        }
    }
    let out = tys.iter().map(|t| go(t, &mut next)).collect();
    (out, extracted)
}

/// How many parameters [`abstract_muts`] makes in types.
fn count_params(tys: &[MonoTy<'_>]) -> u32 {
    abstract_muts(tys, 0).1.len() as u32
}

/// The mutability of an enum that holds values with these
/// mutabilities: the most any of them can be.
fn held_mutability(held: &[MonoMut]) -> Result<Mutability, ErrorKind> {
    if held.contains(&MonoMut::Mut) {
        return Ok(Mutability::Mut);
    }
    let mut params: Vec<u32> = held
        .iter()
        .filter_map(|m| match m {
            MonoMut::Param(i) => Some(*i),
            _ => None,
        })
        .collect();
    params.sort();
    params.dedup();
    match params[..] {
        [] => Ok(Mutability::Imm),
        [p] => Ok(mutability(MonoMut::Param(p))),
        _ => Err(ErrorKind::missing(
            "an object whose mutability is the larger of two parameters, for an enum or a \
             tuple that holds values of both",
        )),
    }
}
