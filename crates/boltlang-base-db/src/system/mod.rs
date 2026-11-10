#[doc(hidden)]
pub mod common;

mod memory;
#[cfg(not(target_arch = "wasm32"))]
mod os;
mod path;
mod test;

pub use common::{System, WritableSystem, file_time_now};
pub use memory::InMemorySystem;
#[cfg(not(target_arch = "wasm32"))]
pub use os::OsSystem;
pub use path::{SystemPath, SystemPathBuf};

pub use test::TestSystem;
