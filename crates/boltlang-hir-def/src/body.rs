//! Bodies: the lowered expressions of functions, consts, enum discriminants
//! and the statements at the top level of a file.

use boltlang_base_db::{Db, File, parse_file};
use boltlang_syntax::{SyntaxNode, ast::{self, Node}};

use crate::{
    builtin_type::BuiltinValue,
    expr_store::{ExprCollector, ExpressionStore, ExpressionStoreSourceMap},
    hir::{BlockKind, Expr, ExprId, Pat, PatId, Path},
    ids::{ConstId, EnumVariantId, FunctionId, ModuleDefId},
    nameres::{Namespace, file_scope},
};

/// Something with a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum DefWithBody<'db> {
    Function(FunctionId<'db>),
    Const(ConstId<'db>),
    /// The discriminant of a variant, like the `1` of `Red = 1`.
    Variant(EnumVariantId<'db>),
    /// The statements at the top level of a file.
    File(File),
}

impl DefWithBody<'_> {
    /// The file the body is in.
    pub fn file(self, db: &dyn Db) -> File {
        match self {
            DefWithBody::Function(id) => id.file(db),
            DefWithBody::Const(id) => id.file(db),
            DefWithBody::Variant(id) => id.parent.file(db),
            DefWithBody::File(file) => file,
        }
    }
}

/// An interned [`DefWithBody`], so that it can be the key of a query.
#[salsa::interned(debug)]
pub struct DefWithBodyId<'db> {
    #[returns(copy)]
    pub def: DefWithBody<'db>,
}

/// A lowered body.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Body {
    pub store: ExpressionStore,
    /// The patterns of the parameters of a function, in order.
    pub params: Box<[PatId]>,
    /// The expression the body evaluates.
    pub body_expr: ExprId,
}

impl std::ops::Deref for Body {
    type Target = ExpressionStore;
    fn deref(&self) -> &ExpressionStore {
        &self.store
    }
}

/// The body of `def`, and where its parts came from.
#[salsa::tracked(returns(ref))]
pub fn body_with_source_map<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> (Body, ExpressionStoreSourceMap) {
    let def = def.def(db);
    let file = def.file(db);
    let root = SyntaxNode::new_root(parse_file(db, file).tree(db).clone());
    let mut collector = ExprCollector::new();
    let mut params = Vec::new();
    let body_expr = match def {
        DefWithBody::Function(id) => {
            let decl = id.item(db).ast_id.to_node(db, file, &root).and_then(ast::FuncDecl::cast);
            match decl {
                Some(decl) => {
                    if let Some(list) = decl.param_list() {
                        params = list.params().map(|p| collector.lower_opt_pat(p.pattern())).collect();
                    }
                    collector.lower_opt_expr(decl.body())
                }
                None => collector.missing_expr(),
            }
        }
        DefWithBody::Const(id) => {
            let value = id.item(db).ast_id.to_node(db, file, &root).and_then(ast::Const::cast).and_then(|c| c.expr());
            collector.lower_opt_expr(value)
        }
        DefWithBody::Variant(id) => {
            let value = id
                .parent
                .item(db)
                .ast_id
                .to_node(db, file, &root)
                .and_then(ast::Enum::cast)
                .and_then(|e| e.variants().into_iter().filter(|v| v.name().is_some()).nth(id.index as usize))
                .and_then(|v| v.expr());
            collector.lower_opt_expr(value)
        }
        DefWithBody::File(_) => {
            let statements = match ast::SourceFile::cast(root.clone()) {
                Some(file) => collector.lower_statements(file.elements()),
                None => Vec::new(),
            };
            collector.store.exprs.alloc(Expr::Block {
                statements: statements.into(),
                tail: None,
                label: None,
                kind: BlockKind::Plain,
            })
        }
    };
    let (mut store, source_map) = collector.finish();
    resolve_variant_patterns(db, file, &mut store);
    (Body { store, params: params.into(), body_expr }, source_map)
}

/// A name on its own in a pattern that's a variant in the scope of the
/// file, like the `None` of `match o { None => 0 }` after `import Option.*;`,
/// matches that variant instead of binding a new name, as in Rust. So do
/// `True` and `False`, unless the file has a value with their name.
fn resolve_variant_patterns(db: &dyn Db, file: File, store: &mut ExpressionStore) {
    let scope = &file_scope(db, file).scope;
    let variants: Vec<_> = store
        .pats
        .iter()
        .filter_map(|(pat, p)| match p {
            Pat::Bind { id, subpat: None } => Some((pat, store.bindings[*id].name.clone())),
            _ => None,
        })
        .filter(|(_, name)| match scope.get(Namespace::Values, name).map(|entry| entry.def) {
            Some(def) => matches!(def, ModuleDefId::Variant(_)),
            None => BuiltinValue::by_name(name.as_str()).is_some(),
        })
        .collect();
    for (pat, name) in variants {
        store.pats[pat] = Pat::Path(Path::from_name(name));
    }
}

/// The body of `def`. Unlike [`body_with_source_map`], this doesn't change
/// when the body only moves, so the queries that depend on it don't rerun.
#[salsa::tracked(returns(ref))]
pub fn body<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> Body {
    body_with_source_map(db, def).0.clone()
}

impl Body {
    /// The patterns of the parameters, with whether they are bindings.
    pub fn param_pats(&self) -> impl Iterator<Item = (PatId, &Pat)> {
        self.params.iter().map(|p| (*p, &self.store[*p]))
    }
}

#[cfg(test)]
mod tests;
