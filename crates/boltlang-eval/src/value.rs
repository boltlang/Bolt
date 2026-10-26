use std::{cell::RefCell, cmp::Ordering, collections::HashMap, fmt, rc::Rc};

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    body::DefWithBodyId,
    hir::{BindingId, ExprId},
    ids::{EnumVariantId, FunctionId, StructId},
    item_tree::FunctionName,
};

/// The storage of a local variable. It's shared, so that closures see and
/// make changes to the variables they capture. `None` until it's assigned.
pub(crate) type Slot<'db> = Rc<RefCell<Option<Value<'db>>>>;

/// The local variables of a call. Binding IDs are unique within a body, so
/// one flat map per call is enough.
pub(crate) type Locals<'db> = HashMap<BindingId, Slot<'db>>;

/// A struct, or a variant of an enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ctor<'db> {
    Struct(StructId<'db>),
    Variant(EnumVariantId<'db>),
}

impl<'db> Ctor<'db> {
    pub fn name(self, db: &'db dyn Db) -> String {
        match self {
            Ctor::Struct(id) => id.item(db).name.as_ref().map_or("?".to_string(), Name::to_string),
            Ctor::Variant(id) => id.name(db).to_string(),
        }
    }
}

/// A closure, with the variables around it.
pub struct Closure<'db> {
    /// The body the closure is in.
    pub(crate) def: DefWithBodyId<'db>,
    /// Its [`Expr::Lambda`](boltlang_hir_def::hir::Expr::Lambda).
    pub(crate) expr: ExprId,
    pub(crate) captured: Locals<'db>,
}

impl fmt::Debug for Closure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closure").field("def", &self.def).field("expr", &self.expr).finish_non_exhaustive()
    }
}

/// The result of evaluating an expression.
#[derive(Debug, Clone)]
pub enum Value<'db> {
    Bool(bool),
    /// Any integer, whatever its type.
    Int(i128),
    Float(f64),
    Char(char),
    Str(Rc<str>),
    /// `(a, b)`, or `()`, which is what statements evaluate to.
    Tuple(Vec<Value<'db>>),
    Array(Vec<Value<'db>>),
    Range { start: Option<Box<Value<'db>>>, end: Option<Box<Value<'db>>>, inclusive: bool },
    /// A struct or variant with its fields, in order. The fields of tuple
    /// structs and variants are named `0`, `1`, ...
    Adt { ctor: Ctor<'db>, fields: Vec<(Name, Value<'db>)> },
    Function(FunctionId<'db>),
    /// A tuple struct or variant used as a function, like `Some`.
    Constructor(Ctor<'db>),
    Closure(Rc<Closure<'db>>),
}

impl<'db> Value<'db> {
    pub fn unit() -> Self {
        Value::Tuple(Vec::new())
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Value::Tuple(items) if items.is_empty())
    }

    pub fn str(s: &str) -> Self {
        Value::Str(s.into())
    }

    /// The field called `name`, of a struct or variant or tuple.
    pub fn field(&self, name: &Name) -> Option<&Value<'db>> {
        match self {
            Value::Adt { fields, .. } => fields.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            Value::Tuple(items) => items.get(name.as_str().parse::<usize>().ok()?),
            _ => None,
        }
    }

    pub(crate) fn field_mut(&mut self, name: &Name) -> Option<&mut Value<'db>> {
        match self {
            Value::Adt { fields, .. } => fields.iter_mut().find(|(n, _)| n == name).map(|(_, v)| v),
            Value::Tuple(items) => items.get_mut(name.as_str().parse::<usize>().ok()?),
            _ => None,
        }
    }

    /// Structural equality. Functions and closures are only equal to
    /// themselves.
    pub fn equals(&self, other: &Value<'db>) -> bool {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) | (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.equals(b))
            }
            (Value::Adt { ctor: c1, fields: f1 }, Value::Adt { ctor: c2, fields: f2 }) => {
                c1 == c2 && f1.len() == f2.len() && f1.iter().zip(f2).all(|((_, a), (_, b))| a.equals(b))
            }
            (
                Value::Range { start: s1, end: e1, inclusive: i1 },
                Value::Range { start: s2, end: e2, inclusive: i2 },
            ) => {
                let eq = |a: &Option<Box<Value<'db>>>, b: &Option<Box<Value<'db>>>| match (a, b) {
                    (Some(a), Some(b)) => a.equals(b),
                    (None, None) => true,
                    _ => false,
                };
                i1 == i2 && eq(s1, s2) && eq(e1, e2)
            }
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::Constructor(a), Value::Constructor(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// The order of numbers, characters, strings, and tuples and arrays of
    /// them. `None` for values that can't be ordered.
    pub fn compare(&self, other: &Value<'db>) -> Option<Ordering> {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Char(a), Value::Char(b)) => Some(a.cmp(b)),
            (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
            (Value::Tuple(a), Value::Tuple(b)) | (Value::Array(a), Value::Array(b)) => {
                for (a, b) in a.iter().zip(b) {
                    match a.compare(b)? {
                        Ordering::Equal => {}
                        ord => return Some(ord),
                    }
                }
                Some(a.len().cmp(&b.len()))
            }
            _ => None,
        }
    }

    /// Show the value like Bolt code, with the names of structs and
    /// variants from `db`.
    pub fn display(&self, db: &'db dyn Db) -> impl fmt::Display + '_ {
        DisplayValue { value: self, db }
    }
}

struct DisplayValue<'a, 'db> {
    value: &'a Value<'db>,
    db: &'db dyn Db,
}

impl<'db> fmt::Display for DisplayValue<'_, 'db> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db;
        let list = |f: &mut fmt::Formatter<'_>, items: &[Value<'db>]| -> fmt::Result {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}", item.display(db))?;
            }
            Ok(())
        };
        match self.value {
            Value::Bool(b) => f.write_str(if *b { "True" } else { "False" }),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x:?}"),
            Value::Char(c) => write!(f, "{c:?}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Tuple(items) => {
                f.write_str("(")?;
                list(f, items)?;
                if items.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            Value::Array(items) => {
                f.write_str("[")?;
                list(f, items)?;
                f.write_str("]")
            }
            Value::Range { start, end, inclusive } => {
                if let Some(start) = start {
                    write!(f, "{}", start.display(db))?;
                }
                f.write_str(if *inclusive { "..=" } else { ".." })?;
                if let Some(end) = end {
                    write!(f, "{}", end.display(db))?;
                }
                Ok(())
            }
            Value::Adt { ctor, fields } => {
                f.write_str(&ctor.name(db))?;
                if fields.is_empty() {
                    return Ok(());
                }
                let is_tuple = fields.iter().all(|(name, _)| name.as_str().parse::<usize>().is_ok());
                if is_tuple {
                    f.write_str("(")?;
                    let values: Vec<_> = fields.iter().map(|(_, v)| v.clone()).collect();
                    list(f, &values)?;
                    f.write_str(")")
                } else {
                    f.write_str(" { ")?;
                    for (i, (name, value)) in fields.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{name}: {}", value.display(db))?;
                    }
                    f.write_str(" }")
                }
            }
            Value::Function(id) => match &id.item(db).name {
                Some(FunctionName::Name(name)) => write!(f, "<fn {name}>"),
                Some(FunctionName::Operator(name)) => write!(f, "<fn ({name})>"),
                None => write!(f, "<fn>"),
            },
            Value::Constructor(ctor) => write!(f, "<constructor {}>", ctor.name(db)),
            Value::Closure(_) => write!(f, "<closure>"),
        }
    }
}
