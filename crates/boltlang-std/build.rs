//! Embed the files of `packages/std` in the crate, with a hash of them, so
//! that the compiler can write them out where users can read them.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../../packages/std").canonicalize().expect("packages/std is missing");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    // FNV-1a over the paths and contents, which says when the files changed
    // within one version of the compiler, as while working on it.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut entries = String::new();
    for (relative, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        for byte in relative.bytes().chain([0]).chain(fs::read(path).unwrap()).chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        entries.push_str(&format!("    ({relative:?}, include_str!({:?})),\n", path.display().to_string()));
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("files.rs");
    let code = format!(
        "/// The files of `std`, by their path in it, like `__prelude__/lib.bolt`.\n\
         pub static FILES: &[(&str, &str)] = &[\n{entries}];\n\n\
         /// A hash of [`FILES`].\n\
         pub const HASH: &str = \"{hash:016x}\";\n"
    );
    fs::write(out, code).unwrap();
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((relative, path));
        }
    }
}
