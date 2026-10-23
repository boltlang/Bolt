//! The diagnostics of a file, from every stage.
//!
//! The stages report problems in terms of IDs, like the [`TypeRefId`] of a
//! type that can't be found, so that their results don't change when code
//! only moves. This is where the problems get a place in the source. An ID
//! only means something in the store it's from, so each diagnostic is looked
//! up in the source map of the signature or body that reported it.
//!
//! Type errors about types that contain [`Ty::Error`] are left out, since
//! another diagnostic already explains where the error came from.

use boltlang_base_db::{Db, File, parse_file};
use boltlang_hir_def::{
    Name,
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    expr_store::{ExpressionStoreSourceMap, SyntaxPtr},
    hir::TypeRefId,
    ids::{ClassId, ConstId, EnumId, EnumVariantId, ExternBlockId, FunctionId, ImplId, ItemContainerId, StructId, TypeAliasId},
    item_tree::{AssocItem, AstId, FunctionName, ModItem, item_tree},
    nameres::{NameResDiagnostic, file_scope},
    resolver::GenericDefId,
    signatures::{
        FieldList, class_signature_with_source_map, const_signature_with_source_map, enum_signature_with_source_map,
        function_signature_with_source_map, impl_signature_with_source_map, struct_signature_with_source_map,
        type_alias_signature_with_source_map,
    },
};
use boltlang_hir_ty::{
    InferenceDiagnostic, InferenceResult, Ty,
    infer_body,
    lower::{
        ClassConstraint, LowerDiagnostic, VariantDef, const_ty, enum_field_types, fn_sig, generic_predicates,
        impl_header, struct_field_types, type_alias_ty,
    },
    scheme::class_impl_method_sig,
    solve::{ExprOrPat, Origin, Reason, TypeError},
};
use boltlang_syntax::{
    SyntaxKind::{self, FIELD_EXPR, NAME, NAME_REF, STRING},
    SyntaxNode,
    rowan::{TextRange, TextSize},
};

/// A problem in a file, and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic<'db> {
    pub file: File,
    pub range: TextRange,
    pub kind: DiagnosticKind<'db>,
}

/// What the problem is. The name or type the source refers to isn't stored
/// when it's the text at the range, like for [`DiagnosticKind::UnresolvedName`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind<'db> {
    /// A syntax error, with the parser's message.
    Syntax(String),

    // Names
    /// Two definitions or imports of the same name in the same namespace.
    DuplicateDefinition { name: Name },
    /// The file to import from doesn't exist, or the path isn't relative.
    UnresolvedImportPath { path: String },
    /// The file to import from has no item with this name.
    UnresolvedImportName { name: Name },
    /// The item to import exists but isn't public.
    PrivateImport { name: Name },
    UnresolvedName,
    UnresolvedType,
    /// A path that names something other than a type, like a class.
    NotAType,
    /// A bound or `where` predicate that isn't a class.
    NotAClass,

    // Types
    /// `because` is what made the expected type expected, if it's known.
    TypeMismatch { expected: Ty<'db>, actual: Ty<'db>, because: Option<Because<'db>> },
    InfiniteType { ty: Ty<'db> },
    NoInstance { constraint: ClassConstraint<'db> },
    /// A class constraint whose types are never found, so it's not known
    /// which instance to use.
    AmbiguousConstraint { constraint: ClassConstraint<'db> },
    NotAFunction { ty: Ty<'db> },
    WrongArgCount { expected: usize, found: usize },
    WrongTypeArgCount { expected: usize, found: usize },
    NoSuchField { name: Name },
    MissingFields { names: Vec<Name> },
    /// A field or index of a value whose type isn't known yet.
    TypeNeeded,
    /// A type alias that refers to itself.
    RecursiveAlias,
    /// A function whose type can't be inferred, because functions without
    /// complete signatures in files that import each other call each other.
    CyclicInference,
    /// A `break` or `continue` without a loop or label to leave.
    BreakOutside,
    /// A function in an impl that its class doesn't declare.
    NotAMethodOf { name: Name, class: ClassId<'db> },
    /// Methods of the class without a default that the impl doesn't define,
    /// in the order the class declares them.
    MissingMethods { names: Vec<Name>, class: ClassId<'db> },

    /// Something the checker doesn't support yet, like `?` or `impl Trait`.
    Unsupported,
}

/// The code that made a type expected, like the annotation of a `let`. It
/// can be in another file, like the parameter of an imported function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Because<'db> {
    pub file: File,
    pub range: TextRange,
    pub kind: BecauseKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BecauseKind<'db> {
    /// A type annotation, like that of `let x: i32`.
    Annotation,
    /// The declared return type of the function.
    ReturnType,
    /// The declared type of the const.
    ConstType,
    /// A parameter of the function being called: its type, or its pattern
    /// if no type is written.
    Parameter { function: Option<Name> },
    /// The declared type of a field.
    Field { name: Name },
    /// Another expression that must have the same type, and its type.
    SameAs { ty: Ty<'db> },
}

/// The diagnostics of `file`, sorted by where they are.
pub fn file_diagnostics<'db>(db: &'db dyn Db, file: File) -> Vec<Diagnostic<'db>> {
    let root = SyntaxNode::new_root(parse_file(db, file).tree(db).clone());
    let mut collector = Collector { db, file, root, fallback: TextRange::empty(TextSize::from(0)), out: Vec::new() };
    collector.syntax_errors();
    collector.name_resolution();
    collector.items();
    collector.body(DefWithBody::File(file));
    let mut out = collector.out;
    out.sort_by_key(|d| (d.range.start(), d.range.end()));
    out.dedup();
    out
}

struct Collector<'db> {
    db: &'db dyn Db,
    file: File,
    root: SyntaxNode,
    /// Where to report a problem whose syntax can't be found: the name of
    /// the item being looked at.
    fallback: TextRange,
    out: Vec<Diagnostic<'db>>,
}

impl<'db> Collector<'db> {
    fn push(&mut self, range: Option<TextRange>, kind: DiagnosticKind<'db>) {
        let range = range.unwrap_or(self.fallback);
        self.out.push(Diagnostic { file: self.file, range, kind });
    }

    fn syntax_errors(&mut self) {
        for error in parse_file(self.db, self.file).errors(self.db) {
            let span = error.span();
            let range = TextRange::new(TextSize::from(span.start as u32), TextSize::from(span.end as u32));
            self.push(Some(range), DiagnosticKind::Syntax(error.message()));
        }
    }

    fn name_resolution(&mut self) {
        for diagnostic in &file_scope(self.db, self.file).diagnostics {
            let (range, kind) = match diagnostic {
                NameResDiagnostic::Duplicate { name, ast_id } => {
                    (self.name_in(*ast_id, name), DiagnosticKind::DuplicateDefinition { name: name.clone() })
                }
                NameResDiagnostic::UnresolvedImportPath { import, path } => {
                    let node = import.to_node(&self.root);
                    let string = node.as_ref().and_then(|n| {
                        n.children_with_tokens().filter_map(|e| e.into_token()).find(|t| t.kind() == STRING)
                    });
                    let range = string.map(|t| t.text_range()).or(node.map(|n| n.text_range()));
                    (range, DiagnosticKind::UnresolvedImportPath { path: path.clone() })
                }
                NameResDiagnostic::UnresolvedImportName { import, name } => {
                    (self.name_in(*import, name), DiagnosticKind::UnresolvedImportName { name: name.clone() })
                }
                NameResDiagnostic::PrivateImport { import, name } => {
                    (self.name_in(*import, name), DiagnosticKind::PrivateImport { name: name.clone() })
                }
            };
            self.push(range, kind);
        }
    }

    fn items(&mut self) {
        let db = self.db;
        let file = self.file;
        let container = ItemContainerId::File(file);
        for item in &item_tree(db, file).top_level {
            match *item {
                ModItem::Function(idx) => self.function(FunctionId::new(db, container, idx)),
                ModItem::Struct(idx) => {
                    let id = StructId::new(db, file, idx);
                    self.fallback = self.item_range(id.item(db).ast_id);
                    let (_, map) = struct_signature_with_source_map(db, id);
                    self.lower(map, &struct_field_types(db, id).diagnostics);
                    self.lower(map, &generic_predicates(db, GenericDefId::Struct(id)).diagnostics);
                }
                ModItem::Enum(idx) => self.enum_(EnumId::new(db, file, idx)),
                ModItem::Class(idx) => {
                    let id = ClassId::new(db, file, idx);
                    self.fallback = self.item_range(id.item(db).ast_id);
                    let (_, map) = class_signature_with_source_map(db, id);
                    self.lower(map, &generic_predicates(db, GenericDefId::Class(id)).diagnostics);
                    self.assoc_items(ItemContainerId::Class(id), &id.item(db).items);
                }
                ModItem::Impl(idx) => {
                    let id = ImplId::new(db, file, idx);
                    self.fallback = self.item_range(id.item(db).ast_id);
                    let (_, map) = impl_signature_with_source_map(db, id);
                    self.lower(map, &impl_header(db, id).diagnostics);
                    self.lower(map, &generic_predicates(db, GenericDefId::Impl(id)).diagnostics);
                    self.impl_methods(id, map);
                    self.assoc_items(ItemContainerId::Impl(id), &id.item(db).items);
                }
                ModItem::Const(idx) => self.const_(ConstId::new(db, container, idx)),
                ModItem::TypeAlias(idx) => self.type_alias(TypeAliasId::new(db, container, idx)),
                ModItem::ExternBlock(idx) => {
                    let id = ExternBlockId::new(db, file, idx);
                    self.assoc_items(ItemContainerId::ExternBlock(id), &id.item(db).items);
                }
                // Imports are checked by name resolution.
                ModItem::Import(_) | ModItem::MacroRules(_) => {}
            }
        }
    }

    /// Check the functions of an impl against the methods of its class:
    /// those the class doesn't declare, like one with a typo in its name,
    /// and those the class declares without a default that are missing.
    /// `map` is the source map of the impl's signature.
    fn impl_methods(&mut self, imp: ImplId<'db>, map: &ExpressionStoreSourceMap) {
        let db = self.db;
        let Some(class) = impl_header(db, imp).class.as_ref().map(|c| c.class) else { return };
        let functions = |items: &'db [AssocItem], container| {
            items.iter().filter_map(move |item| match *item {
                AssocItem::Function(idx) => Some(FunctionId::new(db, container, idx)),
                _ => None,
            })
        };
        let mut defined = Vec::new();
        for f in functions(&imp.item(db).items, ItemContainerId::Impl(imp)) {
            let Some(name) = &f.item(db).name else { continue };
            defined.push(name);
            if class_impl_method_sig(db, f).is_none() {
                let range = self.item_range(f.item(db).ast_id);
                self.push(Some(range), DiagnosticKind::NotAMethodOf { name: function_name(name), class });
            }
        }
        let names: Vec<Name> = functions(&class.item(db).items, ItemContainerId::Class(class))
            .filter(|m| !m.item(db).has_body)
            .filter_map(|m| m.item(db).name.as_ref())
            .filter(|name| !defined.contains(name))
            .map(function_name)
            .collect();
        if !names.is_empty() {
            let (sig, _) = impl_signature_with_source_map(db, imp);
            let range = map.type_syntax(sig.class).map(ptr_range);
            self.push(range, DiagnosticKind::MissingMethods { names, class });
        }
    }

    fn assoc_items(&mut self, container: ItemContainerId<'db>, items: &[AssocItem]) {
        for item in items {
            match *item {
                AssocItem::Function(idx) => self.function(FunctionId::new(self.db, container, idx)),
                AssocItem::Const(idx) => self.const_(ConstId::new(self.db, container, idx)),
                AssocItem::TypeAlias(idx) => self.type_alias(TypeAliasId::new(self.db, container, idx)),
                AssocItem::MacroCall(_) => {}
            }
        }
    }

    fn function(&mut self, id: FunctionId<'db>) {
        let db = self.db;
        self.fallback = self.item_range(id.item(db).ast_id);
        let (_, map) = function_signature_with_source_map(db, id);
        self.lower(map, &fn_sig(db, id).diagnostics);
        self.lower(map, &generic_predicates(db, GenericDefId::Function(id)).diagnostics);
        if id.item(db).has_body {
            self.body(DefWithBody::Function(id));
        }
    }

    fn enum_(&mut self, id: EnumId<'db>) {
        let db = self.db;
        self.fallback = self.item_range(id.item(db).ast_id);
        let (_, map) = enum_signature_with_source_map(db, id);
        for variant in enum_field_types(db, id) {
            self.lower(map, &variant.diagnostics);
        }
        self.lower(map, &generic_predicates(db, GenericDefId::Enum(id)).diagnostics);
        for index in 0..id.item(db).variants.len() {
            self.body(DefWithBody::Variant(EnumVariantId { parent: id, index: index as u32 }));
        }
    }

    fn const_(&mut self, id: ConstId<'db>) {
        let db = self.db;
        self.fallback = self.item_range(id.item(db).ast_id);
        let (_, map) = const_signature_with_source_map(db, id);
        self.lower(map, &const_ty(db, id).diagnostics);
        self.body(DefWithBody::Const(id));
    }

    fn type_alias(&mut self, id: TypeAliasId<'db>) {
        let db = self.db;
        self.fallback = self.item_range(id.item(db).ast_id);
        let (_, map) = type_alias_signature_with_source_map(db, id);
        let ty = type_alias_ty(db, id);
        // The alias is where its cycle is found, so it has no diagnostics of
        // its own about it; its uses do.
        if ty.recursive {
            self.push(None, DiagnosticKind::RecursiveAlias);
        }
        self.lower(map, &ty.diagnostics);
        self.lower(map, &generic_predicates(db, GenericDefId::TypeAlias(id)).diagnostics);
    }

    /// Report problems with type references, from the store `map` is for.
    fn lower(&mut self, map: &ExpressionStoreSourceMap, diagnostics: &[LowerDiagnostic]) {
        for diagnostic in diagnostics {
            let (type_ref, kind): (TypeRefId, _) = match diagnostic {
                LowerDiagnostic::UnresolvedType(id) => (*id, DiagnosticKind::UnresolvedType),
                LowerDiagnostic::NotAType(id) => (*id, DiagnosticKind::NotAType),
                LowerDiagnostic::NotAClass(id) => (*id, DiagnosticKind::NotAClass),
                LowerDiagnostic::WrongArgCount { type_ref, expected, found } => {
                    (*type_ref, DiagnosticKind::WrongTypeArgCount { expected: *expected, found: *found })
                }
                LowerDiagnostic::RecursiveAlias(id) => (*id, DiagnosticKind::RecursiveAlias),
                LowerDiagnostic::Unsupported(id) => (*id, DiagnosticKind::Unsupported),
            };
            self.push(map.type_syntax(type_ref).map(ptr_range), kind);
        }
    }

    fn body(&mut self, def: DefWithBody<'db>) {
        let db = self.db;
        let id = DefWithBodyId::new(db, def);
        let (_, map) = body_with_source_map(db, id);
        let at = |node: ExprOrPat| node_range(map, node);
        let result = infer_body(db, id);
        for diagnostic in result.diagnostics.iter().cloned() {
            let (range, kind) = match diagnostic {
                InferenceDiagnostic::Type(error) => {
                    // The origin is always in the body being inferred.
                    let (origin, kind) = match error {
                        TypeError::Mismatch { origin, expected, actual } => {
                            if has_error(&expected) || has_error(&actual) {
                                continue;
                            }
                            let because = self.because(origin, result);
                            (origin, DiagnosticKind::TypeMismatch { expected, actual, because })
                        }
                        TypeError::InfiniteType { origin, ty } => (origin, DiagnosticKind::InfiniteType { ty }),
                        TypeError::NoInstance { origin, constraint } => {
                            if constraint.args.iter().any(has_error) {
                                continue;
                            }
                            (origin, DiagnosticKind::NoInstance { constraint })
                        }
                        TypeError::Ambiguous { origin, constraint } => {
                            if constraint.args.iter().any(has_error) {
                                continue;
                            }
                            (origin, DiagnosticKind::AmbiguousConstraint { constraint })
                        }
                    };
                    debug_assert_eq!(origin.body, id);
                    (at(origin.node), kind)
                }
                InferenceDiagnostic::Lower(diagnostic) => {
                    self.lower(map, &[diagnostic]);
                    continue;
                }
                InferenceDiagnostic::UnresolvedName { expr } => {
                    (self.unresolved_range(map, expr), DiagnosticKind::UnresolvedName)
                }
                InferenceDiagnostic::NotAFunction { expr, ty } => {
                    if has_error(&ty) {
                        continue;
                    }
                    (at(ExprOrPat::Expr(expr)), DiagnosticKind::NotAFunction { ty })
                }
                InferenceDiagnostic::WrongArgCount { expr, expected, found } => {
                    (at(expr), DiagnosticKind::WrongArgCount { expected, found })
                }
                InferenceDiagnostic::NoSuchField { expr, name } => (at(expr), DiagnosticKind::NoSuchField { name }),
                InferenceDiagnostic::MissingFields { expr, names } => {
                    (at(ExprOrPat::Expr(expr)), DiagnosticKind::MissingFields { names })
                }
                InferenceDiagnostic::TypeNeeded { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::TypeNeeded),
                InferenceDiagnostic::BreakOutside { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::BreakOutside),
                InferenceDiagnostic::Unsupported { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::Unsupported),
                // Reported at the function, which is the fallback.
                InferenceDiagnostic::CyclicInference => (None, DiagnosticKind::CyclicInference),
            };
            self.push(range, kind);
        }
    }

    /// Where the reason of `origin` is, in a body of this file.
    fn because(&self, origin: Origin<'db>, result: &InferenceResult<'db>) -> Option<Because<'db>> {
        let db = self.db;
        let (_, body_map) = body_with_source_map(db, origin.body);
        let (file, ptr, kind) = match origin.reason {
            Reason::None => return None,
            Reason::Annotation(type_ref) => (self.file, body_map.type_syntax(type_ref)?, BecauseKind::Annotation),
            Reason::ReturnType => {
                let DefWithBody::Function(f) = origin.body.def(db) else { return None };
                let (sig, map) = function_signature_with_source_map(db, f);
                (self.file, map.type_syntax(sig.ret_type?)?, BecauseKind::ReturnType)
            }
            Reason::ConstType => {
                let DefWithBody::Const(c) = origin.body.def(db) else { return None };
                let (sig, map) = const_signature_with_source_map(db, c);
                (self.file, map.type_syntax(sig.type_ref)?, BecauseKind::ConstType)
            }
            Reason::Argument { function, index } => {
                let (sig, map) = function_signature_with_source_map(db, function);
                let index = index as usize;
                let name = function.item(db).name.as_ref().map(|n| match n {
                    FunctionName::Name(n) | FunctionName::Operator(n) => n.clone(),
                });
                let ptr = match sig.params.get(index)?.type_ref {
                    Some(type_ref) => map.type_syntax(type_ref)?,
                    None => {
                        let def = DefWithBodyId::new(db, DefWithBody::Function(function));
                        let (body, map) = body_with_source_map(db, def);
                        map.pat_syntax(*body.params.get(index)?)?
                    }
                };
                (function.file(db), ptr, BecauseKind::Parameter { function: name })
            }
            Reason::Field { variant, index } => {
                let (file, fields, map) = match variant {
                    VariantDef::Struct(s) => {
                        let (sig, map) = struct_signature_with_source_map(db, s);
                        (s.file(db), &sig.fields, map)
                    }
                    VariantDef::Variant(v) => {
                        let (sig, map) = enum_signature_with_source_map(db, v.parent);
                        (v.parent.file(db), &sig.variants.get(v.index as usize)?.fields, map)
                    }
                };
                let (FieldList::Record(fields) | FieldList::Tuple(fields)) = fields else { return None };
                let field = fields.get(index as usize)?;
                (file, map.type_syntax(field.type_ref)?, BecauseKind::Field { name: field.name.clone() })
            }
            Reason::SameAs(expr) => {
                let ty = result.expr_ty(expr).clone();
                (self.file, body_map.expr_syntax(expr)?, BecauseKind::SameAs { ty })
            }
        };
        Some(Because { file, range: ptr_range(ptr), kind })
    }

    /// Where a name that can't be found is. For `x.f` or `x.f()`, that's the
    /// `f`.
    fn unresolved_range(&self, map: &ExpressionStoreSourceMap, node: ExprOrPat) -> Option<TextRange> {
        let range = node_range(map, node);
        let ExprOrPat::Expr(expr) = node else { return range };
        let ptr = map.expr_syntax(expr)?;
        if ptr.kind != FIELD_EXPR {
            return range;
        }
        let name = ptr.to_node(&self.root)?.children().filter(|n| n.kind() == NAME_REF).last();
        name.map(|n| n.text_range()).or(range)
    }

    /// The name of an item, or the whole item if it has none.
    fn item_range(&self, ast_id: AstId) -> TextRange {
        let Some(node) = ast_id.to_node(&self.root) else { return self.fallback };
        node.children().find(|n| n.kind() == NAME).unwrap_or(node).text_range()
    }

    /// Where `name` is in the item or import `ast_id`. For an item, that's
    /// the name it defines; for an import, the name in its list.
    fn name_in(&self, ast_id: AstId, name: &Name) -> Option<TextRange> {
        let node = ast_id.to_node(&self.root)?;
        if ast_id.kind != SyntaxKind::IMPORT {
            return Some(self.item_range(ast_id));
        }
        let found = node
            .descendants()
            .filter(|n| matches!(n.kind(), NAME | NAME_REF))
            .filter(|n| n.text() == name.as_str())
            .last();
        Some(found.unwrap_or(node).text_range())
    }
}

fn function_name(name: &FunctionName) -> Name {
    match name {
        FunctionName::Name(n) | FunctionName::Operator(n) => n.clone(),
    }
}

fn ptr_range(ptr: SyntaxPtr) -> TextRange {
    TextRange::new(ptr.start.into(), ptr.end.into())
}

fn node_range(map: &ExpressionStoreSourceMap, node: ExprOrPat) -> Option<TextRange> {
    let ptr = match node {
        ExprOrPat::Expr(e) => map.expr_syntax(e),
        ExprOrPat::Pat(p) => map.pat_syntax(p),
    };
    ptr.map(ptr_range)
}

fn has_error(ty: &Ty<'_>) -> bool {
    ty.any(&mut |t| matches!(t, Ty::Error))
}
