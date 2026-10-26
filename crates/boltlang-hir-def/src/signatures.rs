//! Signatures: everything about an item except its body, lowered into an
//! [`ExpressionStore`], since signatures can contain expressions too
//! (default parameter values, array lengths, const generic arguments).
//!
//! Like bodies, each signature has a query with its source map and one
//! without, so that what depends on a signature doesn't rerun when it only
//! moves.

use boltlang_base_db::{Db, File, parse_file};
use boltlang_syntax::{SyntaxKind::*, SyntaxNode, ast::{self, Node}};

use crate::{
    Name,
    expr_store::{ExprCollector, ExpressionStore, ExpressionStoreSourceMap, unquote},
    hir::{ExprId, TypeRefId},
    ids::{ClassId, ConstId, EnumId, FunctionId, ImplId, StructId, TypeAliasId},
    item_tree::{AstId, FunctionName, RawVisibility},
};

/// The generic parameters of an item and its `where` clauses.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct GenericParams {
    pub type_params: Box<[TypeParamData]>,
    pub const_params: Box<[ConstParamData]>,
    pub where_predicates: Box<[WherePredicate]>,
}

/// `T`, `T: Ord` or `T = Default`
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct TypeParamData {
    pub name: Name,
    pub bounds: Box<[TypeRefId]>,
    pub default: Option<TypeRefId>,
}

/// `const N: usize = 3`
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ConstParamData {
    pub name: Name,
    pub type_ref: TypeRefId,
    pub default: Option<ExprId>,
}

/// `T: Show + Eq`, or a class constraint on its own like `Convert[A, B]`,
/// which has no bounds.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct WherePredicate {
    pub target: TypeRefId,
    pub bounds: Box<[TypeRefId]>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FunctionSignature {
    pub name: Option<FunctionName>,
    pub visibility: RawVisibility,
    pub generic_params: GenericParams,
    /// The parameters, with their types and default values. Their patterns
    /// are in the body.
    pub params: Box<[ParamData]>,
    pub is_variadic: bool,
    pub ret_type: Option<TypeRefId>,
    pub is_async: bool,
    pub is_const: bool,
    /// `Some` for `extern "C" fn`, with the ABI if one is given.
    pub abi: Option<Option<String>>,
    pub store: ExpressionStore,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ParamData {
    pub type_ref: Option<TypeRefId>,
    pub default: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct StructSignature {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    pub generic_params: GenericParams,
    pub fields: FieldList,
    pub store: ExpressionStore,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum FieldList {
    /// `{ x: i32 }`
    Record(Box<[FieldData]>),
    /// `(i32, String)`, with fields named `0`, `1`, ...
    Tuple(Box<[FieldData]>),
    /// No fields, like `struct Unit;` or the variant `None`.
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FieldData {
    pub name: Name,
    pub visibility: RawVisibility,
    pub type_ref: TypeRefId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct EnumSignature {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    pub generic_params: GenericParams,
    /// The variants, in the same order as in the item tree. Their
    /// discriminants are bodies.
    pub variants: Box<[VariantData]>,
    pub store: ExpressionStore,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct VariantData {
    pub name: Name,
    pub fields: FieldList,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ClassSignature {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    /// The class parameters, and the superclasses in the `where` clause.
    pub generic_params: GenericParams,
    pub store: ExpressionStore,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ImplSignature {
    pub generic_params: GenericParams,
    /// The class instance, like `Show[Dog]`. Name resolution checks that
    /// it's a class.
    pub class: TypeRefId,
    pub store: ExpressionStore,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ConstSignature {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    pub type_ref: TypeRefId,
    pub store: ExpressionStore,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct TypeAliasSignature {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    pub generic_params: GenericParams,
    /// The bounds of an associated type, like `type Item: Show;`.
    pub bounds: Box<[TypeRefId]>,
    /// The aliased type. `None` for an associated type without a default.
    pub type_ref: Option<TypeRefId>,
    pub store: ExpressionStore,
}

// For each kind of signature, one query with the source map and one
// without, which only changes when the signature does.

#[salsa::tracked(returns(ref))]
pub fn function_signature_with_source_map<'db>(db: &'db dyn Db, id: FunctionId<'db>) -> (FunctionSignature, ExpressionStoreSourceMap) {
    lower_function(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn function_signature<'db>(db: &'db dyn Db, id: FunctionId<'db>) -> FunctionSignature {
    function_signature_with_source_map(db, id).0.clone()
}

#[salsa::tracked(returns(ref))]
pub fn struct_signature_with_source_map<'db>(db: &'db dyn Db, id: StructId<'db>) -> (StructSignature, ExpressionStoreSourceMap) {
    lower_struct(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn struct_signature<'db>(db: &'db dyn Db, id: StructId<'db>) -> StructSignature {
    struct_signature_with_source_map(db, id).0.clone()
}

#[salsa::tracked(returns(ref))]
pub fn enum_signature_with_source_map<'db>(db: &'db dyn Db, id: EnumId<'db>) -> (EnumSignature, ExpressionStoreSourceMap) {
    lower_enum(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn enum_signature<'db>(db: &'db dyn Db, id: EnumId<'db>) -> EnumSignature {
    enum_signature_with_source_map(db, id).0.clone()
}

#[salsa::tracked(returns(ref))]
pub fn class_signature_with_source_map<'db>(db: &'db dyn Db, id: ClassId<'db>) -> (ClassSignature, ExpressionStoreSourceMap) {
    lower_class(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn class_signature<'db>(db: &'db dyn Db, id: ClassId<'db>) -> ClassSignature {
    class_signature_with_source_map(db, id).0.clone()
}

#[salsa::tracked(returns(ref))]
pub fn impl_signature_with_source_map<'db>(db: &'db dyn Db, id: ImplId<'db>) -> (ImplSignature, ExpressionStoreSourceMap) {
    lower_impl(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn impl_signature<'db>(db: &'db dyn Db, id: ImplId<'db>) -> ImplSignature {
    impl_signature_with_source_map(db, id).0.clone()
}

#[salsa::tracked(returns(ref))]
pub fn const_signature_with_source_map<'db>(db: &'db dyn Db, id: ConstId<'db>) -> (ConstSignature, ExpressionStoreSourceMap) {
    lower_const(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn const_signature<'db>(db: &'db dyn Db, id: ConstId<'db>) -> ConstSignature {
    const_signature_with_source_map(db, id).0.clone()
}

#[salsa::tracked(returns(ref))]
pub fn type_alias_signature_with_source_map<'db>(db: &'db dyn Db, id: TypeAliasId<'db>) -> (TypeAliasSignature, ExpressionStoreSourceMap) {
    lower_type_alias(db, id)
}

#[salsa::tracked(returns(ref))]
pub fn type_alias_signature<'db>(db: &'db dyn Db, id: TypeAliasId<'db>) -> TypeAliasSignature {
    type_alias_signature_with_source_map(db, id).0.clone()
}

/// The syntax of the item at `ast_id` in `file`, if it's an `N`.
fn item_node<N: Node>(db: &dyn Db, file: File, ast_id: AstId) -> Option<N> {
    ast_id.to_node(&SyntaxNode::new_root(parse_file(db, file).tree(db).clone())).and_then(N::cast)
}

fn lower_function<'db>(db: &'db dyn Db, id: FunctionId<'db>) -> (FunctionSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let decl = item_node::<ast::FuncDecl>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let mut params = Vec::new();
    let mut is_variadic = false;
    let mut generic_params = GenericParams::default();
    let mut ret_type = None;
    let mut abi = None;
    if let Some(decl) = &decl {
        generic_params = lower_generic_params(&mut c, decl.generic_param_list(), decl.where_clause().into_iter());
        if let Some(list) = decl.param_list() {
            is_variadic = list.variadic_param().is_some();
            for param in list.params() {
                let type_ref = param.type_expr().map(|t| c.lower_type(t));
                let default = param.default().map(|e| c.lower_expr(e));
                params.push(ParamData { type_ref, default });
            }
        }
        ret_type = decl.ret_type().map(|r| c.lower_opt_type(r.ty()));
        abi = decl.abi().map(|abi| abi.string().map(|s| unquote(s.text())));
    }
    let (store, source_map) = c.finish();
    let sig = FunctionSignature {
        name: item.name.clone(),
        visibility: item.visibility,
        generic_params,
        params: params.into(),
        is_variadic,
        ret_type,
        is_async: item.is_async,
        is_const: item.is_const,
        abi,
        store,
    };
    (sig, source_map)
}

fn lower_struct<'db>(db: &'db dyn Db, id: StructId<'db>) -> (StructSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let node = item_node::<ast::Struct>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let (generic_params, fields) = match &node {
        Some(node) => (
            lower_generic_params(&mut c, node.generic_param_list(), node.where_clause().into_iter()),
            lower_field_list(&mut c, node.field_list()),
        ),
        None => (GenericParams::default(), FieldList::Unit),
    };
    let (store, source_map) = c.finish();
    (StructSignature { name: item.name.clone(), visibility: item.visibility, generic_params, fields, store }, source_map)
}

fn lower_enum<'db>(db: &'db dyn Db, id: EnumId<'db>) -> (EnumSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let node = item_node::<ast::Enum>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let mut generic_params = GenericParams::default();
    let mut variants = Vec::new();
    if let Some(node) = &node {
        generic_params = lower_generic_params(&mut c, node.generic_param_list(), node.where_clause().into_iter());
        for variant in node.variants() {
            // Like the item tree, leave out variants without a name.
            let Some(name) = variant.name() else { continue };
            let fields = lower_field_list(&mut c, variant.field_list());
            variants.push(VariantData { name: Name::from_ident(&name.text()), fields });
        }
    }
    let (store, source_map) = c.finish();
    let sig =
        EnumSignature { name: item.name.clone(), visibility: item.visibility, generic_params, variants: variants.into(), store };
    (sig, source_map)
}

fn lower_class<'db>(db: &'db dyn Db, id: ClassId<'db>) -> (ClassSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let node = item_node::<ast::Class>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let generic_params = match &node {
        Some(node) => lower_generic_params(&mut c, node.generic_param_list(), node.where_clause().into_iter()),
        None => GenericParams::default(),
    };
    let (store, source_map) = c.finish();
    (ClassSignature { name: item.name.clone(), visibility: item.visibility, generic_params, store }, source_map)
}

fn lower_impl<'db>(db: &'db dyn Db, id: ImplId<'db>) -> (ImplSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let node = item_node::<ast::Impl>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let (generic_params, class) = match &node {
        Some(node) => (
            lower_generic_params(&mut c, node.generic_param_list(), node.where_clause().into_iter()),
            c.lower_opt_type(node.ty()),
        ),
        None => (GenericParams::default(), c.lower_opt_type(None)),
    };
    let (store, source_map) = c.finish();
    (ImplSignature { generic_params, class, store }, source_map)
}

fn lower_const<'db>(db: &'db dyn Db, id: ConstId<'db>) -> (ConstSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let node = item_node::<ast::Const>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let type_ref = c.lower_opt_type(node.and_then(|n| n.ty()));
    let (store, source_map) = c.finish();
    (ConstSignature { name: item.name.clone(), visibility: item.visibility, type_ref, store }, source_map)
}

fn lower_type_alias<'db>(db: &'db dyn Db, id: TypeAliasId<'db>) -> (TypeAliasSignature, ExpressionStoreSourceMap) {
    let item = id.item(db);
    let node = item_node::<ast::TypeAlias>(db, id.file(db), item.ast_id);
    let mut c = ExprCollector::new();
    let mut generic_params = GenericParams::default();
    let mut bounds = Vec::new();
    let mut type_ref = None;
    if let Some(node) = &node {
        generic_params = lower_generic_params(&mut c, node.generic_param_list(), node.where_clauses());
        bounds = node.bounds().map(|b| b.bounds().map(|t| c.lower_type(t)).collect()).unwrap_or_default();
        type_ref = node.ty().map(|t| c.lower_type(t));
    }
    let (store, source_map) = c.finish();
    let sig = TypeAliasSignature {
        name: item.name.clone(),
        visibility: item.visibility,
        generic_params,
        bounds: bounds.into(),
        type_ref,
        store,
    };
    (sig, source_map)
}

fn lower_generic_params(
    c: &mut ExprCollector,
    list: Option<ast::GenericParamList>,
    where_clauses: impl Iterator<Item = ast::WhereClause>,
) -> GenericParams {
    let mut type_params = Vec::new();
    let mut const_params = Vec::new();
    for param in list.iter().flat_map(|l| l.params()) {
        match param {
            ast::GenericParam::Type(p) => {
                // A metavariable like `$t` in a macro has no NAME node.
                let name = match p.name() {
                    Some(name) => Name::from_ident(&name.text()),
                    None => match p.syntax().children_with_tokens().find(|t| t.kind() == METAVARIABLE) {
                        Some(token) => Name::new(token.as_token().unwrap().text()),
                        None => continue,
                    },
                };
                let bounds = p.bounds().map(|b| b.bounds().map(|t| c.lower_type(t)).collect()).unwrap_or_default();
                let default = p.default_type().map(|t| c.lower_type(t));
                type_params.push(TypeParamData { name, bounds, default });
            }
            ast::GenericParam::Const(p) => {
                let Some(name) = p.name() else { continue };
                let type_ref = c.lower_opt_type(p.ty());
                let default = p.default_value().map(|e| c.lower_expr(e));
                const_params.push(ConstParamData { name: Name::from_ident(&name.text()), type_ref, default });
            }
        }
    }
    let mut where_predicates = Vec::new();
    for clause in where_clauses {
        for pred in clause.predicates() {
            let target = c.lower_opt_type(pred.ty());
            let bounds = pred.bounds().map(|b| b.bounds().map(|t| c.lower_type(t)).collect()).unwrap_or_default();
            where_predicates.push(WherePredicate { target, bounds });
        }
    }
    GenericParams {
        type_params: type_params.into(),
        const_params: const_params.into(),
        where_predicates: where_predicates.into(),
    }
}

fn lower_field_list(c: &mut ExprCollector, list: Option<ast::FieldList>) -> FieldList {
    let visibility = RawVisibility::from_ast;
    match list {
        None => FieldList::Unit,
        Some(ast::FieldList::Record(list)) => FieldList::Record(
            list.fields()
                .filter_map(|f| {
                    let name = Name::from_ident(&f.name()?.text());
                    Some(FieldData { name, visibility: visibility(f.visibility()), type_ref: c.lower_opt_type(f.ty()) })
                })
                .collect(),
        ),
        Some(ast::FieldList::Tuple(list)) => FieldList::Tuple(
            list.fields()
                .enumerate()
                .map(|(i, f)| FieldData {
                    name: Name::new(i.to_string()),
                    visibility: visibility(f.visibility()),
                    type_ref: c.lower_opt_type(f.ty()),
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests;
