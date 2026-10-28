//! `bolt.toml`, the manifest of a package: its name, its prelude, and the
//! packages it depends on.
//!
//! ```toml
//! [package]
//! name = "shapes-demo"
//! prelude = "std"          # the default; `false` for none
//!
//! [dependencies]
//! shapes = { path = "../shapes" }
//! ```
//!
//! A file is in the package of the nearest `bolt.toml` in its directory or
//! one of its parents ([`package_manifest`]). `import "shapes/circle"` is
//! `circle.bolt` of the dependency `shapes` of that package, whose
//! directory has a `bolt.toml` of its own. `std` comes with the compiler
//! and isn't a dependency. A file without a `bolt.toml` above it is in no
//! package, and can only import `std` and files by their path.

use std::collections::BTreeMap;

use boltlang_base_db::{Db, File, SystemPath, SystemPathBuf, source_text};
use serde::Deserialize;

/// The name of a package's manifest.
pub const MANIFEST: &str = "bolt.toml";

/// A package's manifest.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Manifest {
    pub name: String,
    pub prelude: PreludeChoice,
    /// The packages it depends on, by the name they're imported with, and
    /// the path of their directory as written, relative to the manifest's.
    pub dependencies: Vec<(String, String)>,
}

/// Which prelude the files of a package get.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum PreludeChoice {
    /// The default one, `std`'s.
    Default,
    /// None, with `prelude = false`.
    None,
    /// That of a package: `std`, or one of the dependencies.
    Package(String),
}

impl Manifest {
    /// The path of the dependency imported as `name`, as written.
    pub fn dependency(&self, name: &str) -> Option<&str> {
        self.dependencies.iter().find(|(n, _)| n == name).map(|(_, path)| path.as_str())
    }
}

/// What's wrong with a manifest, and where in it, as a byte range.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ManifestError {
    pub message: String,
    pub range: Option<(u32, u32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    package: RawPackage,
    #[serde(default)]
    dependencies: BTreeMap<String, RawDependency>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackage {
    name: String,
    #[serde(default)]
    prelude: Option<RawPrelude>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPrelude {
    Enabled(bool),
    Package(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDependency {
    path: String,
}

/// The manifest in `file`, a `bolt.toml`.
#[salsa::tracked(returns(ref))]
pub fn manifest(db: &dyn Db, file: File) -> Result<Manifest, ManifestError> {
    parse(source_text(db, file))
}

/// Parse the text of a manifest.
pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let raw: RawManifest = toml::from_str(text).map_err(|e| ManifestError {
        message: e.message().to_string(),
        range: e.span().map(|s| (s.start as u32, s.end as u32)),
    })?;
    let error = |message: String| Err(ManifestError { message, range: None });
    if !is_package_name(&raw.package.name) {
        return error(format!("`{}` can't be the name of a package: use letters, digits, `_` and `-`", raw.package.name));
    }
    let mut dependencies = Vec::new();
    for (name, dependency) in raw.dependencies {
        if !is_package_name(&name) {
            return error(format!("`{name}` can't be the name of a dependency: use letters, digits, `_` and `-`"));
        }
        if name == crate::nameres::STD {
            return error("`std` comes with the compiler, so it can't be a dependency".to_string());
        }
        dependencies.push((name, dependency.path));
    }
    let prelude = match raw.package.prelude {
        None | Some(RawPrelude::Enabled(true)) => PreludeChoice::Default,
        Some(RawPrelude::Enabled(false)) => PreludeChoice::None,
        Some(RawPrelude::Package(name)) => {
            if name != crate::nameres::STD && !dependencies.iter().any(|(n, _)| *n == name) {
                return error(format!("the prelude is that of `{name}`, which isn't a dependency"));
            }
            PreludeChoice::Package(name)
        }
    };
    Ok(Manifest { name: raw.package.name, prelude, dependencies })
}

/// The manifest of the package `file` is in: the nearest `bolt.toml` in its
/// directory or one of its parents. `None` for a file in no package.
#[salsa::tracked(returns(copy))]
pub fn package_manifest(db: &dyn Db, file: File) -> Option<File> {
    let path = file.path(db).as_system_path_buf()?;
    for dir in path.as_std_path().ancestors().skip(1) {
        let candidate = db.files().resolve(db, SystemPath::new(&dir.join(MANIFEST)));
        if candidate.exists(db) {
            return Some(candidate);
        }
    }
    None
}

/// The directory of the package whose manifest is `manifest`.
pub fn package_root(db: &dyn Db, manifest: File) -> Option<SystemPathBuf> {
    let path = manifest.path(db).as_system_path_buf()?;
    path.as_std_path().parent().map(|p| SystemPathBuf::from(p.to_path_buf()))
}

/// Like an identifier, with `-` allowed after the first character.
pub fn is_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests() {
        let m = parse("[package]\nname = \"demo\"\n\n[dependencies]\nshapes = { path = \"../shapes\" }\n").unwrap();
        assert_eq!(m.name, "demo");
        assert_eq!(m.prelude, PreludeChoice::Default);
        assert_eq!(m.dependency("shapes"), Some("../shapes"));
        let m = parse("[package]\nname = \"bare\"\nprelude = false\n").unwrap();
        assert_eq!(m.prelude, PreludeChoice::None);
        let m = parse("[package]\nname = \"p\"\nprelude = \"geo\"\n[dependencies]\ngeo = { path = \"g\" }\n").unwrap();
        assert_eq!(m.prelude, PreludeChoice::Package("geo".into()));
    }

    #[test]
    fn manifest_errors() {
        let message = |text: &str| parse(text).unwrap_err().message;
        assert!(parse("[package]\nname = 1\n").unwrap_err().range.is_some());
        assert!(message("[package]\n").contains("name"));
        assert!(message("[package]\nname = \"a\"\nversion = \"1\"\n").contains("version"));
        assert_eq!(message("[package]\nname = \"a b\"\n"), "`a b` can't be the name of a package: use letters, digits, `_` and `-`");
        assert_eq!(message("[package]\nname = \"a\"\n[dependencies]\nstd = { path = \"s\" }\n"), "`std` comes with the compiler, so it can't be a dependency");
        assert_eq!(message("[package]\nname = \"a\"\nprelude = \"geo\"\n"), "the prelude is that of `geo`, which isn't a dependency");
    }
}
