//! Running a lowered program with Conduit's evaluator, for `bolt run` and
//! `bolt test`. The host provides WASI's functions (see [`crate::wasi`]),
//! writing to the process's stdout and stderr. Externs of other modules
//! have no host here: calling one stops the program with an error.

use std::io::{self, Write};

use conduit_eval::{Arrays, ErrorKind, Host, Machine, Value};
use conduit_ir::{Extern, ExternId, FuncId};

use crate::{
    Lowered,
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

/// A host with WASI, whose files 1 and 2 are stdout and stderr.
#[derive(Default)]
struct Stdio {
    wasi: Wasi,
}

/// Write `bytes` to the file `fd`: 1 is stdout and 2 is stderr.
fn write(fd: i32, bytes: &[u8]) -> io::Result<()> {
    match fd {
        1 => {
            let mut out = io::stdout().lock();
            out.write_all(bytes)?;
            out.flush()
        }
        2 => {
            let mut err = io::stderr().lock();
            err.write_all(bytes)?;
            err.flush()
        }
        _ => Ok(()),
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
        if &*decl.module != wasi::MODULE {
            return Err(format!("there's no host function `{}.{}` here", decl.module, decl.name));
        }
        let (results, output) = self.wasi.call(decl, args, arrays)?;
        if let Output::Write { fd, bytes } = output {
            write(fd, &bytes).map_err(|e| e.to_string())?;
        }
        Ok(results)
    }
}
