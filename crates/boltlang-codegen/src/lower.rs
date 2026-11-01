//! Lowering a body to [`Inst`]s. Bolt's control flow is structured like
//! Wasm's, so expressions are compiled one by one, without a control-flow
//! graph: `if` is `if`, a loop is a `block` around a `loop`, and `break`
//! and `continue` are branches to them.

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_eval::{Ctor, Evaluator, Value};
use boltlang_hir_def::{
    Name,
    body::{Body, DefWithBody, DefWithBodyId, body},
    builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType, BuiltinUint},
    hir::{
        ArithOp, BinaryOp, BindingId, BlockKind, CmpOp, Expr, ExprId, LabelId, Literal, LogicOp, MatchArm, Pat,
        PatId, Path, PathSegment, PrefixOp, Statement,
    },
    ids::{ConstId, EnumId, EnumVariantId, FunctionId, ItemContainerId},
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
    signatures::{FieldList, enum_signature},
};
use boltlang_hir_ty::{
    AdtId, InferenceResult, Scheme, Ty,
    builtin::{Builtin, PrimOp, builtin},
    function_scheme, infer_body,
};

use crate::{
    CodegenError, ErrorKind, Instance,
    ir::{BlockTy, Callee, CompiledFn, Inst, Op, ValTy},
    layout::{self, Layout, Repr},
};

type Result<'db, T> = std::result::Result<T, CodegenError<'db>>;

/// A type the code generator can compile, and how its values behave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scalar {
    Int { bits: u8, signed: bool },
    F32,
    F64,
    Bool,
    /// A pointer to a string in linear memory.
    Str,
    /// An enum whose variants have no fields, like `enum Color { Red, Green }`:
    /// the index of the variant.
    Enum,
}

impl Scalar {
    pub(crate) fn of(db: &dyn Db, ty: &Ty<'_>) -> Option<Scalar> {
        let ty = match ty {
            Ty::Builtin(ty) => ty,
            Ty::Adt(AdtId::Enum(id), ..) if is_fieldless(db, *id) => return Some(Scalar::Enum),
            _ => return None,
        };
        Some(match ty {
            BuiltinType::Bool => Scalar::Bool,
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

    pub(crate) fn val_ty(self) -> ValTy {
        match self {
            Scalar::Int { bits: 64, .. } => ValTy::I64,
            Scalar::Int { .. } | Scalar::Bool | Scalar::Str | Scalar::Enum => ValTy::I32,
            Scalar::F32 => ValTy::F32,
            Scalar::F64 => ValTy::F64,
        }
    }

    /// Whether comparisons and conversions treat it as signed.
    fn signed(self) -> bool {
        matches!(self, Scalar::Int { signed: true, .. })
    }

    /// [`Scalar::of`], or that of the field of an `imm struct` with one
    /// field, like `Char`, which is the value of its field: the `u32` of a
    /// `Char`.
    pub(crate) fn through(db: &dyn Db, ty: &Ty<'_>) -> Option<Scalar> {
        let mut ty = ty.clone();
        // An `imm struct A(A)` has no values, but mustn't hang.
        for _ in 0..64 {
            match layout::repr(db, &ty)? {
                Repr::Scalar(scalar) => return Some(scalar),
                Repr::Newtype(field) => ty = field,
                Repr::Unit | Repr::Boxed => return None,
            }
        }
        None
    }
}

/// The value of the field of an `imm struct` with one field, as compiled
/// code has it, like the code point of a `Char`, and other values as they
/// are.
fn through_newtypes(mut value: Value<'_>) -> Value<'_> {
    loop {
        let inner = match &value {
            Value::Adt { ctor: Ctor::Struct(_), fields } if fields.borrow().len() == 1 => fields.borrow()[0].1.clone(),
            _ => return value,
        };
        value = inner;
    }
}

/// Whether no variant of the enum has fields, so that a value is only the
/// index of its variant. An enum without variants is one too.
fn is_fieldless<'db>(db: &'db dyn Db, id: EnumId<'db>) -> bool {
    enum_signature(db, id).variants.iter().all(|v| matches!(v.fields, FieldList::Unit))
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
                .map_err(|_| CodegenError { kind: ErrorKind::UnknownType, location: Some((def, body.body_expr)) })?,
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
            Ty::Var(_) | Ty::Error | Ty::Bound(_) | Ty::Param(_) => Err(self.error(at, ErrorKind::UnknownType)),
            _ => layout::val_ty(self.db, ty)
                .ok_or_else(|| self.unsupported(at, format!("a value of type `{}`", ty.display(self.db)))),
        }
    }

    fn repr(&self, ty: &Ty<'db>, at: ExprId) -> Result<'db, Repr<'db>> {
        layout::repr(self.db, ty).ok_or_else(|| self.unsupported(at, format!("a value of type `{}`", ty.display(self.db))))
    }

    /// The fields of a struct or tuple of type `ty` in memory.
    fn layout(&self, ty: &Ty<'db>, at: ExprId) -> Result<'db, Layout<'db>> {
        layout::layout(self.db, ty).ok_or_else(|| self.unsupported(at, format!("a value of type `{}`", ty.display(self.db))))
    }

    fn expr_val_ty(&self, expr: ExprId) -> Result<'db, Option<ValTy>> {
        self.val_ty(&self.ty(expr), expr)
    }

    fn scalar(&self, expr: ExprId) -> Result<'db, Scalar> {
        let ty = self.ty(expr);
        Scalar::of(self.db, &ty).ok_or_else(|| self.unsupported(expr, format!("an operation on `{}`", ty.display(self.db))))
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
        let Ty::Fn(params, ret, _) = self.subst(&scheme.ty) else { unreachable!("a function has a function type") };
        // All the parameters first, since locals are numbered after them.
        let mut indices = Vec::new();
        for ty in params.iter() {
            indices.push(self.val_ty(ty, body.body_expr)?.map(|ty| {
                self.params.push(ty);
                self.params.len() as u32 - 1
            }));
        }
        for ((&pat, ty), index) in body.params.iter().zip(params.iter()).zip(indices) {
            match &body[pat] {
                Pat::Bind { id, subpat: None } => {
                    self.bindings.insert(*id, index);
                }
                Pat::Wild => {}
                // A pattern like `(a, b)`, which the checker made sure
                // always matches.
                _ => {
                    self.pattern(pat, index, ty, body.body_expr)?;
                    self.push(Inst::Drop);
                }
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
            Expr::Call { callee, args, .. } => self.call(expr, *callee, args),
            Expr::Cast { expr: inner, .. } => self.cast(expr, *inner),
            Expr::Prefix { op, expr: inner } => self.prefix(expr, *op, *inner),
            Expr::Binary { op, lhs, rhs } => self.binary(expr, op, *lhs, *rhs),
            Expr::Tuple { exprs } if exprs.is_empty() => Ok(()),
            Expr::Tuple { exprs } => {
                let fields: Vec<_> = exprs.iter().enumerate().map(|(i, &e)| (Name::new(i.to_string()), e)).collect();
                self.construct(expr, &fields, None)
            }
            Expr::Record { path, fields, spread, .. } => {
                let resolution = path.as_ref().and_then(|p| self.resolve(expr, p));
                match resolution {
                    Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), unresolved: None })) => {
                        let fields: Vec<_> = fields.iter().map(|f| (f.name.clone(), f.expr)).collect();
                        self.construct(expr, &fields, *spread)
                    }
                    _ => Err(self.unsupported(expr, "an enum with fields")),
                }
            }
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
            Expr::Field { expr: base, name } => {
                if layout::fields(self.db, &self.ty(*base)).is_some() {
                    return self.field(*base, name);
                }
                match expr_as_path(body, expr).and_then(|path| self.resolve(expr, &path)) {
                    Some(ValueResolution::Value(ValueNs::Variant(v))) => self.variant(expr, v),
                    _ => Err(self.unsupported(expr, "a field")),
                }
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

    /// What `path` at `expr` refers to. A name with several functions is
    /// the one the types chose there (#275).
    fn resolve(&self, expr: ExprId, path: &Path) -> Option<ValueResolution<'db>> {
        match Resolver::for_expr(self.db, self.def, expr).resolve_path_in_value_ns(self.db, path)? {
            ValueResolution::Value(ValueNs::Overloaded(_)) => {
                self.infer.overload(expr).map(|f| ValueResolution::Value(ValueNs::Function(f)))
            }
            resolution => Some(resolution),
        }
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
            Some(ValueResolution::Value(ValueNs::Variant(v))) => self.variant(expr, v),
            // A struct without fields, like `struct Unit;`.
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), unresolved: None }))
                if layout::fields(self.db, &self.ty(expr)).is_some_and(|f| f.is_empty()) =>
            {
                self.construct(expr, &[], None)
            }
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), unresolved: None })) => {
                Err(self.unsupported(expr, "a constructor as a value"))
            }
            _ => Err(self.unsupported(expr, "this path")),
        }
    }

    // Structs and tuples

    /// A new struct or tuple of the type of `expr`, with the values of
    /// `given` for its fields, in that order, and copies of those of
    /// `spread` for the others.
    fn construct(&mut self, expr: ExprId, given: &[(Name, ExprId)], spread: Option<ExprId>) -> Result<'db, ()> {
        let ty = self.ty(expr);
        match self.repr(&ty, expr)? {
            // Its one field is the value.
            Repr::Newtype(_) => match (given, spread) {
                ([(_, field)], _) => self.expr(*field),
                ([], Some(base)) => self.expr(base),
                _ => Err(self.unsupported(expr, "this struct")),
            },
            Repr::Boxed => {
                let layout = self.layout(&ty, expr)?;
                // The base first, like the evaluator.
                let base = match spread {
                    Some(base) => self.temp(base)?,
                    None => None,
                };
                let address = self.alloc(layout.size);
                // The copies first, so that the given fields can't change
                // what's copied.
                if let Some(base) = base {
                    for slot in &layout.slots {
                        let Some(val_ty) = slot.val_ty else { continue };
                        if given.iter().any(|(name, _)| *name == slot.name) {
                            continue;
                        }
                        self.push(Inst::LocalGet(address));
                        self.push(Inst::LocalGet(base));
                        self.push(Inst::Load(val_ty, slot.offset));
                        self.push(Inst::Store(val_ty, slot.offset));
                    }
                }
                for (name, value) in given {
                    let Some(slot) = layout.slot(name) else { return Err(self.unsupported(*value, "this field")) };
                    match slot.val_ty {
                        Some(val_ty) => {
                            self.push(Inst::LocalGet(address));
                            self.expr(*value)?;
                            self.push(Inst::Store(val_ty, slot.offset));
                        }
                        None => self.expr_as(*value, None)?,
                    }
                }
                self.push(Inst::LocalGet(address));
                Ok(())
            }
            Repr::Unit | Repr::Scalar(_) => Err(self.unsupported(expr, "this struct")),
        }
    }

    /// Allocate `size` bytes, into a new local that it gives.
    fn alloc(&mut self, size: u32) -> u32 {
        self.push(Inst::I32Const(size as i32));
        self.call_callee(Callee::Alloc);
        let local = self.new_local(ValTy::I32);
        self.push(Inst::LocalSet(local));
        local
    }

    fn call_callee(&mut self, callee: Callee<'db>) {
        if !self.callees.contains(&callee) {
            self.callees.push(callee);
        }
        self.push(Inst::Call(callee));
    }

    /// The field `name` of the struct or tuple that `base` gives.
    fn field(&mut self, base: ExprId, name: &Name) -> Result<'db, ()> {
        let ty = self.ty(base);
        match self.repr(&ty, base)? {
            Repr::Newtype(_) => self.expr(base),
            _ => {
                let layout = self.layout(&ty, base)?;
                let Some(slot) = layout.slot(name) else { return Err(self.unsupported(base, "this field")) };
                self.expr(base)?;
                match slot.val_ty {
                    Some(val_ty) => self.push(Inst::Load(val_ty, slot.offset)),
                    None => self.push(Inst::Drop),
                }
                Ok(())
            }
        }
    }

    /// Store the value on the stack, if `place` has one, in `place`: a
    /// variable, or a field of one, or of a field, and so on. A struct is
    /// changed in place, since it's shared; a tuple is a value, so a copy
    /// with the new field replaces it.
    ///
    /// The places in `place` are evaluated again, which is fine, since
    /// they're only variables and fields: see [`Self::is_place`].
    fn assign(&mut self, place: ExprId) -> Result<'db, ()> {
        let value_ty = self.expr_val_ty(place)?;
        match &self.body[place] {
            Expr::Path(path) => {
                let local = match self.resolve(place, path) {
                    Some(ValueResolution::Value(ValueNs::Local(binding))) => self.binding_local(binding, place)?,
                    _ => return Err(self.unsupported(place, "assigning to this")),
                };
                match local {
                    Some(local) => self.push(Inst::LocalSet(local)),
                    None if value_ty.is_some() => self.push(Inst::Drop),
                    None => {}
                }
                Ok(())
            }
            &Expr::Field { expr: base, ref name } => {
                let name = name.clone();
                let base_ty = self.ty(base);
                if self.repr(&base_ty, base)? != Repr::Boxed {
                    return Err(self.unsupported(place, "assigning to this field"));
                }
                let layout = self.layout(&base_ty, base)?;
                let Some(slot) = layout.slot(&name).cloned() else {
                    return Err(self.unsupported(place, "this field"));
                };
                let value = value_ty.map(|ty| self.new_local(ty));
                if let Some(value) = value {
                    self.push(Inst::LocalSet(value));
                }
                if matches!(base_ty, Ty::Tuple(_)) {
                    let old = self.temp(base)?.expect("a tuple is an address");
                    let new = self.alloc(layout.size);
                    for other in &layout.slots {
                        let Some(val_ty) = other.val_ty else { continue };
                        self.push(Inst::LocalGet(new));
                        match value {
                            Some(value) if other.name == slot.name => self.push(Inst::LocalGet(value)),
                            _ => {
                                self.push(Inst::LocalGet(old));
                                self.push(Inst::Load(val_ty, other.offset));
                            }
                        }
                        self.push(Inst::Store(val_ty, other.offset));
                    }
                    self.push(Inst::LocalGet(new));
                    return self.assign(base);
                }
                match (value, slot.val_ty) {
                    (Some(value), Some(val_ty)) => {
                        self.expr(base)?;
                        self.push(Inst::LocalGet(value));
                        self.push(Inst::Store(val_ty, slot.offset));
                    }
                    _ => self.expr_as(base, None)?,
                }
                Ok(())
            }
            _ => Err(self.unsupported(place, "assigning to an element")),
        }
    }

    /// Whether `expr` is a variable, or a field of a place, so that
    /// evaluating it has no effects.
    fn is_place(&self, expr: ExprId) -> bool {
        match &self.body[expr] {
            Expr::Path(_) => true,
            Expr::Field { expr: base, .. } => self.is_place(*base),
            _ => false,
        }
    }

    /// A variant of an enum without fields: the index of the variant.
    fn variant(&mut self, expr: ExprId, v: EnumVariantId<'db>) -> Result<'db, ()> {
        if self.scalar(expr)? != Scalar::Enum {
            return Err(self.unsupported(expr, "an enum with fields"));
        }
        self.push(Inst::I32Const(v.index as i32));
        Ok(())
    }

    /// A const, whose value the evaluator computes at compile time.
    fn const_value(&mut self, expr: ExprId, id: ConstId<'db>) -> Result<'db, ()> {
        let value = Evaluator::new(self.db)
            .eval_const(id)
            .map_err(|e| self.unsupported(expr, format!("a const whose value can't be computed ({e})")))?;
        let Some(scalar) = Scalar::through(self.db, &self.ty(expr)) else {
            return match value {
                Value::Tuple(items) if items.is_empty() => Ok(()),
                _ => Err(self.unsupported(expr, "a const of this type")),
            };
        };
        match through_newtypes(value) {
            Value::Int(n) => self.int_const(scalar, n),
            Value::Bool(b) => self.int_const(scalar, b as i128),
            Value::Float(x) => self.float_const(scalar, x),
            Value::Adt { ctor: Ctor::Variant(v), .. } if scalar == Scalar::Enum => {
                self.push(Inst::I32Const(v.index as i32))
            }
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
                // Structs, enums and type parameters are compared by the
                // function with the operator's name, as the type checker
                // has it, like `a <$> b`.
                if matches!(self.ty(lhs), Ty::Adt(..) | Ty::Param(_)) {
                    return match self.resolve(expr, &Path::from_name(Name::new(op.as_str()))) {
                        Some(ValueResolution::Value(ValueNs::Function(f))) => {
                            self.call_function(expr, f, None, &[lhs, rhs])
                        }
                        _ => Err(self.unsupported(expr, "this operator")),
                    };
                }
                let scalar = self.scalar(lhs)?;
                if scalar == Scalar::Str {
                    return Err(self.unsupported(expr, "comparing strings"));
                }
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.op(cmp_op(*op, scalar));
                Ok(())
            }
            // The value first, then the place, like the evaluator.
            BinaryOp::Assign(op) => {
                if !self.is_place(lhs) {
                    return Err(self.unsupported(lhs, "assigning to this"));
                }
                match op {
                    None => self.expr(rhs)?,
                    Some(op) => {
                        let scalar = self.scalar(lhs)?;
                        let value = self.temp(rhs)?;
                        self.expr(lhs)?;
                        if let Some(value) = value {
                            self.push(Inst::LocalGet(value));
                        }
                        self.shift_amount(*op, scalar, rhs)?;
                        self.arith(expr, *op, scalar)?;
                    }
                }
                self.assign(lhs)
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
        if from == Scalar::Enum || to == Scalar::Enum {
            return Err(self.unsupported(expr, "a cast from or to an enum"));
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
            // `Wrap(x)`, a tuple struct.
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), unresolved: None })) => {
                let fields: Vec<_> = args.iter().enumerate().map(|(i, &a)| (Name::new(i.to_string()), a)).collect();
                self.construct(expr, &fields, None)
            }
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
        // An operation of a primitive type is what its operator compiles to.
        if let Some(Builtin::Prim(op, _)) = builtin {
            return self.prim(expr, op, args);
        }
        let target = if let Some(builtin) = builtin {
            if !Builtin::HOST.contains(&builtin) {
                return Err(self.unsupported(expr, format!("the builtin `{}`", builtin.name())));
            }
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
        self.call_callee(target);
        if builtin == Some(Builtin::Panic) {
            // The host doesn't return from `panic`.
            self.push(Inst::Unreachable);
        }
        Ok(())
    }

    /// A call of an operation of a primitive type, like `i32_add(a, b)`, as
    /// the operator: inline.
    fn prim(&mut self, expr: ExprId, op: PrimOp, args: &[ExprId]) -> Result<'db, ()> {
        let (lhs, rhs) = (args[0], args.get(1).copied());
        let scalar = self.scalar(lhs)?;
        if scalar == Scalar::Str {
            return Err(self.unsupported(expr, "an operation on strings"));
        }
        match (op.arith(), op.cmp(), rhs) {
            (Some(arith), _, Some(rhs)) => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.shift_amount(arith, scalar, rhs)?;
                self.arith(expr, arith, scalar)
            }
            (_, Some(cmp), Some(rhs)) => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.op(cmp_op(cmp, scalar));
                Ok(())
            }
            _ => match op {
                PrimOp::Neg => self.prefix(expr, PrefixOp::Neg, lhs),
                PrimOp::Not => self.prefix(expr, PrefixOp::Not, lhs),
                _ => Err(self.unsupported(expr, format!("the builtin `{}`", op.name()))),
            },
        }
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
        let scheme = function_scheme(db, f).map_err(|_| self.error(expr, ErrorKind::UnknownType))?;
        let at_call = callee.map(|c| self.ty(c)).filter(|t| matches!(t, Ty::Fn(..))).unwrap_or_else(|| {
            Ty::func(args.iter().map(|&a| self.ty(a)).collect::<Box<[_]>>(), self.ty(expr))
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
            Pat::Tuple { args, rest } => {
                let fields = positional(args, *rest, layout::fields(self.db, ty).map_or(0, |f| f.len()));
                self.field_patterns(&fields, local, ty, at)?;
            }
            Pat::TupleStruct { path, args, rest } => {
                self.struct_pattern_path(path, at)?;
                let fields = positional(args, *rest, layout::fields(self.db, ty).map_or(0, |f| f.len()));
                self.field_patterns(&fields, local, ty, at)?;
            }
            Pat::Record { path, fields, .. } => {
                self.struct_pattern_path(path, at)?;
                let fields: Vec<_> = fields.iter().map(|f| (f.name.clone(), f.pat)).collect();
                self.field_patterns(&fields, local, ty, at)?;
            }
            _ => return Err(self.unsupported(at, "this pattern")),
        }
        Ok(())
    }

    /// That the path of a pattern like `Wrap(x)` names a struct, which a
    /// value of its type always is.
    fn struct_pattern_path(&self, path: &Path, at: ExprId) -> Result<'db, ()> {
        match Resolver::for_body(self.db, self.def).resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), unresolved: None })) => Ok(()),
            _ => Err(self.unsupported(at, "a pattern of an enum with fields")),
        }
    }

    /// Test the fields of the struct or tuple in `local`, of type `ty`,
    /// against their patterns, leaving 1 if all match.
    fn field_patterns(&mut self, fields: &[(Name, PatId)], local: Option<u32>, ty: &Ty<'db>, at: ExprId) -> Result<'db, ()> {
        if let Repr::Newtype(field_ty) = self.repr(ty, at)? {
            return match fields {
                [(_, pat)] => self.pattern(*pat, local, &field_ty, at),
                _ => {
                    self.push(Inst::I32Const(1));
                    Ok(())
                }
            };
        }
        let layout = self.layout(ty, at)?;
        if fields.is_empty() {
            self.push(Inst::I32Const(1));
        }
        for (i, (name, pat)) in fields.iter().enumerate() {
            let Some(slot) = layout.slot(name) else { return Err(self.unsupported(at, "this field")) };
            let field = match (slot.val_ty, local) {
                (Some(val_ty), Some(local)) => {
                    let field = self.new_local(val_ty);
                    self.push(Inst::LocalGet(local));
                    self.push(Inst::Load(val_ty, slot.offset));
                    self.push(Inst::LocalSet(field));
                    Some(field)
                }
                _ => None,
            };
            self.pattern(*pat, field, &slot.ty.clone(), at)?;
            if i > 0 {
                self.op(Op::I32And);
            }
        }
        Ok(())
    }

    fn pattern_operand(&self, ty: &Ty<'db>, local: Option<u32>, at: ExprId) -> Result<'db, (Scalar, u32)> {
        match (Scalar::through(self.db, ty), local) {
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
                match resolution {
                    Some(ValueResolution::Value(ValueNs::Builtin(v))) => {
                        self.push(Inst::I32Const(v.as_bool() as i32));
                        return Ok(());
                    }
                    Some(ValueResolution::Value(ValueNs::Variant(v))) if scalar == Scalar::Enum => {
                        self.push(Inst::I32Const(v.index as i32));
                        return Ok(());
                    }
                    _ => {}
                }
                let Some(ValueResolution::Value(ValueNs::Const(id))) = resolution else {
                    return Err(self.unsupported(at, "a pattern of an enum with fields"));
                };
                let value = Evaluator::new(self.db)
                    .eval_const(id)
                    .map_err(|e| self.unsupported(at, format!("a const whose value can't be computed ({e})")))?;
                match through_newtypes(value) {
                    Value::Int(n) => self.int_const(scalar, n),
                    Value::Bool(b) => self.int_const(scalar, b as i128),
                            Value::Float(x) => self.float_const(scalar, x),
                    Value::Adt { ctor: Ctor::Variant(v), .. } if scalar == Scalar::Enum => {
                        self.push(Inst::I32Const(v.index as i32))
                    }
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
        (Ty::Adt(a, xs, ms, m), Ty::Adt(b, ys, ns, n)) if a == b => {
            all(found, xs, ys);
            all(found, ms, ns);
            bind_type_args(m, n, scheme, found);
        }
        (Ty::Tuple(xs), Ty::Tuple(ys)) => all(found, xs, ys),
        (Ty::Array(m, x), Ty::Array(n, y)) => {
            bind_type_args(m, n, scheme, found);
            bind_type_args(x, y, scheme, found);
        }
        (Ty::Fn(xs, x, m), Ty::Fn(ys, y, n)) => {
            all(found, xs, ys);
            bind_type_args(x, y, scheme, found);
            bind_type_args(m, n, scheme, found);
        }
        _ => {}
    }
}

/// The fields that the patterns of `(a, .., b)` are for, of `count`
/// fields: those before the `..` from the start, and those after it from
/// the end.
fn positional(args: &[PatId], rest: Option<u32>, count: usize) -> Vec<(Name, PatId)> {
    let rest = rest.map_or(args.len(), |r| r as usize);
    args.iter()
        .enumerate()
        .map(|(i, &pat)| {
            let index = if i < rest { i } else { count + i - args.len() };
            (Name::new(index.to_string()), pat)
        })
        .collect()
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
        Scalar::Str | Scalar::Enum => return None,
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
        Scalar::Int { .. } | Scalar::Bool | Scalar::Str | Scalar::Enum => match (op, scalar.signed()) {
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
