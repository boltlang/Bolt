//! The package `std` that comes with the compiler (#98).
//!
//! The files of `packages/std` are part of the compiler's binaries. So that
//! users can read them, and an editor open them, they're written out once
//! as ordinary files, to a directory for this version of the compiler, like
//! `~/.local/share/bolt/0.2.0-<hash>/std`, which isn't changed after. The
//! hash says when the files changed within one version.
//!
//! Which `std` the compiler uses ([`locate`]):
//!
//! 1. The directory in `BOLT_PRELUDE_PATH`, for working on `std` itself: it
//!    points at the package, the directory with `__prelude__` in it.
//! 2. In a debug build of the compiler, `packages/std` of the repository it
//!    was built from, so that running it from there uses what's being
//!    changed.
//! 3. The copy that comes with the compiler.

use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

include!(concat!(env!("OUT_DIR"), "/files.rs"));

/// The variable that says where `std` is instead.
pub const ENV_VAR: &str = "BOLT_PRELUDE_PATH";

/// Where the `std` in use comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// [`ENV_VAR`].
    Env,
    /// The repository a debug build of the compiler was built from.
    Repository,
    /// The copy that comes with the compiler.
    Shipped,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Env => write!(f, "from {ENV_VAR}"),
            Source::Repository => f.write_str("from the repository of this debug build"),
            Source::Shipped => f.write_str("comes with the compiler"),
        }
    }
}

/// Why `std` can't be found.
#[derive(Debug)]
pub enum Error {
    /// [`ENV_VAR`] names something that isn't a directory.
    NotADirectory(PathBuf),
    /// There's no directory to write the shipped copy to, like without a
    /// home directory.
    NoDataDir,
    /// Writing the shipped copy failed.
    Io(PathBuf, io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotADirectory(path) => write!(f, "{ENV_VAR} is `{}`, which isn't a directory", path.display()),
            Error::NoDataDir => write!(
                f,
                "there's no directory to put the standard library in: set HOME, or {ENV_VAR} to the directory of `std`"
            ),
            Error::Io(path, error) => write!(f, "can't write the standard library to `{}`: {error}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

/// The directory of the `std` to use, as a real path, and where it comes
/// from. The shipped copy is written out the first time.
pub fn locate() -> Result<(PathBuf, Source), Error> {
    let (dir, source) = find()?;
    if source != Source::Shipped {
        return Ok((dir, source));
    }
    install(&dir)?;
    let real = dir.canonicalize().map_err(|e| Error::Io(dir.clone(), e))?;
    Ok((real, source))
}

/// Like [`locate`], without writing anything: the shipped copy may not be
/// there yet.
pub fn find() -> Result<(PathBuf, Source), Error> {
    if let Some(path) = env::var_os(ENV_VAR).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return match path.canonicalize() {
            Ok(real) if real.is_dir() => Ok((real, Source::Env)),
            _ => Err(Error::NotADirectory(path)),
        };
    }
    if cfg!(debug_assertions) {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
        if let Ok(real) = repository.canonicalize() {
            return Ok((real, Source::Repository));
        }
    }
    let base = data_dir().ok_or(Error::NoDataDir)?;
    let dir = base.join("bolt").join(format!("{}-{HASH}", env!("CARGO_PKG_VERSION"))).join("std");
    Ok((dir, Source::Shipped))
}

/// Where programs keep their data: `$XDG_DATA_HOME` or `~/.local/share` on
/// Linux and other Unix systems, `~/Library/Application Support` on macOS,
/// and `%LOCALAPPDATA%` on Windows.
fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        return var("LOCALAPPDATA");
    }
    if cfg!(target_os = "macos") {
        return var("HOME").map(|home| home.join("Library").join("Application Support"));
    }
    var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local").join("share")))
}

/// Write [`FILES`] to `dir`, unless they're there already. They're written
/// next to it first, then moved in place, so that a compiler that stops
/// halfway, or another one at the same time, never leaves part of them.
/// They're read-only, since editing them would change `std` for every
/// program without anyone knowing.
pub fn install(dir: &Path) -> Result<(), Error> {
    if dir.join(DONE).is_file() {
        return Ok(());
    }
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |e| Error::Io(path, e)
    };
    let parent = dir.parent().ok_or_else(|| Error::Io(dir.to_path_buf(), io::ErrorKind::NotFound.into()))?;
    fs::create_dir_all(parent).map_err(io(parent))?;
    let staging = parent.join(format!(".std-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    for (relative, text) in FILES {
        let path = staging.join(relative);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io(dir))?;
        }
        fs::write(&path, text).map_err(io(&path))?;
        read_only(&path).map_err(io(&path))?;
    }
    fs::write(staging.join(DONE), "").map_err(io(&staging))?;
    match fs::rename(&staging, dir) {
        Ok(()) => Ok(()),
        // Another compiler got there first.
        Err(_) if dir.join(DONE).is_file() => {
            remove(&staging);
            Ok(())
        }
        Err(e) => {
            remove(&staging);
            Err(Error::Io(dir.to_path_buf(), e))
        }
    }
}

/// The file that says the files of a directory are all there.
const DONE: &str = ".complete";

fn read_only(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions)
}

/// Remove a directory of read-only files.
fn remove(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_of_std_are_embedded() {
        assert!(FILES.iter().any(|(path, _)| *path == "__prelude__.bolt"));
        assert!(FILES.iter().all(|(path, _)| !path.starts_with('/') && !path.contains('\\')));
    }

    #[test]
    fn installing_writes_the_files_once() {
        let base = env::temp_dir().join(format!("boltlang-std-test-{}", std::process::id()));
        let dir = base.join("std");
        install(&dir).unwrap();
        let lib = dir.join("__prelude__.bolt");
        assert_eq!(fs::read_to_string(&lib).unwrap(), FILES.iter().find(|(p, _)| *p == "__prelude__.bolt").unwrap().1);
        assert!(fs::metadata(&lib).unwrap().permissions().readonly());
        // A second time, it's there already.
        install(&dir).unwrap();
        make_writable(&base);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_errors_say_what_to_do() {
        assert_eq!(
            Error::NoDataDir.to_string(),
            format!("there's no directory to put the standard library in: set HOME, or {ENV_VAR} to the directory of `std`")
        );
        let error = Error::Io(PathBuf::from("/x/std"), io::ErrorKind::PermissionDenied.into());
        assert!(error.to_string().starts_with("can't write the standard library to `/x/std`: "), "{error}");
        assert_eq!(Source::Shipped.to_string(), "comes with the compiler");
    }

    #[test]
    fn installing_where_there_is_no_parent_fails() {
        assert!(matches!(install(Path::new("/")), Err(Error::Io(..))));
    }

    /// Read-only files can't always be removed.
    fn make_writable(dir: &Path) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                make_writable(&path);
            } else {
                let mut permissions = fs::metadata(&path).unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                fs::set_permissions(&path, permissions).unwrap();
            }
        }
    }
}
