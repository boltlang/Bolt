//! Lowering syntax into an [`ExpressionStore`], like rust-analyzer's
//! `ExprCollector`.

use boltlang_syntax::{
    SyntaxKind::{self, *},
    SyntaxNode, SyntaxToken,
    ast::{self, Node},
};

use std::collections::HashMap;

use super::{ExpressionStore, ExpressionStoreSourceMap, SyntaxPtr};
use crate::{
    Name,
    arena::{Arena, ArenaMap, Idx},
    hir::{
        BinaryOp, Binding, BindingId, BlockKind, EffectArm, Expr, ExprId, GenericArg, Label,
        LambdaKind, Literal, LogicOp, MatchArm, Mutability, MutabilityRef, ParamKind, Pat, PatId,
        Path, PathSegment, RecordField, RecordFieldPat, Spread, Statement, TupleItem, TypeRef,
        TypeRefId,
    },
};

/// Lowers syntax into a store, keeping track of where everything came from.
#[derive(Default)]
pub struct ExprCollector {
    pub store: ExpressionStore,
    pub source_map: ExpressionStoreSourceMap,
}

impl ExprCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finish(self) -> (ExpressionStore, ExpressionStoreSourceMap) {
        (self.store, self.source_map)
    }

    fn alloc_expr(&mut self, expr: Expr, node: Option<&SyntaxNode>) -> ExprId {
        let map = &mut self.source_map;
        alloc(&mut self.store.exprs, &mut map.expr_map, &mut map.expr_map_back, expr, node)
    }

    fn alloc_pat(&mut self, pat: Pat, node: Option<&SyntaxNode>) -> PatId {
        let map = &mut self.source_map;
        alloc(&mut self.store.pats, &mut map.pat_map, &mut map.pat_map_back, pat, node)
    }

    fn alloc_type(&mut self, type_ref: TypeRef, node: Option<&SyntaxNode>) -> TypeRefId {
        let map = &mut self.source_map;
        alloc(&mut self.store.types, &mut map.type_map, &mut map.type_map_back, type_ref, node)
    }

    pub fn missing_expr(&mut self) -> ExprId {
        self.alloc_expr(Expr::Missing, None)
    }

    // Types

    pub fn lower_opt_type(&mut self, ty: Option<ast::Type>) -> TypeRefId {
        match ty {
            Some(ty) => self.lower_type(ty),
            None => self.alloc_type(TypeRef::Error, None),
        }
    }

    /// A mutability parameter like the `'v` of `struct VecIter[T, 'v]`, as
    /// the mutability on its own that it names, and its bounds.
    pub fn lower_mutability_param(
        &mut self,
        param: &ast::MutabilityParam,
    ) -> Option<(Name, TypeRefId, Box<[TypeRefId]>)> {
        let name = Name::new(param.name_token()?.text().trim_start_matches('\''));
        let type_ref = self.alloc_type(
            TypeRef::Mutability(MutabilityRef::Named(name.clone())),
            Some(param.syntax()),
        );
        let bounds = self.lower_bounds(param.bounds());
        Some((name, type_ref, bounds))
    }

    /// The bounds after a `:`, like `Show + Eq` in `T: Show + Eq`.
    pub(crate) fn lower_bounds(&mut self, bounds: Option<ast::TraitBounds>) -> Box<[TypeRefId]> {
        bounds.map(|b| b.bounds().map(|t| self.lower_type(t)).collect()).unwrap_or_default()
    }

    pub fn lower_type(&mut self, ty: ast::Type) -> TypeRefId {
        let node = ty.syntax().clone();
        let type_ref = match ty {
            // Parentheses only group; the type inside stands for them.
            ast::Type::Nest(ty) => {
                let inner = self.lower_opt_type(ty.ty());
                self.source_map.type_map.insert(SyntaxPtr::new(&node), inner);
                return inner;
            }
            ast::Type::Path(ty) => match ty.path().and_then(|p| self.lower_path(p)) {
                Some(path) => TypeRef::Path(path),
                None => TypeRef::Error,
            },
            ast::Type::Tuple(ty) => {
                let plain =
                    ty.items().all(|item| matches!(item, ast::TupleTypeItem::Positional(_)));
                if plain {
                    TypeRef::Tuple(ty.fields().map(|t| self.lower_type(t)).collect())
                } else {
                    let items = ty
                        .items()
                        .map(|item| match item {
                            ast::TupleTypeItem::Positional(t) => {
                                TupleItem::Positional(self.lower_type(t))
                            }
                            ast::TupleTypeItem::Named(f) => {
                                let name = field_name(f.name().map(|n| n.text()));
                                TupleItem::Named(name, self.lower_opt_type(f.ty()))
                            }
                            ast::TupleTypeItem::Rest(r) => {
                                TupleItem::Spread(spread(r.spread()), self.lower_opt_type(r.ty()))
                            }
                        })
                        .collect();
                    TypeRef::NamedTuple(items)
                }
            }
            ast::Type::Array(ty) => {
                let element = self.lower_opt_type(ty.element());
                let len = ty.length().map(|e| self.lower_expr(e));
                let mutability = if ty.mut_token().is_some() {
                    Some(Mutability::Mut)
                } else if ty.imm_token().is_some() {
                    Some(Mutability::Imm)
                } else if ty.ro_token().is_some() {
                    Some(Mutability::ReadOnly)
                } else {
                    None
                };
                TypeRef::Array { element, len, mutability }
            }
            ast::Type::Mut(ty) => {
                let mutability = if let Some(name) = ty.name_token() {
                    MutabilityRef::Named(Name::new(name.text().trim_start_matches('\'')))
                } else if ty.mut_token().is_some() {
                    MutabilityRef::Written(Mutability::Mut)
                } else if ty.ro_token().is_some() {
                    MutabilityRef::Written(Mutability::ReadOnly)
                } else {
                    MutabilityRef::Written(Mutability::Imm)
                };
                match ty.inner() {
                    Some(inner) => TypeRef::Mut { mutability, inner: self.lower_type(inner) },
                    None => TypeRef::Mutability(mutability),
                }
            }
            ast::Type::Never(_) => TypeRef::Never,
            ast::Type::Bounded(_) => {
                // `A + B + C` is nested to the left; flatten it.
                let mut bounds = Vec::new();
                self.collect_bounds(ast::Type::cast(node.clone()).unwrap(), &mut bounds);
                TypeRef::Bounded(bounds.into())
            }
            ast::Type::Fn(ty) => self.lower_fn_type(&ty),
            ast::Type::ImplTrait(ty) => {
                let generic_params =
                    ty.generic_param_list()
                        .map(|list| {
                            list.params()
                                .filter_map(|param| match param {
                                    ast::GenericParam::Type(p) => name(p.name()),
                                    ast::GenericParam::Const(p) => name(p.name()),
                                    ast::GenericParam::Mutability(_)
                                    | ast::GenericParam::Pack(_) => None,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                let mut bounds = Vec::new();
                if let Some(inner) = ty.ty() {
                    self.collect_bounds(inner, &mut bounds);
                }
                let bounds = bounds.into();
                TypeRef::ImplTrait { generic_params, bounds }
            }
            ast::Type::Macro(ty) => TypeRef::MacroCall { name: macro_name(ty.macro_call()) },
        };
        self.alloc_type(type_ref, Some(&node))
    }

    fn collect_bounds(&mut self, ty: ast::Type, out: &mut Vec<TypeRefId>) {
        match ty {
            ast::Type::Bounded(bounded) => {
                if let Some(lhs) = bounded.lhs() {
                    self.collect_bounds(lhs, out);
                }
                if let Some(rhs) = bounded.rhs() {
                    out.push(self.lower_type(rhs));
                }
            }
            ty => out.push(self.lower_type(ty)),
        }
    }

    /// A part of a tuple literal or of the arguments of a call.
    fn lower_tuple_item(&mut self, item: ast::TupleExprItem) -> TupleItem<ExprId> {
        match item {
            ast::TupleExprItem::Positional(x) => TupleItem::Positional(self.lower_expr(x)),
            ast::TupleExprItem::Named(f) => {
                let name = field_name(f.name_ref().map(|n| n.text()));
                TupleItem::Named(name, self.lower_opt_expr(f.expr()))
            }
            ast::TupleExprItem::Spread(s) => {
                TupleItem::Spread(spread(s.spread()), self.lower_opt_expr(s.expr()))
            }
        }
    }

    fn lower_fn_type(&mut self, ty: &ast::FnType) -> TypeRef {
        let mut params = Vec::new();
        let mut is_variadic = false;
        // After a `*` or `*Ts`, the parameters are keyword-only.
        let mut keyword = false;
        if let Some(list) = ty.param_list() {
            for child in list.syntax().children() {
                match child.kind() {
                    KEYWORD_MARKER => keyword = true,
                    TUPLE_TYPE_REST => {
                        let rest = ast::TupleTypeRest::cast(child).unwrap();
                        let kind = rest_kind(rest.spread(), &mut keyword);
                        params.push((None, kind, self.lower_pack_tuple(&rest)));
                    }
                    PARAM => {
                        let param = ast::Param::cast(child).unwrap();
                        let name = pattern_name(param.pattern());
                        // In a function type, a parameter without a name is
                        // just a type.
                        let type_ref = match (param.type_expr(), param.pattern()) {
                            (Some(ty), _) => self.lower_type(ty),
                            (None, None) => {
                                let ty = param.syntax().children().find_map(ast::Type::cast);
                                self.lower_opt_type(ty)
                            }
                            (None, Some(_)) => self.alloc_type(TypeRef::Error, None),
                        };
                        let kind = if keyword { ParamKind::Keyword } else { ParamKind::Positional };
                        params.push((name, kind, type_ref));
                    }
                    VARIADIC_PARAM => is_variadic = true,
                    _ => {}
                }
            }
        }
        let ret_type = ty.ret_type().map(|r| self.lower_opt_type(r.ty()));
        let effects = self.lower_effect_row(ty.effect_row());
        let has_token = |kind| ty.syntax().children_with_tokens().any(|t| t.kind() == kind);
        TypeRef::Fn {
            params: params.into(),
            is_variadic,
            ret_type,
            effects,
            is_const: has_token(CONST_KEYWORD),
        }
    }

    /// The type of what a rest like `*Ts` or `**Ks` gives: the tuple
    /// `(*Ts)` or `(**Ks)`.
    pub fn lower_pack_tuple(&mut self, rest: &ast::TupleTypeRest) -> TypeRefId {
        let pack = self.lower_opt_type(rest.ty());
        let item = TupleItem::Spread(spread(rest.spread()), pack);
        self.alloc_type(TypeRef::NamedTuple(Box::new([item])), Some(rest.syntax()))
    }

    /// The effects after `does`: one type for each, like `Log` and
    /// `State[i32]` in `does Log + State[i32]`. `None` without `does`.
    pub fn lower_effect_row(&mut self, row: Option<ast::EffectRow>) -> Option<Box<[TypeRefId]>> {
        Some(row?.effects().map(|t| self.lower_type(t)).collect())
    }

    // Paths

    pub fn lower_path(&mut self, path: ast::Path) -> Option<Path> {
        let mut segments = Vec::new();
        for segment in path.segments() {
            let name = Name::from_ident(&segment.name_ref()?.text());
            let generic_args = segment.generic_arg_list().map(|list| self.lower_generic_args(list));
            segments.push(PathSegment { name, generic_args });
        }
        Some(Path { segments: segments.into() })
    }

    fn lower_generic_args(&mut self, list: ast::GenericArgList) -> Box<[GenericArg]> {
        list.args()
            .map(|arg| match arg {
                ast::GenericArg::Type(arg) => {
                    let type_ref = self.lower_opt_type(arg.ty());
                    let bounds = self.lower_bounds(arg.bounds());
                    GenericArg::Type { type_ref, bounds }
                }
                ast::GenericArg::AssocType(arg) => GenericArg::AssocType {
                    name: arg
                        .name_ref()
                        .map(|n| Name::from_ident(&n.text()))
                        .unwrap_or_else(|| Name::new("")),
                    generic_args: arg.generic_arg_list().map(|l| self.lower_generic_args(l)),
                    type_ref: self.lower_opt_type(arg.ty()),
                },
                ast::GenericArg::Const(arg) => GenericArg::Const(self.lower_opt_expr(arg.expr())),
            })
            .collect()
    }

    // Expressions

    pub fn lower_opt_expr(&mut self, expr: Option<ast::Expr>) -> ExprId {
        match expr {
            Some(expr) => self.lower_expr(expr),
            None => self.missing_expr(),
        }
    }

    pub fn lower_expr(&mut self, expr: ast::Expr) -> ExprId {
        use ast::Expr as E;
        let node = expr.syntax().clone();
        let lowered = match expr {
            // Parentheses only group; the expression inside stands for them.
            E::Nest(e) => {
                let inner = self.lower_opt_expr(e.expr());
                self.source_map.expr_map.insert(SyntaxPtr::new(&node), inner);
                return inner;
            }
            E::Path(e) => match e.path().and_then(|p| self.lower_path(p)) {
                Some(path) => Expr::Path(path),
                None => Expr::Missing,
            },
            E::Lit(e) => match e.value().and_then(|t| lower_literal(&t)) {
                Some(literal) => Expr::Literal(literal),
                None => Expr::Missing,
            },
            E::Block(e) => return self.lower_block(&e),
            E::Call(e) if e.arg_items().all(|a| matches!(a, ast::TupleExprItem::Positional(_))) => {
                let callee = self.lower_opt_expr(e.operator());
                let args = e.args().map(|a| self.lower_expr(a)).collect();
                Expr::Call { callee, args }
            }
            E::Call(e) => {
                let callee = self.lower_opt_expr(e.operator());
                let args = e.arg_items().map(|item| self.lower_tuple_item(item)).collect();
                Expr::CallWith { callee, args }
            }
            E::Field(e) => {
                let expr = self.lower_opt_expr(e.expr());
                let name = field_name(e.name_ref().map(|n| n.text()));
                Expr::Field { expr, name }
            }
            E::Try(e) => {
                let expr = self.lower_opt_expr(e.expr());
                let mut hidden =
                    || self.store.bindings.alloc(Binding { name: Name::new("?"), mutable: false });
                let (value, error) = (hidden(), hidden());
                Expr::Try { expr, value, error }
            }
            E::Imm(e) => Expr::Imm { expr: self.lower_opt_expr(e.expr()) },
            E::Mut(e) => {
                // `'n [1]` names its mutability, which is resolved where the
                // literal is checked.
                let mutability = e.name_token().map(|token| {
                    let name = Name::new(token.text().trim_start_matches('\''));
                    let type_ref = TypeRef::Mutability(MutabilityRef::Named(name));
                    self.alloc_type(type_ref, Some(e.syntax()))
                });
                Expr::Mut { expr: self.lower_opt_expr(e.expr()), mutability }
            }
            E::Cast(e) => {
                let expr = self.lower_opt_expr(e.expr());
                let type_ref = self.lower_opt_type(e.type_expr());
                Expr::Cast { expr, type_ref }
            }
            E::Index(e) => {
                let base = self.lower_opt_expr(e.base());
                let index = self.lower_opt_expr(e.index());
                Expr::Index { base, index }
            }
            E::Prefix(e) => {
                let op = match e.operator().map(|t| t.kind()) {
                    Some(MINUS) => "-",
                    _ => "!",
                };
                Expr::Prefix { op: Name::new(op), expr: self.lower_opt_expr(e.expr()) }
            }
            E::Bin(e) => {
                let lhs = self.lower_opt_expr(e.lhs());
                let rhs = self.lower_opt_expr(e.rhs());
                let op = binary_op(e.operator().as_ref().map_or("", |t| t.text()));
                Expr::Binary { op, lhs, rhs }
            }
            E::Range(e) => {
                let inclusive = e.op_token().is_some_and(|t| t.kind() == DOTDOTEQUALS);
                let lhs = e.start().map(|s| self.lower_expr(s));
                let rhs = e.end().map(|s| self.lower_expr(s));
                Expr::Range { lhs, rhs, inclusive }
            }
            E::Tuple(e) => {
                let plain = e.items().all(|item| matches!(item, ast::TupleExprItem::Positional(_)));
                if plain {
                    Expr::Tuple { exprs: e.elements().map(|x| self.lower_expr(x)).collect() }
                } else {
                    let items = e.items().map(|item| self.lower_tuple_item(item)).collect();
                    Expr::NamedTuple { items }
                }
            }
            E::Array(e) => {
                if e.is_repeat() {
                    let value = self.lower_opt_expr(e.elements().next());
                    let count = self.lower_opt_expr(e.repeat_count());
                    Expr::ArrayRepeat { value, count }
                } else {
                    Expr::Array { exprs: e.elements().map(|x| self.lower_expr(x)).collect() }
                }
            }
            E::Record(e) => {
                let path = e.path().and_then(|p| self.lower_path(p));
                let mut fields = Vec::new();
                let mut spread = None;
                if let Some(list) = e.field_list() {
                    for field in list.fields() {
                        let Some(name_ref) = field.name_ref() else { continue };
                        let name = Name::from_ident(&name_ref.text());
                        // `x` is short for `x: x`.
                        let expr = match field.expr() {
                            Some(expr) => self.lower_expr(expr),
                            None => self.alloc_expr(
                                Expr::Path(Path::from_name(name.clone())),
                                Some(field.syntax()),
                            ),
                        };
                        fields.push(RecordField { name, expr });
                    }
                    spread = list.spread().map(|s| self.lower_expr(s));
                }
                Expr::Record { path, fields: fields.into(), spread }
            }
            E::If(e) => {
                let condition = self.lower_opt_expr(e.condition());
                let then_branch = match e.then_branch() {
                    Some(block) => self.lower_block(&block),
                    None => self.missing_expr(),
                };
                let else_branch = e.else_branch().map(|b| self.lower_expr(b));
                Expr::If { condition, then_branch, else_branch }
            }
            E::Let(e) => {
                let pat = self.lower_opt_pat(e.pattern());
                let expr = self.lower_opt_expr(e.expr());
                Expr::Let { pat, expr }
            }
            E::Match(e) => {
                let expr = self.lower_opt_expr(e.expr());
                let mut arms = Vec::new();
                let mut effect_arms = Vec::new();
                for arm in e.arms() {
                    match arm.effect_pattern() {
                        Some(effect) => effect_arms.push(self.lower_effect_arm(&arm, effect)),
                        None => arms.push(MatchArm {
                            pat: self.lower_opt_pat(arm.pattern()),
                            guard: arm.guard().map(|g| self.lower_opt_expr(g.condition())),
                            expr: self.lower_opt_expr(arm.expr()),
                        }),
                    }
                }
                Expr::Match { expr, arms: arms.into(), effect_arms: effect_arms.into() }
            }
            E::While(e) => {
                let label = self.lower_label(e.label());
                let condition = self.lower_opt_expr(e.condition());
                let body = self.lower_opt_block(e.body());
                Expr::While { condition, body, label }
            }
            E::Loop(e) => {
                let label = self.lower_label(e.label());
                let body = self.lower_opt_block(e.body());
                Expr::Loop { body, label }
            }
            E::For(e) => {
                let label = self.lower_label(e.label());
                let pat = self.lower_opt_pat(e.pattern());
                let iterable = self.lower_opt_expr(e.iterable());
                let body = self.lower_opt_block(e.body());
                Expr::For { pat, iterable, body, label }
            }
            E::Break(e) => Expr::Break {
                expr: e.expr().map(|x| self.lower_expr(x)),
                label: e.label().and_then(|l| label_name(&l)),
            },
            E::Continue(e) => Expr::Continue { label: e.label().and_then(|l| label_name(&l)) },
            E::Ret(e) => Expr::Return { expr: e.expr().map(|x| self.lower_expr(x)) },
            E::Closure(e) => self.lower_lambda(
                e.param_list(),
                e.ret_type(),
                e.effect_row(),
                e.body(),
                LambdaKind::Closure,
            ),
            E::Fun(e) => self.lower_lambda(
                e.param_list(),
                e.ret_type(),
                e.effect_row(),
                e.body(),
                LambdaKind::Function,
            ),
            E::Macro(e) => Expr::MacroCall { name: macro_name(e.macro_call()) },
        };
        self.alloc_expr(lowered, Some(&node))
    }

    /// A closure like `|x| x + 1`, or an anonymous function like `fn (x) = x + 1`.
    fn lower_lambda(
        &mut self,
        list: Option<ast::ParamList>,
        ret_type: Option<ast::RetType>,
        effect_row: Option<ast::EffectRow>,
        body: Option<ast::Expr>,
        kind: LambdaKind,
    ) -> Expr {
        let effects = self.lower_effect_row(effect_row);
        let mut params = Vec::new();
        let mut param_types = Vec::new();
        for param in list.iter().flat_map(|l| l.params()) {
            params.push(self.lower_opt_pat(param.pattern()));
            param_types.push(param.type_expr().map(|t| self.lower_type(t)));
        }
        let ret_type = ret_type.map(|r| self.lower_opt_type(r.ty()));
        let body = self.lower_opt_expr(body);
        Expr::Lambda {
            params: params.into(),
            param_types: param_types.into(),
            ret_type,
            effects,
            body,
            kind,
        }
    }

    fn lower_label(&mut self, label: Option<ast::Label>) -> Option<crate::hir::LabelId> {
        let name = label_name(&label?)?;
        Some(self.store.labels.alloc(Label { name }))
    }

    fn lower_opt_block(&mut self, block: Option<ast::BlockExpr>) -> ExprId {
        match block {
            Some(block) => self.lower_block(&block),
            None => self.missing_expr(),
        }
    }

    pub fn lower_block(&mut self, block_expr: &ast::BlockExpr) -> ExprId {
        let kind =
            if block_expr.const_token().is_some() { BlockKind::Const } else { BlockKind::Plain };
        let label = self.lower_label(block_expr.label());
        let mut statements = Vec::new();
        let mut tail = None;
        if let Some(block) = block_expr.block() {
            statements = self.lower_statements(block.elements());
            tail = block.tail_expr().map(|e| self.lower_expr(e));
        }
        self.alloc_expr(
            Expr::Block { statements: statements.into(), tail, label, kind },
            Some(block_expr.syntax()),
        )
    }

    /// Lower the statements of a block or a source file.
    pub fn lower_statements(
        &mut self,
        elements: impl Iterator<Item = ast::SourceElement>,
    ) -> Vec<Statement> {
        elements
            .map(|element| match element {
                ast::SourceElement::VarDecl(decl) => {
                    let pat = self.lower_opt_pat(decl.pattern());
                    let type_ref = decl.type_expr().map(|t| self.lower_type(t));
                    let initializer = decl.expr().map(|e| self.lower_expr(e));
                    let else_branch = decl.else_block().map(|b| self.lower_block(&b));
                    Statement::Let { pat, type_ref, initializer, else_branch }
                }
                ast::SourceElement::ExprStmt(stmt) => {
                    let has_semi = stmt.syntax().children_with_tokens().any(|t| t.kind() == SEMI);
                    Statement::Expr { expr: self.lower_opt_expr(stmt.expr()), has_semi }
                }
                _ => Statement::Item,
            })
            .collect()
    }

    // Patterns

    /// `effect op(args), cont if guard => expr`, or with several operations,
    /// like `effect a(x) | b(x), k`. The parser only accepts operations
    /// written like tuple struct patterns; anything else is lowered as a
    /// missing pattern.
    fn lower_effect_arm(&mut self, arm: &ast::MatchArm, effect: ast::EffectPattern) -> EffectArm {
        let ops: Box<[PatId]> = effect
            .operations()
            .map(|op| match op {
                ast::Pattern::TupleStruct(_) => self.lower_pat(op),
                _ => self.alloc_pat(Pat::Missing, Some(op.syntax())),
            })
            .collect();
        let lists: Vec<&[PatId]> = ops.iter().map(std::slice::from_ref).collect();
        self.share_bindings(&lists);
        EffectArm {
            ops,
            cont: self.lower_opt_pat(effect.continuation()),
            guard: arm.guard().map(|g| self.lower_opt_expr(g.condition())),
            expr: self.lower_opt_expr(arm.expr()),
        }
    }

    pub fn lower_opt_pat(&mut self, pat: Option<ast::Pattern>) -> PatId {
        match pat {
            Some(pat) => self.lower_pat(pat),
            None => self.alloc_pat(Pat::Missing, None),
        }
    }

    pub fn lower_pat(&mut self, pat: ast::Pattern) -> PatId {
        use ast::Pattern as P;
        let node = pat.syntax().clone();
        let lowered = match pat {
            P::Named(p) => self.binding_pat(p.name(), false, None),
            P::Wildcard(_) => Pat::Wild,
            P::Lit(p) => match p.literal().and_then(|t| lower_literal(&t)) {
                Some(literal) => Pat::Lit { literal, negative: p.minus_token().is_some() },
                None => Pat::Missing,
            },
            P::Path(p) => match p.path().and_then(|p| self.lower_path(p)) {
                Some(path) => Pat::Path(path),
                None => Pat::Missing,
            },
            P::Tuple(p) if p.items().all(|item| item.is_ok()) => {
                let (args, rest) = self.lower_pat_list(p.fields());
                Pat::Tuple { args, rest }
            }
            P::Tuple(p) => {
                let mut items = Vec::new();
                let mut rest = None;
                let mut positional = 0;
                for item in p.items() {
                    match item {
                        Ok(ast::Pattern::Rest(_)) => {
                            rest.get_or_insert(positional);
                        }
                        Ok(pat) => {
                            positional += 1;
                            items.push(TupleItem::Positional(self.lower_pat(pat)));
                        }
                        Err(f) => {
                            let name = field_name(f.name_ref().map(|n| n.text()));
                            items.push(TupleItem::Named(name, self.lower_opt_pat(f.pattern())));
                        }
                    }
                }
                Pat::NamedTuple { items: items.into(), rest }
            }
            P::TupleStruct(p) => {
                let Some(path) = p.path().and_then(|p| self.lower_path(p)) else {
                    return self.alloc_pat(Pat::Missing, Some(&node));
                };
                let (args, rest) = self.lower_pat_list(p.fields());
                Pat::TupleStruct { path, args, rest }
            }
            P::Struct(p) => {
                let Some(path) = p.path().and_then(|p| self.lower_path(p)) else {
                    return self.alloc_pat(Pat::Missing, Some(&node));
                };
                let mut fields = Vec::new();
                for field in p.fields() {
                    let mutable = field.mut_token().is_some();
                    if let Some(name_ref) = field.name_ref() {
                        let name = Name::from_ident(&name_ref.text());
                        let pat = self.lower_opt_pat(field.pattern());
                        if mutable {
                            self.mark_mutable(pat);
                        }
                        fields.push(RecordFieldPat { name, pat });
                    } else if let Some(n) = field.name() {
                        // `x` is short for `x: x`.
                        let name = Name::from_ident(&n.text());
                        let bind = self.binding_pat(Some(n), mutable, None);
                        let pat = self.alloc_pat(bind, Some(field.syntax()));
                        fields.push(RecordFieldPat { name, pat });
                    }
                }
                Pat::Record { path, fields: fields.into(), rest: p.rest().is_some() }
            }
            // A `..` on its own is only allowed in a list; see lower_pat_list.
            P::Rest(_) => Pat::Wild,
            P::Slice(p) => {
                let (args, rest) = self.lower_pat_list(p.elements());
                Pat::Slice { args, rest }
            }
            P::Captured(p) => {
                let subpat = p.pattern().map(|s| self.lower_pat(s));
                self.binding_pat(p.name(), false, subpat)
            }
            // `mut x` makes the bindings inside it mutable.
            P::Mut(p) => {
                let inner = self.lower_opt_pat(p.pattern());
                self.mark_mutable(inner);
                self.source_map.pat_map.insert(SyntaxPtr::new(&node), inner);
                return inner;
            }
            P::Range(p) => {
                let inclusive = p.op_token().is_some_and(|t| t.kind() == DOTDOTEQUALS);
                let start = p.start().map(|s| self.lower_pat(s));
                let end = p.end().map(|s| self.lower_pat(s));
                Pat::Range { start, end, inclusive }
            }
            P::Or(p) => {
                let alternatives: Box<[PatId]> = p.patterns().map(|s| self.lower_pat(s)).collect();
                let lists: Vec<&[PatId]> = alternatives.iter().map(std::slice::from_ref).collect();
                self.share_bindings(&lists);
                Pat::Or(alternatives)
            }
            P::ConstBlock(p) => match p.block() {
                Some(block) => Pat::ConstBlock(self.lower_block(&block)),
                None => Pat::Missing,
            },
            P::Macro(p) => Pat::MacroCall { name: macro_name(p.macro_call()) },
        };
        self.alloc_pat(lowered, Some(&node))
    }

    fn binding_pat(
        &mut self,
        name: Option<ast::Name>,
        mutable: bool,
        subpat: Option<PatId>,
    ) -> Pat {
        let Some(name) = name else { return Pat::Missing };
        let id =
            self.store.bindings.alloc(Binding { name: Name::from_ident(&name.text()), mutable });
        Pat::Bind { id, subpat }
    }

    /// Lower the patterns of a tuple, tuple struct or slice pattern, and find
    /// the position of the `..`, if there is one.
    fn lower_pat_list(
        &mut self,
        pats: impl Iterator<Item = ast::Pattern>,
    ) -> (Box<[PatId]>, Option<u32>) {
        let mut args = Vec::new();
        let mut rest = None;
        for pat in pats {
            if matches!(pat, ast::Pattern::Rest(_)) {
                rest.get_or_insert(args.len() as u32);
            } else {
                args.push(self.lower_pat(pat));
            }
        }
        (args.into(), rest)
    }

    /// Make the alternatives of an or-pattern share the bindings of the
    /// first one, by name, so that the `x` of `A(x) | B(x)` is one binding.
    /// Each alternative is a list of patterns, to also cover the arguments of
    /// the operations in an effect arm. A name with another mutability than
    /// in the first alternative keeps a binding of its own, and so does a
    /// name the first doesn't bind: diagnostics report both.
    fn share_bindings(&mut self, alternatives: &[&[PatId]]) {
        let Some((first, rest)) = alternatives.split_first() else { return };
        let mut shared: HashMap<Name, BindingId> = HashMap::new();
        for (_, id) in self.store.binds_in(first) {
            shared.entry(self.store.bindings[id].name.clone()).or_insert(id);
        }
        for alternative in rest {
            for (pat, id) in self.store.binds_in(alternative) {
                let binding = &self.store.bindings[id];
                let Some(&to) = shared.get(&binding.name) else { continue };
                if to != id && self.store.bindings[to].mutable == binding.mutable {
                    let Pat::Bind { id, .. } = &mut self.store.pats[pat] else { unreachable!() };
                    *id = to;
                }
            }
        }
    }

    /// Make the bindings in a pattern mutable, but not the ones after an `@`.
    fn mark_mutable(&mut self, pat: PatId) {
        let mut stack = vec![pat];
        while let Some(pat) = stack.pop() {
            match &self.store.pats[pat] {
                // In `mut x @ Some(y)`, only `x` is mutable.
                Pat::Bind { id, .. } => {
                    let id = *id;
                    self.store.bindings[id].mutable = true;
                }
                pat => stack.extend(pat.children()),
            }
        }
    }
}

/// Allocate `value` in `arena`, and if it was lowered from `node`, remember
/// that in both directions.
fn alloc<T>(
    arena: &mut Arena<T>,
    map: &mut HashMap<SyntaxPtr, Idx<T>>,
    back: &mut ArenaMap<T, SyntaxPtr>,
    value: T,
    node: Option<&SyntaxNode>,
) -> Idx<T> {
    let id = arena.alloc(value);
    if let Some(node) = node {
        let ptr = SyntaxPtr::new(node);
        map.insert(ptr, id);
        back.insert(id, ptr);
    }
    id
}

/// The name of a field, or an empty name where the parser found none.
fn field_name(text: Option<impl AsRef<str>>) -> Name {
    text.map_or_else(|| Name::new(""), |t| Name::from_ident(t.as_ref()))
}

/// What a spread or a rest stands for. One written `...`, which the parser
/// reports, counts as positional.
fn spread(spread: Option<ast::Spread>) -> Spread {
    match spread {
        Some(ast::Spread::Named) => Spread::Named,
        Some(ast::Spread::Positional) | None => Spread::Positional,
    }
}

/// The name of a parameter's pattern, if it's `x` or `mut x`.
pub(crate) fn pattern_name(pattern: Option<ast::Pattern>) -> Option<Name> {
    match pattern? {
        ast::Pattern::Named(p) => name(p.name()),
        ast::Pattern::Mut(p) => pattern_name(p.pattern()),
        _ => None,
    }
}

/// How a rest parameter `*args` or `**opts` is passed. After a positional
/// one, the parameters are `keyword`-only.
pub(crate) fn rest_kind(spread: Option<ast::Spread>, keyword: &mut bool) -> ParamKind {
    match spread {
        Some(ast::Spread::Named) => ParamKind::KeywordRest,
        _ => {
            *keyword = true;
            ParamKind::Rest
        }
    }
}

fn name(name: Option<ast::Name>) -> Option<Name> {
    name.map(|n| Name::from_ident(&n.text()))
}

fn macro_name(call: Option<ast::MacroCall>) -> Option<Name> {
    call?.name_ref().map(|n| Name::from_ident(&n.text()))
}

fn label_name(label: &ast::Label) -> Option<Name> {
    let ident = label.ident()?;
    Some(Name::new(ident.text().trim_start_matches('\'')))
}

/// The operator `op`: `&&`, `||`, an assignment, or a call of the function
/// with its name. A compound assignment like `+=` uses the function `+`.
fn binary_op(op: &str) -> BinaryOp {
    match op {
        "&&" => BinaryOp::Logic(LogicOp::And),
        "||" => BinaryOp::Logic(LogicOp::Or),
        "=" => BinaryOp::Assign(None),
        // `!=`, `<=` and `>=` aren't assignments, and neither is an operator
        // with more than one `=`, like `==`, as the lexer has it.
        "!=" | "<=" | ">=" => BinaryOp::Operator(Name::new(op)),
        _ if op.len() > 1 && op.ends_with('=') && op.matches('=').count() == 1 => {
            BinaryOp::Assign(Some(Name::new(&op[..op.len() - 1])))
        }
        _ => BinaryOp::Operator(Name::new(op)),
    }
}

const NUMERIC_SUFFIXES: &[&str] = &[
    "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "u128", "i128", "isize", "usize", "f32",
    "f64",
];

/// Lower a literal token. Returns `None` for a token that isn't a literal.
pub(crate) fn lower_literal(token: &SyntaxToken) -> Option<Literal> {
    let text = token.text();
    Some(match token.kind() {
        INT_NUMBER => {
            let (digits, suffix) = split_suffix(text, token.kind());
            let digits: String = digits.chars().filter(|c| *c != '_').collect();
            let (radix, digits) = match digits.get(..2) {
                Some("0x") => (16, &digits[2..]),
                Some("0o") => (8, &digits[2..]),
                Some("0b") => (2, &digits[2..]),
                _ => (10, &digits[..]),
            };
            // An overflowing literal is reported by the type checker.
            Literal::Int(u128::from_str_radix(digits, radix).unwrap_or(u128::MAX), suffix)
        }
        FLOAT_NUMBER => {
            let (digits, suffix) = split_suffix(text, token.kind());
            Literal::Float(digits.chars().filter(|c| *c != '_').collect(), suffix)
        }
        CHAR => Literal::Char(unescape(strip_quotes(text, '\'')).chars().next().unwrap_or('\0')),
        BYTE => Literal::Byte(
            unescape_bytes(strip_quotes(&text[1..], '\'')).first().copied().unwrap_or(0),
        ),
        STRING => Literal::String(string_contents(text)),
        BYTE_STRING => Literal::ByteString(byte_string_contents(&text[1..])),
        C_STRING => Literal::CString(string_contents(&text[1..])),
        _ => return None,
    })
}

/// Split a number into its digits and its type suffix, like `10` and `u8`.
fn split_suffix(text: &str, kind: SyntaxKind) -> (&str, Option<Name>) {
    let is_hex = kind == INT_NUMBER && text.starts_with("0x");
    for suffix in NUMERIC_SUFFIXES {
        if let Some(digits) = text.strip_suffix(suffix) {
            // In `0xf32`, `f32` are hex digits, not a suffix.
            if is_hex && suffix.starts_with('f') {
                continue;
            }
            if !digits.is_empty() && digits != "0x" {
                return (digits, Some(Name::new(*suffix)));
            }
        }
    }
    (text, None)
}

/// `text` without the one `quote` it starts with and the one it ends with,
/// so that an escaped quote, like the one of `'\''`, stays.
fn strip_quotes(text: &str, quote: char) -> &str {
    let inner = text.strip_prefix(quote).unwrap_or(text);
    inner.strip_suffix(quote).unwrap_or(inner)
}

/// The text between the quotes of a raw string, like `r#"a"b"#`, which
/// keeps the quotes in it, or `None` if `text` isn't a raw string.
fn raw_contents(text: &str) -> Option<&str> {
    let raw = text.strip_prefix('r')?;
    let hashes = raw.len() - raw.trim_start_matches('#').len();
    let inner = raw.get(hashes..raw.len().saturating_sub(hashes)).unwrap_or("");
    Some(strip_quotes(inner, '"'))
}

/// The contents of a string literal: unescaped, unless it's a raw string.
fn string_contents(text: &str) -> String {
    match raw_contents(text) {
        Some(raw) => raw.to_string(),
        None => unescape(strip_quotes(text, '"')),
    }
}

/// The bytes of a byte string, after its `b`: `\xff` is one byte, not the
/// UTF-8 of the character U+00FF.
fn byte_string_contents(text: &str) -> Vec<u8> {
    match raw_contents(text) {
        Some(raw) => raw.as_bytes().to_vec(),
        None => unescape_bytes(strip_quotes(text, '"')),
    }
}

/// Like [`unescape`], for a byte string or a byte: `\xNN` is the byte
/// `NN`, and any other character is its UTF-8 bytes.
fn unescape_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    unescape_with(text, |piece| match piece {
        Unescaped::Char(c) => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
        Unescaped::Byte(b) => out.push(b),
    });
    out
}

/// The contents of a string literal without its quotes and prefix, like the
/// path of an import or an ABI: unescaped, unless it's a raw string.
pub(crate) fn unquote(text: &str) -> String {
    string_contents(text.trim_start_matches(['b', 'c']))
}

/// Replace escape sequences like `\n` and `\u{1F600}` with what they stand
/// for. The lexer already reported invalid ones.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    unescape_with(text, |piece| match piece {
        Unescaped::Char(c) => out.push(c),
        Unescaped::Byte(b) => out.push(b as char),
    });
    out
}

/// What an escape sequence of [`unescape_with`] stands for.
enum Unescaped {
    Char(char),
    /// A `\xNN`: a character in a string, a byte in a byte string.
    Byte(u8),
}

/// The characters of `text`, with its escape sequences replaced.
fn unescape_with(text: &str, mut out: impl FnMut(Unescaped)) {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out(Unescaped::Char(c));
            continue;
        }
        match chars.next() {
            Some('n') => out(Unescaped::Char('\n')),
            Some('r') => out(Unescaped::Char('\r')),
            Some('t') => out(Unescaped::Char('\t')),
            Some('0') => out(Unescaped::Char('\0')),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(b) = u8::from_str_radix(&hex, 16) {
                    out(Unescaped::Byte(b));
                }
            }
            Some('u') => {
                let hex: String = if chars.peek() == Some(&'{') {
                    chars.next();
                    chars.by_ref().take_while(|c| *c != '}').collect()
                } else {
                    chars.by_ref().take(4).collect()
                };
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out(Unescaped::Char(c));
                }
            }
            // `\\`, `\'`, `\"`, and any other escaped character.
            Some(other) => out(Unescaped::Char(other)),
            None => {}
        }
    }
}
