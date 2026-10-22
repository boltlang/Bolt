//! The expression store, like rust-analyzer's `ExpressionStore`: arenas of
//! expressions, patterns, types, bindings and labels, shared by the body of
//! a function and by signatures, which can contain expressions too (default
//! parameter values, array lengths, const generic arguments).
//!
//! The store doesn't know where things are in the source; that's in the
//! [`ExpressionStoreSourceMap`]. So edits that only move code leave the store
//! unchanged, and Salsa doesn't redo the type checking that depends on it.

mod lower;

use std::collections::HashMap;

use boltlang_syntax::{SyntaxKind, SyntaxNode};

use crate::{
    arena::{Arena, ArenaMap},
    hir::{Binding, BindingId, Expr, ExprId, Label, LabelId, Pat, PatId, TypeRef, TypeRefId},
};

pub use lower::ExprCollector;

#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ExpressionStore {
    pub exprs: Arena<Expr>,
    pub pats: Arena<Pat>,
    pub types: Arena<TypeRef>,
    pub bindings: Arena<Binding>,
    pub labels: Arena<Label>,
}

impl std::ops::Index<ExprId> for ExpressionStore {
    type Output = Expr;
    fn index(&self, id: ExprId) -> &Expr {
        &self.exprs[id]
    }
}

impl std::ops::Index<PatId> for ExpressionStore {
    type Output = Pat;
    fn index(&self, id: PatId) -> &Pat {
        &self.pats[id]
    }
}

impl std::ops::Index<TypeRefId> for ExpressionStore {
    type Output = TypeRef;
    fn index(&self, id: TypeRefId) -> &TypeRef {
        &self.types[id]
    }
}

impl std::ops::Index<BindingId> for ExpressionStore {
    type Output = Binding;
    fn index(&self, id: BindingId) -> &Binding {
        &self.bindings[id]
    }
}

impl std::ops::Index<LabelId> for ExpressionStore {
    type Output = Label;
    fn index(&self, id: LabelId) -> &Label {
        &self.labels[id]
    }
}

/// Where a syntax node is: its kind and text range. Like rowan's
/// `SyntaxNodePtr`, but ours so that Salsa can store it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct SyntaxPtr {
    pub kind: SyntaxKind,
    pub start: u32,
    pub end: u32,
}

impl SyntaxPtr {
    pub fn new(node: &SyntaxNode) -> Self {
        let range = node.text_range();
        SyntaxPtr { kind: node.kind(), start: range.start().into(), end: range.end().into() }
    }

    /// Find the node in the syntax tree it was made from.
    pub fn to_node(self, root: &SyntaxNode) -> Option<SyntaxNode> {
        let range = boltlang_syntax::rowan::TextRange::new(self.start.into(), self.end.into());
        let element = root.covering_element(range);
        let node = match element {
            boltlang_syntax::rowan::NodeOrToken::Node(node) => node,
            boltlang_syntax::rowan::NodeOrToken::Token(token) => token.parent()?,
        };
        node.ancestors().take_while(|n| n.text_range() == range).find(|n| n.kind() == self.kind)
    }
}

/// Maps the contents of an [`ExpressionStore`] back to the syntax they were
/// lowered from, and the other way around, for diagnostics and the IDE.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ExpressionStoreSourceMap {
    pub expr_map: HashMap<SyntaxPtr, ExprId>,
    pub expr_map_back: ArenaMap<Expr, SyntaxPtr>,
    pub pat_map: HashMap<SyntaxPtr, PatId>,
    pub pat_map_back: ArenaMap<Pat, SyntaxPtr>,
    pub type_map: HashMap<SyntaxPtr, TypeRefId>,
    pub type_map_back: ArenaMap<TypeRef, SyntaxPtr>,
}

impl ExpressionStoreSourceMap {
    pub fn expr_syntax(&self, id: ExprId) -> Option<SyntaxPtr> {
        self.expr_map_back.get(id).copied()
    }

    pub fn pat_syntax(&self, id: PatId) -> Option<SyntaxPtr> {
        self.pat_map_back.get(id).copied()
    }

    pub fn type_syntax(&self, id: TypeRefId) -> Option<SyntaxPtr> {
        self.type_map_back.get(id).copied()
    }

    pub fn node_expr(&self, node: &SyntaxNode) -> Option<ExprId> {
        self.expr_map.get(&SyntaxPtr::new(node)).copied()
    }

    pub fn node_pat(&self, node: &SyntaxNode) -> Option<PatId> {
        self.pat_map.get(&SyntaxPtr::new(node)).copied()
    }

    pub fn node_type(&self, node: &SyntaxNode) -> Option<TypeRefId> {
        self.type_map.get(&SyntaxPtr::new(node)).copied()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ptr_tests {
    use boltlang_syntax::{SyntaxKind, SyntaxNode};

    use super::SyntaxPtr;

    #[test]
    fn syntax_ptrs_find_their_nodes() {
        let (green, _) = boltlang_parser::parse_file("let x = (a + b) * c;");
        let root = SyntaxNode::new_root(green);
        for node in root.descendants() {
            assert_eq!(SyntaxPtr::new(&node).to_node(&root), Some(node.clone()));
        }
        // A NEST_EXPR and the BIN_EXPR inside it have different ranges, but a
        // PATH_EXPR and its PATH share one; the kind tells them apart.
        let path = root.descendants().find(|n| n.kind() == SyntaxKind::PATH).unwrap();
        let path_expr = path.parent().unwrap();
        assert_eq!(path.text_range(), path_expr.text_range());
        assert_eq!(SyntaxPtr::new(&path).to_node(&root), Some(path));
        assert_eq!(SyntaxPtr::new(&path_expr).to_node(&root), Some(path_expr));
    }
}
