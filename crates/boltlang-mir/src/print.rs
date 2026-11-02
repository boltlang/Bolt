//! Printing a body in the notation of #324, for tests and debugging.
//!
//! A temporary is `%0`, `%1`, ..., numbered in the order of the body's
//! table, and a variable is its name, with `#` and its index in the table
//! when another local has the same name. A label lowering made is `'l0`,
//! `'l1`, ... A struct's fields are shown by their index, and a variant
//! without fields by its name alone, like `Option.None`.

use std::fmt::{self, Write};

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    ids::{ClassId, EnumVariantId, FunctionId, StructId},
};
use boltlang_hir_ty::Ty;

use crate::{
    Block, Body, Callee, Constant, ConstructKind, LabelId, LocalId, Operand, Place, Rvalue,
    StmtKind, Switch, SwitchTest,
};

impl<'db> Body<'db> {
    /// A value that shows the body in the notation of #324.
    pub fn display<'a>(&'a self, db: &'db dyn Db) -> DisplayBody<'a, 'db> {
        DisplayBody { body: self, db }
    }
}

pub struct DisplayBody<'a, 'db> {
    body: &'a Body<'db>,
    db: &'db dyn Db,
}

impl fmt::Display for DisplayBody<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut p = Printer { body: self.body, db: self.db, out: String::new(), indent: 0 };
        p.body();
        f.write_str(&p.out)
    }
}

struct Printer<'a, 'db> {
    body: &'a Body<'db>,
    db: &'db dyn Db,
    out: String,
    indent: usize,
}

impl<'db> Printer<'_, 'db> {
    fn body(&mut self) {
        self.out.push_str("body");
        self.params(&self.body.params.clone());
        self.out.push(' ');
        self.block(&self.body.block.clone());
        self.out.push('\n');
    }

    fn params(&mut self, params: &[LocalId<'db>]) {
        self.out.push('(');
        for (i, &param) in params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.decl(param);
        }
        self.out.push(')');
    }

    /// A local with its flags and type, as it's introduced.
    fn decl(&mut self, local: LocalId<'db>) {
        let l = &self.body.locals[local];
        if l.cell {
            self.out.push_str("cell ");
        } else if l.mutable {
            self.out.push_str("mut ");
        }
        let name = self.local(local);
        let ty = self.ty(&l.ty);
        let _ = write!(self.out, "{name}: {ty}");
    }

    fn line(&mut self) {
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    /// `{ ... }`, or `block 'l { ... }` with a label. The value is left out
    /// when it's `()`.
    fn block(&mut self, block: &Block<'db>) {
        if let Some(label) = block.label {
            let _ = write!(self.out, "block {} ", self.label(label));
        }
        if block.stmts.is_empty() && block.value == Operand::unit() {
            self.out.push_str("{}");
            return;
        }
        self.out.push('{');
        self.indent += 1;
        for stmt in &block.stmts {
            self.line();
            self.stmt(&stmt.kind);
        }
        if block.value != Operand::unit() {
            self.line();
            let value = self.operand(&block.value);
            self.out.push_str(&value);
        }
        self.indent -= 1;
        self.line();
        self.out.push('}');
    }

    fn stmt(&mut self, stmt: &StmtKind<'db>) {
        match stmt {
            StmtKind::Let { local, value } => {
                self.out.push_str("let ");
                self.decl(*local);
                self.out.push_str(" = ");
                self.rvalue(value);
            }
            StmtKind::Assign { place, value } => {
                let place = self.place(place);
                let value = self.operand(value);
                let _ = write!(self.out, "{place} = {value}");
            }
            StmtKind::SetField { object, field, value } => {
                let object = self.operand(object);
                let value = self.operand(value);
                let _ = write!(self.out, "set {object}.{field} = {value}");
            }
            StmtKind::Eval(value) => self.rvalue(value),
            StmtKind::Break { label, value } => {
                let _ = write!(self.out, "break {}", self.label(*label));
                if let Some(value) = value {
                    let value = self.operand(value);
                    let _ = write!(self.out, " {value}");
                }
            }
            StmtKind::Continue { label } => {
                let _ = write!(self.out, "continue {}", self.label(*label));
            }
            StmtKind::Return(value) => {
                let value = self.operand(value);
                let _ = write!(self.out, "return {value}");
            }
        }
    }

    fn rvalue(&mut self, value: &Rvalue<'db>) {
        match value {
            Rvalue::Use(operand) => {
                let operand = self.operand(operand);
                self.out.push_str(&operand);
            }
            Rvalue::Call { callee, args } => {
                let callee = self.callee(callee);
                let args = self.operands(args);
                let _ = write!(self.out, "call {callee}({args})");
            }
            Rvalue::Construct { kind, args } => {
                let args = self.operands(args);
                let text = match kind {
                    ConstructKind::Tuple => format!("({args})"),
                    ConstructKind::Struct(id) => format!("{}({args})", self.struct_name(*id)),
                    ConstructKind::Variant(id) => with_args(self.variant_name(*id), &args),
                    ConstructKind::Array => format!("[{args}]"),
                    ConstructKind::ArrayRepeat => format!("[{}]", args.replacen(", ", "; ", 1)),
                };
                let _ = write!(self.out, "construct {text}");
            }
            Rvalue::Project { base, field } => {
                let base = self.operand(base);
                let _ = write!(self.out, "project {base}.{field}");
            }
            Rvalue::Imm(operand) => {
                let operand = self.operand(operand);
                let _ = write!(self.out, "imm {operand}");
            }
            Rvalue::Block(block) => self.block(block),
            Rvalue::If { cond, then_block, else_block } => {
                let cond = self.operand(cond);
                let _ = write!(self.out, "if {cond} ");
                self.block(then_block);
                self.out.push_str(" else ");
                self.block(else_block);
            }
            Rvalue::Loop { label, body } => {
                let _ = write!(self.out, "loop {} ", self.label(*label));
                self.block(body);
            }
            Rvalue::Switch(switch) => self.switch(switch),
            Rvalue::Closure { captures, params, body } => {
                let captures: Vec<String> = captures.iter().map(|&c| self.local(c)).collect();
                let _ = write!(self.out, "closure [{}] ", captures.join(", "));
                self.params(params);
                self.out.push(' ');
                self.block(body);
            }
        }
    }

    fn switch(&mut self, switch: &Switch<'db>) {
        let scrutinee = self.operand(&switch.scrutinee);
        let _ = write!(self.out, "switch {scrutinee} {{");
        self.indent += 1;
        for arm in &switch.arms {
            self.line();
            let test = match &arm.test {
                SwitchTest::Variant { variant, fields } => {
                    let fields: Vec<String> = fields
                        .iter()
                        .map(|f| f.map_or_else(|| "_".to_string(), |l| self.local(l)))
                        .collect();
                    with_args(self.variant_name(*variant), &fields.join(", "))
                }
                SwitchTest::Value(constant) => self.constant(constant),
            };
            let _ = write!(self.out, "{test} => ");
            self.block(&arm.body);
        }
        if let Some(default) = &switch.default {
            self.line();
            self.out.push_str("_ => ");
            self.block(default);
        }
        self.indent -= 1;
        self.line();
        self.out.push('}');
    }

    fn operands(&self, operands: &[Operand<'db>]) -> String {
        operands.iter().map(|o| self.operand(o)).collect::<Vec<_>>().join(", ")
    }

    fn operand(&self, operand: &Operand<'db>) -> String {
        match operand {
            Operand::Local(local) => self.local(*local),
            Operand::Const(constant) => self.constant(constant),
        }
    }

    fn constant(&self, constant: &Constant<'db>) -> String {
        match constant {
            Constant::Unit => "()".to_string(),
            Constant::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            Constant::Int { value, negative, ty } => {
                format!("{}{value}{}", if *negative { "-" } else { "" }, self.ty(ty))
            }
            Constant::Float { text, ty } => format!("{text}{}", self.ty(ty)),
            Constant::Char(c) => format!("{c:?}"),
            Constant::String(s) => format!("{s:?}"),
            Constant::Function { id, type_args } => {
                format!("{}{}", self.function_name(*id), self.ty_args(type_args))
            }
        }
    }

    fn callee(&self, callee: &Callee<'db>) -> String {
        match callee {
            Callee::Function { id, type_args } => {
                format!("{}{}", self.function_name(*id), self.ty_args(type_args))
            }
            Callee::ClassMethod { class, class_args, method } => {
                format!(
                    "{}{}.{}",
                    self.class_name(*class),
                    self.ty_args(class_args),
                    self.function_name(*method)
                )
            }
            Callee::Builtin(builtin) => builtin.name().to_string(),
            Callee::Value(operand) => self.operand(operand),
        }
    }

    fn place(&self, place: &Place<'db>) -> String {
        let mut text = self.local(place.local);
        for field in &place.fields {
            let _ = write!(text, ".{field}");
        }
        text
    }

    fn local(&self, local: LocalId<'db>) -> String {
        let locals = &self.body.locals;
        match &locals[local].name {
            Some(name) => {
                let shared =
                    locals.iter().any(|(other, l)| other != local && l.name.as_ref() == Some(name));
                if shared { format!("{name}#{}", local.index()) } else { name.to_string() }
            }
            None => {
                let n =
                    locals.iter().filter(|(other, l)| *other < local && l.name.is_none()).count();
                format!("%{n}")
            }
        }
    }

    fn label(&self, label: LabelId) -> String {
        match &self.body.labels[label].name {
            Some(name) => format!("'{name}"),
            None => format!("'l{}", label.index()),
        }
    }

    fn ty(&self, ty: &Ty<'db>) -> String {
        ty.display(self.db).to_string()
    }

    fn ty_args(&self, args: &[Ty<'db>]) -> String {
        if args.is_empty() {
            return String::new();
        }
        format!("[{}]", args.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", "))
    }

    fn function_name(&self, id: FunctionId<'db>) -> String {
        id.item(self.db).name.as_ref().map_or_else(|| "<fn>".to_string(), |n| n.name().to_string())
    }

    fn class_name(&self, id: ClassId<'db>) -> String {
        name_or(id.item(self.db).name.as_ref(), "<class>")
    }

    fn struct_name(&self, id: StructId<'db>) -> String {
        name_or(id.item(self.db).name.as_ref(), "<struct>")
    }

    fn variant_name(&self, id: EnumVariantId<'db>) -> String {
        format!("{}.{}", name_or(id.parent.item(self.db).name.as_ref(), "<enum>"), id.name(self.db))
    }
}

/// A variant with its fields, or alone when it has none, like `Option.None`.
fn with_args(name: String, args: &str) -> String {
    if args.is_empty() { name } else { format!("{name}({args})") }
}

fn name_or(name: Option<&Name>, missing: &str) -> String {
    name.map_or_else(|| missing.to_string(), |n| n.to_string())
}
