//! The items of a file, without their bodies.
//!
//! Like rust-analyzer's `ItemTree`, this is a summary of a file that only
//! changes when its items do: editing the body of a function leaves it
//! unchanged, so Salsa doesn't redo the work that depends on it, like name
//! resolution.
//!
//! Each item is stored in a list per kind, and refers back to its syntax by
//! an [`AstId`]: its position among the items of the same kind in the file.
//! Unlike a text range, that position doesn't change when a body is edited.

use boltlang_base_db::{Db, File, parse_file};
use boltlang_syntax::{
    SyntaxKind::{self, *},
    SyntaxNode,
    ast::{self, Node},
};

pub use crate::arena::Idx;
use crate::Name;

/// The items of `file`.
#[salsa::tracked(returns(ref))]
pub fn item_tree(db: &dyn Db, file: File) -> ItemTree {
    let parse = parse_file(db, file);
    let root = SyntaxNode::new_root(parse.tree(db).clone());
    let mut tree = ItemTree::default();
    if let Some(source_file) = ast::SourceFile::cast(root) {
        tree.top_level = source_file.elements().filter_map(|element| tree.lower_item(element)).collect();
    }
    tree
}

/// Where to find the syntax of an item: the `index`th node of kind `kind`
/// among the items of the file, in the order they appear. Bodies are
/// skipped, so editing one doesn't change the positions of later items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct AstId {
    pub kind: SyntaxKind,
    pub index: u32,
}

impl AstId {
    /// Find the syntax node this refers to in the syntax tree of the file.
    pub fn to_node(self, root: &SyntaxNode) -> Option<SyntaxNode> {
        item_nodes(root).filter(|node| node.kind() == self.kind).nth(self.index as usize)
    }
}

/// The nodes of the items in a file in the order they appear, including the
/// items in classes, impls and extern blocks, but not those in bodies.
fn item_nodes(root: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    let mut stack = vec![root.clone()];
    let mut out = Vec::new();
    while let Some(node) = stack.pop() {
        let children: Vec<_> = node.children().collect();
        for child in children.into_iter().rev() {
            if is_item_kind(child.kind()) {
                stack.push(child.clone());
            } else if child.kind() == ITEM_LIST {
                stack.push(child);
            }
        }
        if is_item_kind(node.kind()) {
            out.push(node);
        }
    }
    out.into_iter()
}

fn is_item_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        FUNC_DECL | STRUCT | ENUM | CLASS | IMPL | CONST | TYPE_ALIAS | IMPORT | EXTERN_BLOCK | MACRO_RULES
            | MACRO_CALL
    )
}

/// The items of a file. See the [module docs](self).
#[derive(Debug, Default, PartialEq, Eq, salsa::SalsaValue)]
pub struct ItemTree {
    /// The items directly in the file, in order.
    pub top_level: Vec<ModItem>,
    pub functions: Vec<Function>,
    pub structs: Vec<Struct>,
    pub enums: Vec<Enum>,
    pub classes: Vec<Class>,
    pub impls: Vec<Impl>,
    pub consts: Vec<Const>,
    pub type_aliases: Vec<TypeAlias>,
    pub imports: Vec<Import>,
    pub extern_blocks: Vec<ExternBlock>,
    pub macro_rules: Vec<MacroRules>,
    pub macro_calls: Vec<MacroCall>,
}

/// An item that can appear directly in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ModItem {
    Function(Idx<Function>),
    Struct(Idx<Struct>),
    Enum(Idx<Enum>),
    Class(Idx<Class>),
    Impl(Idx<Impl>),
    Const(Idx<Const>),
    TypeAlias(Idx<TypeAlias>),
    Import(Idx<Import>),
    ExternBlock(Idx<ExternBlock>),
    MacroRules(Idx<MacroRules>),
}

/// An item in a class, impl or extern block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum AssocItem {
    Function(Idx<Function>),
    Const(Idx<Const>),
    TypeAlias(Idx<TypeAlias>),
    MacroCall(Idx<MacroCall>),
}

/// A visibility as written, before it's resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum RawVisibility {
    /// No `pub`, or `pub(self)`
    Private,
    /// `pub`
    Public,
    /// `pub(super)`
    Super,
    /// `pub(package)`
    Package,
}

impl RawVisibility {
    fn from_ast(vis: Option<ast::Visibility>) -> Self {
        let Some(vis) = vis else {
            return RawVisibility::Private;
        };
        match vis.restriction().map(|token| token.kind()) {
            None => RawVisibility::Public,
            Some(SELF_KEYWORD) => RawVisibility::Private,
            Some(SUPER_KEYWORD) => RawVisibility::Super,
            Some(_) => RawVisibility::Package,
        }
    }
}

/// The name of a function: an identifier, or an operator like `+` for
/// `fn a + b`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum FunctionName {
    Name(Name),
    Operator(Name),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Function {
    pub name: Option<FunctionName>,
    pub visibility: RawVisibility,
    pub is_async: bool,
    pub is_const: bool,
    /// Whether it has a body, as opposed to a signature like `fn f();`.
    pub has_body: bool,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Struct {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Enum {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    /// The names of the variants, needed to resolve paths like `Color.Red`.
    pub variants: Vec<Name>,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Class {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    pub items: Vec<AssocItem>,
    pub ast_id: AstId,
}

/// `impl Show[Dog] { }`: an instance of a class.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Impl {
    pub items: Vec<AssocItem>,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Const {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    /// Whether it has a value, as opposed to a declaration in a class like
    /// `const N: usize;`.
    pub has_value: bool,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct TypeAlias {
    pub name: Option<Name>,
    pub visibility: RawVisibility,
    /// Whether it has a type, as opposed to an associated type in a class
    /// like `type Item;`.
    pub has_value: bool,
    pub ast_id: AstId,
}

/// `import "./path" ...;`
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Import {
    pub visibility: RawVisibility,
    /// The path in the string, without quotes. `None` if it's missing or
    /// can't be read, and for `import Option.*;`, which has no string.
    pub source: Option<String>,
    pub kind: ImportKind,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum ImportKind {
    /// `import "./path";`
    Plain,
    /// `import "./path" (a, b as c);`, as `(name, alias)` pairs
    List(Vec<(Name, Option<Name>)>),
    /// `import "./path" as name;`
    Alias(Name),
    /// `import "./path" *;`
    Glob,
    /// `import Option.*;`, with the path of the enum, whose variants it
    /// brings into scope.
    Variants(Box<[Name]>),
}

/// `extern "C" { items }`. Its items are part of the file it's in.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ExternBlock {
    /// The ABI string without quotes, like `C`.
    pub abi: Option<String>,
    pub items: Vec<AssocItem>,
    pub ast_id: AstId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct MacroRules {
    pub name: Option<Name>,
    pub ast_id: AstId,
}

/// A macro invocation in a class, impl or extern block.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct MacroCall {
    pub name: Option<Name>,
    pub ast_id: AstId,
}

impl ItemTree {
    /// The next [`AstId`] for an item of `kind`.
    fn next_ast_id(&self, kind: SyntaxKind) -> AstId {
        let count = match kind {
            FUNC_DECL => self.functions.len(),
            STRUCT => self.structs.len(),
            ENUM => self.enums.len(),
            CLASS => self.classes.len(),
            IMPL => self.impls.len(),
            CONST => self.consts.len(),
            TYPE_ALIAS => self.type_aliases.len(),
            IMPORT => self.imports.len(),
            EXTERN_BLOCK => self.extern_blocks.len(),
            MACRO_RULES => self.macro_rules.len(),
            MACRO_CALL => self.macro_calls.len(),
            _ => unreachable!("{kind:?} is not an item"),
        };
        AstId { kind, index: count as u32 }
    }

    /// Lower an item in a file. Statements and expressions aren't items.
    ///
    /// Items are lowered in the order they appear, before the items inside
    /// them, so that [`AstId`]s match the order of [`item_nodes`].
    fn lower_item(&mut self, element: ast::SourceElement) -> Option<ModItem> {
        use ast::SourceElement as E;
        let item = match element {
            E::FuncDecl(f) => ModItem::Function(self.lower_function(&f)),
            E::Struct(s) => {
                let ast_id = self.next_ast_id(STRUCT);
                self.structs.push(Struct {
                    name: name(s.name()),
                    visibility: RawVisibility::from_ast(s.visibility()),
                    ast_id,
                });
                ModItem::Struct(Idx::new(self.structs.len() - 1))
            }
            E::Enum(e) => {
                let ast_id = self.next_ast_id(ENUM);
                self.enums.push(Enum {
                    name: name(e.name()),
                    visibility: RawVisibility::from_ast(e.visibility()),
                    variants: e.variants().iter().filter_map(|v| name(v.name())).collect(),
                    ast_id,
                });
                ModItem::Enum(Idx::new(self.enums.len() - 1))
            }
            E::Class(c) => {
                let ast_id = self.next_ast_id(CLASS);
                let idx = self.classes.len();
                self.classes.push(Class {
                    name: name(c.name()),
                    visibility: RawVisibility::from_ast(c.visibility()),
                    items: Vec::new(),
                    ast_id,
                });
                let items = self.lower_assoc_items(c.item_list());
                self.classes[idx].items = items;
                ModItem::Class(Idx::new(idx))
            }
            E::Impl(i) => {
                let ast_id = self.next_ast_id(IMPL);
                let idx = self.impls.len();
                self.impls.push(Impl { items: Vec::new(), ast_id });
                let items = self.lower_assoc_items(i.item_list());
                self.impls[idx].items = items;
                ModItem::Impl(Idx::new(idx))
            }
            E::ExternBlock(b) => {
                let ast_id = self.next_ast_id(EXTERN_BLOCK);
                let idx = self.extern_blocks.len();
                let abi = b.abi().and_then(|abi| abi.string()).map(|s| unquote(s.text()));
                self.extern_blocks.push(ExternBlock { abi, items: Vec::new(), ast_id });
                let items = self.lower_assoc_items(b.item_list());
                self.extern_blocks[idx].items = items;
                ModItem::ExternBlock(Idx::new(idx))
            }
            E::Const(c) => ModItem::Const(self.lower_const(&c)),
            E::TypeAlias(t) => ModItem::TypeAlias(self.lower_type_alias(&t)),
            E::Import(i) => {
                let ast_id = self.next_ast_id(IMPORT);
                let kind = if let Some(path) = i.path() {
                    // Pre-order, so `a.b` is `PATH(PATH(a) . b)` from left to right.
                    let names = path
                        .syntax()
                        .descendants()
                        .filter(|n| n.kind() == PATH_SEGMENT)
                        .filter_map(|segment| segment.children().find(|n| n.kind() == NAME_REF))
                        .map(|name_ref| Name::new(name_ref.text().to_string()))
                        .collect();
                    ImportKind::Variants(names)
                } else if let Some(list) = i.import_list() {
                    // Keep the order of the names as written.
                    let mut entries: Vec<(u32, Name, Option<Name>)> = Vec::new();
                    for name_ref in list.names() {
                        let start = name_ref.syntax().text_range().start().into();
                        entries.push((start, Name::new(name_ref.text()), None));
                    }
                    for alias in list.aliases() {
                        let start = alias.syntax().text_range().start().into();
                        if let Some(name_ref) = alias.name_ref() {
                            entries.push((start, Name::new(name_ref.text()), name(alias.alias())));
                        }
                    }
                    entries.sort_by_key(|(start, ..)| *start);
                    ImportKind::List(entries.into_iter().map(|(_, name, alias)| (name, alias)).collect())
                } else if let Some(alias) = name(i.alias()) {
                    ImportKind::Alias(alias)
                } else if i.star_token().is_some() {
                    ImportKind::Glob
                } else {
                    ImportKind::Plain
                };
                self.imports.push(Import {
                    visibility: RawVisibility::from_ast(i.visibility()),
                    source: i.source().map(|s| unquote(s.text())),
                    kind,
                    ast_id,
                });
                ModItem::Import(Idx::new(self.imports.len() - 1))
            }
            E::MacroRules(m) => {
                let ast_id = self.next_ast_id(MACRO_RULES);
                self.macro_rules.push(MacroRules { name: name(m.name()), ast_id });
                ModItem::MacroRules(Idx::new(self.macro_rules.len() - 1))
            }
            E::VarDecl(_) | E::ExprStmt(_) | E::MacroCall(_) => return None,
        };
        Some(item)
    }

    fn lower_assoc_items(&mut self, list: Option<ast::ItemList>) -> Vec<AssocItem> {
        let Some(list) = list else {
            return Vec::new();
        };
        list.items()
            .filter_map(|element| {
                use ast::SourceElement as E;
                Some(match element {
                    E::FuncDecl(f) => AssocItem::Function(self.lower_function(&f)),
                    E::Const(c) => AssocItem::Const(self.lower_const(&c)),
                    E::TypeAlias(t) => AssocItem::TypeAlias(self.lower_type_alias(&t)),
                    E::MacroCall(m) => {
                        let ast_id = self.next_ast_id(MACRO_CALL);
                        let name = m.name_ref().map(|n| Name::new(n.text()));
                        self.macro_calls.push(MacroCall { name, ast_id });
                        AssocItem::MacroCall(Idx::new(self.macro_calls.len() - 1))
                    }
                    // Other items can't appear here; the parser reports them.
                    _ => return None,
                })
            })
            .collect()
    }

    fn lower_function(&mut self, f: &ast::FuncDecl) -> Idx<Function> {
        let ast_id = self.next_ast_id(FUNC_DECL);
        let fn_name = if let Some(op) = f.operator_name().and_then(|op| op.operator()) {
            Some(FunctionName::Operator(Name::new(op.text())))
        } else {
            name(f.name()).map(FunctionName::Name)
        };
        self.functions.push(Function {
            name: fn_name,
            visibility: RawVisibility::from_ast(f.visibility()),
            is_async: f.async_token().is_some(),
            is_const: f.const_token().is_some(),
            has_body: f.body().is_some(),
            ast_id,
        });
        Idx::new(self.functions.len() - 1)
    }

    fn lower_const(&mut self, c: &ast::Const) -> Idx<Const> {
        let ast_id = self.next_ast_id(CONST);
        self.consts.push(Const {
            name: name(c.name()),
            visibility: RawVisibility::from_ast(c.visibility()),
            has_value: c.expr().is_some(),
            ast_id,
        });
        Idx::new(self.consts.len() - 1)
    }

    fn lower_type_alias(&mut self, t: &ast::TypeAlias) -> Idx<TypeAlias> {
        let ast_id = self.next_ast_id(TYPE_ALIAS);
        self.type_aliases.push(TypeAlias {
            name: name(t.name()),
            visibility: RawVisibility::from_ast(t.visibility()),
            has_value: t.ty().is_some(),
            ast_id,
        });
        Idx::new(self.type_aliases.len() - 1)
    }
}

fn name(name: Option<ast::Name>) -> Option<Name> {
    name.map(|n| Name::new(n.text()))
}

/// The contents of a string literal without its quotes. Escapes are left as
/// they are, since import paths and ABIs rarely contain them.
fn unquote(text: &str) -> String {
    text.trim_start_matches(['b', 'c', 'r', '#']).trim_start_matches('"').trim_end_matches('#').trim_end_matches('"').to_string()
}

#[cfg(test)]
mod tests;
