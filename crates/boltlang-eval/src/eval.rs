use std::{cell::RefCell, collections::HashMap, rc::Rc};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::{Body, DefWithBody, DefWithBodyId, body},
    builtin_type::BuiltinType,
    hir::{
        ArithOp, BinaryOp, BindingId, BlockKind, CmpOp, Expr, ExprId, LabelId, Literal, LogicOp, Mutability, Pat, PatId,
        Path, PathSegment, PrefixOp, Statement, TypeRef,
    },
    ids::{ConstId, FunctionId, ItemContainerId, ModuleDefId},
    nameres::{Namespace, file_scope, package_of},
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
    signatures::{FieldList, enum_signature, struct_signature},
};

use boltlang_hir_ty::{
    builtin::{Builtin, builtin},
    infer_body,
    lang::LangClass,
};

use crate::{
    dispatch::{impl_const, impl_for_args, impl_method, method_class},
    error::{ErrorKind, EvalError},
    host::{Host, NoEffects},
    value::{Closure, Ctor, Fields, Locals, Shared, Slot, Value},
};

/// Bounds on an evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How deeply calls can nest.
    pub max_depth: usize,
    /// How many expressions can be evaluated in total, or `None` for no
    /// limit. Code from an editor should always have one.
    pub fuel: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_depth: 10_000, fuel: None }
    }
}

/// When less than this much native stack is left, a new segment is
/// allocated.
const RED_ZONE: usize = 128 * 1024;
const STACK_GROWTH: usize = 2 * 1024 * 1024;

/// How evaluation leaves an expression early.
enum Unwind<'db> {
    Break(Option<Name>, Value<'db>),
    Continue(Option<Name>),
    Return(Value<'db>),
    Error(EvalError<'db>),
}

impl<'db> From<ErrorKind> for Unwind<'db> {
    fn from(kind: ErrorKind) -> Self {
        Unwind::Error(kind.into())
    }
}

type Res<'db, T> = Result<T, Unwind<'db>>;

fn fail<'db, T>(kind: ErrorKind) -> Res<'db, T> {
    Err(kind.into())
}

fn mismatch<'db, T>(msg: impl Into<String>) -> Res<'db, T> {
    fail(ErrorKind::TypeMismatch(msg.into()))
}

/// A call in progress.
struct Frame<'db> {
    def: DefWithBodyId<'db>,
    body: &'db Body,
    locals: Locals<'db>,
}

/// What an assignment writes to: a variable, a field of a struct, or an
/// element.
enum Place<'db> {
    Variable(Slot<'db>),
    Field(Shared<Fields<'db>>, Name),
    /// An element of an array, a `Vec` or another type, which is read and
    /// written through its impls of `Index` and `IndexMut`. `at` is the
    /// expression `c[i]`.
    Indexed { container: Value<'db>, index: Value<'db>, at: ExprId },
}

/// What a `for` loop goes over: the items of a string or a range, or an
/// iterator of a class.
enum Items<'db> {
    Builtin(Box<dyn Iterator<Item = Res<'db, Value<'db>>> + 'db>),
    Class(Value<'db>),
}

/// The part of `value` that the fields of tuples in `fields` lead to.
fn tuple_field<'a, 'db>(value: Option<&'a mut Value<'db>>, fields: &[Name]) -> Result<&'a mut Value<'db>, ErrorKind> {
    let mut current = value.ok_or_else(|| ErrorKind::Uninitialized(Name::new("variable")))?;
    for name in fields {
        current = match current {
            Value::Tuple(items) => name.as_str().parse::<usize>().ok().and_then(|i| items.get_mut(i)),
            _ => None,
        }
        .ok_or_else(|| ErrorKind::NoSuchField(name.clone()))?;
    }
    Ok(current)
}

impl<'db> Place<'db> {
    /// Run `f` on the value at the place, which is `None` for a variable
    /// that isn't assigned yet. Only this place is borrowed while `f` runs.
    fn update<T>(&self, f: impl FnOnce(Option<&mut Value<'db>>) -> Res<'db, T>) -> Res<'db, T> {
        match self {
            Place::Variable(slot) => f(slot.borrow_mut().as_mut()),
            Place::Field(fields, name) => f(fields.borrow_mut().iter_mut().find(|(n, _)| n == name).map(|(_, v)| v)),
            // Assignments handle it before.
            Place::Indexed { .. } => fail(ErrorKind::NotAssignable),
        }
    }
}

/// Evaluates Bolt code. It caches the values of consts, so reuse it for
/// several evaluations of the same program.
pub struct Evaluator<'db> {
    db: &'db dyn Db,
    limits: Limits,
    fuel: Option<u64>,
    depth: usize,
    /// `None` while the const is being evaluated, to find cycles.
    consts: HashMap<ConstId<'db>, Option<Value<'db>>>,
    host: Box<dyn Host>,
    /// How many consts are being evaluated, which can't have effects.
    in_consts: usize,
    resolutions: HashMap<(DefWithBodyId<'db>, ExprId), Option<ValueResolution<'db>>>,
    /// Whether to check what the type checker should make sure of: that an
    /// `imm` array or struct is never changed, and only holds `imm` ones.
    mutability_checks: bool,
    /// The files being run, whose impls a class method can use when the
    /// impl is chosen at run time.
    files: Vec<File>,
}

impl<'db> Evaluator<'db> {
    pub fn new(db: &'db dyn Db) -> Self {
        Self::with_limits(db, Limits::default())
    }

    pub fn with_limits(db: &'db dyn Db, limits: Limits) -> Self {
        Evaluator {
            db,
            limits,
            fuel: limits.fuel,
            depth: 0,
            consts: HashMap::new(),
            resolutions: HashMap::new(),
            files: Vec::new(),
            host: Box::new(NoEffects),
            in_consts: 0,
            mutability_checks: false,
        }
    }

    /// Check what the type checker should make sure of, and stop with
    /// [`ErrorKind::MutabilityViolation`] if it didn't: that an `imm` array
    /// or struct is never changed, and only holds `imm` ones. For tests: a
    /// program the type checker accepts never fails these checks. Each array
    /// and struct knows how it was made either way; this only adds a check
    /// to each change and to each `imm` one that's made.
    pub fn with_mutability_checks(mut self) -> Self {
        self.mutability_checks = true;
        self
    }

    /// Check that `value`, just made, holds only `imm` things if it's an
    /// `imm` array or struct.
    fn made(&self, value: Value<'db>) -> Res<'db, Value<'db>> {
        if !self.mutability_checks {
            return Ok(value);
        }
        let deep = match &value {
            Value::Array(items) if items.mutability == Mutability::Imm => {
                items.borrow().iter().all(Value::holds_only_imm)
            }
            Value::Adt { ctor: Ctor::Struct(_), fields } if fields.mutability == Mutability::Imm => {
                fields.borrow().iter().all(|(_, v)| v.holds_only_imm())
            }
            _ => true,
        };
        if !deep {
            return fail(ErrorKind::MutabilityViolation("an `imm` value was made holding a `mut` one"));
        }
        Ok(value)
    }

    /// Check that an array or struct made with `mutability` can be changed.
    fn changing(&self, mutability: Mutability) -> Res<'db, ()> {
        if self.mutability_checks && mutability == Mutability::Imm {
            return fail(ErrorKind::MutabilityViolation("an `imm` value was changed"));
        }
        Ok(())
    }

    /// Use `host` for effects, like printing, instead of refusing them.
    pub fn with_host(mut self, host: impl Host + 'static) -> Self {
        self.host = Box::new(host);
        self
    }

    /// Give back all the fuel of the [`Limits`].
    pub fn refuel(&mut self) {
        self.fuel = self.limits.fuel;
    }

    /// Run the statements at the top level of a file. The value is that of
    /// the last statement if it's an expression, like `f(x);`, or `()`.
    pub fn eval_file(&mut self, file: File) -> Result<Value<'db>, EvalError<'db>> {
        if !self.files.contains(&file) {
            self.files.push(file);
        }
        let def = DefWithBodyId::new(self.db, DefWithBody::File(file));
        let body = body(self.db, def);
        let mut frame = Frame { def, body, locals: Locals::new() };
        let Expr::Block { statements, .. } = &body[body.body_expr] else {
            unreachable!("the body of a file is a block")
        };
        let result = (|| {
            let mut last = Value::unit();
            for statement in statements.iter() {
                last = self.eval_statement(&mut frame, statement)?;
            }
            Ok(last)
        })();
        finish(result)
    }

    /// The function called `name` at the top level of `file`.
    pub fn function(&self, file: File, name: &str) -> Option<FunctionId<'db>> {
        match file_scope(self.db, file).scope.get(Namespace::Values, &Name::new(name))?.def {
            ModuleDefId::Function(id) => Some(id),
            _ => None,
        }
    }

    pub fn call_function(
        &mut self,
        function: FunctionId<'db>,
        args: Vec<Value<'db>>,
    ) -> Result<Value<'db>, EvalError<'db>> {
        finish(self.call_function_inner(function, args))
    }

    /// Call any callable value: a function, closure or constructor.
    pub fn call(&mut self, callee: Value<'db>, args: Vec<Value<'db>>) -> Result<Value<'db>, EvalError<'db>> {
        finish(self.apply(callee, args))
    }

    pub fn eval_const(&mut self, id: ConstId<'db>) -> Result<Value<'db>, EvalError<'db>> {
        finish(self.eval_const_inner(id))
    }

    fn tick(&mut self) -> Res<'db, ()> {
        if let Some(fuel) = &mut self.fuel {
            if *fuel == 0 {
                return fail(ErrorKind::OutOfFuel);
            }
            *fuel -= 1;
        }
        Ok(())
    }

    fn enter(&mut self) -> Res<'db, ()> {
        if self.depth >= self.limits.max_depth {
            return fail(ErrorKind::StackOverflow);
        }
        self.depth += 1;
        Ok(())
    }

    fn eval_const_inner(&mut self, id: ConstId<'db>) -> Res<'db, Value<'db>> {
        let db = self.db;
        let name = || id.item(db).name.as_ref().map_or("?".to_string(), Name::to_string);
        match self.consts.get(&id) {
            Some(Some(value)) => return Ok(value.clone()),
            Some(None) => return fail(ErrorKind::ConstCycle(name())),
            None => {}
        }
        if !id.item(db).has_value {
            return fail(ErrorKind::NoBody(name()));
        }
        let def = DefWithBodyId::new(db, DefWithBody::Const(id));
        let body = body(db, def);
        self.consts.insert(id, None);
        self.enter()?;
        self.in_consts += 1;
        let mut frame = Frame { def, body, locals: Locals::new() };
        let result = self.eval_body(&mut frame, body.body_expr);
        self.in_consts -= 1;
        self.depth -= 1;
        match result {
            Ok(value) => {
                self.consts.insert(id, Some(value.clone()));
                Ok(value)
            }
            Err(e) => {
                self.consts.remove(&id);
                Err(e)
            }
        }
    }

    /// Evaluate the body of a function, closure or const, where `return`
    /// ends.
    fn eval_body(&mut self, frame: &mut Frame<'db>, expr: ExprId) -> Res<'db, Value<'db>> {
        match self.eval(frame, expr) {
            Ok(value) | Err(Unwind::Return(value)) => Ok(value),
            Err(Unwind::Break(..) | Unwind::Continue(_)) => fail(ErrorKind::BreakOutsideLoop),
            Err(e) => Err(e),
        }
    }

    fn call_function_inner(&mut self, function: FunctionId<'db>, args: Vec<Value<'db>>) -> Res<'db, Value<'db>> {
        let db = self.db;
        if let Some(builtin) = builtin(db, function) {
            return self.call_builtin(builtin, args);
        }
        // A class method whose impl the types didn't decide, like `show` in
        // `fn f(x) = show(x)`: the arguments decide it.
        if method_class(db, function).is_some() {
            match impl_for_args(db, &self.files, function, &args) {
                Some(imp) => {
                    let target = impl_method(db, imp, function);
                    if target != function {
                        return self.call_function_inner(target, args);
                    }
                }
                None if !function.item(db).has_body => {
                    return fail(ErrorKind::Unsupported("calling a class method whose impl isn't known"));
                }
                None => {}
            }
        }
        let item = function.item(db);
        if !item.has_body {
            if let ItemContainerId::ExternBlock(_) = function.container(db) {
                return fail(ErrorKind::Unsupported("calling an extern function"));
            }
            let name = match &item.name {
                Some(name) => name.name().to_string(),
                None => "?".to_string(),
            };
            return fail(ErrorKind::NoBody(name));
        }
        let def = DefWithBodyId::new(db, DefWithBody::Function(function));
        let body = body(db, def);
        if args.len() != body.params.len() {
            return fail(ErrorKind::WrongArgCount { expected: body.params.len(), found: args.len() });
        }
        self.enter()?;
        let mut frame = Frame { def, body, locals: Locals::new() };
        let result = (|| {
            for (&pat, arg) in body.params.iter().zip(&args) {
                if !self.match_pat(&mut frame, pat, arg)? {
                    return fail(ErrorKind::NoMatch);
                }
            }
            self.eval_body(&mut frame, body.body_expr)
        })();
        self.depth -= 1;
        result
    }

    fn call_builtin(&mut self, builtin: Builtin, args: Vec<Value<'db>>) -> Res<'db, Value<'db>> {
        if builtin.is_effect() && self.in_consts > 0 {
            return fail(ErrorKind::Effect(builtin.name()));
        }
        match (builtin, &args[..]) {
            (Builtin::Print, [Value::Str(text)]) => {
                self.host.print(text)?;
                Ok(Value::unit())
            }
            (Builtin::Panic, [Value::Str(text)]) => fail(ErrorKind::Panic(text.to_string())),
            (Builtin::ArrayLen, [Value::Array(items)]) => Ok(Value::Int(items.borrow().len() as i128)),
            (Builtin::ArrayAlloc, [Value::Int(len)]) => {
                let len = usize::try_from(*len).map_err(|_| ErrorKind::Overflow)?;
                Ok(Value::array(Mutability::Mut, vec![Value::Unset; len]))
            }
            (Builtin::ArrayGet, [Value::Array(items), Value::Int(i)]) => {
                let items = items.borrow();
                element(&items, in_bounds(*i, items.len())?)
            }
            (Builtin::ArraySet, [Value::Array(items), Value::Int(i), x]) => {
                self.changing(items.mutability)?;
                let mut items = items.borrow_mut();
                let i = in_bounds(*i, items.len())?;
                items[i] = x.clone();
                Ok(Value::unit())
            }
            (
                Builtin::ArrayCopy,
                [Value::Array(dst), Value::Int(dst_start), Value::Array(src), Value::Int(src_start), Value::Int(count)],
            ) => {
                self.changing(dst.mutability)?;
                let src_range = range(*src_start, *count, src.borrow().len())?;
                let dst_range = range(*dst_start, *count, dst.borrow().len())?;
                // Taken out first, since `src` and `dst` can be the same
                // array. Unset elements are copied as they are, not read.
                let items: Vec<Value<'db>> = src.borrow()[src_range].to_vec();
                dst.borrow_mut()[dst_range].clone_from_slice(&items);
                Ok(Value::unit())
            }
            // An `imm` array never changes, so it's its own copy.
            (Builtin::ArrayFreeze, [Value::Array(items)]) if items.mutability == Mutability::Imm => {
                Ok(Value::Array(items.clone()))
            }
            (Builtin::ArrayFreeze, [Value::Array(items)]) => {
                self.made(Value::array(Mutability::Imm, items.borrow().clone()))
            }
            _ => mismatch(format!("bad arguments to `{}`", builtin.name())),
        }
    }

    fn apply(&mut self, callee: Value<'db>, args: Vec<Value<'db>>) -> Res<'db, Value<'db>> {
        match callee {
            Value::Function(id) => self.call_function_inner(id, args),
            Value::Closure(closure) => self.call_closure(&closure, args),
            Value::Constructor(ctor) => {
                let fields = args.into_iter().enumerate().map(|(i, v)| (Name::new(i.to_string()), v)).collect();
                self.made(Value::adt(ctor, Mutability::Imm, fields))
            }
            _ => fail(ErrorKind::NotCallable),
        }
    }

    fn call_closure(&mut self, closure: &Closure<'db>, args: Vec<Value<'db>>) -> Res<'db, Value<'db>> {
        let body = body(self.db, closure.def);
        let Expr::Lambda { params, body: lambda_body, .. } = &body[closure.expr] else {
            unreachable!("a closure is made from a lambda")
        };
        if args.len() != params.len() {
            return fail(ErrorKind::WrongArgCount { expected: params.len(), found: args.len() });
        }
        self.enter()?;
        let mut frame = Frame { def: closure.def, body, locals: closure.captured.clone() };
        let result = (|| {
            for (&pat, arg) in params.iter().zip(&args) {
                if !self.match_pat(&mut frame, pat, arg)? {
                    return fail(ErrorKind::NoMatch);
                }
            }
            self.eval_body(&mut frame, *lambda_body)
        })();
        self.depth -= 1;
        result
    }

    // Expressions

    fn eval(&mut self, f: &mut Frame<'db>, expr: ExprId) -> Res<'db, Value<'db>> {
        self.tick()?;
        // Grow the native stack as needed, so that deep recursion is bounded
        // by `Limits::max_depth` and not by the size of the thread's stack.
        stacker::maybe_grow(RED_ZONE, STACK_GROWTH, || self.eval_inner(f, expr)).map_err(|unwind| match unwind {
            Unwind::Error(mut e) => {
                // An error in the code of a package, like `std`'s array
                // impls, is reported at the program's code that led to it,
                // like `xs[5]`.
                let db = self.db;
                let in_package = |def: DefWithBodyId<'db>| package_of(db, def.def(db).file(db)).is_some();
                match e.location {
                    Some((def, _)) if !in_package(def) || in_package(f.def) => {}
                    _ => e.location = Some((f.def, expr)),
                }
                Unwind::Error(e)
            }
            other => other,
        })
    }

    fn eval_inner(&mut self, f: &mut Frame<'db>, expr: ExprId) -> Res<'db, Value<'db>> {
        let body = f.body;
        match &body[expr] {
            Expr::Missing => fail(ErrorKind::Missing),
            Expr::Path(path) => self.eval_path(f, expr, path),
            Expr::Literal(literal) => literal_value(literal),
            Expr::Block { statements, tail, label, kind } => {
                if *kind == BlockKind::Async {
                    return fail(ErrorKind::Unsupported("`async`"));
                }
                let result = self.eval_block(f, statements, *tail);
                match (result, label) {
                    // A labeled block is left with `break 'label value`.
                    (Err(Unwind::Break(Some(name), value)), Some(label)) if body[*label].name == name => Ok(value),
                    (result, _) => result,
                }
            }
            // `mut Wrap(x)` makes a `mut` struct; other calls ignore `mut`.
            Expr::Call { callee, args, is_mut: true } => match self.eval(f, *callee)? {
                Value::Constructor(ctor @ Ctor::Struct(_)) => {
                    let args = self.eval_all(f, args)?;
                    let fields = args.into_iter().enumerate().map(|(i, v)| (Name::new(i.to_string()), v)).collect();
                    Ok(Value::adt(ctor, Mutability::Mut, fields))
                }
                _ => self.eval_call(f, expr, *callee, args),
            },
            Expr::Call { callee, args, .. } => self.eval_call(f, expr, *callee, args),
            Expr::Field { expr: base, name } => {
                if let Some(value) = self.eval_extended_path(f, expr)? {
                    return Ok(value);
                }
                let base = self.eval(f, *base)?;
                match base.field(name) {
                    Some(value) => Ok(value),
                    None => fail(ErrorKind::NoSuchField(name.clone())),
                }
            }
            Expr::Await { .. } => fail(ErrorKind::Unsupported("`await`")),
            Expr::Yield { .. } => fail(ErrorKind::Unsupported("`yield`")),
            Expr::MacroCall { .. } => fail(ErrorKind::Unsupported("a macro call")),
            Expr::Try { expr: inner } => {
                let value = self.eval(f, *inner)?;
                let Value::Adt { ctor: Ctor::Variant(variant), fields } = &value else {
                    return mismatch("`?` needs an Option or a Result");
                };
                match variant.name(self.db).as_str() {
                    "Some" | "Ok" => match fields.borrow().first() {
                        Some((_, inner)) => Ok(inner.clone()),
                        None => mismatch("`?` needs an Option or a Result"),
                    },
                    "None" | "Err" => Err(Unwind::Return(value)),
                    _ => mismatch("`?` needs an Option or a Result"),
                }
            }
            Expr::Cast { expr: inner, type_ref } => {
                let value = self.eval(f, *inner)?;
                cast(value, &body[*type_ref])
            }
            Expr::Index { base, index } => {
                // Through the impl of `Index` for the type, arrays too.
                let base = self.eval(f, *base)?;
                let index = self.eval(f, *index)?;
                self.call_lang(f, expr, LangClass::Index, vec![base, index])
            }
            Expr::Prefix { op, expr: inner } => {
                let value = self.eval(f, *inner)?;
                match (op, value) {
                    (PrefixOp::Neg, Value::Int(n)) => n.checked_neg().map(Value::Int).ok_or(ErrorKind::Overflow.into()),
                    (PrefixOp::Neg, Value::Float(x)) => Ok(Value::Float(-x)),
                    (PrefixOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (PrefixOp::Not, Value::Int(n)) => Ok(Value::Int(!n)),
                    _ => mismatch("bad operand of a prefix operator"),
                }
            }
            Expr::Binary { op, lhs, rhs } => self.eval_binary(f, expr, op, *lhs, *rhs),
            Expr::Range { lhs, rhs, inclusive } => {
                let start = lhs.map(|e| self.eval(f, e)).transpose()?.map(Box::new);
                let end = rhs.map(|e| self.eval(f, e)).transpose()?.map(Box::new);
                Ok(Value::Range { start, end, inclusive: *inclusive })
            }
            Expr::Tuple { exprs } => Ok(Value::Tuple(self.eval_all(f, exprs)?)),
            Expr::Array { exprs, is_mut } => {
                let items = self.eval_all(f, exprs)?;
                self.made(Value::array(mutability(*is_mut), items))
            }
            Expr::ArrayRepeat { value, count, is_mut } => {
                let value = self.eval(f, *value)?;
                let Value::Int(count) = self.eval(f, *count)? else {
                    return mismatch("the length of an array must be an integer");
                };
                let count = usize::try_from(count).map_err(|_| ErrorKind::Overflow)?;
                // The value is evaluated once, so `mut [mut []; 2]` holds the
                // same array twice.
                self.made(Value::array(mutability(*is_mut), vec![value; count]))
            }
            Expr::Record { path, fields, spread, is_mut } => {
                let Some(path) = path else {
                    return fail(ErrorKind::Unsupported("a record without a name"));
                };
                let Some(ctor) = Self::resolve_ctor(self.resolve_value(f.def, expr, path)) else {
                    return fail(ErrorKind::UnresolvedName(path_to_string(path)));
                };
                let mut values = match spread {
                    Some(base) => match self.eval(f, *base)? {
                        // A new struct, with copies of the fields.
                        Value::Adt { ctor: base_ctor, fields } if base_ctor == ctor => fields.borrow().clone(),
                        _ => return mismatch("the base of `..` must be of the same struct"),
                    },
                    None => Vec::new(),
                };
                for field in fields.iter() {
                    let value = self.eval(f, field.expr)?;
                    match values.iter_mut().find(|(n, _)| *n == field.name) {
                        Some((_, slot)) => *slot = value,
                        None => values.push((field.name.clone(), value)),
                    }
                }
                self.made(Value::adt(ctor, mutability(*is_mut), values))
            }
            Expr::If { condition, then_branch, else_branch } => {
                if self.eval_bool(f, *condition)? {
                    self.eval(f, *then_branch)
                } else if let Some(else_branch) = else_branch {
                    self.eval(f, *else_branch)
                } else {
                    Ok(Value::unit())
                }
            }
            Expr::Let { pat, expr: inner } => {
                let value = self.eval(f, *inner)?;
                Ok(Value::Bool(self.match_pat(f, *pat, &value)?))
            }
            Expr::Match { expr: scrutinee, arms } => {
                let value = self.eval(f, *scrutinee)?;
                for arm in arms.iter() {
                    if !self.match_pat(f, arm.pat, &value)? {
                        continue;
                    }
                    if let Some(guard) = arm.guard
                        && !self.eval_bool(f, guard)?
                    {
                        continue;
                    }
                    return self.eval(f, arm.expr);
                }
                fail(ErrorKind::NoMatch)
            }
            Expr::While { condition, body: loop_body, label } => {
                while self.eval_bool(f, *condition)? {
                    match self.eval(f, *loop_body) {
                        Ok(_) => {}
                        Err(Unwind::Break(target, _)) if targets(body, *label, &target) => break,
                        Err(Unwind::Continue(target)) if targets(body, *label, &target) => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::unit())
            }
            Expr::Loop { body: loop_body, label } => loop {
                match self.eval(f, *loop_body) {
                    Ok(_) => {}
                    Err(Unwind::Break(target, value)) if targets(body, *label, &target) => return Ok(value),
                    Err(Unwind::Continue(target)) if targets(body, *label, &target) => {}
                    Err(e) => return Err(e),
                }
            },
            Expr::For { pat, iterable: iterable_expr, body: loop_body, label } => {
                let iterable = self.eval(f, *iterable_expr)?;
                let mut items = match iterable {
                    // Ranges and strings aren't type-checked yet, and have no
                    // impls.
                    Value::Str(_) | Value::Range { .. } => Items::Builtin(iterate(iterable)?),
                    // Through the impl of `Iterable` for the type, arrays too,
                    // whose iterator gives the items.
                    _ => Items::Class(self.call_lang(f, *iterable_expr, LangClass::Iterable, vec![iterable])?),
                };
                loop {
                    let item = match &mut items {
                        Items::Builtin(items) => match items.next() {
                            Some(item) => item?,
                            None => break,
                        },
                        Items::Class(iterator) => {
                            let next = self.call_lang(f, expr, LangClass::Iterator, vec![iterator.clone()])?;
                            match option_value(self.db, next)? {
                                Some(item) => item,
                                None => break,
                            }
                        }
                    };
                    if !self.match_pat(f, *pat, &item)? {
                        return fail(ErrorKind::NoMatch);
                    }
                    match self.eval(f, *loop_body) {
                        Ok(_) => {}
                        Err(Unwind::Break(target, _)) if targets(body, *label, &target) => break,
                        Err(Unwind::Continue(target)) if targets(body, *label, &target) => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::unit())
            }
            Expr::Break { expr: value, label } => {
                let value = match value {
                    Some(e) => self.eval(f, *e)?,
                    None => Value::unit(),
                };
                Err(Unwind::Break(label.clone(), value))
            }
            Expr::Continue { label } => Err(Unwind::Continue(label.clone())),
            Expr::Return { expr: value } => {
                let value = match value {
                    Some(e) => self.eval(f, *e)?,
                    None => Value::unit(),
                };
                Err(Unwind::Return(value))
            }
            Expr::Lambda { .. } => {
                Ok(Value::Closure(Rc::new(Closure { def: f.def, expr, captured: f.locals.clone() })))
            }
        }
    }

    fn eval_all(&mut self, f: &mut Frame<'db>, exprs: &[ExprId]) -> Res<'db, Vec<Value<'db>>> {
        exprs.iter().map(|&e| self.eval(f, e)).collect()
    }

    fn eval_bool(&mut self, f: &mut Frame<'db>, expr: ExprId) -> Res<'db, bool> {
        match self.eval(f, expr)? {
            Value::Bool(b) => Ok(b),
            _ => mismatch("expected a bool"),
        }
    }

    fn eval_block(
        &mut self,
        f: &mut Frame<'db>,
        statements: &[Statement],
        tail: Option<ExprId>,
    ) -> Res<'db, Value<'db>> {
        for statement in statements {
            self.eval_statement(f, statement)?;
        }
        match tail {
            Some(tail) => self.eval(f, tail),
            None => Ok(Value::unit()),
        }
    }

    /// Run a statement. Its value is that of its expression, even with a
    /// `;`, or `()` for other statements.
    fn eval_statement(&mut self, f: &mut Frame<'db>, statement: &Statement) -> Res<'db, Value<'db>> {
        match statement {
            Statement::Let { pat, initializer: Some(init), else_branch, .. } => {
                let value = self.eval(f, *init)?;
                if !self.match_pat(f, *pat, &value)? {
                    match else_branch {
                        Some(else_branch) => {
                            self.eval(f, *else_branch)?;
                            return mismatch("the `else` of a `let` must not finish");
                        }
                        None => return fail(ErrorKind::NoMatch),
                    }
                }
                Ok(Value::unit())
            }
            Statement::Let { pat, initializer: None, .. } => {
                // `let x;` declares `x`, to be assigned later.
                let mut bindings = Vec::new();
                pat_bindings(f.body, *pat, &mut bindings);
                for binding in bindings {
                    f.locals.insert(binding, Rc::new(RefCell::new(None)));
                }
                Ok(Value::unit())
            }
            Statement::Expr { expr, .. } => self.eval(f, *expr),
            // Items in bodies aren't lowered yet.
            Statement::Item => Ok(Value::unit()),
        }
    }

    fn eval_binary(
        &mut self,
        f: &mut Frame<'db>,
        expr: ExprId,
        op: &BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
    ) -> Res<'db, Value<'db>> {
        match op {
            BinaryOp::Logic(op) => {
                let lhs = self.eval_bool(f, lhs)?;
                let value = match op {
                    LogicOp::And => lhs && self.eval_bool(f, rhs)?,
                    LogicOp::Or => lhs || self.eval_bool(f, rhs)?,
                };
                Ok(Value::Bool(value))
            }
            BinaryOp::Arith(op) => {
                let lhs = self.eval(f, lhs)?;
                let rhs = self.eval(f, rhs)?;
                arith(*op, lhs, rhs)
            }
            BinaryOp::Cmp(op) => {
                let lhs = self.eval(f, lhs)?;
                let rhs = self.eval(f, rhs)?;
                let result = match op {
                    CmpOp::Eq => lhs.equals(&rhs),
                    CmpOp::NotEq => !lhs.equals(&rhs),
                    _ => {
                        let Some(ord) = lhs.compare(&rhs) else {
                            return mismatch("these values can't be compared");
                        };
                        match op {
                            CmpOp::Lt => ord.is_lt(),
                            CmpOp::LtEq => ord.is_le(),
                            CmpOp::Gt => ord.is_gt(),
                            CmpOp::GtEq => ord.is_ge(),
                            CmpOp::Eq | CmpOp::NotEq => unreachable!(),
                        }
                    }
                };
                Ok(Value::Bool(result))
            }
            BinaryOp::Assign(op) => {
                let rhs = self.eval(f, rhs)?;
                self.assign(f, lhs, *op, rhs)?;
                Ok(Value::unit())
            }
            // `a <$> b` calls the function `(<$>)` in scope.
            BinaryOp::Custom(name) => {
                let path = Path::from_name(name.clone());
                let callee = match self.resolve_value(f.def, expr, &path) {
                    Some(ValueResolution::Value(value)) => self.value_at(f, expr, value)?,
                    _ => return fail(ErrorKind::UnresolvedName(name.to_string())),
                };
                let lhs = self.eval(f, lhs)?;
                let rhs = self.eval(f, rhs)?;
                self.apply(callee, vec![lhs, rhs])
            }
        }
    }

    fn eval_call(&mut self, f: &mut Frame<'db>, expr: ExprId, callee: ExprId, args: &[ExprId]) -> Res<'db, Value<'db>> {
        // `x.f(y)` is a field holding a function, a longer path like `m.f`,
        // or `f(x, y)` with the `f` in scope.
        if let Expr::Field { expr: receiver, name } = &f.body[callee] {
            let callee_value = match self.eval_extended_path(f, callee)? {
                Some(value) => value,
                None => {
                    let receiver = self.eval(f, *receiver)?;
                    if let Value::Adt { .. } = &receiver
                        && let Some(field) = receiver.field(name)
                    {
                        field
                    } else {
                        let path = Path::from_name(name.clone());
                        let method = match self.resolve_value(f.def, expr, &path) {
                            Some(ValueResolution::Value(value)) => self.value_at(f, callee, value)?,
                            _ => return fail(ErrorKind::UnresolvedName(name.to_string())),
                        };
                        let mut values = vec![receiver];
                        values.extend(self.eval_all(f, args)?);
                        return self.apply(method, values);
                    }
                }
            };
            let args = self.eval_all(f, args)?;
            return self.apply(callee_value, args);
        }
        let callee = self.eval(f, callee)?;
        let args = self.eval_all(f, args)?;
        self.apply(callee, args)
    }

    // Paths

    fn resolve_value(&mut self, def: DefWithBodyId<'db>, expr: ExprId, path: &Path) -> Option<ValueResolution<'db>> {
        let db = self.db;
        *self
            .resolutions
            .entry((def, expr))
            .or_insert_with(|| Resolver::for_expr(db, def, expr).resolve_path_in_value_ns(db, path))
    }

    /// The constructor a path names, for struct literals and patterns.
    fn resolve_ctor(resolution: Option<ValueResolution<'db>>) -> Option<Ctor<'db>> {
        match resolution? {
            ValueResolution::Value(ValueNs::Variant(id))
            | ValueResolution::Type(TypeResolution { ty: TypeNs::Variant(id), unresolved: None }) => {
                Some(Ctor::Variant(id))
            }
            ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(id), unresolved: None }) => Some(Ctor::Struct(id)),
            _ => None,
        }
    }

    fn eval_path(&mut self, f: &mut Frame<'db>, expr: ExprId, path: &Path) -> Res<'db, Value<'db>> {
        match self.resolve_value(f.def, expr, path) {
            Some(ValueResolution::Value(value)) => self.value_at(f, expr, value),
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(id), unresolved: None })) => {
                Ok(self.ctor_value(Ctor::Struct(id)))
            }
            _ => fail(ErrorKind::UnresolvedName(path_to_string(path))),
        }
    }

    /// Call the method of `lang`, a class of the prelude that syntax uses,
    /// for the syntax at `at`: that of the impl the type checker chose
    /// there, or else the one the arguments decide.
    fn call_lang(
        &mut self,
        f: &Frame<'db>,
        at: ExprId,
        lang: LangClass,
        args: Vec<Value<'db>>,
    ) -> Res<'db, Value<'db>> {
        let db = self.db;
        let file = f.def.def(db).file(db);
        let Some(method) = lang.class(db, file).and_then(|class| Some((class, lang.method(db, class)?))) else {
            return fail(ErrorKind::NoLangClass(lang.name()));
        };
        let (class, method) = method;
        let target = match infer_body(db, f.def).instance(at, class) {
            Some(imp) => impl_method(db, imp, method),
            None => method,
        };
        self.call_function_inner(target, args)
    }

    /// The value of what the name used by `expr` refers to. A class method
    /// or constant is the impl's, when the types decide which impl it is.
    fn value_at(&mut self, f: &mut Frame<'db>, expr: ExprId, value: ValueNs<'db>) -> Res<'db, Value<'db>> {
        let db = self.db;
        match value {
            // A name with several functions is the one the types chose (#275).
            ValueNs::Overloaded(_) => {
                return match infer_body(db, f.def).overload(expr) {
                    Some(id) => self.value_at(f, expr, ValueNs::Function(id)),
                    None => fail(ErrorKind::Unsupported("a name with several functions that the types don't decide")),
                };
            }
            ValueNs::Function(id) => {
                if let Some(class) = method_class(db, id)
                    && let Some(imp) = infer_body(db, f.def).instance(expr, class)
                {
                    return Ok(Value::Function(impl_method(db, imp, id)));
                }
            }
            ValueNs::Const(id) => {
                if let ItemContainerId::Class(class) = id.container(db)
                    && let Some(imp) = infer_body(db, f.def).instance(expr, class)
                {
                    return self.eval_const_inner(impl_const(db, imp, id));
                }
            }
            _ => {}
        }
        self.value_of(f, value)
    }

    fn value_of(&mut self, f: &mut Frame<'db>, value: ValueNs<'db>) -> Res<'db, Value<'db>> {
        match value {
            ValueNs::Local(binding) => {
                let slot = f.locals.get(&binding).and_then(|slot| slot.borrow().clone());
                slot.ok_or_else(|| ErrorKind::Uninitialized(f.body[binding].name.clone()).into())
            }
            ValueNs::Function(id) => Ok(Value::Function(id)),
            ValueNs::Overloaded(_) => fail(ErrorKind::Unsupported("a name with several functions that the types don't decide")),
            ValueNs::Const(id) => self.eval_const_inner(id),
            ValueNs::Variant(id) => Ok(self.ctor_value(Ctor::Variant(id))),
            ValueNs::ConstParam { .. } => fail(ErrorKind::Unsupported("a const parameter")),
            ValueNs::Builtin(v) => Ok(Value::Bool(v.as_bool())),
        }
    }

    /// A unit struct or variant is a value; the others are functions.
    fn ctor_value(&self, ctor: Ctor<'db>) -> Value<'db> {
        let db = self.db;
        let is_unit = match ctor {
            Ctor::Struct(id) => matches!(struct_signature(db, id).fields, FieldList::Unit),
            Ctor::Variant(id) => {
                matches!(enum_signature(db, id.parent).variants[id.index as usize].fields, FieldList::Unit)
            }
        };
        if is_unit { Value::adt(ctor, Mutability::Imm, Vec::new()) } else { Value::Constructor(ctor) }
    }

    /// `Color.Red` and `m.f` are parsed as field accesses. When `expr` is
    /// one and its base isn't a value, evaluate it as a longer path.
    fn eval_extended_path(&mut self, f: &mut Frame<'db>, expr: ExprId) -> Res<'db, Option<Value<'db>>> {
        let Expr::Field { expr: base, name } = &f.body[expr] else { return Ok(None) };
        let Some(base_path) = expr_as_path(f.body, *base) else { return Ok(None) };
        if let Some(ValueResolution::Value(_)) = self.resolve_value(f.def, *base, &base_path) {
            return Ok(None);
        }
        let mut segments = base_path.segments.into_vec();
        segments.push(PathSegment { name: name.clone(), generic_args: None });
        let path = Path { segments: segments.into() };
        match self.resolve_value(f.def, expr, &path) {
            Some(ValueResolution::Value(value)) => Ok(Some(self.value_of(f, value)?)),
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(id), unresolved: None })) => {
                Ok(Some(self.ctor_value(Ctor::Struct(id))))
            }
            _ => Ok(None),
        }
    }

    // Assignment

    fn assign(&mut self, f: &mut Frame<'db>, target: ExprId, op: Option<ArithOp>, value: Value<'db>) -> Res<'db, ()> {
        let (place, fields) = self.place(f, target)?;
        if let Place::Variable(slot) = &place
            && fields.is_empty()
            && op.is_none()
        {
            *slot.borrow_mut() = Some(value);
            return Ok(());
        }
        // `c[i] = x` through `IndexMut`, arrays too. For `c[i] += x` or
        // `c[i].0 = x`, the element is read through `Index` first, changed,
        // and set back.
        if let Place::Indexed { container, index, at } = place {
            let value = if op.is_none() && fields.is_empty() {
                value
            } else {
                let mut element = self.call_lang(f, at, LangClass::Index, vec![container.clone(), index.clone()])?;
                let current = tuple_field(Some(&mut element), &fields)?;
                *current = match op {
                    Some(op) => arith(op, current.clone(), value)?,
                    None => value,
                };
                element
            };
            self.call_lang(f, at, LangClass::IndexMut, vec![container, index, value])?;
            return Ok(());
        }
        // A field of a struct, or of a tuple in one: the struct changes.
        if let Place::Field(fields, _) = &place {
            self.changing(fields.mutability)?;
        }
        place.update(|current| {
            let current = tuple_field(current, &fields)?;
            *current = match op {
                Some(op) => arith(op, current.clone(), value)?,
                None => value,
            };
            Ok(())
        })
    }

    /// Where an assignment writes, and the fields of the tuples on the way
    /// to the part it changes.
    ///
    /// Arrays and structs are shared, so `xs[i] = e` and `p.x = e` change
    /// the array or struct that `xs` or `p` evaluates to, and every name for
    /// it sees the change. A tuple is a value, kept where it is: `t.0 = e`
    /// changes the variable, element or field that holds `t`.
    fn place(&mut self, f: &mut Frame<'db>, expr: ExprId) -> Res<'db, (Place<'db>, Vec<Name>)> {
        match &f.body[expr] {
            Expr::Path(path) => match self.resolve_value(f.def, expr, path) {
                Some(ValueResolution::Value(ValueNs::Local(binding))) => {
                    let slot = f.locals.entry(binding).or_insert_with(|| Rc::new(RefCell::new(None))).clone();
                    Ok((Place::Variable(slot), Vec::new()))
                }
                _ => fail(ErrorKind::NotAssignable),
            },
            Expr::Index { base, index } => {
                let container = self.eval(f, *base)?;
                let index = self.eval(f, *index)?;
                Ok((Place::Indexed { container, index, at: expr }, Vec::new()))
            }
            Expr::Field { expr: base, name } => {
                let (place, mut fields) = self.place(f, *base)?;
                let shared_fields = |value: &mut Value<'db>| match value {
                    Value::Adt { fields, .. } => Some(fields.clone()),
                    _ => None,
                };
                let shared = match &place {
                    // An element, through `Index`: a struct is shared, so a
                    // field of it can be changed in place.
                    Place::Indexed { container, index, at } => {
                        let args = vec![container.clone(), index.clone()];
                        let mut element = self.call_lang(f, *at, LangClass::Index, args)?;
                        shared_fields(tuple_field(Some(&mut element), &fields)?)
                    }
                    _ => place.update(|current| Ok(shared_fields(tuple_field(current, &fields)?)))?,
                };
                match shared {
                    Some(adt) => {
                        if !adt.borrow().iter().any(|(n, _)| n == name) {
                            return fail(ErrorKind::NoSuchField(name.clone()));
                        }
                        Ok((Place::Field(adt, name.clone()), Vec::new()))
                    }
                    None => {
                        fields.push(name.clone());
                        Ok((place, fields))
                    }
                }
            }
            _ => fail(ErrorKind::NotAssignable),
        }
    }

    // Patterns

    /// Whether `value` matches `pat`, binding its variables if it does.
    fn match_pat(&mut self, f: &mut Frame<'db>, pat: PatId, value: &Value<'db>) -> Res<'db, bool> {
        let body = f.body;
        match &body[pat] {
            Pat::Missing => fail(ErrorKind::Missing),
            Pat::Wild => Ok(true),
            Pat::Bind { id, subpat } => {
                if let Some(subpat) = subpat
                    && !self.match_pat(f, *subpat, value)?
                {
                    return Ok(false);
                }
                f.locals.insert(*id, Rc::new(RefCell::new(Some(value.clone()))));
                Ok(true)
            }
            Pat::Lit { .. } | Pat::Path(_) => {
                if let Pat::Path(path) = &body[pat]
                    && let Some(ctor) = Self::resolve_ctor(self.resolve_pat_path(f.def, path))
                {
                    return Ok(matches!(value, Value::Adt { ctor: c, .. } if *c == ctor));
                }
                Ok(self.pat_value(f, pat)?.equals(value))
            }
            Pat::Tuple { args, rest } => match value {
                Value::Tuple(items) => self.match_seq(f, args, *rest, items),
                _ => Ok(false),
            },
            Pat::Slice { args, rest } => match value {
                Value::Array(items) => {
                    let items = items.borrow();
                    let items = (0..items.len()).map(|i| element(&items, i)).collect::<Res<'db, Vec<_>>>()?;
                    self.match_seq(f, args, *rest, &items)
                }
                _ => Ok(false),
            },
            Pat::TupleStruct { path, args, rest } => {
                let Some(ctor) = Self::resolve_ctor(self.resolve_pat_path(f.def, path)) else {
                    return fail(ErrorKind::UnresolvedName(path_to_string(path)));
                };
                match value {
                    Value::Adt { ctor: c, fields } if *c == ctor => {
                        let items: Vec<_> = fields.borrow().iter().map(|(_, v)| v.clone()).collect();
                        self.match_seq(f, args, *rest, &items)
                    }
                    _ => Ok(false),
                }
            }
            Pat::Record { path, fields: field_pats, .. } => {
                let Some(ctor) = Self::resolve_ctor(self.resolve_pat_path(f.def, path)) else {
                    return fail(ErrorKind::UnresolvedName(path_to_string(path)));
                };
                let Value::Adt { ctor: c, .. } = value else { return Ok(false) };
                if *c != ctor {
                    return Ok(false);
                }
                for field in field_pats.iter() {
                    let Some(field_value) = value.field(&field.name) else {
                        return fail(ErrorKind::NoSuchField(field.name.clone()));
                    };
                    if !self.match_pat(f, field.pat, &field_value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Pat::Range { start, end, inclusive } => {
                if let Some(start) = start
                    && !matches!(self.pat_value(f, *start)?.compare(value), Some(ord) if ord.is_le())
                {
                    return Ok(false);
                }
                if let Some(end) = end {
                    let Some(ord) = value.compare(&self.pat_value(f, *end)?) else { return Ok(false) };
                    return Ok(if *inclusive { ord.is_le() } else { ord.is_lt() });
                }
                Ok(true)
            }
            Pat::Or(alternatives) => {
                for &alternative in alternatives.iter() {
                    if self.match_pat(f, alternative, value)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Pat::ConstBlock(expr) => Ok(self.eval(f, *expr)?.equals(value)),
            Pat::MacroCall { .. } => fail(ErrorKind::Unsupported("a macro in a pattern")),
        }
    }

    /// Match the items of a tuple or array, with a `..` at `rest`.
    fn match_seq(
        &mut self,
        f: &mut Frame<'db>,
        pats: &[PatId],
        rest: Option<u32>,
        items: &[Value<'db>],
    ) -> Res<'db, bool> {
        let (before, after) = match rest {
            None if pats.len() != items.len() => return Ok(false),
            None => (pats, &[][..]),
            Some(_) if pats.len() > items.len() => return Ok(false),
            Some(rest) => pats.split_at((rest as usize).min(pats.len())),
        };
        let after_items = &items[items.len() - after.len()..];
        for (&pat, item) in before.iter().zip(items).chain(after.iter().zip(after_items)) {
            if !self.match_pat(f, pat, item)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The value of a literal or constant pattern, like the ends of a range.
    fn pat_value(&mut self, f: &mut Frame<'db>, pat: PatId) -> Res<'db, Value<'db>> {
        match &f.body[pat] {
            Pat::Lit { literal, negative } => {
                let value = literal_value(literal)?;
                if !negative {
                    return Ok(value);
                }
                match value {
                    Value::Int(n) => Ok(Value::Int(-n)),
                    Value::Float(x) => Ok(Value::Float(-x)),
                    _ => mismatch("only numbers can be negative"),
                }
            }
            Pat::Path(path) => match self.resolve_pat_path(f.def, path) {
                Some(ValueResolution::Value(ValueNs::Const(id))) => self.eval_const_inner(id),
                Some(ValueResolution::Value(ValueNs::Builtin(v))) => Ok(Value::Bool(v.as_bool())),
                _ => fail(ErrorKind::UnresolvedName(path_to_string(path))),
            },
            _ => mismatch("expected a literal or a constant"),
        }
    }

    /// Paths in patterns name constants and variants, not local variables.
    fn resolve_pat_path(&self, def: DefWithBodyId<'db>, path: &Path) -> Option<ValueResolution<'db>> {
        Resolver::for_body(self.db, def).resolve_path_in_value_ns(self.db, path)
    }
}

/// Turn the outcome of a top-level evaluation into a result.
fn finish<'db>(result: Res<'db, Value<'db>>) -> Result<Value<'db>, EvalError<'db>> {
    match result {
        Ok(value) | Err(Unwind::Return(value)) => Ok(value),
        Err(Unwind::Break(..) | Unwind::Continue(_)) => Err(ErrorKind::BreakOutsideLoop.into()),
        Err(Unwind::Error(e)) => Err(e),
    }
}

/// Whether a `break` or `continue` to `target` leaves the loop with `label`.
fn targets(body: &Body, label: Option<LabelId>, target: &Option<Name>) -> bool {
    match target {
        None => true,
        Some(name) => label.is_some_and(|label| body[label].name == *name),
    }
}

/// `a`, `a.b` or `a.b.c` as a path, if `expr` is one.
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

fn path_to_string(path: &Path) -> String {
    path.segments.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(".")
}

/// The bindings a pattern introduces.
fn pat_bindings(body: &Body, pat: PatId, out: &mut Vec<BindingId>) {
    match &body[pat] {
        Pat::Bind { id, subpat } => {
            out.push(*id);
            if let Some(subpat) = subpat {
                pat_bindings(body, *subpat, out);
            }
        }
        Pat::Tuple { args, .. } | Pat::TupleStruct { args, .. } | Pat::Slice { args, .. } | Pat::Or(args) => {
            for &arg in args.iter() {
                pat_bindings(body, arg, out);
            }
        }
        Pat::Record { fields, .. } => {
            for field in fields.iter() {
                pat_bindings(body, field.pat, out);
            }
        }
        Pat::Missing
        | Pat::Wild
        | Pat::Lit { .. }
        | Pat::Path(_)
        | Pat::Range { .. }
        | Pat::ConstBlock(_)
        | Pat::MacroCall { .. } => {}
    }
}

fn literal_value<'db>(literal: &Literal) -> Res<'db, Value<'db>> {
    Ok(match literal {
        Literal::Int(n, _) => Value::Int(i128::try_from(*n).map_err(|_| ErrorKind::Overflow)?),
        Literal::Float(text, _) => match text.parse() {
            Ok(x) => Value::Float(x),
            Err(_) => return mismatch(format!("bad float literal `{text}`")),
        },
        Literal::Char(c) => Value::Char(*c),
        Literal::Byte(b) => Value::Int(*b as i128),
        Literal::String(s) | Literal::CString(s) => Value::str(s),
        Literal::ByteString(bytes) => {
            Value::array(Mutability::Imm, bytes.iter().map(|b| Value::Int(*b as i128)).collect())
        }
    })
}

fn arith<'db>(op: ArithOp, lhs: Value<'db>, rhs: Value<'db>) -> Res<'db, Value<'db>> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => {
            if matches!(op, ArithOp::Div | ArithOp::Rem) && b == 0 {
                return fail(ErrorKind::DivisionByZero);
            }
            let shift = || u32::try_from(b).ok();
            let result = match op {
                ArithOp::Add => a.checked_add(b),
                ArithOp::Sub => a.checked_sub(b),
                ArithOp::Mul => a.checked_mul(b),
                ArithOp::Div => a.checked_div(b),
                ArithOp::Rem => a.checked_rem(b),
                ArithOp::BitAnd => Some(a & b),
                ArithOp::BitOr => Some(a | b),
                ArithOp::BitXor => Some(a ^ b),
                ArithOp::Shl => shift().and_then(|b| a.checked_shl(b)),
                ArithOp::Shr => shift().and_then(|b| a.checked_shr(b)),
            };
            result.map(Value::Int).ok_or(ErrorKind::Overflow.into())
        }
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(match op {
            ArithOp::Add => a + b,
            ArithOp::Sub => a - b,
            ArithOp::Mul => a * b,
            ArithOp::Div => a / b,
            ArithOp::Rem => a % b,
            _ => return mismatch("bitwise operators need integers"),
        })),
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(match op {
            ArithOp::BitAnd => a & b,
            ArithOp::BitOr => a | b,
            ArithOp::BitXor => a ^ b,
            _ => return mismatch("arithmetic needs numbers"),
        })),
        (Value::Str(a), Value::Str(b)) if op == ArithOp::Add => Ok(Value::Str(format!("{a}{b}").into())),
        _ => mismatch("bad operands of an arithmetic operator"),
    }
}

/// `x as T`, for the primitive types. Other casts leave the value as it is.
fn cast<'db>(value: Value<'db>, type_ref: &TypeRef) -> Res<'db, Value<'db>> {
    let TypeRef::Path(path) = type_ref else { return Ok(value) };
    let Some(ty) = path.as_name().and_then(|n| BuiltinType::by_name(n.as_str())) else { return Ok(value) };
    Ok(match (value, ty) {
        (Value::Int(n), BuiltinType::Float(_)) => Value::Float(n as f64),
        (Value::Float(x), BuiltinType::Int(_) | BuiltinType::Uint(_)) => Value::Int(x as i128),
        (Value::Char(c), BuiltinType::Int(_) | BuiltinType::Uint(_)) => Value::Int(c as i128),
        (Value::Bool(b), BuiltinType::Int(_) | BuiltinType::Uint(_)) => Value::Int(b as i128),
        (Value::Int(n), BuiltinType::Char) => match u32::try_from(n).ok().and_then(char::from_u32) {
            Some(c) => Value::Char(c),
            None => return mismatch(format!("{n} is not a character")),
        },
        (value, _) => value,
    })
}

/// The `count` elements from `start` in an array of `len` elements, if
/// they're in it.
fn range<'db>(start: i128, count: i128, len: usize) -> Res<'db, std::ops::Range<usize>> {
    let end = start.saturating_add(count);
    let fits = |n: i128| usize::try_from(n).ok().filter(|&n| n <= len);
    match (fits(start), fits(end)) {
        (Some(s), Some(e)) if s <= e => Ok(s..e),
        _ => fail(ErrorKind::RangeOutOfBounds { start, end, len }),
    }
}

/// `index` as a position in an array of `len` elements, if it's in bounds.
fn in_bounds<'db>(index: i128, len: usize) -> Res<'db, usize> {
    usize::try_from(index).ok().filter(|&i| i < len).ok_or(ErrorKind::IndexOutOfBounds { index, len }.into())
}

/// The element of `items` at `index`, which is in bounds, unless it isn't
/// set yet.
fn element<'db>(items: &[Value<'db>], index: usize) -> Res<'db, Value<'db>> {
    match &items[index] {
        Value::Unset => fail(ErrorKind::UnsetElement { index }),
        item => Ok(item.clone()),
    }
}

/// The mutability of a literal, written `mut [..]` or `[..]`.
fn mutability(is_mut: bool) -> Mutability {
    if is_mut { Mutability::Mut } else { Mutability::Imm }
}

/// The item in `Some(item)`, or `None` for `None`.
fn option_value<'db>(db: &'db dyn Db, value: Value<'db>) -> Res<'db, Option<Value<'db>>> {
    let Value::Adt { ctor: Ctor::Variant(variant), fields } = &value else {
        return mismatch("`next` must give an Option");
    };
    match variant.name(db).as_str() {
        "Some" => Ok(fields.borrow().first().map(|(_, v)| v.clone())),
        "None" => Ok(None),
        _ => mismatch("`next` must give an Option"),
    }
}

/// The items of a string or range that a `for` loop goes over.
fn iterate<'db>(value: Value<'db>) -> Res<'db, Box<dyn Iterator<Item = Res<'db, Value<'db>>> + 'db>> {
    match value {
        Value::Str(s) => Ok(Box::new(s.chars().collect::<Vec<_>>().into_iter().map(|c| Ok(Value::Char(c))))),
        Value::Range { start: Some(start), end, inclusive } => {
            let Value::Int(start) = *start else { return mismatch("only ranges of integers can be iterated") };
            let end = match end.map(|e| *e) {
                Some(Value::Int(end)) => Some(if inclusive { end } else { end - 1 }),
                None => None,
                Some(_) => return mismatch("only ranges of integers can be iterated"),
            };
            let mut next = Some(start);
            Ok(Box::new(std::iter::from_fn(move || {
                let n = next?;
                if end.is_some_and(|end| n > end) {
                    return None;
                }
                next = n.checked_add(1);
                Some(Ok(Value::Int(n)))
            })))
        }
        _ => mismatch("this value can't be iterated"),
    }
}
