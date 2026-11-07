//! What compiled code asks of its host, in the module `bolt`: two
//! functions on integers, which any host can provide, whether Conduit's
//! evaluator, a Wasm engine or JavaScript, without reading Bolt's objects.
//!
//! - `write_file(file: i32, byte: i32)` adds a byte to what the file has
//!   waiting: 1 is stdout and 2 is stderr.
//! - `flush(file: i32)` writes out what the file has waiting.
//!
//! `print(text)` is a function of the module, `bolt_print`, that writes
//! each byte of `text`, then a newline, to stdout, and flushes it. A
//! `match` that no arm takes calls `bolt_unmatched`, which says so on
//! stderr, and then traps. Once Conduit has linear memory, a whole string
//! can be handed over at once instead.

use conduit_ir::{
    Atom, CtorId, Extern, ExternId, Func, FuncId, Lit, Module, Prim, Sig, Sign, Ty,
    build::FuncBuilder,
};

use crate::ErrorKind;

/// The files of `write_file` and `flush`.
pub(crate) const STDOUT: i32 = 1;
pub(crate) const STDERR: i32 = 2;

/// The host functions, once declared.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Io {
    pub write_file: ExternId,
    pub flush: ExternId,
}

/// Declare `write_file` and `flush`, unless they are already.
pub(crate) fn declare_io(module: &mut Module, io: &mut Option<Io>) -> Io {
    if let Some(io) = *io {
        return io;
    }
    let mut declare = |name: &str, params: Vec<Ty>| {
        let id = ExternId(module.externs.len() as u64);
        let sig = Sig::new(params, Vec::new());
        let debug_name = Some(name.into());
        module.externs.insert(id, Extern { debug_name, ..Extern::new("bolt", name, sig) });
        id
    };
    let write_file = declare("write_file", vec![Ty::I32, Ty::I32]);
    let flush = declare("flush", vec![Ty::I32]);
    *io.insert(Io { write_file, flush })
}

/// The functions of the module that call the host, made once they're used.
#[derive(Debug, Default)]
pub(crate) struct Helpers {
    /// The type of the `String` that `print` takes, once something prints.
    pub string: Option<Ty>,
    /// `bolt_print`.
    pub print: Option<FuncId>,
    /// `bolt_unmatched`.
    pub unmatched: Option<FuncId>,
}

/// `bolt_print(text)`: each byte of the `String` `text`, whose one field is
/// its array of bytes, then a newline, written to stdout, which is then
/// flushed.
pub(crate) fn print_func(module: &Module, io: Io, string: &Ty) -> Result<Func, ErrorKind> {
    let Ty::Data(data, ..) = string else {
        return Err(ErrorKind::unsupported("a `String` that isn't a data type"));
    };
    let mut b = FuncBuilder::new(module, vec![string.clone()], Vec::new());
    let text = b.param(0);
    let bytes = b.get(CtorId { data: *data, index: 0 }, 0, text);
    let len = b.array_len(bytes);
    let stdout = Atom::Lit(Lit::I32(STDOUT));
    // for i in 0..len, then the newline.
    let next = b.join_rec(&[Ty::I32]);
    b.jump(&next, [Atom::Lit(Lit::I32(0))]);
    b.switch_to(next.block());
    let i = next.param(0);
    let more = b.prim(Prim::Lt(Sign::Signed), [Atom::Var(i), Atom::Var(len)]);
    let case = b.case(more);
    b.arm(case, 0);
    b.call_extern(io.write_file, [stdout.clone(), Atom::Lit(Lit::I32(i32::from(b'\n')))]);
    b.call_extern(io.flush, [stdout.clone()]);
    b.ret(());
    b.default(case);
    // The host takes a byte as an `i32`.
    let byte = b.array_get(bytes, i);
    let byte = b.convert(Ty::I32, Sign::Unsigned, byte);
    b.call_extern(io.write_file, [stdout, Atom::Var(byte)]);
    let i = b.prim(Prim::Add, [Atom::Var(i), Atom::Lit(Lit::I32(1))]);
    b.jump(&next, [Atom::Var(i)]);
    finish(b)
}

/// `bolt_unmatched()`: a line on stderr saying that no arm of a `match`
/// matched. The `match` traps after it.
pub(crate) fn unmatched_func(module: &Module, io: Io) -> Result<Func, ErrorKind> {
    let mut b = FuncBuilder::new(module, Vec::new(), Vec::new());
    let stderr = Atom::Lit(Lit::I32(STDERR));
    for byte in "error: no arm of a match matched\n".bytes() {
        b.call_extern(io.write_file, [stderr.clone(), Atom::Lit(Lit::I32(i32::from(byte)))]);
    }
    b.call_extern(io.flush, [stderr]);
    b.ret(());
    finish(b)
}

fn finish(b: FuncBuilder<'_, Module>) -> Result<Func, ErrorKind> {
    b.finish().map_err(|es| {
        ErrorKind::Invalid(es.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "))
    })
}
