//! Compiling a lowered program to a WebAssembly module, for `bolt build`,
//! with Conduit's Wasm backend, which puts objects in linear memory.
//!
//! The module exports the entry file's top-level statements as `main`, and
//! imports what the program asks of its host: the module `bolt`'s
//! `write_file` and `flush` (see [`crate::host`]), and the functions of its
//! `extern` blocks, from the modules they name.
//!
//! Before it's emitted, the program is optimized, unless asked not to be,
//! and its reference counts are made explicit, so that memory is freed,
//! with the cycles that counts alone can't free collected by trial
//! deletion. A handler whose clauses don't all resume at their end or
//! never resume is lowered by evidence passing first, since that has to
//! come before the counts.

use conduit_ir::{FuncId, Module, clause};
use conduit_wasm::{Options, emit_module_with};

use crate::Lowered;

/// The Wasm module of `program`, optimized if `optimize` says so, which
/// exports the entry file's top-level statements as `main`.
pub fn build(program: &Lowered, optimize: bool) -> Result<Vec<u8>, String> {
    build_exporting(program, optimize, &[(program.entry, "main")])
}

/// The Wasm module of `program`, which exports `exports`, each a function
/// and its name, like the tests for `bolt test --wasm`.
pub fn build_exporting(
    program: &Lowered,
    optimize: bool,
    exports: &[(FuncId, &str)],
) -> Result<Vec<u8>, String> {
    let mut m = program.module.clone();
    if optimize {
        conduit_opt::optimize_module(&mut m);
    }
    if !direct_handlers(&m) {
        conduit_opt::effects::lower_effects(&mut m)
            .map_err(|e| format!("lowering effects: {e}"))?;
    }
    conduit_opt::ownership::insert_counts(&mut m)
        .map_err(|e| format!("making reference counts explicit: {e}"))?;
    let options = Options { stack_switching: false, counts: true, cycles: true };
    emit_module_with(&m, exports, &options).map_err(|e| e.to_string())
}

/// Whether each clause of each handler of `m` resumes at its end or never
/// resumes, so that Conduit's Wasm backend lowers it directly.
fn direct_handlers(m: &Module) -> bool {
    m.handlers.values().all(|h| {
        h.ops.iter().all(|c| {
            m.funcs
                .get(c)
                .is_some_and(|f| clause::plain_call(f).is_some() || clause::aborting(f).is_some())
        })
    })
}
