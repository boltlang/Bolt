//! `#[cfg(..)]`: which targets an item is compiled for.
//!
//! ```text
//! #[cfg(pointer_width = 64)]
//! type Size = u64;
//!
//! #[cfg(all(host = "wasm", not(endian = "big")))]
//! fn f() {}
//! ```
//!
//! The keys are a closed set, and so are their values: one that's unknown
//! is an error, not just false, so that a typo doesn't quietly leave an
//! item out. An item with several `#[cfg]`s is compiled when they all
//! hold, and one with an error in a `#[cfg]` is left out.

use boltlang_base_db::{Arch, Endian, Host, Target};
use boltlang_syntax::{
    SyntaxKind::{self, *},
    SyntaxNode,
    ast::{self, Node},
    rowan::NodeOrToken,
};

/// What a `#[cfg(..)]` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cfg {
    Host(Host),
    Arch(Arch),
    PointerWidth(u32),
    Endian(Endian),
    All(Vec<Cfg>),
    Any(Vec<Cfg>),
    Not(Box<Cfg>),
}

impl Cfg {
    /// Whether it holds for `target`.
    pub fn holds(&self, target: Target) -> bool {
        match self {
            Cfg::Host(h) => target.host == *h,
            Cfg::Arch(a) => target.arch == *a,
            Cfg::PointerWidth(w) => target.arch.pointer_width() == *w,
            Cfg::Endian(e) => target.arch.endian() == *e,
            Cfg::All(cfgs) => cfgs.iter().all(|c| c.holds(target)),
            Cfg::Any(cfgs) => cfgs.iter().any(|c| c.holds(target)),
            Cfg::Not(cfg) => !cfg.holds(target),
        }
    }
}

/// What's wrong with a `#[cfg(..)]`.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum CfgError {
    /// `#[cfg]`, without what it's about, or something that isn't a
    /// condition, like `#[cfg(1)]`.
    Malformed,
    /// A key that isn't one of [`KEYS`].
    UnknownKey(String),
    /// A key that will mean something later, like `os`.
    Reserved(String),
    /// A value that the key doesn't have, with the ones it has.
    UnknownValue { key: &'static str, value: String, expected: Vec<String> },
    /// `not` with other than one condition.
    NotArity,
    /// A `#[cfg]` on something that isn't an item outside a body, like a
    /// statement.
    NotAnItem,
}

/// The keys of `#[cfg(key = value)]`.
pub const KEYS: &[&str] = &["host", "arch", "pointer_width", "endian"];

/// Keys that are kept for later.
const RESERVED: &[&str] = &["os"];

/// Whether `node`, an item, is compiled for `target`: whether each of its
/// `#[cfg]`s holds. One with an error leaves it out.
pub fn enabled(node: &SyntaxNode, target: Target) -> bool {
    node.children()
        .filter_map(ast::Attr::cast)
        .filter_map(|attr| attr.meta())
        .filter(is_cfg)
        .all(|meta| parse(&meta).is_ok_and(|cfg| cfg.holds(target)))
}

/// Whether `meta` is that of a `#[cfg..]`.
pub fn is_cfg(meta: &ast::Meta) -> bool {
    meta.path().is_some_and(|path| path.syntax().text().to_string().trim() == "cfg")
}

/// What the `#[cfg(..)]` of `meta` says.
pub fn parse(meta: &ast::Meta) -> Result<Cfg, CfgError> {
    if meta.expr().is_some() {
        return Err(CfgError::Malformed);
    }
    let tree = meta.token_tree().ok_or(CfgError::Malformed)?;
    match args(tree.syntax())?.as_slice() {
        [one] => condition(one),
        _ => Err(CfgError::Malformed),
    }
}

/// A part of the arguments of a `#[cfg]`: a token, or the parentheses of
/// `all(..)` and the like.
#[derive(Debug, Clone)]
enum Part {
    Token(SyntaxKind, String),
    Tree(SyntaxNode),
}

/// The arguments between the parentheses of `tree`, split at its commas,
/// without trivia.
fn args(tree: &SyntaxNode) -> Result<Vec<Vec<Part>>, CfgError> {
    let mut parts: Vec<Part> = tree
        .children_with_tokens()
        .filter_map(|e| match e {
            NodeOrToken::Node(n) => Some(Part::Tree(n)),
            NodeOrToken::Token(t) if t.kind().is_trivia() => None,
            NodeOrToken::Token(t) => Some(Part::Token(t.kind(), t.text().to_string())),
        })
        .collect();
    match (parts.first(), parts.last()) {
        (Some(Part::Token(L_PAREN, _)), Some(Part::Token(R_PAREN, _))) if parts.len() >= 2 => {
            parts.remove(0);
            parts.pop();
        }
        _ => return Err(CfgError::Malformed),
    }
    let mut out = vec![Vec::new()];
    for part in parts {
        match part {
            Part::Token(COMMA, _) => out.push(Vec::new()),
            part => out.last_mut().unwrap().push(part),
        }
    }
    // A comma at the end is allowed, as in `all(a, b,)`.
    if out.last().is_some_and(|p| p.is_empty()) {
        out.pop();
    }
    if out.iter().any(|p| p.is_empty()) {
        return Err(CfgError::Malformed);
    }
    Ok(out)
}

/// One condition, like `host = "emu"` or `all(..)`.
fn condition(part: &[Part]) -> Result<Cfg, CfgError> {
    match part {
        [Part::Token(IDENT, f), Part::Tree(tree)] if f == "all" || f == "any" || f == "not" => {
            let cfgs = args(tree)?.iter().map(|p| condition(p)).collect::<Result<Vec<_>, _>>()?;
            Ok(match f.as_str() {
                "all" => Cfg::All(cfgs),
                "any" => Cfg::Any(cfgs),
                _ => {
                    let [cfg] = <[Cfg; 1]>::try_from(cfgs).map_err(|_| CfgError::NotArity)?;
                    Cfg::Not(Box::new(cfg))
                }
            })
        }
        [Part::Token(IDENT, key), Part::Token(EQUALS, _), Part::Token(kind, value)] => {
            key_value(key, *kind, value)
        }
        [Part::Token(IDENT, key), ..] if !KEYS.contains(&key.as_str()) => Err(unknown_key(key)),
        _ => Err(CfgError::Malformed),
    }
}

fn unknown_key(key: &str) -> CfgError {
    if RESERVED.contains(&key) {
        CfgError::Reserved(key.to_string())
    } else {
        CfgError::UnknownKey(key.to_string())
    }
}

/// `key = value`, with `value` a token of kind `kind`.
fn key_value(key: &str, kind: SyntaxKind, value: &str) -> Result<Cfg, CfgError> {
    let names = |all: &mut dyn Iterator<Item = &'static str>| -> Vec<String> {
        all.map(|n| format!("\"{n}\"")).collect()
    };
    let string = (kind == STRING).then(|| crate::expr_store::unquote(value));
    let unknown = |key: &'static str, expected: Vec<String>| CfgError::UnknownValue {
        key,
        value: value.to_string(),
        expected,
    };
    match key {
        "host" => {
            let expected = names(&mut Host::ALL.iter().map(|h| h.name()));
            let found = string.and_then(|s| Host::ALL.iter().copied().find(|h| h.name() == s));
            found.map(Cfg::Host).ok_or_else(|| unknown("host", expected))
        }
        "arch" => {
            let expected = names(&mut Arch::ALL.iter().map(|a| a.name()));
            string
                .and_then(|s| Arch::by_name(&s))
                .map(Cfg::Arch)
                .ok_or_else(|| unknown("arch", expected))
        }
        "pointer_width" => {
            let expected = vec!["32".to_string(), "64".to_string()];
            match (kind, value) {
                (INT_NUMBER, "32") => Ok(Cfg::PointerWidth(32)),
                (INT_NUMBER, "64") => Ok(Cfg::PointerWidth(64)),
                _ => Err(unknown("pointer_width", expected)),
            }
        }
        "endian" => {
            let all = [Endian::Little, Endian::Big];
            let expected = names(&mut all.iter().map(|e| e.name()));
            let found = string.and_then(|s| all.iter().copied().find(|e| e.name() == s));
            found.map(Cfg::Endian).ok_or_else(|| unknown("endian", expected))
        }
        _ => Err(unknown_key(key)),
    }
}

#[cfg(test)]
mod tests;
