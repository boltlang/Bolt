//! Running a compiled module with [wasmi], for `bolt run --wasm`.
//!
//! The host provides the builtins of the module `bolt` through an
//! evaluator [`Host`], so that compiled code prints where the evaluator
//! would. The result of `main` becomes a [`Value`] of the type that the
//! checker inferred for it, so it can be shown like the evaluator's.

use std::fmt;

use boltlang_base_db::{Db, File};
use boltlang_eval::{Ctor, ErrorKind, Host, Value};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body},
    hir::{Expr, Mutability, Statement},
    ids::EnumVariantId,
};
use boltlang_hir_ty::{AdtId, Ty, builtin::Builtin, infer_body};
use wasmi::{Caller, Config, Engine, Extern, Linker, Module, Store, TrapCode, Val};

use crate::{
    ir::{Failure, Narrow, ValTy},
    layout::{self, Repr},
    lower::Scalar,
};

/// Why running a module failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// What the evaluator would have failed with too, like a panic, a
    /// division by zero or running out of fuel.
    Eval(ErrorKind),
    /// A trap that the evaluator has no error for, with its message.
    Trap(&'static str),
    /// An import that only a host which embeds the module can provide, like
    /// a function of an `extern` block.
    MissingImport { module: String, name: String },
    /// A module that wasmi couldn't load or start.
    Invalid(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Eval(kind) => write!(f, "{kind}"),
            RunError::Trap(message) => write!(f, "the program trapped: {message}"),
            RunError::MissingImport { module, name } => {
                write!(f, "`{module}.{name}` isn't available when running the compiled module")
            }
            RunError::Invalid(message) => write!(f, "the compiled module couldn't be run: {message}"),
        }
    }
}

/// What host functions can reach.
pub(crate) struct State<H> {
    pub host: H,
    /// Why a host function failed. Wasmi only passes on that it did.
    failure: Option<RunError>,
}

impl<H> State<H> {
    pub fn new(host: H) -> Self {
        State { host, failure: None }
    }
}

/// An engine that counts fuel when `fuel` is given.
pub(crate) fn engine(fuel: Option<u64>) -> Engine {
    let mut config = Config::default();
    config.consume_fuel(fuel.is_some());
    Engine::new(&config)
}

/// A linker that provides the builtins of the module `bolt`.
pub(crate) fn linker<H: Host + 'static>(engine: &Engine) -> Linker<State<H>> {
    let mut linker = Linker::new(engine);
    for builtin in Builtin::HOST {
        let result = match builtin {
            Builtin::Print => linker.func_wrap("bolt", "print", |mut caller: Caller<'_, State<H>>, text: i32| {
                let result = match read_str(memory_of(&caller), text) {
                    Some(text) => caller.data_mut().host.print(&text).map_err(RunError::Eval),
                    None => Err(RunError::Trap(TrapCode::MemoryOutOfBounds.trap_message())),
                };
                result.map_err(|error| fail(&mut caller, error))
            }),
            Builtin::Panic => linker.func_wrap(
                "bolt",
                "panic",
                |mut caller: Caller<'_, State<H>>, message: i32| -> Result<(), wasmi::Error> {
                    let error = match read_str(memory_of(&caller), message) {
                        Some(message) => RunError::Eval(ErrorKind::Panic(message)),
                        None => RunError::Trap(TrapCode::MemoryOutOfBounds.trap_message()),
                    };
                    Err(fail(&mut caller, error))
                },
            ),
            Builtin::ArrayLen
            | Builtin::ArrayAlloc
            | Builtin::ArrayGet
            | Builtin::ArraySet
            | Builtin::ArrayCopy
            | Builtin::ArrayFreeze
            | Builtin::Prim(..) => {
                unreachable!("not in `Builtin::HOST`")
            }
        };
        result.expect("each builtin is defined once");
    }
    // The errors that compiled code stops with, as the evaluator's.
    linker
        .func_wrap(
            "bolt",
            Failure::IndexOutOfBounds.name(),
            |mut caller: Caller<'_, State<H>>, index: i32, len: i32| -> Result<(), wasmi::Error> {
                let kind = ErrorKind::IndexOutOfBounds { index: index as i128, len: len as u32 as usize };
                Err(fail(&mut caller, RunError::Eval(kind)))
            },
        )
        .expect("each failure is defined once");
    linker
        .func_wrap(
            "bolt",
            Failure::RangeOutOfBounds.name(),
            |mut caller: Caller<'_, State<H>>, start: i32, count: i32, len: i32| -> Result<(), wasmi::Error> {
                let (start, end) = (start as i128, start as i128 + count as i128);
                let kind = ErrorKind::RangeOutOfBounds { start, end, len: len as u32 as usize };
                Err(fail(&mut caller, RunError::Eval(kind)))
            },
        )
        .expect("each failure is defined once");
    linker
        .func_wrap("bolt", Failure::Overflow.name(), |mut caller: Caller<'_, State<H>>| -> Result<(), wasmi::Error> {
            Err(fail(&mut caller, RunError::Eval(ErrorKind::Overflow)))
        })
        .expect("each failure is defined once");
    linker
}

/// Keep why a host function failed, and give the error that stops the module.
fn fail<H>(caller: &mut Caller<'_, State<H>>, error: RunError) -> wasmi::Error {
    let stop = wasmi::Error::new(error.to_string());
    caller.data_mut().failure = Some(error);
    stop
}

fn memory_of<'a, H>(caller: &'a Caller<'_, State<H>>) -> &'a [u8] {
    match caller.get_export("memory").and_then(Extern::into_memory) {
        Some(memory) => memory.data(caller),
        None => &[],
    }
}

/// The string at `address` in `memory`: its length as a little-endian `u32`,
/// then its UTF-8 bytes.
pub(crate) fn read_str(memory: &[u8], address: i32) -> Option<String> {
    let start = address as u32 as usize;
    let len = u32::from_le_bytes(memory.get(start..start.checked_add(4)?)?.try_into().ok()?) as usize;
    let bytes = memory.get(start + 4..(start + 4).checked_add(len)?)?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// The error a call stopped with: why a host function failed, or the trap.
pub(crate) fn run_error<H>(store: &mut Store<State<H>>, error: wasmi::Error) -> RunError {
    if let Some(failure) = store.data_mut().failure.take() {
        return failure;
    }
    match error.as_trap_code() {
        Some(TrapCode::OutOfFuel) => RunError::Eval(ErrorKind::OutOfFuel),
        Some(TrapCode::IntegerDivisionByZero) => RunError::Eval(ErrorKind::DivisionByZero),
        // Like `i32::MIN / -1`.
        Some(TrapCode::IntegerOverflow) => RunError::Eval(ErrorKind::Overflow),
        Some(TrapCode::StackOverflow) => RunError::Eval(ErrorKind::StackOverflow),
        Some(code) => RunError::Trap(code.trap_message()),
        None => RunError::Invalid(error.to_string()),
    }
}

/// Run the module that [`link`](crate::link) compiled from `file`: call its
/// `main`, with `host` for the builtins, and give the value it returns.
/// With `fuel`, stop with [`ErrorKind::OutOfFuel`] after about that many
/// instructions.
pub fn run<'db, H: Host + 'static>(
    db: &'db dyn Db,
    file: File,
    bytes: &[u8],
    host: H,
    fuel: Option<u64>,
) -> Result<Value<'db>, RunError> {
    let (results, memory) = call_main(bytes, host, fuel)?;
    match entry_ty(db, file) {
        Some(ty) => to_value(db, &ty, results.into_iter().last(), &memory),
        None => Ok(Value::unit()),
    }
}

/// Run the module that [`link_function`](crate::link_function) compiled
/// from a test, like [`run`], without its result: a test passes when it
/// returns.
pub fn run_test<H: Host + 'static>(bytes: &[u8], host: H, fuel: Option<u64>) -> Result<(), RunError> {
    call_main(bytes, host, fuel).map(|_| ())
}

/// Call the `main` of the module `bytes`, and give what it returns and the
/// memory after.
fn call_main<H: Host + 'static>(bytes: &[u8], host: H, fuel: Option<u64>) -> Result<(Vec<Val>, Vec<u8>), RunError> {
    let engine = engine(fuel);
    let module = Module::new(&engine, bytes).map_err(|e| RunError::Invalid(e.to_string()))?;
    for import in module.imports() {
        let known = Builtin::HOST.iter().any(|b| b.name() == import.name())
            || Failure::ALL.iter().any(|f| f.name() == import.name());
        if import.module() != "bolt" || !known {
            return Err(RunError::MissingImport { module: import.module().into(), name: import.name().into() });
        }
    }
    let mut store = Store::new(&engine, State::new(host));
    if let Some(fuel) = fuel {
        store.set_fuel(fuel).expect("the engine counts fuel");
    }
    let instance = match linker(&engine).instantiate_and_start(&mut store, &module) {
        Ok(instance) => instance,
        Err(error) => return Err(run_error(&mut store, error)),
    };
    let main = instance.get_func(&store, "main").ok_or_else(|| RunError::Invalid("no export `main`".into()))?;
    let mut results: Vec<Val> =
        main.ty(&store).results().iter().map(|ty| Val::default_for_ty(*ty)).collect();
    if let Err(error) = main.call(&mut store, &[], &mut results) {
        return Err(run_error(&mut store, error));
    }
    let memory = instance.get_memory(&store, "memory").map_or(Vec::new(), |m| m.data(&store).to_vec());
    Ok((results, memory))
}

/// The type of the last top-level statement of `file`, which `main` returns
/// when it's an expression.
fn entry_ty<'db>(db: &'db dyn Db, file: File) -> Option<Ty<'db>> {
    let def = DefWithBodyId::new(db, DefWithBody::File(file));
    let body = body(db, def);
    let Expr::Block { statements, .. } = &body[body.body_expr] else { unreachable!("a file's body is a block") };
    match statements.last()? {
        Statement::Expr { expr, .. } => Some(infer_body(db, def).expr_ty(*expr).clone()),
        _ => None,
    }
}

/// A value that compiled code gave for `ty`, as the evaluator's value:
/// `None` for a type without values. Structs and tuples are read from
/// `memory`.
fn to_value<'db>(db: &'db dyn Db, ty: &Ty<'db>, val: Option<Val>, memory: &[u8]) -> Result<Value<'db>, RunError> {
    let unexpected = || RunError::Invalid(format!("`main` gave {val:?} for a value of type `{}`", ty.display(db)));
    let repr = layout::repr(db, ty).ok_or_else(unexpected)?;
    let scalar = match (repr, &val) {
        (Repr::Unit, None) => return Ok(Value::unit()),
        (Repr::Scalar(scalar), Some(_)) => scalar,
        (Repr::Newtype(field_ty), _) => {
            let Ty::Adt(AdtId::Struct(id), ..) = ty else { return Err(unexpected()) };
            let fields = layout::fields(db, ty).ok_or_else(unexpected)?;
            let value = to_value(db, &field_ty, val.clone(), memory)?;
            let name = fields.into_iter().next().ok_or_else(unexpected)?.0;
            return Ok(Value::adt(Ctor::Struct(*id), Mutability::Imm, vec![(name, value)]));
        }
        (Repr::Boxed, Some(Val::I32(address))) => {
            let layout = layout::layout(db, ty).ok_or_else(unexpected)?;
            let mut fields = Vec::new();
            for slot in &layout.slots {
                let field = match slot.val_ty {
                    Some(val_ty) => {
                        let at = (*address as u32).checked_add(slot.offset).ok_or_else(unexpected)?;
                        Some(load(memory, at, val_ty).ok_or_else(unexpected)?)
                    }
                    None => None,
                };
                fields.push((slot.name.clone(), to_value(db, &slot.ty, field, memory)?));
            }
            return Ok(match ty {
                Ty::Tuple(_) => Value::Tuple(fields.into_iter().map(|(_, v)| v).collect()),
                Ty::Adt(AdtId::Struct(id), _, _, own) => {
                    let mutability =
                        if **own == Ty::Mutability(Mutability::Mut) { Mutability::Mut } else { Mutability::Imm };
                    Value::adt(Ctor::Struct(*id), mutability, fields)
                }
                _ => return Err(unexpected()),
            });
        }
        (Repr::Enum, Some(Val::I32(address))) => {
            let Ty::Adt(AdtId::Enum(parent), ..) = ty else { return Err(unexpected()) };
            let address = *address as u32;
            let index = match load(memory, address, ValTy::I32) {
                Some(Val::I32(index)) => index as u32,
                _ => return Err(unexpected()),
            };
            let variant = EnumVariantId { parent: *parent, index };
            let layout = layout::variant_layout(db, ty, variant).ok_or_else(unexpected)?;
            let mut fields = Vec::new();
            for slot in &layout.slots {
                let field = match slot.val_ty {
                    Some(val_ty) => Some(load(memory, address + slot.offset, val_ty).ok_or_else(unexpected)?),
                    None => None,
                };
                fields.push((slot.name.clone(), to_value(db, &slot.ty, field, memory)?));
            }
            return Ok(Value::adt(Ctor::Variant(variant), Mutability::Imm, fields));
        }
        (Repr::Array(elem_ty), Some(Val::I32(address))) => {
            let el = layout::element(db, &elem_ty).ok_or_else(unexpected)?;
            let address = *address as u32;
            let len = match load(memory, address, ValTy::I32) {
                Some(Val::I32(len)) => len as u32,
                _ => return Err(unexpected()),
            };
            let mut items = Vec::new();
            for i in 0..len {
                let at = i.checked_mul(el.size).and_then(|n| n.checked_add(address + el.start)).ok_or_else(unexpected)?;
                let item = match (el.narrow, el.val_ty) {
                    (Some(narrow), _) => Some(load_narrow(memory, at, narrow).ok_or_else(unexpected)?),
                    (None, Some(val_ty)) => Some(load(memory, at, val_ty).ok_or_else(unexpected)?),
                    (None, None) => None,
                };
                items.push(to_value(db, &elem_ty, item, memory)?);
            }
            let Ty::Array(mutability, _) = ty else { return Err(unexpected()) };
            let mutability =
                if **mutability == Ty::Mutability(Mutability::Mut) { Mutability::Mut } else { Mutability::Imm };
            return Ok(Value::array(mutability, items));
        }
        _ => return Err(unexpected()),
    };
    let val = val.clone().ok_or_else(unexpected)?;
    let value = match (scalar, &val) {
        (Scalar::Bool, Val::I32(n)) => Value::Bool(*n != 0),
        (Scalar::Enum, Val::I32(index)) => {
            let Ty::Adt(AdtId::Enum(parent), ..) = ty else { return Err(unexpected()) };
            let variant = EnumVariantId { parent: *parent, index: *index as u32 };
            Value::adt(Ctor::Variant(variant), Mutability::Imm, Vec::new())
        }
        (Scalar::Int { signed: true, .. }, Val::I32(n)) => Value::Int(*n as i128),
        (Scalar::Int { signed: false, .. }, Val::I32(n)) => Value::Int(*n as u32 as i128),
        (Scalar::Int { signed: true, .. }, Val::I64(n)) => Value::Int(*n as i128),
        (Scalar::Int { signed: false, .. }, Val::I64(n)) => Value::Int(*n as u64 as i128),
        (Scalar::F32, Val::F32(x)) => Value::Float(f32::from(*x) as f64),
        (Scalar::F64, Val::F64(x)) => Value::Float(f64::from(*x)),
        _ => return Err(unexpected()),
    };
    Ok(value)
}

/// The `i32` kept in fewer bytes at `address` in `memory`, as `narrow` says.
fn load_narrow(memory: &[u8], address: u32, narrow: Narrow) -> Option<Val> {
    let start = address as usize;
    let bytes = memory.get(start..start.checked_add(narrow.size() as usize)?)?;
    Some(Val::I32(match narrow {
        Narrow::U8 => bytes[0] as i32,
        Narrow::S8 => bytes[0] as i8 as i32,
        Narrow::U16 => u16::from_le_bytes(bytes.try_into().ok()?) as i32,
        Narrow::S16 => i16::from_le_bytes(bytes.try_into().ok()?) as i32,
    }))
}

/// The value of type `ty` at `address` in `memory`, little-endian.
fn load(memory: &[u8], address: u32, ty: ValTy) -> Option<Val> {
    let start = address as usize;
    let bytes = memory.get(start..start.checked_add(ty.size() as usize)?)?;
    Some(match ty {
        ValTy::I32 => Val::I32(i32::from_le_bytes(bytes.try_into().ok()?)),
        ValTy::I64 => Val::I64(i64::from_le_bytes(bytes.try_into().ok()?)),
        ValTy::F32 => Val::F32(f32::from_le_bytes(bytes.try_into().ok()?).into()),
        ValTy::F64 => Val::F64(f64::from_le_bytes(bytes.try_into().ok()?).into()),
    })
}
