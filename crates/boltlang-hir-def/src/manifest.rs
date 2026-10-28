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
use toml::Spanned;

/// The name of a package's manifest.
pub const MANIFEST: &str = "bolt.toml";

/// A package's manifest.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Manifest {
    pub name: String,
    pub prelude: PreludeChoice,
    /// The packages it depends on, by the name they're imported with.
    pub dependencies: Vec<Dependency>,
}

/// A dependency of a package, `name = { path = "..." }`.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Dependency {
    /// The name it's imported with.
    pub name: String,
    /// The path of its directory as written, relative to the manifest's.
    pub path: String,
    /// Where the path is written in the manifest, as a byte range.
    pub path_range: (u32, u32),
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
        self.dependencies.iter().find(|d| d.name == name).map(|d| d.path.as_str())
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
    dependencies: BTreeMap<Spanned<String>, RawDependency>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackage {
    name: Spanned<String>,
    #[serde(default)]
    prelude: Option<Spanned<RawPrelude>>,
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
    path: Spanned<String>,
}

fn range<T>(spanned: &Spanned<T>) -> Option<(u32, u32)> {
    let span = spanned.span();
    Some((span.start as u32, span.end as u32))
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
    let error = |message: String, range| Err(ManifestError { message, range });
    let name = raw.package.name.get_ref();
    if !is_package_name(name) {
        return error(format!("`{name}` can't be the name of a package: use letters, digits, `_` and `-`"), range(&raw.package.name));
    }
    let mut dependencies = Vec::new();
    for (key, dependency) in raw.dependencies {
        let name = key.get_ref();
        if !is_package_name(name) {
            return error(format!("`{name}` can't be the name of a dependency: use letters, digits, `_` and `-`"), range(&key));
        }
        if name == crate::nameres::STD {
            return error("`std` comes with the compiler, so it can't be a dependency".to_string(), range(&key));
        }
        let path_range = range(&dependency.path).unwrap_or_default();
        dependencies.push(Dependency { name: name.clone(), path: dependency.path.into_inner(), path_range });
    }
    let prelude = match raw.package.prelude {
        None => PreludeChoice::Default,
        Some(prelude) => match prelude.get_ref() {
            RawPrelude::Enabled(true) => PreludeChoice::Default,
            RawPrelude::Enabled(false) => PreludeChoice::None,
            RawPrelude::Package(name) => {
                if name != crate::nameres::STD && !dependencies.iter().any(|d| d.name == *name) {
                    return error(format!("the prelude is that of `{name}`, which isn't a dependency"), range(&prelude));
                }
                PreludeChoice::Package(name.clone())
            }
        },
    };
    Ok(Manifest { name: raw.package.name.into_inner(), prelude, dependencies })
}

/// The directory of a dependency written as `path` in the manifest
/// `manifest_file`, by its real path, if it's a package: if it has a
/// `bolt.toml`.
pub fn dependency_dir(db: &dyn Db, manifest_file: File, path: &str) -> Option<SystemPathBuf> {
    let base = package_root(db, manifest_file)?;
    let dir = SystemPath::absolute(SystemPath::new(&base.as_std_path().join(path)), SystemPath::new("/"));
    let dir = db.system().canonicalize_path(&dir).unwrap_or(dir);
    db.files().resolve(db, SystemPath::new(&dir.as_std_path().join(MANIFEST))).exists(db).then_some(dir)
}

/// What's wrong with the manifest in `file`, with where: that it can't be
/// read, or a dependency that isn't a package.
pub fn manifest_problems(db: &dyn Db, file: File) -> Vec<ManifestError> {
    match manifest(db, file) {
        Err(error) => vec![error.clone()],
        Ok(manifest) => manifest
            .dependencies
            .iter()
            .filter(|d| dependency_dir(db, file, &d.path).is_none())
            .map(|d| ManifestError {
                message: format!("there's no package at `{}`: it has no `{MANIFEST}`", d.path),
                range: Some(d.path_range),
            })
            .collect(),
    }
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
        let text = "[package]\nname = \"demo\"\n\n[dependencies]\nshapes = { path = \"../shapes\" }\n";
        let m = parse(text).unwrap();
        assert_eq!(m.name, "demo");
        assert_eq!(m.prelude, PreludeChoice::Default);
        assert_eq!(m.dependency("shapes"), Some("../shapes"));
        let (start, end) = m.dependencies[0].path_range;
        assert_eq!(&text[start as usize..end as usize], "\"../shapes\"");
        let m = parse("[package]\nname = \"bare\"\nprelude = false\n").unwrap();
        assert_eq!(m.prelude, PreludeChoice::None);
        let m = parse("[package]\nname = \"p\"\nprelude = \"geo\"\n[dependencies]\ngeo = { path = \"g\" }\n").unwrap();
        assert_eq!(m.prelude, PreludeChoice::Package("geo".into()));
    }

    #[test]
    fn manifest_errors() {
        let message = |text: &str| parse(text).unwrap_err().message;
        assert!(parse("[package]\nname = 1\n").unwrap_err().range.is_some());
        // Every error says where it is.
        let text = "[package]\nname = \"a b\"\n";
        let (start, end) = parse(text).unwrap_err().range.unwrap();
        assert_eq!(&text[start as usize..end as usize], "\"a b\"");
        assert!(parse("[package]\nname = \"a\"\n[dependencies]\nstd = { path = \"s\" }\n").unwrap_err().range.is_some());
        assert!(message("[package]\n").contains("name"));
        assert!(message("[package]\nname = \"a\"\nversion = \"1\"\n").contains("version"));
        assert_eq!(message("[package]\nname = \"a b\"\n"), "`a b` can't be the name of a package: use letters, digits, `_` and `-`");
        assert_eq!(message("[package]\nname = \"a\"\n[dependencies]\nstd = { path = \"s\" }\n"), "`std` comes with the compiler, so it can't be a dependency");
        assert_eq!(message("[package]\nname = \"a\"\nprelude = \"geo\"\n"), "the prelude is that of `geo`, which isn't a dependency");
    }
}
