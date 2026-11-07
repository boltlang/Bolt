//! WASI preview 1, the module `wasi_snapshot_preview1`, for the evaluator.
//!
//! `std` declares WASI's functions with `#[link(wasm, .., abi = flat)]`. In
//! Wasm the host is the engine's, like Node's or Wasmtime's. Conduit's
//! evaluator passes values instead of memory, so this is its host: a
//! function is given the arrays themselves, and writes the `mut` ones.
//!
//! The program writes to stdout and stderr and reads stdin, has no
//! arguments, environment or preopened directories, and reads the clocks
//! and random bytes of the machine it runs on. Everything else, like files
//! and sockets, answers `nosys`, as a host without them does.

use std::{
    io::Read,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use conduit_eval::{Arrays, Value};
use conduit_ir::{Extern, Ty};

/// WASI's module name.
pub(crate) const MODULE: &str = "wasi_snapshot_preview1";

/// `errno` values.
const SUCCESS: i64 = 0;
const BADF: i64 = 8;
const INVAL: i64 = 28;
const NOSYS: i64 = 52;

/// What WASI's functions keep between calls.
pub(crate) struct Wasi {
    start: Instant,
    /// The state of the random bytes, a xorshift.
    random: u64,
    /// The code `proc_exit` stopped the program with.
    pub exit: Option<u32>,
}

impl Default for Wasi {
    fn default() -> Self {
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        Wasi { start: Instant::now(), random: seed | 1, exit: None }
    }
}

/// What a call of a WASI function writes: bytes for a file.
pub(crate) enum Output {
    None,
    Write { fd: i32, bytes: Vec<u8> },
}

impl Wasi {
    /// Call the WASI function of `decl`: its results, and what it writes.
    /// An `Err` stops the program, as `proc_exit` does.
    pub(crate) fn call(
        &mut self,
        decl: &Extern,
        args: &[Value],
        arrays: &mut Arrays<'_, '_>,
    ) -> Result<(Vec<Value>, Output), String> {
        let int = |i: usize| -> i64 {
            match args.get(i) {
                Some(Value::I8(n)) => *n as u8 as i64,
                Some(Value::I16(n)) => *n as u16 as i64,
                Some(Value::I32(n)) => *n as u32 as i64,
                Some(Value::I64(n)) => *n,
                _ => 0,
            }
        };
        let mut output = Output::None;
        let results: Vec<i64> = match &*decl.name {
            "fd_write" => {
                let fd = int(0) as i32;
                let bytes = iovecs(arrays, &args[1]);
                if fd != 1 && fd != 2 {
                    vec![BADF, 0]
                } else {
                    let n = bytes.len() as i64;
                    output = Output::Write { fd, bytes };
                    vec![SUCCESS, n]
                }
            }
            "fd_read" if int(0) == 0 => {
                let targets: Vec<Value> =
                    arrays.get(&args[1]).map(<[Value]>::to_vec).unwrap_or_default();
                let mut total = 0;
                for target in targets {
                    let Some(buf) = arrays.get_mut(&target) else { continue };
                    let mut bytes = vec![0u8; buf.len()];
                    let n = std::io::stdin().read(&mut bytes).unwrap_or(0);
                    for (slot, b) in buf.iter_mut().zip(&bytes[..n]) {
                        *slot = Value::I8(*b as i8);
                    }
                    total += n;
                    if n < bytes.len() {
                        break;
                    }
                }
                vec![SUCCESS, total as i64]
            }
            "fd_read" => vec![BADF, 0],
            // No arguments and no environment.
            "args_sizes_get" | "environ_sizes_get" => vec![SUCCESS, 0, 0],
            "args_get" | "environ_get" => vec![SUCCESS],
            "clock_res_get" => vec![SUCCESS, 1],
            "clock_time_get" => match int(0) {
                0 => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                    vec![SUCCESS, now.as_nanos() as i64]
                }
                1..=3 => vec![SUCCESS, self.start.elapsed().as_nanos() as i64],
                _ => vec![INVAL, 0],
            },
            "random_get" => {
                if let Some(buf) = arrays.get_mut(&args[0]) {
                    for slot in buf.iter_mut() {
                        self.random ^= self.random << 13;
                        self.random ^= self.random >> 7;
                        self.random ^= self.random << 17;
                        *slot = Value::I8(self.random as i8);
                    }
                }
                vec![SUCCESS]
            }
            "sched_yield" => vec![SUCCESS],
            "proc_exit" => {
                self.exit = Some(int(0) as u32);
                return Err(format!("the program exited with code {}", int(0)));
            }
            // Files, sockets and the rest aren't there.
            _ => vec![NOSYS],
        };
        Ok((typed(&decl.sig.results, &results), output))
    }
}

/// The bytes of an iovec list, an array of `[i8]`, one after another.
fn iovecs(arrays: &Arrays<'_, '_>, list: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    for buf in arrays.get(list).unwrap_or_default() {
        for b in arrays.get(buf).unwrap_or_default() {
            if let Value::I8(b) = b {
                out.push(*b as u8);
            }
        }
    }
    out
}

/// `values` as the results an extern declares, with `0` for any it
/// leaves out.
fn typed(tys: &[Ty], values: &[i64]) -> Vec<Value> {
    tys.iter()
        .enumerate()
        .map(|(i, t)| {
            let n = values.get(i).copied().unwrap_or(0);
            match t {
                Ty::I8 => Value::I8(n as i8),
                Ty::I16 => Value::I16(n as i16),
                Ty::I64 => Value::I64(n),
                _ => Value::I32(n as i32),
            }
        })
        .collect()
}
