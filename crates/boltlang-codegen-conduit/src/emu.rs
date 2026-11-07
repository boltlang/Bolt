//! The evaluator's host: the functions of `#[link(emu, ..)]`, which std's
//! `emu.bolt` declares. They're like POSIX's and WASI's, but simpler: one
//! buffer instead of a list of them, paths as they are, from the current
//! directory, and an `Error` first in what each gives back.
//!
//! Conduit's evaluator passes values, not memory: a function gets Bolt's
//! arrays themselves, and writes the `mut` ones. It can't make objects, so
//! what it gives back that isn't a number goes into a buffer the caller
//! passes, as `read` does.
//!
//! An `Error` of `emu.bolt` is passed as its index, which [`Errno`] lists
//! in the same order.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use conduit_eval::{Arrays, HostError, Value};
use conduit_ir::{Extern, Ty};

/// The errors of `emu.bolt`'s `Error`, by their index there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub(crate) enum Errno {
    Success,
    NotFound,
    Exists,
    Denied,
    BadFd,
    Invalid,
    IsDir,
    NotDir,
    NotEmpty,
    Io,
}

impl From<&io::Error> for Errno {
    fn from(e: &io::Error) -> Errno {
        use io::ErrorKind as K;
        match e.kind() {
            K::NotFound => Errno::NotFound,
            K::AlreadyExists => Errno::Exists,
            K::PermissionDenied => Errno::Denied,
            K::InvalidInput | K::InvalidData => Errno::Invalid,
            K::IsADirectory => Errno::IsDir,
            K::NotADirectory => Errno::NotDir,
            K::DirectoryNotEmpty => Errno::NotEmpty,
            _ => Errno::Io,
        }
    }
}

/// The flags of `open`, as `emu.bolt` has them.
const READ: i64 = 1;
const WRITE: i64 = 2;
const CREATE: i64 = 4;
const TRUNCATE: i64 = 8;
const APPEND: i64 = 16;

/// What the host keeps between calls.
pub(crate) struct Emu {
    /// The program's arguments, after its own name.
    args: Vec<String>,
    /// The files it has open, from 3: 0, 1 and 2 are stdin, stdout and
    /// stderr.
    files: HashMap<i64, File>,
    next_fd: i64,
    start: Instant,
    /// The state of the random bytes, a xorshift.
    random: u64,
}

impl Emu {
    pub(crate) fn new(args: Vec<String>) -> Emu {
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        Emu { args, files: HashMap::new(), next_fd: 3, start: Instant::now(), random: seed | 1 }
    }

    /// Call the function `name`, declared as `decl`: its results, as the
    /// extern declares them.
    pub(crate) fn call(
        &mut self,
        name: &str,
        decl: &Extern,
        args: &[Value],
        arrays: &mut Arrays<'_, '_>,
    ) -> Result<Vec<Value>, HostError> {
        let int = |i: usize| -> i64 {
            match args.get(i) {
                Some(Value::I8(n)) => *n as u8 as i64,
                Some(Value::I16(n)) => *n as u16 as i64,
                Some(Value::I32(n)) => *n as i64,
                Some(Value::I64(n)) => *n,
                _ => 0,
            }
        };
        let bytes = |arrays: &Arrays<'_, '_>, i: usize| -> Vec<u8> {
            let elems = args.get(i).and_then(|v| arrays.get(v)).unwrap_or_default();
            elems.iter().map(|b| if let Value::I8(b) = b { *b as u8 } else { 0 }).collect()
        };
        let path = |arrays: &Arrays<'_, '_>, i: usize| -> String {
            String::from_utf8_lossy(&bytes(arrays, i)).into_owned()
        };
        let ok = |n: i64| vec![Errno::Success as i64, n];
        let err = |e: Errno| vec![e as i64, 0];
        let results: Vec<i64> = match name {
            "write" => {
                let data = bytes(arrays, 1);
                match self.write(int(0), &data) {
                    Ok(()) => ok(data.len() as i64),
                    Err(e) => err(e),
                }
            }
            "read" => {
                let len = args.get(1).and_then(|v| arrays.get(v)).map_or(0, <[Value]>::len);
                let mut data = vec![0u8; len];
                match self.read(int(0), &mut data) {
                    Ok(n) => {
                        fill(arrays, &args[1], &data[..n]);
                        ok(n as i64)
                    }
                    Err(e) => err(e),
                }
            }
            "open" => {
                let flags = int(1);
                let mut options = OpenOptions::new();
                options
                    .read(flags & READ != 0)
                    .write(flags & (WRITE | APPEND) != 0)
                    .append(flags & APPEND != 0)
                    .create(flags & CREATE != 0)
                    .truncate(flags & TRUNCATE != 0);
                match options.open(path(arrays, 0)) {
                    Ok(file) => {
                        let fd = self.next_fd;
                        self.next_fd += 1;
                        self.files.insert(fd, file);
                        ok(fd)
                    }
                    Err(e) => err(Errno::from(&e)),
                }
            }
            "close" => match self.files.remove(&int(0)) {
                Some(_) => vec![Errno::Success as i64],
                None => vec![Errno::BadFd as i64],
            },
            "seek" => {
                let from = match int(2) {
                    0 => SeekFrom::Start(int(1).max(0) as u64),
                    1 => SeekFrom::Current(int(1)),
                    _ => SeekFrom::End(int(1)),
                };
                match self.files.get_mut(&int(0)) {
                    Some(file) => match file.seek(from) {
                        Ok(at) => ok(at as i64),
                        Err(e) => err(Errno::from(&e)),
                    },
                    None => vec![Errno::BadFd as i64, 0],
                }
            }
            "file_size" => match self.files.get(&int(0)) {
                Some(file) => match file.metadata() {
                    Ok(m) => ok(m.len() as i64),
                    Err(e) => err(Errno::from(&e)),
                },
                None => vec![Errno::BadFd as i64, 0],
            },
            "remove" => single(fs::remove_file(path(arrays, 0))),
            "rename" => single(fs::rename(path(arrays, 0), path(arrays, 1))),
            "make_dir" => single(fs::create_dir(path(arrays, 0))),
            "remove_dir" => single(fs::remove_dir(path(arrays, 0))),
            // The names in a directory, sorted, each ending in a zero: as
            // many bytes as fit, and how many there are in all.
            "read_dir" => match fs::read_dir(path(arrays, 0)) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect();
                    names.sort();
                    let all = zero_ended(names.iter().map(String::as_str));
                    fill(arrays, &args[1], &all);
                    ok(all.len() as i64)
                }
                Err(e) => err(Errno::from(&e)),
            },
            // The arguments: how many, and their bytes with a zero after
            // each.
            "args_size" => {
                vec![
                    self.args.len() as i64,
                    zero_ended(self.args.iter().map(String::as_str)).len() as i64,
                ]
            }
            "args" => {
                fill(arrays, &args[0], &zero_ended(self.args.iter().map(String::as_str)));
                vec![Errno::Success as i64]
            }
            // The environment, as `KEY=value`, sorted.
            "env_size" => {
                let env = environment();
                vec![env.len() as i64, zero_ended(env.iter().map(String::as_str)).len() as i64]
            }
            "env" => {
                fill(arrays, &args[0], &zero_ended(environment().iter().map(String::as_str)));
                vec![Errno::Success as i64]
            }
            // In nanoseconds: since 1970 for the real time, and since the
            // program started for the monotonic one.
            "clock" => match int(0) {
                0 => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                    vec![now.as_nanos() as i64]
                }
                _ => vec![self.start.elapsed().as_nanos() as i64],
            },
            "random" => {
                if let Some(buf) = args.first().and_then(|v| arrays.get_mut(v)) {
                    for slot in buf.iter_mut() {
                        self.random ^= self.random << 13;
                        self.random ^= self.random >> 7;
                        self.random ^= self.random << 17;
                        *slot = Value::I8(self.random as i8);
                    }
                }
                Vec::new()
            }
            "exit" => return Err(HostError::Exit(int(0) as i32)),
            _ => return Err(format!("there's no host function `{name}` here").into()),
        };
        Ok(typed(&decl.sig.results, &results))
    }

    fn write(&mut self, fd: i64, data: &[u8]) -> Result<(), Errno> {
        let result = match fd {
            1 => {
                let mut out = io::stdout().lock();
                out.write_all(data).and_then(|()| out.flush())
            }
            2 => {
                let mut err = io::stderr().lock();
                err.write_all(data).and_then(|()| err.flush())
            }
            fd => self.file(fd)?.write_all(data),
        };
        result.map_err(|e| Errno::from(&e))
    }

    fn read(&mut self, fd: i64, data: &mut [u8]) -> Result<usize, Errno> {
        let result = match fd {
            0 => io::stdin().read(data),
            fd => self.file(fd)?.read(data),
        };
        result.map_err(|e| Errno::from(&e))
    }

    /// The open file `fd`, other than stdin, stdout and stderr.
    fn file(&mut self, fd: i64) -> Result<&mut File, Errno> {
        self.files.get_mut(&fd).ok_or(Errno::BadFd)
    }
}

/// What a function that only says whether it worked gives back.
fn single(result: io::Result<()>) -> Vec<i64> {
    vec![result.as_ref().map_or_else(|e| Errno::from(e) as i64, |_| Errno::Success as i64)]
}

/// The environment of the process, as `KEY=value`, sorted.
fn environment() -> Vec<String> {
    let mut env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    env.sort();
    env
}

/// `parts`, each with a zero after it.
fn zero_ended<'a>(parts: impl Iterator<Item = &'a str>) -> Vec<u8> {
    let mut out = Vec::new();
    for part in parts {
        out.extend_from_slice(part.as_bytes());
        out.push(0);
    }
    out
}

/// Write `data` to the `mut` array `buf`, as much as fits.
fn fill(arrays: &mut Arrays<'_, '_>, buf: &Value, data: &[u8]) {
    if let Some(buf) = arrays.get_mut(buf) {
        for (slot, b) in buf.iter_mut().zip(data) {
            *slot = Value::I8(*b as i8);
        }
    }
}

/// `values` as the results an extern declares, with `0` for any it leaves
/// out.
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
