//! Lowering a body to [`Inst`]s. Bolt's control flow is structured like
//! Wasm's, so expressions are compiled one by one, without a control-flow
//! graph: `if` is `if`, a loop is a `block` around a `loop`, and `break`
//! and `continue` are branches to them.

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_eval::{Evaluator, Value};
use boltlang_hir_def::{
    Name,
    body::{Body, DefWithBody, DefWithBodyId, body},
    builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType, BuiltinUint},
    hir::{
        ArithOp, BinaryOp, BindingId, BlockKind, CmpOp, Expr, ExprId, LabelId, Literal, LogicOp, MatchArm, Pat,
        PatId, Path, PathSegment, PrefixOp, Statement,
    },
    ids::{ConstId, FunctionId, ItemContainerId},
    resolver::{Resolver, ValueNs, ValueResolution},
};
use boltlang_hir_ty::{
    InferenceResult, Scheme, Ty,
    builtin::{Builtin, builtin},
    function_scheme, infer_body,
};

use crate::{
    CodegenError, ErrorKind, Instance,
    ir::{BlockTy, Callee, CompiledFn, Inst, Op, ValTy},
};

type Result<'db, T> = std::result::Result<T, CodegenError<'db>>;

/// A type the code generator can compile, and how its values behave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scalar {
    Int { bits: u8, signed: bool },
    F32,
    F64,
    Bool,
    Char,
    /// A pointer to a string in linear memory.
    Str,
}

impl Scalar {
    fn of(ty: &Ty<'_>) -> Option<Scalar> {
        let Ty::Builtin(ty) = ty else { return None };
        Some(match ty {
            BuiltinType::Bool => Scalar::Bool,
            BuiltinType::Char => Scalar::Char,
            BuiltinType::String => Scalar::Str,
            BuiltinType::Int(int) => match int {
                BuiltinInt::I8 => Scalar::Int { bits: 8, signed: true },
                BuiltinInt::I16 => Scalar::Int { bits: 16, signed: true },
                BuiltinInt::I32 | BuiltinInt::Isize => Scalar::Int { bits: 32, signed: true },
                BuiltinInt::I64 => Scalar::Int { bits: 64, signed: true },
                BuiltinInt::I128 => return None,
            },
            BuiltinType::Uint(uint) => match uint {
                BuiltinUint::U8 => Scalar::Int { bits: 8, signed: false },
                BuiltinUint::U16 => Scalar::Int { bits: 16, signed: false },
                BuiltinUint::U32 | BuiltinUint::Usize => Scalar::Int { bits: 32, signed: false },
                BuiltinUint::U64 => Scalar::Int { bits: 64, signed: false },
                BuiltinUint::U128 => return None,
            },
            BuiltinType::Float(BuiltinFloat::F32) => Scalar::F32,
            BuiltinType::Float(BuiltinFloat::F64) => Scalar::F64,
        })
    }

    fn val_ty(self) -> ValTy {
        match self {
            Scalar::Int { bits: 64, .. } => ValTy::I64,
            Scalar::Int { .. } | Scalar::Bool | Scalar::Char | Scalar::Str => ValTy::I32,
            Scalar::F32 => ValTy::F32,
            Scalar::F64 => ValTy::F64,
        }
    }

    /// Whether comparisons and conversions treat it as signed.
    fn signed(self) -> bool {
        matches!(self, Scalar::Int { signed: true, .. })
    }
}

/// What a structured instruction is for, to find the targets of `break`
/// and `continue`.
#[derive(Debug, Clone, Copy)]
enum Frame {
    Plain,
    /// What `break` leaves: a loop, or a labeled block. A loop's block
    /// gives the value of `break value`.
    Break { label: Option<LabelId>, is_loop: bool, result: Option<ValTy> },
    /// What `continue` goes back to: the `loop` of a loop.
    Continue { label: Option<LabelId> },
}

pub(crate) fn compile<'db>(db: &'db dyn Db, instance: Instance<'db>) -> Result<'db, CompiledFn<'db>> {
    let def = instance.body(db);
    let body = body(db, def);
    let scheme = match def.def(db) {
        DefWithBody::Function(f) => Some(
            function_scheme(db, f)
                .map_err(|()| CodegenError { kind: ErrorKind::UnknownType, location: Some((def, body.body_expr)) })?,
        ),
        DefWithBody::File(_) => None,
        DefWithBody::Const(_) | DefWithBody::Variant(_) => {
            return Err(CodegenError {
                kind: ErrorKind::Unsupported("compiling the body of a const".into()),
                location: Some((def, body.body_expr)),
            });
        }
    };
    let mut c = Compiler {
        db,
        def,
        body,
        infer: infer_body(db, def),
        scheme,
        type_args: instance.type_args(db),
        params: Vec::new(),
        result: None,
        locals: Vec::new(),
        bindings: HashMap::new(),
        code: Vec::new(),
        callees: Vec::new(),
        frames: Vec::new(),
    };
    match scheme {
        Some(scheme) => c.function(scheme)?,
        None => c.entry()?,
    }
    c.code.push(Inst::End);
    Ok(CompiledFn { params: c.params, result: c.result, locals: c.locals, code: c.code, callees: c.callees })
}

struct Compiler<'db> {
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    body: &'db Body,
    infer: &'db InferenceResult<'db>,
    scheme: Option<&'db Scheme<'db>>,
    type_args: &'db [Ty<'db>],
    params: Vec<ValTy>,
    result: Option<ValTy>,
    /// The locals after the parameters.
    locals: Vec<ValTy>,
    /// The local of each variable, or `None` for one of type `()`.
    bindings: HashMap<BindingId, Option<u32>>,
    code: Vec<Inst<'db>>,
    callees: Vec<Callee<'db>>,
    frames: Vec<Frame>,
}

impl<'db> Compiler<'db> {
    fn error(&self, at: ExprId, kind: ErrorKind) -> CodegenError<'db> {
        CodegenError { kind, location: Some((self.def, at)) }
    }

    fn unsupported(&self, at: ExprId, what: impl Into<String>) -> CodegenError<'db> {
        self.error(at, ErrorKind::Unsupported(what.into()))
    }

    fn push(&mut self, inst: Inst<'db>) {
        self.code.push(inst);
    }

    fn op(&mut self, op: Op) {
        self.code.push(Inst::Op(op));
    }

    fn open(&mut self, inst: Inst<'db>, frame: Frame) {
        self.code.push(inst);
        self.frames.push(frame);
    }

    fn close(&mut self) {
        self.code.push(Inst::End);
        self.frames.pop();
    }

    // Types

    /// A type of the body, with the instance's type arguments for the
    /// parameters and bound variables of the function's scheme.
    fn subst(&self, ty: &Ty<'db>) -> Ty<'db> {
        let Some(scheme) = self.scheme else { return ty.clone() };
        ty.fold(&mut |t| match t {
            Ty::Param(p) => scheme.params.iter().position(|q| q == p).map(|i| self.type_args[i].clone()),
            Ty::Bound(i) => self.type_args.get(scheme.params.len() + *i as usize).cloned(),
            _ => None,
        })
    }

    fn ty(&self, expr: ExprId) -> Ty<'db> {
        self.subst(self.infer.expr_ty(expr))
    }

    /// The Wasm type of values of `ty`, or `None` for `()` and `!`.
    fn val_ty(&self, ty: &Ty<'db>, at: ExprId) -> Result<'db, Option<ValTy>> {
        match ty {
            Ty::Never => Ok(None),
            Ty::Tuple(tys) if tys.is_empty() => Ok(None),
            Ty::Var(_) | Ty::Error | Ty::Bound(_) | Ty::Param(_) => Err(self.error(at, ErrorKind::UnknownType)),
            _ => match Scalar::of(ty) {
                Some(scalar) => Ok(Some(scalar.val_ty())),
                None => Err(self.unsupported(at, format!("a value of type `{}`", ty.display(self.db)))),
            },
        }
    }

    fn expr_val_ty(&self, expr: ExprId) -> Result<'db, Option<ValTy>> {
        self.val_ty(&self.ty(expr), expr)
    }

    fn scalar(&self, expr: ExprId) -> Result<'db, Scalar> {
        let ty = self.ty(expr);
        Scalar::of(&ty).ok_or_else(|| self.unsupported(expr, format!("an operation on `{}`", ty.display(self.db))))
    }

    // Locals

    fn new_local(&mut self, ty: ValTy) -> u32 {
        self.locals.push(ty);
        (self.params.len() + self.locals.len() - 1) as u32
    }

    /// The local of a variable, made the first time it's needed.
    fn binding_local(&mut self, binding: BindingId, at: ExprId) -> Result<'db, Option<u32>> {
        if let Some(local) = self.bindings.get(&binding) {
            return Ok(*local);
        }
        let ty = self.subst(self.infer.binding_ty(binding));
        let local = self.val_ty(&ty, at)?.map(|ty| self.new_local(ty));
        self.bindings.insert(binding, local);
        Ok(local)
    }

    /// Evaluate `expr` into a new local, for a value that's used several
    /// times, like the scrutinee of a `match`.
    fn temp(&mut self, expr: ExprId) -> Result<'db, Option<u32>> {
        self.expr(expr)?;
        let local = self.expr_val_ty(expr)?.map(|ty| self.new_local(ty));
        if let Some(local) = local {
            self.push(Inst::LocalSet(local));
        }
        Ok(local)
    }

    // Bodies

    fn function(&mut self, scheme: &'db Scheme<'db>) -> Result<'db, ()> {
        let body = self.body;
        let Ty::Fn(params, ret) = self.subst(&scheme.ty) else { unreachable!("a function has a function type") };
        for (&pat, ty) in body.params.iter().zip(params.iter()) {
            let index = self.val_ty(ty, body.body_expr)?.map(|ty| {
                self.params.push(ty);
                self.params.len() as u32 - 1
            });
            match &body[pat] {
                Pat::Bind { id, subpat: None } => {
                    self.bindings.insert(*id, index);
                }
                Pat::Wild => {}
                _ => return Err(self.unsupported(body.body_expr, "a pattern in a parameter")),
            }
        }
        self.result = self.val_ty(&ret, body.body_expr)?;
        self.expr_as(body.body_expr, self.result)
    }

    /// The top-level statements, which return the value of the last one if
    /// it's an expression.
    fn entry(&mut self) -> Result<'db, ()> {
        let body = self.body;
        let Expr::Block { statements, .. } = &body[body.body_expr] else { unreachable!("a file's body is a block") };
        if let Some(Statement::Expr { expr, .. }) = statements.last() {
            self.result = self.expr_val_ty(*expr)?;
        }
        for (i, statement) in statements.iter().enumerate() {
            match statement {
                Statement::Expr { expr, .. } if i + 1 == statements.len() => self.expr(*expr)?,
                statement => self.statement(statement)?,
            }
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Statement) -> Result<'db, ()> {
        match statement {
            Statement::Let { pat, initializer, else_branch, .. } => self.let_stmt(*pat, *initializer, *else_branch),
            Statement::Expr { expr, .. } => self.expr_as(*expr, None),
            Statement::Item => Ok(()),
        }
    }

    fn let_stmt(&mut self, pat: PatId, initializer: Option<ExprId>, else_branch: Option<ExprId>) -> Result<'db, ()> {
        let body = self.body;
        let Some(init) = initializer else {
            // `let x;` only needs the local.
            return match &body[pat] {
                Pat::Bind { id, subpat: None } => self.binding_local(*id, body.body_expr).map(|_| ()),
                _ => Err(self.unsupported(body.body_expr, "a `let` without a value and with a pattern")),
            };
        };
        match (&body[pat], else_branch) {
            (Pat::Bind { id, subpat: None }, None) => {
                self.expr(init)?;
                match self.binding_local(*id, init)? {
                    Some(local) => self.push(Inst::LocalSet(local)),
                    None if self.expr_val_ty(init)?.is_some() => self.push(Inst::Drop),
                    None => {}
                }
                Ok(())
            }
            (Pat::Wild, None) => self.expr_as(init, None),
            _ => {
                // A pattern that may not match: the `else` runs when it
                // doesn't, and must not finish.
                let ty = self.ty(init);
                let value = self.temp(init)?;
                self.pattern(pat, value, &ty, init)?;
                self.op(Op::I32Eqz);
                self.open(Inst::If(BlockTy::Empty), Frame::Plain);
                if let Some(else_branch) = else_branch {
                    self.expr_as(else_branch, None)?;
                }
                self.push(Inst::Unreachable);
                self.close();
                Ok(())
            }
        }
    }

    // Expressions

    /// Compile an expression whose value, if it has one, is dropped when
    /// `want` is `None`.
    fn expr_as(&mut self, expr: ExprId, want: Option<ValTy>) -> Result<'db, ()> {
        self.expr(expr)?;
        if want.is_none() && self.expr_val_ty(expr)?.is_some() {
            self.push(Inst::Drop);
        }
        Ok(())
    }

    /// Compile an expression, which leaves its value on the stack if its
    /// type has values.
    fn expr(&mut self, expr: ExprId) -> Result<'db, ()> {
        let body = self.body;
        match &body[expr] {
            Expr::Missing => Err(self.error(expr, ErrorKind::UnknownType)),
            Expr::Literal(literal) => self.literal(expr, literal, false),
            Expr::Path(path) => self.path(expr, path),
            Expr::Block { statements, tail, label, kind } => {
                if *kind != BlockKind::Plain {
                    return Err(self.unsupported(expr, "an `async` or `const` block"));
                }
                let result = self.expr_val_ty(expr)?;
                if let Some(label) = label {
                    self.open(
                        Inst::Block(result.into()),
                        Frame::Break { label: Some(*label), is_loop: false, result },
                    );
                }
                for statement in statements.iter() {
                    self.statement(statement)?;
                }
                if let Some(tail) = tail {
                    self.expr_as(*tail, result)?;
                }
                if label.is_some() {
                    self.close();
                }
                Ok(())
            }
            Expr::Call { callee, args } => self.call(expr, *callee, args),
            Expr::Cast { expr: inner, .. } => self.cast(expr, *inner),
            Expr::Prefix { op, expr: inner } => self.prefix(expr, *op, *inner),
            Expr::Binary { op, lhs, rhs } => self.binary(expr, op, *lhs, *rhs),
            Expr::Tuple { exprs } if exprs.is_empty() => Ok(()),
            Expr::If { condition, then_branch, else_branch } => {
                self.expr(*condition)?;
                let result = self.expr_val_ty(expr)?;
                self.open(Inst::If(result.into()), Frame::Plain);
                self.expr_as(*then_branch, result)?;
                if let Some(else_branch) = else_branch {
                    self.push(Inst::Else);
                    self.expr_as(*else_branch, result)?;
                }
                self.close();
                Ok(())
            }
            // `let pattern = value` in a condition: whether it matches.
            Expr::Let { pat, expr: value } => {
                let ty = self.ty(*value);
                let local = self.temp(*value)?;
                self.pattern(*pat, local, &ty, *value)
            }
            Expr::Match { expr: scrutinee, arms } => self.match_expr(expr, *scrutinee, arms),
            Expr::While { condition, body: loop_body, label } => {
                self.open(Inst::Block(BlockTy::Empty), Frame::Break { label: *label, is_loop: true, result: None });
                self.open(Inst::Loop(BlockTy::Empty), Frame::Continue { label: *label });
                self.expr(*condition)?;
                self.op(Op::I32Eqz);
                self.push(Inst::BrIf(1));
                self.expr_as(*loop_body, None)?;
                self.push(Inst::Br(0));
                self.close();
                self.close();
                Ok(())
            }
            Expr::Loop { body: loop_body, label } => {
                let result = self.expr_val_ty(expr)?;
                self.open(Inst::Block(result.into()), Frame::Break { label: *label, is_loop: true, result });
                self.open(Inst::Loop(BlockTy::Empty), Frame::Continue { label: *label });
                self.expr_as(*loop_body, None)?;
                self.push(Inst::Br(0));
                self.close();
                // A loop is only left with `break`.
                self.push(Inst::Unreachable);
                self.close();
                Ok(())
            }
            Expr::Break { expr: value, label } => {
                let (depth, result) = self.break_target(expr, label.as_ref())?;
                if let Some(value) = value {
                    self.expr_as(*value, result)?;
                }
                self.push(Inst::Br(depth));
                Ok(())
            }
            Expr::Continue { label } => {
                let depth = self.continue_target(expr, label.as_ref())?;
                self.push(Inst::Br(depth));
                Ok(())
            }
            Expr::Return { expr: value } => {
                if let Some(value) = value {
                    self.expr_as(*value, self.result)?;
                }
                self.push(Inst::Return);
                Ok(())
            }
            other => Err(self.unsupported(expr, describe(other))),
        }
    }

    fn depth(&self, frame: usize) -> u32 {
        (self.frames.len() - 1 - frame) as u32
    }

    fn break_target(&self, expr: ExprId, label: Option<&Name>) -> Result<'db, (u32, Option<ValTy>)> {
        for (i, frame) in self.frames.iter().enumerate().rev() {
            if let Frame::Break { label: frame_label, is_loop, result } = *frame {
                let found = match label {
                    Some(name) => frame_label.is_some_and(|l| self.body[l].name == *name),
                    None => is_loop,
                };
                if found {
                    return Ok((self.depth(i), result));
                }
            }
        }
        Err(self.unsupported(expr, "a `break` outside of a loop"))
    }

    fn continue_target(&self, expr: ExprId, label: Option<&Name>) -> Result<'db, u32> {
        for (i, frame) in self.frames.iter().enumerate().rev() {
            if let Frame::Continue { label: frame_label } = *frame {
                let found = match label {
                    Some(name) => frame_label.is_some_and(|l| self.body[l].name == *name),
                    None => true,
                };
                if found {
                    return Ok(self.depth(i));
                }
            }
        }
        Err(self.unsupported(expr, "a `continue` outside of a loop"))
    }

    fn resolve(&self, expr: ExprId, path: &Path) -> Option<ValueResolution<'db>> {
        Resolver::for_expr(self.db, self.def, expr).resolve_path_in_value_ns(self.db, path)
    }

    fn path(&mut self, expr: ExprId, path: &Path) -> Result<'db, ()> {
        match self.resolve(expr, path) {
            Some(ValueResolution::Value(ValueNs::Local(binding))) => {
                if let Some(local) = self.binding_local(binding, expr)? {
                    self.push(Inst::LocalGet(local));
                }
                Ok(())
            }
            Some(ValueResolution::Value(ValueNs::Const(id))) => self.const_value(expr, id),
            Some(ValueResolution::Value(ValueNs::Builtin(v))) => {
                self.push(Inst::I32Const(v.as_bool() as i32));
                Ok(())
            }
            Some(ValueResolution::Value(ValueNs::Function(_))) => Err(self.unsupported(expr, "a function as a value")),
            Some(ValueResolution::Value(ValueNs::Variant(_))) => Err(self.unsupported(expr, "an enum")),
            _ => Err(self.unsupported(expr, "this path")),
        }
    }

    /// A const, whose value the evaluator computes at compile time.
    fn const_value(&mut self, expr: ExprId, id: ConstId<'db>) -> Result<'db, ()> {
        let value = Evaluator::new(self.db)
            .eval_const(id)
            .map_err(|e| self.unsupported(expr, format!("a const whose value can't be computed ({e})")))?;
        let Some(scalar) = Scalar::of(&self.ty(expr)) else {
            return match value {
                Value::Tuple(items) if items.is_empty() => Ok(()),
                _ => Err(self.unsupported(expr, "a const of this type")),
            };
        };
        match value {
            Value::Int(n) => self.int_const(scalar, n),
            Value::Bool(b) => self.int_const(scalar, b as i128),
            Value::Char(c) => self.int_const(scalar, c as i128),
            Value::Float(x) => self.float_const(scalar, x),
            Value::Str(s) => self.push(Inst::Str(s.as_ref().into())),
            _ => return Err(self.unsupported(expr, "a const of this type")),
        }
        Ok(())
    }

    fn int_const(&mut self, scalar: Scalar, n: i128) {
        match scalar.val_ty() {
            ValTy::I64 => self.push(Inst::I64Const(n as i64)),
            ValTy::F32 => self.push(Inst::F32Const((n as f32).to_bits())),
            ValTy::F64 => self.push(Inst::F64Const((n as f64).to_bits())),
            ValTy::I32 => self.push(Inst::I32Const(n as i32)),
        }
    }

    fn float_const(&mut self, scalar: Scalar, x: f64) {
        match scalar {
            Scalar::F32 => self.push(Inst::F32Const((x as f32).to_bits())),
            _ => self.push(Inst::F64Const(x.to_bits())),
        }
    }

    fn literal(&mut self, expr: ExprId, literal: &Literal, negative: bool) -> Result<'db, ()> {
        let sign = if negative { -1 } else { 1 };
        match literal {
            Literal::Char(c) => self.push(Inst::I32Const(*c as i32)),
            Literal::Byte(b) => self.push(Inst::I32Const(*b as i32)),
            Literal::Int(n, _) => {
                let scalar = self.scalar(expr)?;
                self.int_const(scalar, sign * *n as i128);
            }
            Literal::Float(text, _) => {
                let scalar = self.scalar(expr)?;
                let x: f64 = text.parse().map_err(|_| self.error(expr, ErrorKind::UnknownType))?;
                self.float_const(scalar, sign as f64 * x);
            }
            Literal::String(s) => self.push(Inst::Str(s.as_str().into())),
            Literal::CString(_) | Literal::ByteString(_) => {
                return Err(self.unsupported(expr, "a C string or a byte string"));
            }
        }
        Ok(())
    }

    /// After arithmetic on an 8- or 16-bit integer in an `i32`, make the
    /// value fit again: wrap it around like the type would.
    fn wrap(&mut self, scalar: Scalar) {
        match scalar {
            Scalar::Int { bits: 8, signed: true } => self.op(Op::I32Extend8S),
            Scalar::Int { bits: 16, signed: true } => self.op(Op::I32Extend16S),
            Scalar::Int { bits: 8, signed: false } => {
                self.push(Inst::I32Const(0xff));
                self.op(Op::I32And);
            }
            Scalar::Int { bits: 16, signed: false } => {
                self.push(Inst::I32Const(0xffff));
                self.op(Op::I32And);
            }
            _ => {}
        }
    }

    fn prefix(&mut self, expr: ExprId, op: PrefixOp, inner: ExprId) -> Result<'db, ()> {
        let scalar = self.scalar(inner)?;
        match (op, scalar) {
            (PrefixOp::Neg, Scalar::F32) => {
                self.expr(inner)?;
                self.op(Op::F32Neg);
            }
            (PrefixOp::Neg, Scalar::F64) => {
                self.expr(inner)?;
                self.op(Op::F64Neg);
            }
            (PrefixOp::Neg, Scalar::Int { bits: 64, .. }) => {
                self.push(Inst::I64Const(0));
                self.expr(inner)?;
                self.op(Op::I64Sub);
            }
            (PrefixOp::Neg, Scalar::Int { .. }) => {
                self.push(Inst::I32Const(0));
                self.expr(inner)?;
                self.op(Op::I32Sub);
                self.wrap(scalar);
            }
            (PrefixOp::Not, Scalar::Bool) => {
                self.expr(inner)?;
                self.op(Op::I32Eqz);
            }
            (PrefixOp::Not, Scalar::Int { bits: 64, .. }) => {
                self.expr(inner)?;
                self.push(Inst::I64Const(-1));
                self.op(Op::I64Xor);
            }
            (PrefixOp::Not, Scalar::Int { .. }) => {
                self.expr(inner)?;
                self.push(Inst::I32Const(-1));
                self.op(Op::I32Xor);
                self.wrap(scalar);
            }
            _ => return Err(self.unsupported(expr, "this prefix operator on this type")),
        }
        Ok(())
    }

    fn binary(&mut self, expr: ExprId, op: &BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<'db, ()> {
        match op {
            BinaryOp::Logic(op) => {
                self.expr(lhs)?;
                self.open(Inst::If(BlockTy::Value(ValTy::I32)), Frame::Plain);
                match op {
                    LogicOp::And => {
                        self.expr(rhs)?;
                        self.push(Inst::Else);
                        self.push(Inst::I32Const(0));
                    }
                    LogicOp::Or => {
                        self.push(Inst::I32Const(1));
                        self.push(Inst::Else);
                        self.expr(rhs)?;
                    }
                }
                self.close();
                Ok(())
            }
            BinaryOp::Arith(op) => {
                let scalar = self.scalar(lhs)?;
                if scalar == Scalar::Str {
                    return Err(self.unsupported(expr, "joining strings"));
                }
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.shift_amount(*op, scalar, rhs)?;
                self.arith(expr, *op, scalar)
            }
            BinaryOp::Cmp(op) => {
                let scalar = self.scalar(lhs)?;
                if scalar == Scalar::Str {
                    return Err(self.unsupported(expr, "comparing strings"));
                }
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.op(cmp_op(*op, scalar));
                Ok(())
            }
            BinaryOp::Assign(op) => {
                let local = match &self.body[lhs] {
                    Expr::Path(path) => match self.resolve(lhs, path) {
                        Some(ValueResolution::Value(ValueNs::Local(binding))) => self.binding_local(binding, lhs)?,
                        _ => return Err(self.unsupported(lhs, "assigning to this")),
                    },
                    _ => return Err(self.unsupported(lhs, "assigning to a field or an element")),
                };
                let Some(local) = local else { return self.expr_as(rhs, None) };
                match op {
                    None => self.expr(rhs)?,
                    Some(op) => {
                        let scalar = self.scalar(lhs)?;
                        self.push(Inst::LocalGet(local));
                        self.expr(rhs)?;
                        self.shift_amount(*op, scalar, rhs)?;
                        self.arith(expr, *op, scalar)?;
                    }
                }
                self.push(Inst::LocalSet(local));
                Ok(())
            }
            // `a <$> b` calls the function `(<$>)`.
            BinaryOp::Custom(name) => match self.resolve(expr, &Path::from_name(name.clone())) {
                Some(ValueResolution::Value(ValueNs::Function(f))) => self.call_function(expr, f, None, &[lhs, rhs]),
                _ => Err(self.unsupported(expr, "this operator")),
            },
        }
    }

    /// The right side of a shift can be of another integer type than the
    /// left; Wasm needs them to be of the same size.
    fn shift_amount(&mut self, op: ArithOp, scalar: Scalar, rhs: ExprId) -> Result<'db, ()> {
        if !matches!(op, ArithOp::Shl | ArithOp::Shr) {
            return Ok(());
        }
        match (scalar.val_ty(), self.expr_val_ty(rhs)?) {
            (ValTy::I32, Some(ValTy::I64)) => self.op(Op::I32WrapI64),
            (ValTy::I64, Some(ValTy::I32)) => self.op(Op::I64ExtendI32U),
            _ => {}
        }
        Ok(())
    }

    fn arith(&mut self, expr: ExprId, op: ArithOp, scalar: Scalar) -> Result<'db, ()> {
        let Some(wasm) = arith_op(op, scalar) else {
            return Err(self.unsupported(expr, "this operator on this type"));
        };
        self.op(wasm);
        if matches!(op, ArithOp::Add | ArithOp::Sub | ArithOp::Mul | ArithOp::Shl) {
            self.wrap(scalar);
        }
        Ok(())
    }

    fn cast(&mut self, expr: ExprId, inner: ExprId) -> Result<'db, ()> {
        let from = self.scalar(inner)?;
        let to = self.scalar(expr)?;
        if to == Scalar::Bool && from != Scalar::Bool {
            return Err(self.unsupported(expr, "a cast to `Bool`"));
        }
        if (from == Scalar::Str) != (to == Scalar::Str) {
            return Err(self.unsupported(expr, "a cast from or to `String`"));
        }
        self.expr(inner)?;
        use ValTy::*;
        let ops: &[Op] = match (from.val_ty(), to.val_ty()) {
            (I32, I32) | (I64, I64) | (F32, F32) | (F64, F64) => &[],
            (I32, I64) if from.signed() => &[Op::I64ExtendI32S],
            (I32, I64) => &[Op::I64ExtendI32U],
            (I64, I32) => &[Op::I32WrapI64],
            (I32, F32) if from.signed() => &[Op::F32ConvertI32S],
            (I32, F32) => &[Op::F32ConvertI32U],
            (I32, F64) if from.signed() => &[Op::F64ConvertI32S],
            (I32, F64) => &[Op::F64ConvertI32U],
            (I64, F32) if from.signed() => &[Op::F32ConvertI64S],
            (I64, F32) => &[Op::F32ConvertI64U],
            (I64, F64) if from.signed() => &[Op::F64ConvertI64S],
            (I64, F64) => &[Op::F64ConvertI64U],
            (F32, I32) if to.signed() => &[Op::I32TruncSatF32S],
            (F32, I32) => &[Op::I32TruncSatF32U],
            (F64, I32) if to.signed() => &[Op::I32TruncSatF64S],
            (F64, I32) => &[Op::I32TruncSatF64U],
            (F32, I64) if to.signed() => &[Op::I64TruncSatF32S],
            (F32, I64) => &[Op::I64TruncSatF32U],
            (F64, I64) if to.signed() => &[Op::I64TruncSatF64S],
            (F64, I64) => &[Op::I64TruncSatF64U],
            (F64, F32) => &[Op::F32DemoteF64],
            (F32, F64) => &[Op::F64PromoteF32],
        };
        for &op in ops {
            self.op(op);
        }
        if matches!(to, Scalar::Int { .. }) && to != from {
            self.wrap(to);
        }
        Ok(())
    }

    // Calls

    fn call(&mut self, expr: ExprId, callee: ExprId, args: &[ExprId]) -> Result<'db, ()> {
        // `x.f(y)` is `f(x, y)`.
        if let Some(&f) = self.infer.method_calls.get(expr) {
            let Expr::Field { expr: receiver, .. } = &self.body[callee] else { unreachable!("a method call") };
            let mut all = vec![*receiver];
            all.extend_from_slice(args);
            return self.call_function(expr, f, Some(callee), &all);
        }
        let path = match &self.body[callee] {
            Expr::Path(path) => Some(path.clone()),
            // `m.f(x)` with `m` an imported file.
            Expr::Field { .. } => expr_as_path(self.body, callee),
            _ => None,
        };
        match path.and_then(|p| self.resolve(callee, &p)) {
            Some(ValueResolution::Value(ValueNs::Function(f))) => self.call_function(expr, f, Some(callee), args),
            _ => Err(self.unsupported(callee, "calling something other than a function")),
        }
    }

    fn call_function(
        &mut self,
        expr: ExprId,
        f: FunctionId<'db>,
        callee: Option<ExprId>,
        args: &[ExprId],
    ) -> Result<'db, ()> {
        let db = self.db;
        let builtin = builtin(db, f);
        let target = if let Some(builtin) = builtin {
            Callee::Builtin(builtin)
        } else if f.item(db).has_body {
            Callee::Instance(self.instance(expr, f, callee, args)?)
        } else {
            match f.container(db) {
                ItemContainerId::ExternBlock(_) => Callee::Import(f),
                ItemContainerId::Class(_) => return Err(self.unsupported(expr, "calling a class method")),
                _ => return Err(self.unsupported(expr, "calling a function without a body")),
            }
        };
        for &arg in args {
            self.expr(arg)?;
        }
        if !self.callees.contains(&target) {
            self.callees.push(target);
        }
        self.push(Inst::Call(target));
        if builtin == Some(Builtin::Panic) {
            // The host doesn't return from `panic`.
            self.push(Inst::Unreachable);
        }
        Ok(())
    }

    /// The instance a call calls: the type arguments of `f`'s scheme, found
    /// by matching its type with the type it has at the call.
    fn instance(
        &self,
        expr: ExprId,
        f: FunctionId<'db>,
        callee: Option<ExprId>,
        args: &[ExprId],
    ) -> Result<'db, Instance<'db>> {
        let db = self.db;
        let scheme = function_scheme(db, f).map_err(|()| self.error(expr, ErrorKind::UnknownType))?;
        let at_call = callee.map(|c| self.ty(c)).filter(|t| matches!(t, Ty::Fn(..))).unwrap_or_else(|| {
            Ty::Fn(args.iter().map(|&a| self.ty(a)).collect(), Box::new(self.ty(expr)))
        });
        let mut found = vec![None; scheme.params.len() + scheme.bound as usize];
        bind_type_args(&scheme.ty, &at_call, scheme, &mut found);
        let mut type_args = Vec::new();
        for ty in found {
            // A parameter that isn't in the function's type doesn't decide
            // anything about its code.
            let ty = ty.unwrap_or_else(Ty::unit);
            if ty.any(&mut |t| matches!(t, Ty::Var(_) | Ty::Param(_) | Ty::Bound(_) | Ty::Error)) {
                return Err(self.error(expr, ErrorKind::UnknownType));
            }
            type_args.push(ty);
        }
        let type_args: Box<[Ty<'db>]> = type_args.into();
        Ok(Instance::new(db, DefWithBodyId::new(db, DefWithBody::Function(f)), type_args))
    }

    // Patterns

    /// Test whether the value in `local`, of type `ty`, matches `pat`,
    /// leaving 1 or 0, and set the pattern's variables. `local` is `None`
    /// for a value of type `()`.
    fn pattern(&mut self, pat: PatId, local: Option<u32>, ty: &Ty<'db>, at: ExprId) -> Result<'db, ()> {
        let body = self.body;
        match &body[pat] {
            Pat::Wild => self.push(Inst::I32Const(1)),
            Pat::Bind { id, subpat } => {
                match subpat {
                    Some(subpat) => self.pattern(*subpat, local, ty, at)?,
                    None => self.push(Inst::I32Const(1)),
                }
                if let (Some(binding), Some(value)) = (self.binding_local(*id, at)?, local) {
                    self.push(Inst::LocalGet(value));
                    self.push(Inst::LocalSet(binding));
                }
            }
            Pat::Lit { .. } | Pat::Path(_) => {
                let (scalar, value) = self.pattern_operand(ty, local, at)?;
                self.push(Inst::LocalGet(value));
                self.pattern_const(pat, scalar, at)?;
                self.op(cmp_op(CmpOp::Eq, scalar));
            }
            Pat::Range { start, end, inclusive } => {
                let (scalar, value) = self.pattern_operand(ty, local, at)?;
                let mut tests = 0;
                if let Some(start) = start {
                    self.push(Inst::LocalGet(value));
                    self.pattern_const(*start, scalar, at)?;
                    self.op(cmp_op(CmpOp::GtEq, scalar));
                    tests += 1;
                }
                if let Some(end) = end {
                    self.push(Inst::LocalGet(value));
                    self.pattern_const(*end, scalar, at)?;
                    self.op(cmp_op(if *inclusive { CmpOp::LtEq } else { CmpOp::Lt }, scalar));
                    tests += 1;
                }
                match tests {
                    0 => self.push(Inst::I32Const(1)),
                    2 => self.op(Op::I32And),
                    _ => {}
                }
            }
            Pat::Or(alternatives) => {
                for (i, &alternative) in alternatives.iter().enumerate() {
                    self.pattern(alternative, local, ty, at)?;
                    if i > 0 {
                        self.op(Op::I32Or);
                    }
                }
            }
            _ => return Err(self.unsupported(at, "this pattern")),
        }
        Ok(())
    }

    fn pattern_operand(&self, ty: &Ty<'db>, local: Option<u32>, at: ExprId) -> Result<'db, (Scalar, u32)> {
        match (Scalar::of(ty), local) {
            (Some(scalar), Some(local)) if scalar != Scalar::Str => Ok((scalar, local)),
            _ => Err(self.unsupported(at, format!("matching a value of type `{}`", ty.display(self.db)))),
        }
    }

    /// The value of a literal or const pattern.
    fn pattern_const(&mut self, pat: PatId, scalar: Scalar, at: ExprId) -> Result<'db, ()> {
        match &self.body[pat] {
            Pat::Lit { literal, negative } => match literal {
                Literal::Int(n, _) => {
                    let n = *n as i128;
                    self.int_const(scalar, if *negative { -n } else { n });
                }
                Literal::Float(text, _) => {
                    let x: f64 = text.parse().map_err(|_| self.error(at, ErrorKind::UnknownType))?;
                    self.float_const(scalar, if *negative { -x } else { x });
                }
                Literal::Char(c) => self.push(Inst::I32Const(*c as i32)),
                Literal::Byte(b) => self.push(Inst::I32Const(*b as i32)),
                _ => return Err(self.unsupported(at, "a string pattern")),
            },
            Pat::Path(path) => {
                let resolution = Resolver::for_body(self.db, self.def).resolve_path_in_value_ns(self.db, path);
                if let Some(ValueResolution::Value(ValueNs::Builtin(v))) = resolution {
                    self.push(Inst::I32Const(v.as_bool() as i32));
                    return Ok(());
                }
                let Some(ValueResolution::Value(ValueNs::Const(id))) = resolution else {
                    return Err(self.unsupported(at, "a pattern of an enum"));
                };
                let value = Evaluator::new(self.db)
                    .eval_const(id)
                    .map_err(|e| self.unsupported(at, format!("a const whose value can't be computed ({e})")))?;
                match value {
                    Value::Int(n) => self.int_const(scalar, n),
                    Value::Bool(b) => self.int_const(scalar, b as i128),
                    Value::Char(c) => self.int_const(scalar, c as i128),
                    Value::Float(x) => self.float_const(scalar, x),
                    _ => return Err(self.unsupported(at, "a const of this type in a pattern")),
                }
            }
            _ => return Err(self.unsupported(at, "this pattern")),
        }
        Ok(())
    }

    fn match_expr(&mut self, expr: ExprId, scrutinee: ExprId, arms: &[MatchArm]) -> Result<'db, ()> {
        let ty = self.ty(scrutinee);
        let local = self.temp(scrutinee)?;
        let result = self.expr_val_ty(expr)?;
        self.open(Inst::Block(result.into()), Frame::Plain);
        for arm in arms {
            self.pattern(arm.pat, local, &ty, arm.expr)?;
            if let Some(guard) = arm.guard {
                self.open(Inst::If(BlockTy::Value(ValTy::I32)), Frame::Plain);
                self.expr(guard)?;
                self.push(Inst::Else);
                self.push(Inst::I32Const(0));
                self.close();
            }
            self.open(Inst::If(BlockTy::Empty), Frame::Plain);
            self.expr_as(arm.expr, result)?;
            // Out of the `if` and to the end of the `match`'s block.
            self.push(Inst::Br(1));
            self.close();
        }
        // No arm matched.
        self.push(Inst::Unreachable);
        self.close();
        Ok(())
    }
}

/// Match a function's scheme type with the type it has at a call, to find
/// the types of its parameters and bound variables.
fn bind_type_args<'db>(pattern: &Ty<'db>, ty: &Ty<'db>, scheme: &Scheme<'db>, found: &mut [Option<Ty<'db>>]) {
    let all = |found: &mut [Option<Ty<'db>>], xs: &[Ty<'db>], ys: &[Ty<'db>]| {
        for (x, y) in xs.iter().zip(ys) {
            bind_type_args(x, y, scheme, found);
        }
    };
    match (pattern, ty) {
        (Ty::Param(p), _) => {
            if let Some(i) = scheme.params.iter().position(|q| q == p) {
                found[i].get_or_insert_with(|| ty.clone());
            }
        }
        (Ty::Bound(i), _) => {
            if let Some(slot) = found.get_mut(scheme.params.len() + *i as usize) {
                slot.get_or_insert_with(|| ty.clone());
            }
        }
        (Ty::Adt(a, xs), Ty::Adt(b, ys)) if a == b => all(found, xs, ys),
        (Ty::Tuple(xs), Ty::Tuple(ys)) => all(found, xs, ys),
        (Ty::Array(x), Ty::Array(y)) => bind_type_args(x, y, scheme, found),
        (Ty::Fn(xs, x), Ty::Fn(ys, y)) => {
            all(found, xs, ys);
            bind_type_args(x, y, scheme, found);
        }
        _ => {}
    }
}

/// `a.b.c` as a path, if `expr` is one.
fn expr_as_path(body: &Body, expr: ExprId) -> Option<Path> {
    match &body[expr] {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { expr, name } => {
            let mut segments = expr_as_path(body, *expr)?.segments.into_vec();
            segments.push(PathSegment { name: name.clone(), generic_args: None });
            Some(Path { segments: segments.into() })
        }
        _ => None,
    }
}

fn describe(expr: &Expr) -> &'static str {
    match expr {
        Expr::Lambda { .. } => "a closure",
        Expr::Record { .. } => "a struct",
        Expr::Tuple { .. } => "a tuple",
        Expr::Array { .. } | Expr::ArrayRepeat { .. } => "an array",
        Expr::Field { .. } => "a field",
        Expr::Index { .. } => "indexing",
        Expr::Range { .. } => "a range",
        Expr::For { .. } => "a `for` loop",
        Expr::Try { .. } => "`?`",
        Expr::Await { .. } => "`await`",
        Expr::Yield { .. } => "`yield`",
        Expr::MacroCall { .. } => "a macro call",
        _ => "this expression",
    }
}

fn arith_op(op: ArithOp, scalar: Scalar) -> Option<Op> {
    use ArithOp::*;
    Some(match scalar {
        Scalar::Int { bits: 64, signed } => match op {
            Add => Op::I64Add,
            Sub => Op::I64Sub,
            Mul => Op::I64Mul,
            Div if signed => Op::I64DivS,
            Div => Op::I64DivU,
            Rem if signed => Op::I64RemS,
            Rem => Op::I64RemU,
            BitAnd => Op::I64And,
            BitOr => Op::I64Or,
            BitXor => Op::I64Xor,
            Shl => Op::I64Shl,
            Shr if signed => Op::I64ShrS,
            Shr => Op::I64ShrU,
        },
        Scalar::Int { signed, .. } => match op {
            Add => Op::I32Add,
            Sub => Op::I32Sub,
            Mul => Op::I32Mul,
            Div if signed => Op::I32DivS,
            Div => Op::I32DivU,
            Rem if signed => Op::I32RemS,
            Rem => Op::I32RemU,
            BitAnd => Op::I32And,
            BitOr => Op::I32Or,
            BitXor => Op::I32Xor,
            Shl => Op::I32Shl,
            Shr if signed => Op::I32ShrS,
            Shr => Op::I32ShrU,
        },
        Scalar::Bool => match op {
            BitAnd => Op::I32And,
            BitOr => Op::I32Or,
            BitXor => Op::I32Xor,
            _ => return None,
        },
        Scalar::F32 => match op {
            Add => Op::F32Add,
            Sub => Op::F32Sub,
            Mul => Op::F32Mul,
            Div => Op::F32Div,
            _ => return None,
        },
        Scalar::F64 => match op {
            Add => Op::F64Add,
            Sub => Op::F64Sub,
            Mul => Op::F64Mul,
            Div => Op::F64Div,
            _ => return None,
        },
        Scalar::Char | Scalar::Str => return None,
    })
}

fn cmp_op(op: CmpOp, scalar: Scalar) -> Op {
    use CmpOp::*;
    match scalar {
        Scalar::Int { bits: 64, signed } => match (op, signed) {
            (Eq, _) => Op::I64Eq,
            (NotEq, _) => Op::I64Ne,
            (Lt, true) => Op::I64LtS,
            (Lt, false) => Op::I64LtU,
            (LtEq, true) => Op::I64LeS,
            (LtEq, false) => Op::I64LeU,
            (Gt, true) => Op::I64GtS,
            (Gt, false) => Op::I64GtU,
            (GtEq, true) => Op::I64GeS,
            (GtEq, false) => Op::I64GeU,
        },
        Scalar::Int { .. } | Scalar::Bool | Scalar::Char | Scalar::Str => match (op, scalar.signed()) {
            (Eq, _) => Op::I32Eq,
            (NotEq, _) => Op::I32Ne,
            (Lt, true) => Op::I32LtS,
            (Lt, false) => Op::I32LtU,
            (LtEq, true) => Op::I32LeS,
            (LtEq, false) => Op::I32LeU,
            (Gt, true) => Op::I32GtS,
            (Gt, false) => Op::I32GtU,
            (GtEq, true) => Op::I32GeS,
            (GtEq, false) => Op::I32GeU,
        },
        Scalar::F32 => match op {
            Eq => Op::F32Eq,
            NotEq => Op::F32Ne,
            Lt => Op::F32Lt,
            LtEq => Op::F32Le,
            Gt => Op::F32Gt,
            GtEq => Op::F32Ge,
        },
        Scalar::F64 => match op {
            Eq => Op::F64Eq,
            NotEq => Op::F64Ne,
            Lt => Op::F64Lt,
            LtEq => Op::F64Le,
            Gt => Op::F64Gt,
            GtEq => Op::F64Ge,
        },
    }
}
