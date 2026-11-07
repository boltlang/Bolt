//! What a program is compiled for: which host provides its `#[link]`
//! functions, and the machine it runs on. `#[cfg(..)]` picks items by it.

use std::fmt;

/// Who provides the functions linked to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Host {
    /// A Wasm engine: `#[link(wasm, ..)]`, like WASI.
    Wasm,
    /// The C functions of a native target: `#[link(c, ..)]`.
    C,
    /// Conduit's evaluator, as `bolt run` and `bolt test` use it:
    /// `#[link(emu, ..)]`.
    Emu,
}

impl Host {
    pub const ALL: &[Host] = &[Host::Wasm, Host::C, Host::Emu];

    /// How `#[cfg(host = "..")]` writes it.
    pub fn name(self) -> &'static str {
        match self {
            Host::Wasm => "wasm",
            Host::C => "c",
            Host::Emu => "emu",
        }
    }
}

/// The machine a program runs on, or that the evaluator imitates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    Wasm32,
    X86,
    X86_64,
    Aarch64,
}

impl Arch {
    pub const ALL: &[Arch] = &[Arch::Wasm32, Arch::X86, Arch::X86_64, Arch::Aarch64];

    /// How `#[cfg(arch = "..")]` and `--target` write it.
    pub fn name(self) -> &'static str {
        match self {
            Arch::Wasm32 => "wasm32",
            Arch::X86 => "x86",
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }

    pub fn by_name(name: &str) -> Option<Arch> {
        Arch::ALL.iter().copied().find(|a| a.name() == name)
    }

    /// The width of a pointer, and of `isize` and `usize`, in bits.
    pub fn pointer_width(self) -> u32 {
        match self {
            Arch::Wasm32 | Arch::X86 => 32,
            Arch::X86_64 | Arch::Aarch64 => 64,
        }
    }

    /// The byte order. All of these are little-endian.
    pub fn endian(self) -> Endian {
        Endian::Little
    }

    /// The machine the compiler itself runs on, or `x86_64` when it's none
    /// of these.
    pub fn current() -> Arch {
        if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else if cfg!(target_arch = "x86") {
            Arch::X86
        } else if cfg!(target_arch = "wasm32") {
            Arch::Wasm32
        } else {
            Arch::X86_64
        }
    }
}

/// The byte order of a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    /// How `#[cfg(endian = "..")]` writes it.
    pub fn name(self) -> &'static str {
        match self {
            Endian::Little => "little",
            Endian::Big => "big",
        }
    }
}

/// A host and a machine, like the evaluator imitating `x86_64`, or Wasm on
/// `wasm32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target {
    pub host: Host,
    pub arch: Arch,
}

impl Target {
    /// What `bolt build` and `--wasm` compile for.
    pub const WASM: Target = Target { host: Host::Wasm, arch: Arch::Wasm32 };

    /// The evaluator, imitating `arch`.
    pub fn emu(arch: Arch) -> Target {
        Target { host: Host::Emu, arch }
    }
}

/// The evaluator imitating the machine the compiler runs on, as `bolt run`
/// does when nothing else is said.
impl Default for Target {
    fn default() -> Self {
        Target::emu(Arch::current())
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} on {}", self.host.name(), self.arch.name())
    }
}
