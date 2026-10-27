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
    hir::{Expr, Statement},
    ids::EnumVariantId,
};
use boltlang_hir_ty::{AdtId, Ty, builtin::Builtin, infer_body};
use wasmi::{Caller, Config, Engine, Extern, Linker, Module, Store, TrapCode, Val};

use crate::lower::Scalar;

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
            Builtin::ArrayLen => unreachable!("not in `Builtin::HOST`"),
        };
        result.expect("each builtin is defined once");
    }
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
    let engine = engine(fuel);
    let module = Module::new(&engine, bytes).map_err(|e| RunError::Invalid(e.to_string()))?;
    for import in module.imports() {
        if import.module() != "bolt" || !Builtin::HOST.iter().any(|b| b.name() == import.name()) {
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
    let memory = instance.get_memory(&store, "memory").map_or(&[][..], |m| m.data(&store));
    match (entry_ty(db, file), results.pop()) {
        (Some(ty), Some(val)) => to_value(db, &ty, val, memory),
        _ => Ok(Value::unit()),
    }
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

/// A value that compiled code gave for `ty`, as the evaluator's value.
fn to_value<'db>(db: &'db dyn Db, ty: &Ty<'db>, val: Val, memory: &[u8]) -> Result<Value<'db>, RunError> {
    let unexpected = || RunError::Invalid(format!("`main` gave {val:?} for a value of type `{}`", ty.display(db)));
    let value = match (Scalar::of(db, ty).ok_or_else(unexpected)?, &val) {
        (Scalar::Bool, Val::I32(n)) => Value::Bool(*n != 0),
        (Scalar::Char, Val::I32(n)) => Value::Char(char::from_u32(*n as u32).ok_or_else(unexpected)?),
        (Scalar::Str, Val::I32(address)) => Value::str(&read_str(memory, *address).ok_or_else(unexpected)?),
        (Scalar::Enum, Val::I32(index)) => {
            let Ty::Adt(AdtId::Enum(parent), ..) = ty else { return Err(unexpected()) };
            let variant = EnumVariantId { parent: *parent, index: *index as u32 };
            Value::adt(Ctor::Variant(variant), Vec::new())
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
