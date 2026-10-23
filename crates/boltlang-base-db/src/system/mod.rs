
#[doc(hidden)]
pub mod common;

mod path;
mod memory;
#[cfg(not(target_arch = "wasm32"))]
mod os;
mod test;

pub use common::{System, WritableSystem, file_time_now};
pub use memory::InMemorySystem;
#[cfg(not(target_arch = "wasm32"))]
pub use os::OsSystem;
pub use path::{SystemPath, SystemPathBuf, SystemVirtualPath, SystemVirtualPathBuf};

pub use test::TestSystem;
