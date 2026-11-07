//! Running a lowered program with Conduit's evaluator, for `bolt run` and
//! `bolt test`. The host provides the module `bolt`'s `write_file` and
//! `flush` (see [`crate::host`]), writing to the process's stdout and
//! stderr, and WASI's functions (see [`crate::wasi`]). Externs of other
//! modules have no host here: calling one stops the program with an
//! error.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use conduit_eval::{Arrays, ErrorKind, Host, Machine, Value};
use conduit_ir::{Extern, ExternId, FuncId};

use crate::{
    Lowered,
    host::{STDERR, STDOUT},
    wasi::{self, Output, Wasi},
};

/// Why a run stopped before its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunError {
    pub message: String,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Run the function `f` of `program`, with at most `fuel` steps if it's
/// given. What it prints goes to stdout and stderr.
pub fn run(program: &Lowered, f: FuncId, fuel: Option<u64>) -> Result<(), RunError> {
    let mut machine = Machine::new(&program.module, Stdio::default());
    machine.fuel = fuel;
    let result = machine.call(f, Vec::new());
    // What wasn't flushed is written anyway, like at the end of a process.
    let _ = machine.host.flush_all();
    // `proc_exit(0)` ends the program as its end does.
    match machine.host.wasi.exit {
        Some(0) => return Ok(()),
        Some(code) => {
            return Err(RunError { message: format!("the program exited with code {code}") });
        }
        None => {}
    }
    result.map(|_| ()).map_err(|e| {
        let message = match &e.kind {
            ErrorKind::Trap => "the program stopped with an error".to_string(),
            ErrorKind::OutOfFuel => "the program ran out of fuel".to_string(),
            // Like `fn3: out of bounds`, with the function's name instead.
            _ => {
                let text = e.to_string();
                let named = e.func.and_then(|f| {
                    let rest = text.strip_prefix(&format!("{f}: "))?;
                    Some(format!("{rest} (in {})", program.names.get(&f)?))
                });
                named.unwrap_or(text)
            }
        };
        RunError { message }
    })
}

/// A host that writes files 1 and 2 to stdout and stderr, and has WASI.
#[derive(Default)]
struct Stdio {
    /// What each file has waiting, by its number.
    files: BTreeMap<i32, Vec<u8>>,
    wasi: Wasi,
}

impl Stdio {
    fn flush(&mut self, file: i32) -> io::Result<()> {
        let bytes = self.files.remove(&file).unwrap_or_default();
        match file {
            STDOUT => {
                let mut out = io::stdout().lock();
                out.write_all(&bytes)?;
                out.flush()
            }
            STDERR => {
                let mut err = io::stderr().lock();
                err.write_all(&bytes)?;
                err.flush()
            }
            _ => Ok(()),
        }
    }

    fn flush_all(&mut self) -> io::Result<()> {
        let files: Vec<i32> = self.files.keys().copied().collect();
        for file in files {
            self.flush(file)?;
        }
        Ok(())
    }
}

impl Host for Stdio {
    fn call(
        &mut self,
        _: ExternId,
        decl: &Extern,
        args: &[Value],
        arrays: &mut Arrays<'_, '_>,
    ) -> Result<Vec<Value>, String> {
        if &*decl.module == wasi::MODULE {
            let (results, output) = self.wasi.call(decl, args, arrays)?;
            // In order with what `print` wrote before it.
            if let Output::Write { fd, bytes } = output {
                self.files.entry(fd).or_default().extend(bytes);
                self.flush(fd).map_err(|e| e.to_string())?;
            }
            return Ok(results);
        }
        match (&*decl.module, &*decl.name, args) {
            ("bolt", "write_file", [Value::I32(file), Value::I32(byte)]) => {
                self.files.entry(*file).or_default().push(*byte as u8);
                Ok(Vec::new())
            }
            ("bolt", "flush", [Value::I32(file)]) => {
                self.flush(*file).map_err(|e| e.to_string())?;
                Ok(Vec::new())
            }
            (module, name, _) => Err(format!("there's no host function `{module}.{name}` here")),
        }
    }
}
