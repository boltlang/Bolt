use boltlang_base_db::File;

use super::*;
use crate::{
    body::DefWithBody,
    ids::{FunctionId, ItemContainerId},
    item_tree::{ModItem, item_tree},
    test_db::TestDb,
};

/// For each path expression `x` in the first function of `source`, in
/// order: its name and where the binding it refers to was introduced, as
/// the text of the binding's pattern followed by its position, or `-` if it
/// isn't a local.
fn resolutions(source: &str) -> Vec<String> {
    let db = TestDb::new();
    let file: File = db.file("/main.bolt", source);
    let ModItem::Function(idx) = item_tree(&db, file).top_level[0] else { panic!() };
    let def = DefWithBodyId::new(&db, DefWithBody::Function(FunctionId::new(&db, ItemContainerId::File(file), idx)));
    let (body, source_map) = crate::body::body_with_source_map(&db, def);
    let scopes = expr_scopes(&db, def);

    // Where each binding is: the start of the pattern that introduced it.
    let binding_pos = |binding: BindingId| {
        body.pats
            .iter()
            .find(|(_, pat)| matches!(pat, Pat::Bind { id, .. } if *id == binding))
            .and_then(|(id, _)| source_map.pat_syntax(id))
            .map_or("self".to_string(), |ptr| ptr.start.to_string())
    };

    let mut paths: Vec<_> = body
        .exprs
        .iter()
        .filter_map(|(id, expr)| match expr {
            Expr::Path(path) => Some((source_map.expr_syntax(id)?.start, id, path.as_name()?.clone())),
            _ => None,
        })
        .collect();
    paths.sort_by_key(|(start, ..)| *start);
    paths
        .into_iter()
        .map(|(_, id, name)| {
            let scope = scopes.scope_for(id).unwrap();
            match scopes.resolve_name(scope, &name) {
                Some(binding) => format!("{name}@{}", binding_pos(binding)),
                None => format!("{name}-"),
            }
        })
        .collect()
}

#[test]
fn params_and_shadowing() {
    assert_eq!(
        resolutions("fn f(self, x) { let x = x + 1; let y = x; y + z + self }"),
        ["x@11", "x@20", "y@35", "z-", "self@self"]
    );
}

#[test]
fn blocks_end_their_bindings() {
    assert_eq!(resolutions("fn f() { { let a = 1; a }; a }"), ["a@15", "a-"]);
}

#[test]
fn match_arms_lambdas_and_loops() {
    assert_eq!(
        resolutions("fn f(o) { match o { Some(v) if v > 0 => v, _ => v }; |w| w; for i in i { i } }"),
        ["o@5", "v@25", "v@25", "v-", "w@54", "i-", "i@64"]
    );
}

#[test]
fn let_chains_bind_for_the_rest_of_the_condition() {
    assert_eq!(
        resolutions("fn f(o) { if let Some(x) = o && x > 0 { x } else { x } }"),
        ["o@5", "x@22", "x@22", "x-"]
    );
}

#[test]
fn labels() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "fn f() { 'a: loop { 'b: while true { break 'a; break; } } }");
    let ModItem::Function(idx) = item_tree(&db, file).top_level[0] else { panic!() };
    let def = DefWithBodyId::new(&db, DefWithBody::Function(FunctionId::new(&db, ItemContainerId::File(file), idx)));
    let body = body(&db, def);
    let scopes = expr_scopes(&db, def);
    let breaks: Vec<_> = body.exprs.iter().filter(|(_, e)| matches!(e, Expr::Break { .. })).collect();
    let resolve = |(id, expr): (ExprId, &Expr)| {
        let Expr::Break { label, .. } = expr else { unreachable!() };
        let label = scopes.resolve_label(scopes.scope_for(id).unwrap(), label.as_ref()).unwrap();
        body[label].name.to_string()
    };
    assert_eq!(resolve(breaks[0]), "a");
    assert_eq!(resolve(breaks[1]), "b");
}
