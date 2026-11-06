//! MIR bodies and their nodes.

use std::ops;

use boltlang_hir_def::{
    Name,
    arena::ArenaMap,
    body::DefWithBodyId,
    hir::{self, BindingId, Literal, LogicOp},
    ids::{ConstId, FunctionId, ImplId},
};
use boltlang_hir_ty::{Ty, builtin::Builtin};
use boltlang_pattern::decision::{Ctor, Decision};

/// A node of a [`Body`], by the order it was added in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct ExprId(pub u32);

/// A label of a [`Body`], by the order it was added in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct LabelId(pub u32);

/// What a body is the code of.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum BodyOwner<'db> {
    /// A function, a constant, or the top-level statements of a file.
    Def(DefWithBodyId<'db>),
    /// A lambda, the expression `expr` in the HIR of `parent`. Each lambda
    /// has a body of its own, which the [`ExprKind::Closure`] that makes it
    /// refers to.
    Lambda { parent: DefWithBodyId<'db>, expr: hir::ExprId },
    /// Code that no source has, which a pass makes.
    Shim(Shim<'db>),
}

/// A body that the compiler makes, rather than one written in the source,
/// as rustc's shims are.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Shim<'db> {
    /// The deep copy of `imm e` for values of this type.
    ImmDeep(Ty<'db>),
    /// The constructor of a tuple struct or a variant with fields, as a
    /// function: `Some` in `xs.map(Some)`. It takes the fields, in order,
    /// and makes the value.
    Ctor(Ctor<'db>),
}

/// A body: its nodes, from [`Body::root`], and its locals.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Body<'db> {
    pub owner: BodyOwner<'db>,
    pub exprs: Vec<Expr<'db>>,
    /// The locals of the source, by their binding in HIR.
    pub bindings: ArenaMap<hir::Binding, LocalDecl<'db>>,
    /// The locals that the lowering or a pass adds, by number.
    pub temps: Vec<LocalDecl<'db>>,
    pub labels: Vec<Label>,
    pub params: Vec<Local>,
    /// What a lambda captures, which its closure gives it, before its
    /// parameters. Other bodies capture nothing.
    pub captures: Vec<Capture>,
    pub root: ExprId,
}

/// A node, with its type and where it comes from.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Expr<'db> {
    pub kind: ExprKind<'db>,
    /// Its type, which can contain the body's type parameters.
    pub ty: Ty<'db>,
    /// The HIR expression it was built from, for errors. It's `None` for a
    /// node that a pass made.
    pub origin: Option<hir::ExprId>,
}

/// A local: one of the source, or one that the lowering or a pass adds.
///
/// The locals of the source are those of HIR, so that the bindings of a
/// [`Decision`] are locals as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub enum Local {
    Binding(BindingId),
    Temp(u32),
}

/// A local's name, if it has one, its type, and whether it's assigned to.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct LocalDecl<'db> {
    pub name: Option<Name>,
    pub ty: Ty<'db>,
    /// Whether it's declared `mut`, so that it can be assigned to.
    pub mutable: bool,
}

/// A label of a block or a loop, which `break` and `continue` name.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Label {
    /// The name of a label in the source. Each loop has a label, so that its
    /// `break`s can name it, and those without a name in the source have
    /// none here.
    pub name: Option<Name>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum ExprKind<'db> {
    // Values.
    Literal(Literal),
    /// `True` or `False`, which are names in HIR, not literals.
    Bool(bool),
    Local(Local),
    /// A function or constant, used as a value or called. The node's type
    /// is the type it's used at, which is what tells monomorphization the
    /// instance: for a function, its parameters and result.
    Item(ItemRef<'db>),
    /// A call. The operators, prefixes like `-a`, reading an index like
    /// `xs[i]` and method calls are calls too, of the function they name.
    Call {
        callee: ExprId,
        args: Box<[ExprId]>,
    },
    /// A tuple, struct or variant, with its fields in the order they're
    /// declared. A struct literal, `Wrapper(1)`, `None` and `a..b` are one.
    Construct {
        ctor: Ctor<'db>,
        fields: Box<[ExprId]>,
    },
    Array {
        elems: Box<[ExprId]>,
    },
    /// `[value; count]`
    ArrayRepeat {
        value: ExprId,
        count: ExprId,
    },
    /// The `index`th field of a value that `ctor` made.
    Field {
        base: ExprId,
        ctor: Ctor<'db>,
        index: u32,
    },
    /// `expr as T`, with `T` the node's type.
    Cast {
        expr: ExprId,
    },
    /// A closure or anonymous function: the lambda `lambda` of the HIR of
    /// the body, whose own body is [`BodyOwner::Lambda`], with what it
    /// captures.
    Closure {
        lambda: hir::ExprId,
        captures: Box<[Capture]>,
    },
    /// `imm e`: a copy that can't be changed, all the way down.
    Imm {
        expr: ExprId,
    },

    // Control flow.
    Block {
        stmts: Box<[Stmt<'db>]>,
        tail: Option<ExprId>,
        label: Option<LabelId>,
    },
    If {
        cond: ExprId,
        then: ExprId,
        else_: Option<ExprId>,
    },
    /// `a && b` or `a || b`, which only evaluate `b` if they need it.
    Logic {
        op: LogicOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    /// `let pattern = scrutinee` in a condition, which is true when the
    /// pattern matches.
    Let {
        scrutinee: ExprId,
        decision: Box<Decision<'db>>,
    },
    /// A `match`. Its decision picks the arm, which [`Arm`]s gives in order.
    Match {
        scrutinee: ExprId,
        decision: Box<Decision<'db>>,
        arms: Box<[Arm]>,
    },
    Loop {
        body: ExprId,
        label: LabelId,
    },
    While {
        cond: ExprId,
        body: ExprId,
        label: LabelId,
    },
    /// `for pat in iterable { body }`, which calls `iter` on the iterable
    /// once, and then `next` on what it gives until that gives `None`.
    For {
        pat: Pattern<'db>,
        iterable: ExprId,
        body: ExprId,
        label: LabelId,
        iter: Callee<'db>,
        next: Callee<'db>,
    },
    Break {
        label: LabelId,
        value: Option<ExprId>,
    },
    Continue {
        label: LabelId,
    },
    Return {
        value: Option<ExprId>,
    },
    /// `place = value`, or with `op` a compound assignment like `place +=
    /// value`, which is `place = op(place, value)` with `place` evaluated
    /// once.
    Assign {
        place: Place<'db>,
        op: Option<Callee<'db>>,
        value: ExprId,
    },

    /// Code that isn't supported yet, like `e?`, with what it is. It's a
    /// node, so that the rest of the body is still built, and the error
    /// stays at its expression.
    Unsupported(&'static str),
}

/// What a use of a function or constant refers to. Which instance it is
/// follows from the type it's used at: that of its [`ExprKind::Item`], or
/// of its [`Callee`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ItemRef<'db> {
    /// A function with a body.
    Fn(FunctionId<'db>),
    /// A constant with a body.
    Const(ConstId<'db>),
    /// A method or constant of a class. Monomorphization picks the impl, or
    /// a dictionary in erased code.
    Method {
        item: ClassItem<'db>,
        /// The class's arguments, like the `[i32]` of `Show[i32]`, when the
        /// checker knows them. Without them, the type of the use decides.
        class_args: Option<Box<[Ty<'db>]>>,
        /// The impl that the checker chose, if it did.
        imp: Option<ImplId<'db>>,
    },
    /// A function that the compiler provides, like `i32_add`.
    Builtin(Builtin),
    /// A function of the host, in an `extern` block.
    Extern(FunctionId<'db>),
    /// The constructor of a tuple struct or a variant, used as a function,
    /// which is a [`Shim::Ctor`]. Calling it directly is a
    /// [`ExprKind::Construct`] instead.
    Ctor(Ctor<'db>),
}

/// A function that something other than a call calls, like the `next` of a
/// `for`, with the type it's used at, as an [`ExprKind::Item`] has it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct Callee<'db> {
    pub item: ItemRef<'db>,
    pub ty: Ty<'db>,
}

/// A method or constant of a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ClassItem<'db> {
    Function(FunctionId<'db>),
    Const(ConstId<'db>),
}

/// What an assignment changes.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Place<'db> {
    Local(Local),
    /// The `index`th field of a value that `ctor` made.
    Field {
        base: ExprId,
        ctor: Ctor<'db>,
        index: u32,
    },
    /// `base[index]`, which `set` changes. `get` reads it first, for a
    /// compound assignment, and is `None` for any other.
    Index {
        base: ExprId,
        index: ExprId,
        get: Option<Callee<'db>>,
        set: Callee<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Stmt<'db> {
    /// `let pat = init else { .. };`
    Let {
        pat: Pattern<'db>,
        init: Option<ExprId>,
        else_: Option<ExprId>,
    },
    Expr(ExprId),
}

/// What a `let` or a `for` binds.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum Pattern<'db> {
    /// A single local, like `let x = ..`, or `_` with none.
    Local(Option<Local>),
    /// Any other pattern, as a decision with a single arm: it can fail,
    /// which a `let` with `else` allows.
    Decision(Box<Decision<'db>>),
}

/// An arm of a `match`, in the order they're written.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Arm {
    pub guard: Option<ExprId>,
    pub body: ExprId,
}

/// A local that a closure captures, by its value, or by a reference to it
/// when it's assigned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub struct Capture {
    pub local: Local,
    pub by_ref: bool,
}

impl<'db> Body<'db> {
    /// An empty body for `owner`, whose root is an [`ExprKind::Unsupported`]
    /// until the builder sets it.
    pub fn new(owner: BodyOwner<'db>) -> Self {
        let root =
            Expr { kind: ExprKind::Unsupported("an empty body"), ty: Ty::unit(), origin: None };
        Body {
            owner,
            exprs: vec![root],
            bindings: ArenaMap::default(),
            temps: Vec::new(),
            labels: Vec::new(),
            params: Vec::new(),
            captures: Vec::new(),
            root: ExprId(0),
        }
    }

    /// Add a node.
    pub fn alloc(
        &mut self,
        kind: ExprKind<'db>,
        ty: Ty<'db>,
        origin: Option<hir::ExprId>,
    ) -> ExprId {
        self.exprs.push(Expr { kind, ty, origin });
        ExprId(self.exprs.len() as u32 - 1)
    }

    /// Replace what the node `e` is, keeping its type and origin.
    pub fn replace(&mut self, e: ExprId, kind: ExprKind<'db>) {
        self[e].kind = kind;
    }

    /// Add a local that the source doesn't have.
    pub fn new_temp(&mut self, name: Option<Name>, ty: Ty<'db>) -> Local {
        self.temps.push(LocalDecl { name, ty, mutable: false });
        Local::Temp(self.temps.len() as u32 - 1)
    }

    /// Add a label.
    pub fn new_label(&mut self, name: Option<Name>) -> LabelId {
        self.labels.push(Label { name });
        LabelId(self.labels.len() as u32 - 1)
    }

    /// A node that reads a local, of the local's type.
    pub fn local_expr(&mut self, local: Local) -> ExprId {
        let ty = self.local(local).map_or_else(|| Ty::Error, |d| d.ty.clone());
        self.alloc(ExprKind::Local(local), ty, None)
    }

    /// A call of `item`, used at the type `fn(args) -> ret`, with `args`.
    pub fn call(&mut self, item: ItemRef<'db>, args: Vec<ExprId>, ret: Ty<'db>) -> ExprId {
        let params: Vec<Ty<'db>> = args.iter().map(|&a| self[a].ty.clone()).collect();
        let callee = self.alloc(ExprKind::Item(item), Ty::func(params, ret.clone()), None);
        self.alloc(ExprKind::Call { callee, args: args.into() }, ret, None)
    }

    /// A block without a label, of the type of its tail, or `()`.
    pub fn block(&mut self, stmts: Vec<Stmt<'db>>, tail: Option<ExprId>) -> ExprId {
        let ty = tail.map_or_else(Ty::unit, |t| self[t].ty.clone());
        self.alloc(ExprKind::Block { stmts: stmts.into(), tail, label: None }, ty, None)
    }

    /// A new local, with the `let` that gives it `value`'s value.
    pub fn let_temp(&mut self, name: &str, value: ExprId, mutable: bool) -> (Local, Stmt<'db>) {
        let local = self.new_temp(Some(Name::new(name)), self[value].ty.clone());
        if let Local::Temp(i) = local {
            self.temps[i as usize].mutable = mutable;
        }
        (local, Stmt::Let { pat: Pattern::Local(Some(local)), init: Some(value), else_: None })
    }

    pub fn local(&self, local: Local) -> Option<&LocalDecl<'db>> {
        match local {
            Local::Binding(b) => self.bindings.get(b),
            Local::Temp(i) => self.temps.get(i as usize),
        }
    }

    /// The nodes, from the first one added.
    pub fn exprs(&self) -> impl Iterator<Item = (ExprId, &Expr<'db>)> {
        self.exprs.iter().enumerate().map(|(i, e)| (ExprId(i as u32), e))
    }
}

impl<'db> ops::Index<ExprId> for Body<'db> {
    type Output = Expr<'db>;
    fn index(&self, e: ExprId) -> &Expr<'db> {
        &self.exprs[e.0 as usize]
    }
}

impl<'db> ops::IndexMut<ExprId> for Body<'db> {
    fn index_mut(&mut self, e: ExprId) -> &mut Expr<'db> {
        &mut self.exprs[e.0 as usize]
    }
}

impl ops::Index<LabelId> for Body<'_> {
    type Output = Label;
    fn index(&self, l: LabelId) -> &Label {
        &self.labels[l.0 as usize]
    }
}
