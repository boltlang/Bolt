//! A body as text, for tests and debugging.
//!
//! It looks like Bolt, with what MIR adds written out: calls name the
//! function they call, with the types they give it, like `fac(x)` or
//! `` `+`[i32](x, 1) ``, and a `match` shows its decision, as
//! [`Decision::display`] gives it, before its arms. Locals are their names,
//! and a name that two locals share gets the local's number too, like
//! `x#b0` and `x#b1`. With types, each node is written `(e : T)`.

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    body::DefWithBody,
    hir::{Literal, LogicOp},
    ids::{ConstId, FunctionId, ItemContainerId},
};
use boltlang_hir_ty::{Ty, lower::impl_header};
use boltlang_pattern::decision::{Ctor, Decision};

use crate::body::{
    Arm, Body, BodyOwner, Capture, ClassItem, ExprId, ExprKind, ItemRef, LabelId, Local, Pattern,
    Place, Shim, Stmt,
};

impl<'db> Body<'db> {
    /// The body as text: see the module's documentation.
    pub fn display(&self, db: &'db dyn Db) -> String {
        Printer { db, body: self, types: false }.body()
    }

    /// The body as text, with the type of each node.
    pub fn display_with_types(&self, db: &'db dyn Db) -> String {
        Printer { db, body: self, types: true }.body()
    }
}

struct Printer<'a, 'db> {
    db: &'db dyn Db,
    body: &'a Body<'db>,
    types: bool,
}

const INDENT: &str = "  ";

impl<'db> Printer<'_, 'db> {
    fn body(&self) -> String {
        let params: Vec<String> = self
            .body
            .params
            .iter()
            .map(|&p| format!("{}: {}", self.local(p), self.local_ty(p)))
            .collect();
        let ret = self.ty(&self.body[self.body.root].ty);
        let captures = if self.body.captures.is_empty() {
            String::new()
        } else {
            format!(" capturing [{}]", self.captures(&self.body.captures))
        };
        format!(
            "{}{captures}({}) -> {ret} = {}\n",
            self.owner(),
            params.join(", "),
            self.expr(self.body.root, 0)
        )
    }

    fn owner(&self) -> String {
        let db = self.db;
        match &self.body.owner {
            BodyOwner::Def(def) => match def.def(db) {
                DefWithBody::File(_) => "<file>".to_string(),
                DefWithBody::Function(f) => format!("fn {}", fn_name(db, f)),
                DefWithBody::Const(c) => format!("const {}", const_name(db, c)),
                DefWithBody::Variant(_) => "<discriminant>".to_string(),
            },
            BodyOwner::Lambda { expr, .. } => format!("lambda#{}", expr.index()),
            BodyOwner::Shim(Shim::ImmDeep(t)) => format!("shim imm_deep[{}]", self.ty(t)),
            BodyOwner::Shim(Shim::Ctor(c)) => format!("shim {}", ctor_name(db, *c)),
        }
    }

    fn ty(&self, t: &Ty<'db>) -> String {
        t.display(self.db).to_string()
    }

    fn local_ty(&self, l: Local) -> String {
        self.body.local(l).map_or_else(|| "?".to_string(), |d| self.ty(&d.ty))
    }

    /// A local's name, with its number when another local has it too.
    fn local(&self, l: Local) -> String {
        let number = match l {
            Local::Binding(b) => format!("b{}", b.index()),
            Local::Temp(i) => format!("t{i}"),
        };
        let Some(name) = self.body.local(l).and_then(|d| d.name.as_ref()) else {
            return format!("_{number}");
        };
        let shared = self
            .body
            .bindings
            .iter()
            .map(|(_, d)| d)
            .chain(&self.body.temps)
            .filter(|d| d.name.as_ref() == Some(name))
            .count()
            > 1;
        if shared { format!("{name}#{number}") } else { name.to_string() }
    }

    fn captures(&self, captures: &[Capture]) -> String {
        let cs: Vec<String> = captures
            .iter()
            .map(|c| {
                let l = self.local(c.local);
                if c.by_ref { format!("ref {l}") } else { l }
            })
            .collect();
        cs.join(", ")
    }

    fn label(&self, l: LabelId) -> String {
        match &self.body[l].name {
            Some(name) => format!("'{name}"),
            None => format!("'l{}", l.0),
        }
    }

    fn labelled(&self, l: Option<LabelId>) -> String {
        l.map_or_else(String::new, |l| format!("{}: ", self.label(l)))
    }

    fn exprs(&self, es: &[ExprId], indent: usize) -> String {
        es.iter().map(|&e| self.expr(e, indent)).collect::<Vec<_>>().join(", ")
    }

    fn expr(&self, e: ExprId, indent: usize) -> String {
        let s = self.expr_kind(e, indent);
        if self.types { format!("({s} : {})", self.ty(&self.body[e].ty)) } else { s }
    }

    fn expr_kind(&self, e: ExprId, indent: usize) -> String {
        let db = self.db;
        let pad = INDENT.repeat(indent);
        let x = |e: ExprId| self.expr(e, indent);
        match &self.body[e].kind {
            ExprKind::Literal(lit) => literal(lit),
            ExprKind::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
            ExprKind::Local(l) => self.local(*l),
            ExprKind::Item(item) => self.item(item),
            ExprKind::Call { callee, args } => {
                let callee = match self.body[*callee].kind {
                    ExprKind::Item(_) | ExprKind::Local(_) => x(*callee),
                    _ => format!("({})", x(*callee)),
                };
                format!("{callee}({})", self.exprs(args, indent))
            }
            ExprKind::Construct { ctor, fields } => match ctor {
                Ctor::Tuple(1) => format!("({},)", self.exprs(fields, indent)),
                Ctor::Tuple(_) => format!("({})", self.exprs(fields, indent)),
                _ if fields.is_empty() => ctor_name(db, *ctor),
                _ => format!("{}({})", ctor_name(db, *ctor), self.exprs(fields, indent)),
            },
            ExprKind::Array { elems } => format!("[{}]", self.exprs(elems, indent)),
            ExprKind::ArrayRepeat { value, count } => format!("[{}; {}]", x(*value), x(*count)),
            ExprKind::Field { base, ctor, index } => {
                format!("{}.{}", x(*base), field(db, *ctor, *index))
            }
            ExprKind::Cast { expr } => format!("{} as {}", x(*expr), self.ty(&self.body[e].ty)),
            ExprKind::Closure { lambda, captures } => {
                let mut s = format!("lambda#{}", lambda.index());
                if !captures.is_empty() {
                    s.push_str(&format!(" capturing [{}]", self.captures(captures)));
                }
                s
            }
            ExprKind::Imm { expr } => format!("imm {}", x(*expr)),
            ExprKind::Block { stmts, tail, label } => {
                let inner = INDENT.repeat(indent + 1);
                let mut s = format!("{}{{\n", self.labelled(*label));
                for stmt in stmts.iter() {
                    s.push_str(&format!("{inner}{};\n", self.stmt(stmt, indent + 1)));
                }
                if let Some(t) = tail {
                    s.push_str(&format!("{inner}{}\n", self.expr(*t, indent + 1)));
                }
                s.push_str(&format!("{pad}}}"));
                s
            }
            ExprKind::If { cond, then, else_ } => {
                let mut s = format!("if {} {}", x(*cond), x(*then));
                if let Some(other) = else_ {
                    s.push_str(&format!(" else {}", x(*other)));
                }
                s
            }
            ExprKind::Logic { op, lhs, rhs } => {
                let op = match op {
                    LogicOp::And => "&&",
                    LogicOp::Or => "||",
                };
                format!("{} {op} {}", x(*lhs), x(*rhs))
            }
            ExprKind::Let { scrutinee, decision } => {
                format!("let {} = {}", self.decision(decision, indent), x(*scrutinee))
            }
            ExprKind::Match { scrutinee, decision, arms } => {
                let inner = INDENT.repeat(indent + 1);
                let mut s = format!("match {} {{\n", x(*scrutinee));
                for line in self.decision_lines(decision) {
                    s.push_str(&format!("{inner}{line}\n"));
                }
                for (i, arm) in arms.iter().enumerate() {
                    s.push_str(&format!("{inner}{}\n", self.arm(i, arm, None, indent + 1)));
                }
                s.push_str(&format!("{pad}}}"));
                s
            }
            // `handle body { .. }`: the value arms, then those of each
            // operation, after `effect`, with their continuations.
            ExprKind::Handle { body, value, arms, ops } => {
                let inner = INDENT.repeat(indent + 1);
                let inner2 = INDENT.repeat(indent + 2);
                let mut s = format!("handle {} {{\n", x(*body));
                for line in self.decision_lines(value) {
                    s.push_str(&format!("{inner}{line}\n"));
                }
                for (i, arm) in arms.iter().enumerate() {
                    s.push_str(&format!("{inner}{}\n", self.arm(i, arm, None, indent + 1)));
                }
                for op in ops.iter() {
                    s.push_str(&format!("{inner}effect {} {{\n", fn_name(db, op.op)));
                    for line in self.decision_lines(&op.decision) {
                        s.push_str(&format!("{inner2}{line}\n"));
                    }
                    for (i, (arm, k)) in op.arms.iter().zip(op.conts.iter()).enumerate() {
                        let k = Some(k.map_or_else(|| "_".to_string(), |k| self.local(k)));
                        s.push_str(&format!("{inner2}{}\n", self.arm(i, arm, k, indent + 2)));
                    }
                    s.push_str(&format!("{inner}}}\n"));
                }
                s.push_str(&format!("{pad}}}"));
                s
            }
            ExprKind::Loop { body, label } => format!("{}: loop {}", self.label(*label), x(*body)),
            ExprKind::While { cond, body, label } => {
                format!("{}: while {} {}", self.label(*label), x(*cond), x(*body))
            }
            ExprKind::For { pat, iterable, body, label, iter, next } => format!(
                "{}: for {} in {} with {}, {} {}",
                self.label(*label),
                self.pattern(pat, indent),
                x(*iterable),
                self.item(&iter.item),
                self.item(&next.item),
                x(*body)
            ),
            ExprKind::Break { label, value } => match value {
                Some(v) => format!("break {} {}", self.label(*label), x(*v)),
                None => format!("break {}", self.label(*label)),
            },
            ExprKind::Continue { label } => format!("continue {}", self.label(*label)),
            ExprKind::Return { value } => match value {
                Some(v) => format!("return {}", x(*v)),
                None => "return".to_string(),
            },
            ExprKind::Assign { place, op, value } => {
                let (place, via) = match place {
                    Place::Local(l) => (self.local(*l), String::new()),
                    Place::Field { base, ctor, index } => {
                        (format!("{}.{}", x(*base), field(db, *ctor, *index)), String::new())
                    }
                    Place::Index { base, index, get, set } => {
                        let via = match get {
                            Some(get) => {
                                format!(" with {}, {}", self.item(&get.item), self.item(&set.item))
                            }
                            None => format!(" with {}", self.item(&set.item)),
                        };
                        (format!("{}[{}]", x(*base), x(*index)), via)
                    }
                };
                match op {
                    // `n += 1`, with the function `+` calls if it's another.
                    Some(op) => {
                        let f = self.item(&op.item);
                        let symbol = f.strip_prefix('`').and_then(|f| f.strip_suffix('`'));
                        match symbol {
                            Some(symbol) => format!("{place} {symbol}= {}{via}", x(*value)),
                            None => format!("{place} op= {} with {f}{via}", x(*value)),
                        }
                    }
                    None => format!("{place} = {}{via}", x(*value)),
                }
            }
            ExprKind::Unsupported(what) => format!("unsupported({what:?})"),
        }
    }

    /// `arm i if guard, k => body`, with the continuation of an arm of an
    /// operation.
    fn arm(&self, i: usize, arm: &Arm, k: Option<String>, indent: usize) -> String {
        let guard = arm.guard.map_or_else(String::new, |g| format!(" if {}", self.expr(g, indent)));
        let k = k.map_or_else(String::new, |k| format!(", {k}"));
        format!("arm {i}{guard}{k} => {}", self.expr(arm.body, indent))
    }

    fn stmt(&self, stmt: &Stmt<'db>, indent: usize) -> String {
        match stmt {
            Stmt::Let { pat, init, else_ } => {
                let mut s = format!("let {}", self.pattern(pat, indent));
                if let Some(init) = init {
                    s.push_str(&format!(" = {}", self.expr(*init, indent)));
                }
                if let Some(other) = else_ {
                    s.push_str(&format!(" else {}", self.expr(*other, indent)));
                }
                s
            }
            Stmt::Expr(e) => self.expr(*e, indent),
        }
    }

    fn pattern(&self, pat: &Pattern<'db>, indent: usize) -> String {
        match pat {
            Pattern::Local(Some(l)) => match self.body.local(*l) {
                Some(d) if d.mutable => format!("mut {}", self.local(*l)),
                _ => self.local(*l),
            },
            Pattern::Local(None) => "_".to_string(),
            Pattern::Decision(d) => self.decision(d, indent),
        }
    }

    /// A decision outside a `match`, in braces.
    fn decision(&self, d: &Decision<'db>, indent: usize) -> String {
        let inner = INDENT.repeat(indent + 1);
        let mut s = "{\n".to_string();
        for line in self.decision_lines(d) {
            s.push_str(&format!("{inner}{line}\n"));
        }
        s.push_str(&format!("{}}}", INDENT.repeat(indent)));
        s
    }

    /// The lines of a decision, with its bindings by the names of their
    /// locals, like `with n = $1` rather than `with b1 = $1`.
    fn decision_lines(&self, d: &Decision<'db>) -> Vec<String> {
        d.display(self.db)
            .lines()
            .map(|line| {
                let mut line = line.to_string();
                for (b, _) in self.body.bindings.iter() {
                    let numbered = format!(" b{} = $", b.index());
                    if line.contains(&numbered) {
                        let named = format!(" {} = $", self.local(Local::Binding(b)));
                        line = line.replace(&numbered, &named);
                    }
                }
                line
            })
            .collect()
    }

    fn item(&self, item: &ItemRef<'db>) -> String {
        let db = self.db;
        let args = |args: &[Ty<'db>]| {
            if args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = args.iter().map(|t| self.ty(t)).collect();
                format!("[{}]", args.join(", "))
            }
        };
        match item {
            ItemRef::Fn(f) => fn_name(db, *f),
            ItemRef::Const(c) => const_name(db, *c),
            ItemRef::Method { item, class_args, .. } => {
                let (container, name) = match item {
                    ClassItem::Function(f) => {
                        (f.container(db), quote(f.item(db).name.as_ref().map(|n| n.name())))
                    }
                    ClassItem::Const(c) => (c.container(db), quote(c.item(db).name.as_ref())),
                };
                let class = match container {
                    ItemContainerId::Class(class) => name_or(class.item(db).name.as_ref()),
                    _ => "?".to_string(),
                };
                let class_args = class_args.as_deref().map_or_else(String::new, args);
                format!("{class}{class_args}.{name}")
            }
            ItemRef::Builtin(b) => format!("builtin.{}", b.name()),
            ItemRef::Extern(f) => format!("extern.{}", fn_name(db, *f)),
            ItemRef::Ctor(c) => ctor_name(db, *c),
            ItemRef::Op(f) => format!("perform.{}", fn_name(db, *f)),
        }
    }
}

fn literal(lit: &Literal) -> String {
    let suffix = |s: &Option<Name>| s.as_ref().map_or_else(String::new, |s| s.to_string());
    match lit {
        Literal::Int(n, s) => format!("{n}{}", suffix(s)),
        Literal::Float(text, s) => format!("{text}{}", suffix(s)),
        Literal::Char(c) => format!("{c:?}"),
        Literal::Byte(b) => format!("b'{}'", b.escape_ascii()),
        Literal::String(s) => format!("{s:?}"),
        Literal::ByteString(bs) => format!("b\"{}\"", bs.escape_ascii()),
        Literal::CString(s) => format!("c{s:?}"),
    }
}

fn name_or(name: Option<&Name>) -> String {
    name.map_or_else(|| "?".to_string(), |n| n.to_string())
}

/// A name, in backticks when it's an operator like `+`.
fn quote(name: Option<&Name>) -> String {
    let name = name_or(name);
    if name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '?') {
        name
    } else {
        format!("`{name}`")
    }
}

/// A function's name, after its class, impl or effect, like `Show.show`.
fn fn_name(db: &dyn Db, f: FunctionId<'_>) -> String {
    format!(
        "{}{}",
        container_prefix(db, f.container(db)),
        quote(f.item(db).display_name().as_ref())
    )
}

fn const_name(db: &dyn Db, c: ConstId<'_>) -> String {
    format!("{}{}", container_prefix(db, c.container(db)), quote(c.item(db).name.as_ref()))
}

/// `Show[i32].` for an item of an impl, `Show.` for one of a class, and
/// `State.` for an operation of an effect.
fn container_prefix(db: &dyn Db, container: ItemContainerId<'_>) -> String {
    match container {
        ItemContainerId::Impl(imp) => match &impl_header(db, imp).class {
            Some(c) => {
                let args: Vec<String> = c.args.iter().map(|t| t.display(db).to_string()).collect();
                format!("{}[{}].", name_or(c.class.item(db).name.as_ref()), args.join(", "))
            }
            None => "impl.".to_string(),
        },
        ItemContainerId::Class(class) => format!("{}.", name_or(class.item(db).name.as_ref())),
        ItemContainerId::Effect(effect) => match &effect.item(db).name {
            Some(name) => format!("{name}."),
            None => String::new(),
        },
        ItemContainerId::File(_) | ItemContainerId::ExternBlock(_) => String::new(),
    }
}

/// What made a value, as a constructor: `Point`, or `Option.Some`.
fn ctor_name(db: &dyn Db, ctor: Ctor<'_>) -> String {
    match ctor {
        Ctor::Tuple(_) => String::new(),
        Ctor::Struct(s) => name_or(s.item(db).name.as_ref()),
        Ctor::Variant(v) => format!("{}.{}", name_or(v.parent.item(db).name.as_ref()), v.name(db)),
    }
}

/// A field, as the decisions show it: `0` of a tuple, `Point.0` of a
/// struct, and `Some.0` of a variant.
fn field(db: &dyn Db, ctor: Ctor<'_>, index: u32) -> String {
    match ctor {
        Ctor::Tuple(_) => index.to_string(),
        Ctor::Struct(s) => format!("{}.{index}", name_or(s.item(db).name.as_ref())),
        Ctor::Variant(v) => format!("{}.{index}", v.name(db)),
    }
}
