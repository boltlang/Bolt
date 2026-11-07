//! Running a lowered program with Conduit's evaluator, for `bolt run` and
//! `bolt test`. The host provides the functions of `#[link(emu, ..)]` (see
//! [`crate::emu`]), with the process's stdout, stderr and files. Calling an
//! extern linked for another target stops the program with an error.

use conduit_eval::{Arrays, ErrorKind, Host, HostError, Machine, Value};
use conduit_ir::{Extern, FuncId};

use crate::{Lowered, emu::Emu};

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
/// given, and no arguments. What it prints goes to stdout and stderr.
pub fn run(program: &Lowered, f: FuncId, fuel: Option<u64>) -> Result<(), RunError> {
    run_with_args(program, f, fuel, Vec::new())
}

/// [`run`], with `args` as the program's arguments.
pub fn run_with_args(
    program: &Lowered,
    f: FuncId,
    fuel: Option<u64>,
    args: Vec<String>,
) -> Result<(), RunError> {
    let mut machine = Machine::new(&program.module, Stdio(Emu::new(args)));
    machine.fuel = fuel;
    let Err(e) = machine.call(f, Vec::new()) else { return Ok(()) };
    let message = match &e.kind {
        // `exit(0)` ends the program as its end does.
        ErrorKind::Exit(0) => return Ok(()),
        ErrorKind::Exit(code) => format!("the program exited with code {code}"),
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
    Err(RunError { message })
}

/// The evaluator's host, [`Emu`].
struct Stdio(Emu);

impl Host for Stdio {
    fn call(
        &mut self,
        name: &str,
        decl: &Extern,
        args: &[Value],
        arrays: &mut Arrays<'_, '_>,
    ) -> Result<Vec<Value>, HostError> {
        self.0.call(name, decl, args, arrays)
    }
}
