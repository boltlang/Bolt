//! A language server for Bolt, without a transport: [`Server`] answers the
//! requests and notifications of the protocol. The `bolt-language-server`
//! binary runs it over stdio, and `bolt-language-server-wasm` in the
//! browser.

mod db;
pub mod position;
pub mod server;
pub mod system;
pub mod workspace;

pub use ls_types;
pub use server::{NAME, NotAFile, Server, VERSION, path_of};
