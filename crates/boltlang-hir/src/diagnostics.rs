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
use boltlang_hir_def::manifest::{manifest, package_manifest};
use boltlang_hir_def::{
    Name,
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    expr_store::{ExpressionStoreSourceMap, SyntaxPtr},
    hir::{MutabilityRef, TypeRef, TypeRefId},
    ids::{ClassId, ConstId, EnumId, EnumVariantId, ExternBlockId, FunctionId, ImplId, ItemContainerId, StructId, TypeAliasId},
    item_tree::{AssocItem, AstId, ModItem, item_tree},
    nameres::{ImportPathError, NameResDiagnostic, file_scope},
    resolver::GenericDefId,
    signatures::{
        FieldList, class_signature_with_source_map, const_signature_with_source_map, enum_signature_with_source_map,
        function_signature, function_signature_with_source_map, impl_signature, impl_signature_with_source_map, struct_signature_with_source_map,
        type_alias_signature_with_source_map,
    },
};
use boltlang_hir_ty::{
    InferenceDiagnostic, InferenceResult, Ty, function_scheme,
    infer::mut_inside_imm,
    builtin::{Builtin, is_builtin_block},
    kind::Kind,
    inhabited::{field_without_values, has_values},
    check::{unmet_assoc_bounds, unmet_assoc_equalities},
    infer_body,
    overlap::{overlapped_functions, overlapped_impls},
    lower::{
        ClassConstraint, LowerDiagnostic, VariantDef, assoc_type_bounds, const_ty, enum_field_types, fn_sig,
        generic_predicates, impl_header, named_mutabilities, struct_field_types, subst_params, type_alias_ty,
        unused_impl_params,
    },
    ty::GenericParamId,
    solve::{ConstraintSource, ExprOrPat, Held, Origin, Reason, TypeError},
};
use boltlang_syntax::{
    SyntaxKind::{
        self, FIELD_EXPR, FIELD_PATT, GENERIC_PARAM_LIST, NAME, NAME_REF, PATH, PATH_SEGMENT, PATH_TYPE,
        RECORD_EXPR_FIELD, STRING, STRUCT_PATT, TYPE_PARAM,
    },
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
    /// The `bolt.toml` of the file's package, `manifest`, has an error, at
    /// `range` in it if that's known.
    BadManifest { manifest: File, message: String, range: Option<TextRange> },

    // Names
    /// Two definitions or imports of the same name in the same namespace.
    DuplicateDefinition { name: Name },
    /// The path of an import doesn't lead to a file.
    UnresolvedImportPath { path: String, error: ImportPathError },
    /// The file to import from has no item with this name.
    UnresolvedImportName { name: Name },
    /// The item to import, or that a path like `m.f` names, exists but isn't
    /// visible here: it's private, or `pub(package)` in another package
    /// when `package_only` is set.
    NotVisible { name: Name, package_only: bool },
    /// The enum of `import Option.*;` can't be found.
    UnresolvedImportEnum { path: String },
    /// The path of `import Point.*;` isn't an enum.
    NotAnEnum { path: String },
    /// The prelude that was asked for can't be found.
    UnresolvedPrelude { package: String, error: ImportPathError },
    /// The `Vec` of `fn Vec.new()` isn't a struct or enum of the same file.
    UnresolvedOwner { name: Name },
    /// `fn Vec.new()` somewhere other than the top level of a file.
    OwnerNotTopLevel,
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
    NoInstance { constraint: ClassConstraint<'db>, because: Option<Because<'db>> },
    /// A class constraint whose types are never found, so it's not known
    /// which instance to use.
    AmbiguousConstraint { constraint: ClassConstraint<'db>, because: Option<Because<'db>> },
    NotAFunction { ty: Ty<'db> },
    WrongArgCount { expected: usize, found: usize },
    WrongTypeArgCount { expected: usize, found: usize },
    /// A type of the wrong kind, like `i32` where a type constructor like
    /// `Option` is expected.
    KindMismatch { expected: Kind, found: Kind },
    /// A field that the type doesn't have. `ty` is `None` for a field in a
    /// struct literal or pattern, where the struct is named.
    NoSuchField { ty: Option<Ty<'db>>, name: Name, because: Option<Because<'db>> },
    MissingFields { names: Vec<Name> },
    /// A field or index of a value whose type isn't known yet.
    TypeNeeded,
    /// `Iterator[Itme = T]`, where the class has no associated type `name`.
    NotAnAssocTypeOf { class: Name, name: Name },
    /// An associated type given, like `Iterator[Item = T]`, where that
    /// can't be assumed, like in an impl's instance.
    EqualityNotAllowed,
    /// None of the functions called `name` fits its use, of type `ty`.
    /// `candidates` are their types, as text (#275).
    NoOverload { name: Name, ty: Ty<'db>, candidates: Vec<String> },
    /// More than one of the functions called `name` fits its use when the
    /// body is done.
    AmbiguousOverload { name: Name, candidates: Vec<String> },
    /// A function with `#[test]` that can't be run as a test.
    BadTest { reason: BadTest },
    /// A call could fit both this function, of type `ty`, and one with the
    /// same name before it, of type `other`, whose name is at `other_range`
    /// in `other_file` (#275).
    OverlappingFunction { name: Name, ty: String, other: String, other_file: File, other_range: TextRange },
    /// A type alias that refers to itself.
    RecursiveAlias,
    /// A function whose type can't be inferred, because functions without
    /// complete signatures in files that import each other call each other.
    CyclicInference,
    /// A `break` or `continue` without a loop or label to leave.
    BreakOutside,
    /// An assignment to the variable `name`, or to a field of it, while it
    /// isn't `mut`.
    AssignToImmutable { name: Name },
    /// An assignment to an element of an array, or a field of a struct, of
    /// type `ty`, which isn't `mut`.
    CannotChange { ty: Ty<'db> },
    /// `mut` or `imm` before something that isn't an array or a struct.
    NoMutability,
    /// An `imm` array or struct of type `ty` that holds a `mut` one, `inner`.
    MutInsideImm { ty: Ty<'db>, inner: Ty<'db> },
    /// `held`, a type or a mutability, isn't at most the mutability `bound`
    /// (#243), like a value of a type parameter in an `imm` array.
    NotAtMost { held: Ty<'db>, is_mutability: bool, bound: Ty<'db>, because: Option<Because<'db>> },
    /// `mut` in the type of a field.
    MutField,
    /// A mutability on its own where a type is expected.
    MutabilityAlone,
    /// A named mutability that isn't one of the function's or impl's, or one
    /// that the struct declares.
    UnknownMutability,
    /// A parameter of an impl, `name` (with its `'` for a mutability), that
    /// its instance doesn't use, so that choosing the impl can't decide it
    /// (#251). For a mutability, `suggestion` is the first type in the
    /// instance without a written one, with it instead, like `'v Vec[T]`.
    UnusedImplParam { name: String, instance: ClassConstraint<'db>, suggestion: Option<Ty<'db>> },
    /// Some types fit both this impl's instance and that of an impl before
    /// it, `other`, whose instance is at `other_range` in `other_file`
    /// (#276).
    OverlappingImpl { instance: ClassConstraint<'db>, other: ClassConstraint<'db>, other_file: File, other_range: TextRange },
    /// More mutability arguments than the struct declares.
    WrongMutabilityArgCount { expected: usize, found: usize },
    /// A mutability parameter of an item that can't have one, like an enum.
    MutabilityParamNotAllowed,
    /// A path like `C.Item` where no class of `C` declares the associated
    /// type.
    NoAssocType,
    /// A path like `C.Item` where several classes of `C` declare it.
    AmbiguousAssocType,
    /// Syntax that needs a class of the prelude, like `xs[i]` needs
    /// `Index`, without a prelude that defines it.
    NoLangClass { class: &'static str },
    /// The type an impl gives an associated type doesn't meet a bound the
    /// class puts on it, like `type Iter: Iterator;`.
    UnmetAssocBound { name: Name, class: ClassId<'db>, constraint: ClassConstraint<'db> },
    /// An impl's type for the associated type `name` gives another `item`
    /// than its class's bound says, like the `Item = Item` of `type Iter:
    /// Iterator[Item = Item];`.
    UnmetAssocEquality { name: Name, item: Name, class: ClassId<'db>, expected: Ty<'db>, actual: Ty<'db> },
    /// An assignment whose left side isn't a variable, a field or an
    /// element.
    NotAPlace,
    /// An item in an impl that its class doesn't declare.
    NotInClass { kind: ClassItemKind, name: Name, class: ClassId<'db> },
    /// Items of the class without a default that the impl doesn't define,
    /// in the order the class declares them.
    MissingItems { items: Vec<(ClassItemKind, Name)>, class: ClassId<'db> },

    /// A struct or enum without values: building one needs a value of a
    /// type without values, like itself. `field` is a field of the struct,
    /// or of `variant` for an enum, that needs such a value.
    NoValues { name: Name, variant: Option<Name>, field: Name, field_ty: Ty<'db> },

    /// An item of an `extern "bolt"` block that isn't a builtin.
    UnknownBuiltin { name: Name },
    /// A builtin declared with another type than it has.
    BuiltinSignature { name: Name, expected: Ty<'db> },

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
    /// The declaration of the item in the class an impl is for.
    ClassItem { class: ClassId<'db> },
    /// The use in the body of a function that needs what its type asks for,
    /// like the `p.x` of `fn get_x(p) = p.x`.
    UseIn { function: Option<Name> },
    /// A bound or `where` clause that asks for it, and the function, struct
    /// or enum it's of, if any.
    Bound { item: Option<Name> },
}

/// Why a function with `#[test]` can't be run as a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadTest {
    /// It's in a class, an impl, an extern block or a type's namespace.
    NotTopLevel,
    /// It takes parameters, which nothing gives it.
    Params,
    /// It has type parameters, which nothing decides.
    Generic,
    /// It gives something other than `()`, which nothing looks at.
    Result,
}

/// What an item of a class or impl is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassItemKind {
    Method,
    Const,
    Type,
}

/// An item of a class, impl or extern block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssocItemDef<'db> {
    Function(FunctionId<'db>),
    Const(ConstId<'db>),
    TypeAlias(TypeAliasId<'db>),
}

impl<'db> AssocItemDef<'db> {
    fn all(db: &'db dyn Db, container: ItemContainerId<'db>, items: &[AssocItem]) -> Vec<Self> {
        items
            .iter()
            .filter_map(|item| match *item {
                AssocItem::Function(idx) => Some(AssocItemDef::Function(FunctionId::new(db, container, idx))),
                AssocItem::Const(idx) => Some(AssocItemDef::Const(ConstId::new(db, container, idx))),
                AssocItem::TypeAlias(idx) => Some(AssocItemDef::TypeAlias(TypeAliasId::new(db, container, idx))),
                AssocItem::MacroCall(_) => None,
            })
            .collect()
    }

    fn kind(self) -> ClassItemKind {
        match self {
            AssocItemDef::Function(_) => ClassItemKind::Method,
            AssocItemDef::Const(_) => ClassItemKind::Const,
            AssocItemDef::TypeAlias(_) => ClassItemKind::Type,
        }
    }

    fn name(self, db: &'db dyn Db) -> Option<Name> {
        match self {
            AssocItemDef::Function(id) => id.item(db).name.as_ref().map(|n| n.name().clone()),
            AssocItemDef::Const(id) => id.item(db).name.clone(),
            AssocItemDef::TypeAlias(id) => id.item(db).name.clone(),
        }
    }

    fn ast_id(self, db: &'db dyn Db) -> AstId {
        match self {
            AssocItemDef::Function(id) => id.item(db).ast_id,
            AssocItemDef::Const(id) => id.item(db).ast_id,
            AssocItemDef::TypeAlias(id) => id.item(db).ast_id,
        }
    }

    /// Whether an impl can leave it out: a method with a body, a const with a
    /// value, or a type with a definition.
    fn has_default(self, db: &'db dyn Db) -> bool {
        match self {
            AssocItemDef::Function(id) => id.item(db).has_body,
            AssocItemDef::Const(id) => id.item(db).has_value,
            AssocItemDef::TypeAlias(id) => id.item(db).has_value,
        }
    }
}

/// The diagnostics of `file`, sorted by where they are.
pub fn file_diagnostics<'db>(db: &'db dyn Db, file: File) -> Vec<Diagnostic<'db>> {
    let root = SyntaxNode::new_root(parse_file(db, file).tree(db).clone());
    let mut collector = Collector { db, file, root, fallback: TextRange::empty(TextSize::from(0)), out: Vec::new() };
    collector.syntax_errors();
    collector.manifest();
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

    /// An error in the `bolt.toml` of the file's package, reported at the
    /// start of each file of it, which can't use its dependencies.
    fn manifest(&mut self) {
        let Some(manifest_file) = package_manifest(self.db, self.file) else { return };
        if let Err(error) = manifest(self.db, manifest_file) {
            let range = error.range.map(|(start, end)| TextRange::new(start.into(), end.into()));
            let kind = DiagnosticKind::BadManifest { manifest: manifest_file, message: error.message.clone(), range };
            self.push(Some(TextRange::empty(TextSize::from(0))), kind);
        }
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
                NameResDiagnostic::UnresolvedImportPath { import, path, error } => {
                    let node = import.to_node(self.db, self.file, &self.root);
                    let string = node.as_ref().and_then(|n| {
                        n.children_with_tokens().filter_map(|e| e.into_token()).find(|t| t.kind() == STRING)
                    });
                    let range = string.map(|t| t.text_range()).or(node.map(|n| n.text_range()));
                    (range, DiagnosticKind::UnresolvedImportPath { path: path.clone(), error: error.clone() })
                }
                NameResDiagnostic::UnresolvedImportName { import, name } => {
                    (self.name_in(*import, name), DiagnosticKind::UnresolvedImportName { name: name.clone() })
                }
                NameResDiagnostic::PrivateImport { import, name, package_only } => {
                    let kind = DiagnosticKind::NotVisible { name: name.clone(), package_only: *package_only };
                    (self.name_in(*import, name), kind)
                }
                NameResDiagnostic::UnresolvedImportEnum { import, path } => {
                    (self.import_path_range(*import), DiagnosticKind::UnresolvedImportEnum { path: path.clone() })
                }
                NameResDiagnostic::NotAnEnum { import, path } => {
                    (self.import_path_range(*import), DiagnosticKind::NotAnEnum { path: path.clone() })
                }
                NameResDiagnostic::UnresolvedOwner { function, name } => {
                    (self.owner_range(*function), DiagnosticKind::UnresolvedOwner { name: name.clone() })
                }
                NameResDiagnostic::OwnerNotTopLevel { function } => {
                    (self.owner_range(*function), DiagnosticKind::OwnerNotTopLevel)
                }
                // It's about the whole file, so it's at its start.
                NameResDiagnostic::UnresolvedPrelude { package, error } => (
                    Some(TextRange::empty(TextSize::from(0))),
                    DiagnosticKind::UnresolvedPrelude { package: package.clone(), error: error.clone() },
                ),
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
                    self.no_values(GenericDefId::Struct(id));
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
                    let (sig, map) = impl_signature_with_source_map(db, id);
                    self.lower(map, &impl_header(db, id).diagnostics);
                    self.lower(map, &generic_predicates(db, GenericDefId::Impl(id)).diagnostics);
                    if let Some(instance) = &impl_header(db, id).class {
                        for arg in instance.args.iter() {
                            self.written_type(map, Some(sig.class), arg);
                        }
                    }
                    self.unused_impl_params(id, map);
                    self.overlapping_impls(id, map);
                    self.impl_items(id, map);
                    self.assoc_items(ItemContainerId::Impl(id), &id.item(db).items);
                }
                ModItem::Const(idx) => self.const_(ConstId::new(db, container, idx)),
                ModItem::TypeAlias(idx) => self.type_alias(TypeAliasId::new(db, container, idx)),
                ModItem::ExternBlock(idx) => {
                    let id = ExternBlockId::new(db, file, idx);
                    self.assoc_items(ItemContainerId::ExternBlock(id), &id.item(db).items);
                    if is_builtin_block(db, id) {
                        self.builtins(id);
                    }
                }
                // Imports are checked by name resolution.
                ModItem::Import(_) | ModItem::MacroRules(_) => {}
            }
        }
    }

    /// Report why `f`, with `#[test]`, can't be run as a test, at its name.
    fn test(&mut self, f: FunctionId<'db>) {
        let db = self.db;
        let item = f.item(db);
        if !item.is_test {
            return;
        }
        let range = Some(self.item_range(item.ast_id));
        if item.owner.is_some() || !matches!(f.container(db), ItemContainerId::File(_)) {
            self.push(range, DiagnosticKind::BadTest { reason: BadTest::NotTopLevel });
            return;
        }
        let sig = function_signature(db, f);
        if !sig.params.is_empty() {
            self.push(range, DiagnosticKind::BadTest { reason: BadTest::Params });
        }
        if !sig.generic_params.type_params.is_empty() {
            self.push(range, DiagnosticKind::BadTest { reason: BadTest::Generic });
        }
        if let Ok(scheme) = function_scheme(db, f)
            && let Ty::Fn(_, ret, _) = &scheme.ty
            && !ret.is_unit()
            && !ret.any(&mut |t| *t == Ty::Error)
        {
            self.push(range, DiagnosticKind::BadTest { reason: BadTest::Result });
        }
    }

    /// Report the functions with the name of `f` before it that it overlaps,
    /// at its name.
    fn overlapping_functions(&mut self, f: FunctionId<'db>) {
        let db = self.db;
        let range = self.item_range(f.item(db).ast_id);
        for &other in overlapped_functions(db, f) {
            let other_file = other.file(db);
            let root = SyntaxNode::new_root(parse_file(db, other_file).tree(db).clone());
            let Some(node) = other.item(db).ast_id.to_node(db, other_file, &root) else { continue };
            let other_range = node.children().find(|n| n.kind() == NAME).unwrap_or(node).text_range();
            let kind = DiagnosticKind::OverlappingFunction {
                name: overload_name(db, &[f]),
                ty: overload_types(db, &[f]).remove(0),
                other: overload_types(db, &[other]).remove(0),
                other_file,
                other_range,
            };
            self.push(Some(range), kind);
        }
    }

    /// Report the impls before `imp` that it overlaps, at its instance.
    /// `map` is the source map of the impl's signature.
    fn overlapping_impls(&mut self, imp: ImplId<'db>, map: &ExpressionStoreSourceMap) {
        let db = self.db;
        let Some(instance) = &impl_header(db, imp).class else { return };
        let range = map.type_syntax(impl_signature(db, imp).class).map(ptr_range);
        for &other in overlapped_impls(db, imp) {
            let Some(other_instance) = &impl_header(db, other).class else { continue };
            let (sig, other_map) = impl_signature_with_source_map(db, other);
            let Some(other_range) = other_map.type_syntax(sig.class).map(ptr_range) else { continue };
            let kind = DiagnosticKind::OverlappingImpl {
                instance: instance.clone(),
                other: other_instance.clone(),
                other_file: other.file(db),
                other_range,
            };
            self.push(range, kind);
        }
    }

    /// Report the parameters of `imp` that its instance doesn't use, at
    /// their declaration, or for a mutability that isn't declared, where
    /// it's first written. `map` is the source map of the impl's signature.
    fn unused_impl_params(&mut self, imp: ImplId<'db>, map: &ExpressionStoreSourceMap) {
        let db = self.db;
        let unused = unused_impl_params(db, imp);
        let Some(instance) = impl_header(db, imp).class.as_ref() else { return };
        if unused.is_empty() {
            return;
        }
        let def = GenericDefId::Impl(imp);
        let params = def.generic_params(db);
        let named = named_mutabilities(db, def);
        let list = imp
            .item(db)
            .ast_id
            .to_node(db, self.file, &self.root)
            .and_then(|n| n.children().find(|c| c.kind() == GENERIC_PARAM_LIST));
        for index in unused {
            let index = index as usize;
            let (name, range, suggestion) = match params.type_params.get(index) {
                Some(param) => {
                    let node = list.as_ref().and_then(|l| l.children().filter(|c| c.kind() == TYPE_PARAM).nth(index));
                    let range = node.map(|n| n.children().find(|c| c.kind() == NAME).unwrap_or(n).text_range());
                    (param.name.to_string(), range, None)
                }
                None => {
                    let name = &named[index - params.type_params.len()];
                    let declared = params.mutability_params.iter().find(|p| &p.name == name).map(|p| p.type_ref);
                    let written = || {
                        impl_signature(db, imp).store.types.iter().find_map(|(id, t)| match t {
                            TypeRef::Mut { mutability: MutabilityRef::Named(n), .. }
                            | TypeRef::Mutability(MutabilityRef::Named(n))
                                if n == name =>
                            {
                                Some(id)
                            }
                            _ => None,
                        })
                    };
                    let range = declared.or_else(written).and_then(|id| map.type_syntax(id)).map(ptr_range);
                    let param = Ty::Param(GenericParamId { def, index: index as u32 });
                    let first_hidden = GenericParamId { def, index: (params.type_params.len() + named.len()) as u32 };
                    (format!("'{name}"), range, with_hidden_mutability(instance, first_hidden, param))
                }
            };
            self.push(range, DiagnosticKind::UnusedImplParam { name, instance: instance.clone(), suggestion });
        }
    }

    /// Check the items of an impl against those of its class: the ones the
    /// class doesn't declare, like a method with a typo in its name, the
    /// ones the class declares without a default that are missing, and the
    /// types of consts. `map` is the source map of the impl's signature.
    fn impl_items(&mut self, imp: ImplId<'db>, map: &ExpressionStoreSourceMap) {
        let db = self.db;
        let Some(constraint) = impl_header(db, imp).class.as_ref() else { return };
        let class = constraint.class;
        let declared = AssocItemDef::all(db, ItemContainerId::Class(class), &class.item(db).items);
        let mut defined = Vec::new();
        for item in AssocItemDef::all(db, ItemContainerId::Impl(imp), &imp.item(db).items) {
            let Some(name) = item.name(db) else { continue };
            let kind = item.kind();
            let declaration = declared.iter().find(|d| d.kind() == kind && d.name(db).as_ref() == Some(&name));
            match (item, declaration) {
                (_, None) => {
                    let range = self.item_range(item.ast_id(db));
                    self.push(Some(range), DiagnosticKind::NotInClass { kind, name: name.clone(), class });
                }
                (AssocItemDef::Const(c), Some(&AssocItemDef::Const(declaration))) => {
                    self.impl_const_ty(c, declaration, constraint);
                }
                _ => {}
            }
            defined.push((kind, name));
        }
        // The types the impl gives the class's associated types must meet
        // their bounds.
        for (alias, constraint) in unmet_assoc_bounds(db, imp) {
            let Some(name) = alias.item(db).name.clone() else { continue };
            let own = AssocItemDef::all(db, ItemContainerId::Impl(imp), &imp.item(db).items)
                .into_iter()
                .find(|item| matches!(item, AssocItemDef::TypeAlias(_)) && item.name(db).as_ref() == Some(&name));
            let range = own.map(|item| self.item_range(item.ast_id(db)));
            self.push(range, DiagnosticKind::UnmetAssocBound { name, class, constraint: constraint.clone() });
        }
        for (alias, of, expected, actual) in unmet_assoc_equalities(db, imp) {
            let (Some(name), Some(item)) = (alias.item(db).name.clone(), of.item(db).name.clone()) else { continue };
            let own = AssocItemDef::all(db, ItemContainerId::Impl(imp), &imp.item(db).items)
                .into_iter()
                .find(|item| matches!(item, AssocItemDef::TypeAlias(_)) && item.name(db).as_ref() == Some(&name));
            let range = own.map(|item| self.item_range(item.ast_id(db)));
            let (expected, actual) = (expected.clone(), actual.clone());
            self.push(range, DiagnosticKind::UnmetAssocEquality { name, item, class, expected, actual });
        }
        let missing: Vec<(ClassItemKind, Name)> = declared
            .iter()
            .filter(|d| !d.has_default(db))
            .filter_map(|d| Some((d.kind(), d.name(db)?)))
            .filter(|item| !defined.contains(item))
            .collect();
        if !missing.is_empty() {
            let (sig, _) = impl_signature_with_source_map(db, imp);
            self.fallback = self.item_range(imp.item(db).ast_id);
            let range = map.type_syntax(sig.class).map(ptr_range);
            self.push(range, DiagnosticKind::MissingItems { items: missing, class });
        }
    }

    /// Check that a const of an impl has the type its class declares, in
    /// terms of the impl's class arguments.
    fn impl_const_ty(&mut self, c: ConstId<'db>, declaration: ConstId<'db>, constraint: &ClassConstraint<'db>) {
        let db = self.db;
        let class_def = GenericDefId::Class(constraint.class);
        let expected = subst_params(&const_ty(db, declaration).ty, class_def, &constraint.args);
        let actual = const_ty(db, c).ty.clone();
        if expected == actual || has_error(&expected) || has_error(&actual) {
            return;
        }
        let (sig, map) = const_signature_with_source_map(db, c);
        let (declared_sig, declared_map) = const_signature_with_source_map(db, declaration);
        let because = declared_map.type_syntax(declared_sig.type_ref).map(|ptr| Because {
            file: declaration.file(db),
            range: ptr_range(ptr),
            kind: BecauseKind::ClassItem { class: constraint.class },
        });
        self.fallback = self.item_range(c.item(db).ast_id);
        let range = map.type_syntax(sig.type_ref).map(ptr_range);
        self.push(range, DiagnosticKind::TypeMismatch { expected, actual, because });
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
        self.overlapping_functions(id);
        self.test(id);
        let (sig, map) = function_signature_with_source_map(db, id);
        self.lower(map, &fn_sig(db, id).diagnostics);
        self.lower(map, &generic_predicates(db, GenericDefId::Function(id)).diagnostics);
        if id.item(db).has_body {
            self.body(DefWithBody::Function(id));
        } else {
            // Without a body, like a class method, nothing else looks at
            // the types it's given.
            let lowered = fn_sig(db, id);
            for (param, ty) in sig.params.iter().zip(lowered.params.iter()) {
                if let Some(ty) = ty {
                    self.written_type(map, param.type_ref, ty);
                }
            }
            if let Some(ret) = &lowered.ret {
                self.written_type(map, sig.ret_type, ret);
            }
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
        self.no_values(GenericDefId::Enum(id));
    }

    /// Check the declarations of an `extern "bolt"` block: they must be
    /// builtins, with their types.
    fn builtins(&mut self, block: ExternBlockId<'db>) {
        let db = self.db;
        for item in AssocItemDef::all(db, ItemContainerId::ExternBlock(block), &block.item(db).items) {
            let Some(name) = item.name(db) else { continue };
            let range = Some(self.item_range(item.ast_id(db)));
            let (AssocItemDef::Function(f), Some(builtin)) = (item, Builtin::by_name(name.as_str())) else {
                self.push(range, DiagnosticKind::UnknownBuiltin { name });
                continue;
            };
            let sig = fn_sig(db, f);
            let declared = Ty::func(
                sig.params.iter().map(|t| t.clone().unwrap_or(Ty::Error)).collect::<Box<[_]>>(),
                sig.ret.clone().unwrap_or(Ty::Error),
            );
            if !builtin.has_signature(&declared) && !has_error(&declared) {
                self.push(range, DiagnosticKind::BuiltinSignature { name, expected: builtin.signature() });
            }
        }
    }

    /// Warn about a struct or enum that has no values, at its name. An enum
    /// without variants is left alone: it's meant to have none, like `!`.
    fn no_values(&mut self, def: GenericDefId<'db>) {
        let db = self.db;
        if has_values(db, def) {
            return;
        }
        let (ast_id, name, culprit) = match def {
            GenericDefId::Struct(id) => {
                let fields = &struct_field_types(db, id).fields;
                (id.item(db).ast_id, &id.item(db).name, field_without_values(db, fields).map(|f| (None, f)))
            }
            GenericDefId::Enum(id) => {
                let variants = &enum_signature_with_source_map(db, id).0.variants;
                let culprit = variants.iter().zip(enum_field_types(db, id)).find_map(|(variant, fields)| {
                    field_without_values(db, &fields.fields).map(|f| (Some(variant.name.clone()), f))
                });
                (id.item(db).ast_id, &id.item(db).name, culprit)
            }
            _ => return,
        };
        let (Some(name), Some((variant, (field, field_ty)))) = (name, culprit) else { return };
        let kind = DiagnosticKind::NoValues { name: name.clone(), variant, field: field.clone(), field_ty: field_ty.clone() };
        self.push(Some(self.item_range(ast_id)), kind);
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
        let (sig, map) = type_alias_signature_with_source_map(db, id);
        let ty = type_alias_ty(db, id);
        if let Some(aliased) = &ty.ty {
            self.written_type(map, sig.type_ref, aliased);
        }
        // The alias is where its cycle is found, so it has no diagnostics of
        // its own about it; its uses do.
        if ty.recursive {
            self.push(None, DiagnosticKind::RecursiveAlias);
        }
        self.lower(map, &ty.diagnostics);
        self.lower(map, &generic_predicates(db, GenericDefId::TypeAlias(id)).diagnostics);
        self.lower(map, &assoc_type_bounds(db, id).diagnostics);
    }

    /// Report a type written at `type_ref` that can't have values, like
    /// `mut VecIter[mut [i32], imm]`, whose `data` would be an `imm` array
    /// of `mut` ones. A body's types are checked with the body; this is for
    /// the types no body looks at, like those of a class method.
    fn written_type(&mut self, map: &ExpressionStoreSourceMap, type_ref: Option<TypeRefId>, ty: &Ty<'db>) {
        if let Some((ty, inner)) = mut_inside_imm(self.db, ty) {
            let range = type_ref.and_then(|t| map.type_syntax(t)).map(ptr_range);
            self.push(range, DiagnosticKind::MutInsideImm { ty, inner });
        }
    }

    /// Report problems with type references, from the store `map` is for.
    fn lower(&mut self, map: &ExpressionStoreSourceMap, diagnostics: &[LowerDiagnostic]) {
        for diagnostic in diagnostics {
            let names_the_type = matches!(
                diagnostic,
                LowerDiagnostic::UnresolvedType(_)
                    | LowerDiagnostic::NotVisible { .. }
                    | LowerDiagnostic::NotAType(_)
                    | LowerDiagnostic::NotAClass(_)
            );
            let (type_ref, kind): (TypeRefId, _) = match diagnostic {
                LowerDiagnostic::UnresolvedType(id) => (*id, DiagnosticKind::UnresolvedType),
                LowerDiagnostic::NotVisible { type_ref, hidden } => {
                    (*type_ref, DiagnosticKind::NotVisible { name: hidden.name.clone(), package_only: hidden.package_only })
                }
                LowerDiagnostic::NotAType(id) => (*id, DiagnosticKind::NotAType),
                LowerDiagnostic::NotAClass(id) => (*id, DiagnosticKind::NotAClass),
                LowerDiagnostic::WrongArgCount { type_ref, expected, found } => {
                    (*type_ref, DiagnosticKind::WrongTypeArgCount { expected: *expected, found: *found })
                }
                LowerDiagnostic::KindMismatch { type_ref, expected, found } => {
                    (*type_ref, DiagnosticKind::KindMismatch { expected: expected.clone(), found: found.clone() })
                }
                LowerDiagnostic::RecursiveAlias(id) => (*id, DiagnosticKind::RecursiveAlias),
                LowerDiagnostic::Unsupported(id) => (*id, DiagnosticKind::Unsupported),
                LowerDiagnostic::NoMutability(id) => (*id, DiagnosticKind::NoMutability),
                LowerDiagnostic::MutField(id) => (*id, DiagnosticKind::MutField),
                LowerDiagnostic::MutabilityAlone(id) => (*id, DiagnosticKind::MutabilityAlone),
                LowerDiagnostic::UnknownMutability(id) => (*id, DiagnosticKind::UnknownMutability),
                LowerDiagnostic::WrongMutabilityCount { type_ref, expected, found } => {
                    (*type_ref, DiagnosticKind::WrongMutabilityArgCount { expected: *expected, found: *found })
                }
                LowerDiagnostic::MutabilityParamNotAllowed(id) => (*id, DiagnosticKind::MutabilityParamNotAllowed),
                LowerDiagnostic::NoAssocType { type_ref } => (*type_ref, DiagnosticKind::NoAssocType),
                LowerDiagnostic::AmbiguousAssocType { type_ref } => (*type_ref, DiagnosticKind::AmbiguousAssocType),
                LowerDiagnostic::NotAnAssocTypeOf { type_ref, class, name } => {
                    (*type_ref, DiagnosticKind::NotAnAssocTypeOf { class: class.clone(), name: name.clone() })
                }
                LowerDiagnostic::EqualityNotAllowed(id) => (*id, DiagnosticKind::EqualityNotAllowed),
            };
            let ptr = map.type_syntax(type_ref);
            let range = match ptr {
                Some(ptr) if names_the_type => self.type_name_range(ptr).or(Some(ptr_range(ptr))),
                _ => ptr.map(ptr_range),
            };
            self.push(range, kind);
        }
    }

    fn body(&mut self, def: DefWithBody<'db>) {
        let db = self.db;
        let id = DefWithBodyId::new(db, def);
        let (body, map) = body_with_source_map(db, id);
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
                        TypeError::NoOverload { origin, ty, candidates } => {
                            if has_error(&ty) {
                                continue;
                            }
                            let name = overload_name(db, &candidates);
                            (origin, DiagnosticKind::NoOverload { name, ty, candidates: overload_types(db, &candidates) })
                        }
                        TypeError::AmbiguousOverload { origin, candidates, .. } => {
                            let name = overload_name(db, &candidates);
                            (origin, DiagnosticKind::AmbiguousOverload { name, candidates: overload_types(db, &candidates) })
                        }
                        TypeError::NoInstance { origin, constraint } => {
                            if constraint.args.iter().any(has_error) {
                                continue;
                            }
                            let because = self.because(origin, result);
                            (origin, DiagnosticKind::NoInstance { constraint, because })
                        }
                        TypeError::Ambiguous { origin, constraint } => {
                            if constraint.args.iter().any(has_error) {
                                continue;
                            }
                            let because = self.because(origin, result);
                            (origin, DiagnosticKind::AmbiguousConstraint { constraint, because })
                        }
                        TypeError::NoSuchField { origin, ty, name } => {
                            if has_error(&ty) {
                                continue;
                            }
                            debug_assert_eq!(origin.body, id);
                            // At the name of the field, for `p.z`.
                            let range = self.unresolved_range(map, origin.node);
                            let because = self.because(origin, result);
                            self.push(range, DiagnosticKind::NoSuchField { ty: Some(ty), name, because });
                            continue;
                        }
                        TypeError::TypeNeeded { origin } => (origin, DiagnosticKind::TypeNeeded),
                        TypeError::CannotChange { origin, ty } => (origin, DiagnosticKind::CannotChange { ty }),
                        TypeError::NotAtMost { origin, constraint } => {
                            if has_error(constraint.held.ty()) || has_error(&constraint.bound) {
                                continue;
                            }
                            let because = self.because(origin, result);
                            let is_mutability = matches!(constraint.held, Held::Mutability(_));
                            let held = constraint.held.ty().clone();
                            (origin, DiagnosticKind::NotAtMost { held, is_mutability, bound: constraint.bound, because })
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
                InferenceDiagnostic::NotVisible { expr, hidden } => (
                    self.unresolved_range(map, expr),
                    DiagnosticKind::NotVisible { name: hidden.name, package_only: hidden.package_only },
                ),
                InferenceDiagnostic::NotAFunction { expr, ty } => {
                    if has_error(&ty) {
                        continue;
                    }
                    (at(ExprOrPat::Expr(expr)), DiagnosticKind::NotAFunction { ty })
                }
                InferenceDiagnostic::WrongArgCount { expr, expected, found } => {
                    (at(expr), DiagnosticKind::WrongArgCount { expected, found })
                }
                InferenceDiagnostic::NoSuchField { expr, name } => {
                    let range = self.field_name_range(map, expr, &name).or_else(|| at(expr));
                    (range, DiagnosticKind::NoSuchField { ty: None, name, because: None })
                }
                InferenceDiagnostic::MissingFields { expr, names } => {
                    (at(ExprOrPat::Expr(expr)), DiagnosticKind::MissingFields { names })
                }
                InferenceDiagnostic::BreakOutside { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::BreakOutside),
                InferenceDiagnostic::AssignToImmutable { expr, binding } => {
                    (at(ExprOrPat::Expr(expr)), DiagnosticKind::AssignToImmutable { name: body[binding].name.clone() })
                }
                InferenceDiagnostic::NoMutability { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::NoMutability),
                InferenceDiagnostic::MutInsideImm { at: node, ty, inner } => {
                    if has_error(&ty) {
                        continue;
                    }
                    (at(node), DiagnosticKind::MutInsideImm { ty, inner })
                }
                InferenceDiagnostic::NotAPlace { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::NotAPlace),
                InferenceDiagnostic::Unsupported { expr } => (at(ExprOrPat::Expr(expr)), DiagnosticKind::Unsupported),
                InferenceDiagnostic::NoLangClass { expr, class } => {
                    (at(ExprOrPat::Expr(expr)), DiagnosticKind::NoLangClass { class })
                }
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
                let name = function.item(db).display_name();
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
            Reason::RequiredBy(ConstraintSource::Body { body, node }) => {
                let (_, map) = body_with_source_map(db, body);
                let ptr = match node {
                    ExprOrPat::Expr(e) => map.expr_syntax(e)?,
                    ExprOrPat::Pat(p) => map.pat_syntax(p)?,
                };
                let function = match body.def(db) {
                    DefWithBody::Function(f) => f.item(db).display_name(),
                    _ => None,
                };
                (body.def(db).file(db), ptr, BecauseKind::UseIn { function })
            }
            Reason::RequiredBy(ConstraintSource::Declared { def, type_ref }) => {
                let (file, map, item) = match def {
                    GenericDefId::Function(f) => {
                        let name = f.item(db).display_name();
                        (f.file(db), &function_signature_with_source_map(db, f).1, name)
                    }
                    GenericDefId::Struct(s) => {
                        (s.file(db), &struct_signature_with_source_map(db, s).1, s.item(db).name.clone())
                    }
                    GenericDefId::Enum(e) => {
                        (e.file(db), &enum_signature_with_source_map(db, e).1, e.item(db).name.clone())
                    }
                    GenericDefId::Impl(i) => (i.file(db), &impl_signature_with_source_map(db, i).1, None),
                    GenericDefId::Class(c) => (c.file(db), &class_signature_with_source_map(db, c).1, None),
                    GenericDefId::TypeAlias(_) => return None,
                };
                (file, map.type_syntax(type_ref)?, BecauseKind::Bound { item })
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

    /// The path of a type like `a.List[T]` without its arguments: `a.List`.
    /// Errors about the name of a type point there, and say only that.
    fn type_name_range(&self, ptr: SyntaxPtr) -> Option<TextRange> {
        if ptr.kind != PATH_TYPE {
            return None;
        }
        let path = ptr.to_node(&self.root)?.children().find(|n| n.kind() == PATH)?;
        let segment = path.children().filter(|n| n.kind() == PATH_SEGMENT).last()?;
        let name = segment.children().find(|n| n.kind() == NAME_REF)?;
        Some(TextRange::new(path.text_range().start(), name.text_range().end()))
    }

    /// The path of `import Option.*;`, or the whole import.
    fn import_path_range(&self, import: AstId) -> Option<TextRange> {
        let node = import.to_node(self.db, self.file, &self.root)?;
        let path = node.children().find(|n| n.kind() == PATH);
        Some(path.unwrap_or(node).text_range())
    }

    /// The name of the field `name` of a record: the `z` of `Point { z: 1 }`,
    /// for its value `1`, or of the pattern `Point { z, .. }`.
    fn field_name_range(&self, map: &ExpressionStoreSourceMap, node: ExprOrPat, name: &Name) -> Option<TextRange> {
        let ptr = node_ptr(map, node)?;
        let node = ptr.to_node(&self.root)?;
        let name_in = |field: &SyntaxNode| {
            field.children().find(|n| matches!(n.kind(), NAME | NAME_REF)).map(|n| n.text_range())
        };
        if node.kind() == STRUCT_PATT {
            let field = node.children().filter(|n| n.kind() == FIELD_PATT).find(|field| {
                field.children().any(|n| matches!(n.kind(), NAME | NAME_REF) && n.text() == name.as_str())
            })?;
            return name_in(&field);
        }
        let field = if node.kind() == RECORD_EXPR_FIELD { node } else { node.parent()? };
        if field.kind() != RECORD_EXPR_FIELD {
            return None;
        }
        name_in(&field)
    }

    /// The name of an item, or the whole item if it has none.
    fn item_range(&self, ast_id: AstId) -> TextRange {
        let Some(node) = ast_id.to_node(self.db, self.file, &self.root) else { return self.fallback };
        node.children().find(|n| n.kind() == NAME).unwrap_or(node).text_range()
    }

    /// Where `name` is in the item or import `ast_id`. For an item, that's
    /// the name it defines; for an import, the name in its list.
    /// The `Vec` of `fn Vec.new()`, or the whole function if it's gone.
    fn owner_range(&self, function: AstId) -> Option<TextRange> {
        let node = function.to_node(self.db, self.file, &self.root)?;
        let owner = node.children().find(|n| n.kind() == NAME_REF);
        Some(owner.unwrap_or(node).text_range())
    }

    fn name_in(&self, ast_id: AstId, name: &Name) -> Option<TextRange> {
        let node = ast_id.to_node(self.db, self.file, &self.root)?;
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

/// The first array, struct or function in `instance` whose mutability is
/// hidden, that is, not written, with `mutability` instead. The hidden
/// parameters are numbered from `first_hidden`.
fn with_hidden_mutability<'db>(
    instance: &ClassConstraint<'db>,
    first_hidden: GenericParamId<'db>,
    mutability: Ty<'db>,
) -> Option<Ty<'db>> {
    let mut found = None;
    for arg in instance.args.iter() {
        arg.any(&mut |t| {
            let hidden = matches!(t.held_mutability(), Some(Ty::Param(p)) if p.def == first_hidden.def && p.index >= first_hidden.index);
            if hidden && found.is_none() {
                found = Some(match t.clone() {
                    Ty::Array(_, elem) => Ty::Array(Box::new(mutability.clone()), elem),
                    Ty::Adt(id, args, ms, _) => Ty::Adt(id, args, ms, Box::new(mutability.clone())),
                    Ty::Fn(params, ret, _) => Ty::Fn(params, ret, Box::new(mutability.clone())),
                    t => t,
                });
            }
            hidden
        });
    }
    found
}

fn ptr_range(ptr: SyntaxPtr) -> TextRange {
    TextRange::new(ptr.start.into(), ptr.end.into())
}

fn node_ptr(map: &ExpressionStoreSourceMap, node: ExprOrPat) -> Option<SyntaxPtr> {
    match node {
        ExprOrPat::Expr(e) => map.expr_syntax(e),
        ExprOrPat::Pat(p) => map.pat_syntax(p),
    }
}

fn node_range(map: &ExpressionStoreSourceMap, node: ExprOrPat) -> Option<TextRange> {
    node_ptr(map, node).map(ptr_range)
}

fn has_error(ty: &Ty<'_>) -> bool {
    ty.any(&mut |t| matches!(t, Ty::Error))
}

/// The name of the functions that a use could be (#275).
fn overload_name(db: &dyn Db, candidates: &[FunctionId<'_>]) -> Name {
    candidates.first().and_then(|f| f.item(db).display_name()).unwrap_or_else(|| Name::new("?"))
}

/// The types of the functions that a use could be, as text.
fn overload_types(db: &dyn Db, candidates: &[FunctionId<'_>]) -> Vec<String> {
    candidates
        .iter()
        .map(|&f| function_scheme(db, f).map_or_else(|_| "<cycle>".to_string(), |s| s.display(db)))
        .collect()
}
