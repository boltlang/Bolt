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
        ArithOp, BinaryOp, Binding, BlockKind, CmpOp, Expr, ExprId, GenericArg, Label, LambdaKind, Literal, LogicOp,
        MatchArm, Pat, PatId, Path, PathSegment, PrefixOp, RecordField, RecordFieldPat, Statement, TypeRef, TypeRefId,
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

    pub fn lower_type(&mut self, ty: ast::Type) -> TypeRefId {
        let node = ty.syntax().clone();
        let type_ref = match ty {
            ast::Type::Path(ty) => match ty.path().and_then(|p| self.lower_path(p)) {
                Some(path) => TypeRef::Path(path),
                None => TypeRef::Error,
            },
            ast::Type::Tuple(ty) => TypeRef::Tuple(ty.fields().map(|t| self.lower_type(t)).collect()),
            ast::Type::Array(ty) => {
                let element = self.lower_opt_type(ty.element());
                let len = ty.length().map(|e| self.lower_expr(e));
                TypeRef::Array { element, len }
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
                let generic_params = ty
                    .generic_param_list()
                    .map(|list| {
                        list.params()
                            .filter_map(|param| match param {
                                ast::GenericParam::Type(p) => name(p.name()),
                                ast::GenericParam::Const(p) => name(p.name()),
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

    fn lower_fn_type(&mut self, ty: &ast::FnType) -> TypeRef {
        let mut params = Vec::new();
        let mut is_variadic = false;
        if let Some(list) = ty.param_list() {
            for child in list.syntax().children() {
                match child.kind() {
                    PARAM => {
                        let param = ast::Param::cast(child).unwrap();
                        let name = match param.pattern() {
                            Some(ast::Pattern::Named(p)) => name(p.name()),
                            _ => None,
                        };
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
                        params.push((name, type_ref));
                    }
                    VARIADIC_PARAM => is_variadic = true,
                    _ => {}
                }
            }
        }
        let ret_type = ty.ret_type().map(|r| self.lower_opt_type(r.ty()));
        let has_token = |kind| ty.syntax().children_with_tokens().any(|t| t.kind() == kind);
        let abi = ty.abi().map(|abi| abi.string().map(|s| unquote(s.text())));
        TypeRef::Fn {
            params: params.into(),
            is_variadic,
            ret_type,
            is_async: has_token(ASYNC_KEYWORD),
            is_const: has_token(CONST_KEYWORD),
            abi,
        }
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
                    let bounds = arg.bounds().map(|b| b.bounds().map(|t| self.lower_type(t)).collect()).unwrap_or_default();
                    GenericArg::Type { type_ref, bounds }
                }
                ast::GenericArg::AssocType(arg) => GenericArg::AssocType {
                    name: arg.name_ref().map(|n| Name::from_ident(&n.text())).unwrap_or_else(|| Name::new("")),
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
            E::Call(e) => {
                let callee = self.lower_opt_expr(e.operator());
                let args = e.args().map(|a| self.lower_expr(a)).collect();
                Expr::Call { callee, args }
            }
            E::Field(e) => {
                let expr = self.lower_opt_expr(e.expr());
                let name = e.name_ref().map(|n| Name::from_ident(&n.text())).unwrap_or_else(|| Name::new(""));
                Expr::Field { expr, name }
            }
            E::Await(e) => Expr::Await { expr: self.lower_opt_expr(e.expr()) },
            E::Try(e) => Expr::Try { expr: self.lower_opt_expr(e.expr()) },
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
                    Some(MINUS) => PrefixOp::Neg,
                    _ => PrefixOp::Not,
                };
                Expr::Prefix { op, expr: self.lower_opt_expr(e.expr()) }
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
            E::Tuple(e) => Expr::Tuple { exprs: e.elements().map(|x| self.lower_expr(x)).collect() },
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
                            None => self.alloc_expr(Expr::Path(Path::from_name(name.clone())), Some(field.syntax())),
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
                let arms = e
                    .arms()
                    .into_iter()
                    .map(|arm| MatchArm {
                        pat: self.lower_opt_pat(arm.pattern()),
                        guard: arm.guard().map(|g| self.lower_opt_expr(g.condition())),
                        expr: self.lower_opt_expr(arm.expr()),
                    })
                    .collect();
                Expr::Match { expr, arms }
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
            E::Yield(e) => Expr::Yield { expr: e.expr().map(|x| self.lower_expr(x)) },
            E::Closure(e) => {
                let kind = if e.async_token().is_some() { LambdaKind::AsyncClosure } else { LambdaKind::Closure };
                self.lower_lambda(e.param_list(), e.ret_type(), e.body(), kind)
            }
            E::Fun(e) => self.lower_lambda(e.param_list(), e.ret_type(), e.body(), LambdaKind::Function),
            E::Macro(e) => Expr::MacroCall { name: macro_name(e.macro_call()) },
        };
        self.alloc_expr(lowered, Some(&node))
    }

    /// A closure like `|x| x + 1`, or an anonymous function like `fn (x) = x + 1`.
    fn lower_lambda(
        &mut self,
        list: Option<ast::ParamList>,
        ret_type: Option<ast::RetType>,
        body: Option<ast::Expr>,
        kind: LambdaKind,
    ) -> Expr {
        let mut params = Vec::new();
        let mut param_types = Vec::new();
        for param in list.iter().flat_map(|l| l.params()) {
            params.push(self.lower_opt_pat(param.pattern()));
            param_types.push(param.type_expr().map(|t| self.lower_type(t)));
        }
        let ret_type = ret_type.map(|r| self.lower_opt_type(r.ty()));
        let body = self.lower_opt_expr(body);
        Expr::Lambda { params: params.into(), param_types: param_types.into(), ret_type, body, kind }
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
        let kind = if block_expr.async_token().is_some() {
            BlockKind::Async
        } else if block_expr.const_token().is_some() {
            BlockKind::Const
        } else {
            BlockKind::Plain
        };
        let label = self.lower_label(block_expr.label());
        let mut statements = Vec::new();
        let mut tail = None;
        if let Some(block) = block_expr.block() {
            statements = self.lower_statements(block.elements());
            tail = block.tail_expr().map(|e| self.lower_expr(e));
        }
        self.alloc_expr(Expr::Block { statements: statements.into(), tail, label, kind }, Some(block_expr.syntax()))
    }

    /// Lower the statements of a block or a source file.
    pub fn lower_statements(&mut self, elements: impl Iterator<Item = ast::SourceElement>) -> Vec<Statement> {
        elements
            .map(|element| match element {
                ast::SourceElement::VarDecl(decl) => {
                    let pat = self.lower_opt_pat(decl.pattern());
                    let type_ref = decl.type_expr().map(|t| self.lower_type(t));
                    let initializer = decl.expr().map(|e| self.lower_expr(e));
                    let else_branch = decl.else_block().map(|b| self.lower_block(&b));
                    // `let mut x` makes the binding mutable.
                    if decl.mut_token().is_some() {
                        self.mark_mutable(pat);
                    }
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
            P::Tuple(p) => {
                let (args, rest) = self.lower_pat_list(p.fields());
                Pat::Tuple { args, rest }
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
            P::Or(p) => Pat::Or(p.patterns().map(|s| self.lower_pat(s)).collect()),
            P::ConstBlock(p) => match p.block() {
                Some(block) => Pat::ConstBlock(self.lower_block(&block)),
                None => Pat::Missing,
            },
            P::Macro(p) => Pat::MacroCall { name: macro_name(p.macro_call()) },
        };
        self.alloc_pat(lowered, Some(&node))
    }

    fn binding_pat(&mut self, name: Option<ast::Name>, mutable: bool, subpat: Option<PatId>) -> Pat {
        let Some(name) = name else { return Pat::Missing };
        let id = self.store.bindings.alloc(Binding { name: Name::from_ident(&name.text()), mutable });
        Pat::Bind { id, subpat }
    }

    /// Lower the patterns of a tuple, tuple struct or slice pattern, and find
    /// the position of the `..`, if there is one.
    fn lower_pat_list(&mut self, pats: impl Iterator<Item = ast::Pattern>) -> (Box<[PatId]>, Option<u32>) {
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

fn binary_op(op: &str) -> BinaryOp {
    use ArithOp::*;
    match op {
        "+" => BinaryOp::Arith(Add),
        "-" => BinaryOp::Arith(Sub),
        "*" => BinaryOp::Arith(Mul),
        "/" => BinaryOp::Arith(Div),
        "%" => BinaryOp::Arith(Rem),
        "&" => BinaryOp::Arith(BitAnd),
        "|" => BinaryOp::Arith(BitOr),
        "^" => BinaryOp::Arith(BitXor),
        "<<" => BinaryOp::Arith(Shl),
        ">>" => BinaryOp::Arith(Shr),
        "==" => BinaryOp::Cmp(CmpOp::Eq),
        "!=" => BinaryOp::Cmp(CmpOp::NotEq),
        "<" => BinaryOp::Cmp(CmpOp::Lt),
        "<=" => BinaryOp::Cmp(CmpOp::LtEq),
        ">" => BinaryOp::Cmp(CmpOp::Gt),
        ">=" => BinaryOp::Cmp(CmpOp::GtEq),
        "&&" => BinaryOp::Logic(LogicOp::And),
        "||" => BinaryOp::Logic(LogicOp::Or),
        "=" => BinaryOp::Assign(None),
        "+=" => BinaryOp::Assign(Some(Add)),
        "-=" => BinaryOp::Assign(Some(Sub)),
        "*=" => BinaryOp::Assign(Some(Mul)),
        "/=" => BinaryOp::Assign(Some(Div)),
        "%=" => BinaryOp::Assign(Some(Rem)),
        "&=" => BinaryOp::Assign(Some(BitAnd)),
        "|=" => BinaryOp::Assign(Some(BitOr)),
        "^=" => BinaryOp::Assign(Some(BitXor)),
        "<<=" => BinaryOp::Assign(Some(Shl)),
        ">>=" => BinaryOp::Assign(Some(Shr)),
        other => BinaryOp::Custom(Name::new(other)),
    }
}

const NUMERIC_SUFFIXES: &[&str] =
    &["u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "u128", "i128", "isize", "usize", "f32", "f64"];

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
        CHAR => Literal::Char(unescape(text.trim_matches('\'')).chars().next().unwrap_or('\0')),
        BYTE => Literal::Byte(unescape(text[1..].trim_matches('\'')).chars().next().map_or(0, |c| c as u8)),
        STRING => Literal::String(string_contents(text)),
        BYTE_STRING => Literal::ByteString(string_contents(&text[1..]).into_bytes()),
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

/// The contents of a string literal: unescaped, unless it's a raw string.
fn string_contents(text: &str) -> String {
    if let Some(raw) = text.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let inner = &raw[hashes..raw.len() - hashes];
        return inner.trim_start_matches('"').trim_end_matches('"').to_string();
    }
    let inner = text.strip_prefix('"').unwrap_or(text);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    unescape(inner)
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
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    out.push(byte as char);
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
                    out.push(c);
                }
            }
            // `\\`, `\'`, `\"`, and any other escaped character.
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}
