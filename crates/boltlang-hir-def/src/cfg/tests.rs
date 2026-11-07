use boltlang_base_db::{Arch, Host, Target, parse_file};
use boltlang_syntax::{
    SyntaxNode,
    ast::{self, HasAttrs, Node},
};

use super::*;
use crate::test_db::TestDb;

/// What the `#[cfg]` before `fn f() {}` says.
fn cfg(attr: &str) -> Result<Cfg, CfgError> {
    let db = TestDb::new();
    let file = db.file("/main.bolt", &format!("#[{attr}]\nfn f() {{}}\n"));
    let root = SyntaxNode::new_root(parse_file(&db, file).tree(&db).clone());
    let f = root.descendants().find_map(ast::FuncDecl::cast).unwrap();
    let meta = f.attrs().filter_map(|a| a.meta()).next().unwrap();
    parse(&meta)
}

const EMU_64: Target = Target { host: Host::Emu, arch: Arch::X86_64 };
const EMU_32: Target = Target { host: Host::Emu, arch: Arch::X86 };

fn holds(attr: &str, target: Target) -> bool {
    cfg(attr).unwrap().holds(target)
}

#[test]
fn keys_hold_for_the_targets_they_name() {
    assert!(holds(r#"cfg(host = "emu")"#, EMU_64));
    assert!(!holds(r#"cfg(host = "wasm")"#, EMU_64));
    assert!(holds(r#"cfg(host = "wasm")"#, Target::WASM));
    assert!(holds(r#"cfg(arch = "x86")"#, EMU_32));
    assert!(holds("cfg(pointer_width = 64)", EMU_64));
    assert!(!holds("cfg(pointer_width = 64)", EMU_32));
    assert!(holds("cfg(pointer_width = 32)", Target::WASM));
    assert!(holds(r#"cfg(endian = "little")"#, EMU_64));
    assert!(!holds(r#"cfg(endian = "big")"#, EMU_64));
}

#[test]
fn all_any_and_not_combine_conditions() {
    let attr = r#"cfg(all(host = "emu", not(pointer_width = 32)))"#;
    assert!(holds(attr, EMU_64));
    assert!(!holds(attr, EMU_32));
    assert!(!holds(attr, Target::WASM));
    let attr = r#"cfg(any(host = "wasm", arch = "x86",))"#;
    assert!(holds(attr, Target::WASM));
    assert!(holds(attr, EMU_32));
    assert!(!holds(attr, EMU_64));
    // Nothing to hold: `all()` always does, `any()` never.
    assert!(holds("cfg(all())", EMU_64));
    assert!(!holds("cfg(any())", EMU_64));
}

#[test]
fn an_unknown_key_or_value_is_an_error() {
    assert_eq!(cfg(r#"cfg(hots = "emu")"#), Err(CfgError::UnknownKey("hots".into())));
    assert_eq!(cfg(r#"cfg(os = "linux")"#), Err(CfgError::Reserved("os".into())));
    let Err(CfgError::UnknownValue { key, value, expected }) = cfg(r#"cfg(host = "wsam")"#) else {
        panic!()
    };
    assert_eq!((key, value.as_str()), ("host", "\"wsam\""));
    assert_eq!(expected, ["\"wasm\"", "\"c\"", "\"emu\""]);
    // A width is a number, not a string.
    assert!(matches!(cfg(r#"cfg(pointer_width = "64")"#), Err(CfgError::UnknownValue { .. })));
    assert!(matches!(cfg("cfg(pointer_width = 16)"), Err(CfgError::UnknownValue { .. })));
}

#[test]
fn malformed_cfgs_are_errors() {
    assert_eq!(cfg("cfg"), Err(CfgError::Malformed));
    assert_eq!(cfg("cfg()"), Err(CfgError::Malformed));
    assert_eq!(cfg("cfg(1)"), Err(CfgError::Malformed));
    assert_eq!(cfg(r#"cfg(host = "emu", arch = "x86")"#), Err(CfgError::Malformed));
    assert_eq!(cfg(r#"cfg(not(host = "emu", arch = "x86"))"#), Err(CfgError::NotArity));
    assert_eq!(cfg("cfg(not())"), Err(CfgError::NotArity));
    assert_eq!(cfg("cfg(all(,))"), Err(CfgError::Malformed));
}
