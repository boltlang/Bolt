use std::{cell::RefCell, cmp::Ordering, collections::HashMap, fmt, ops::Deref, rc::Rc};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::DefWithBodyId,
    hir::{BindingId, ExprId, Mutability},
    ids::{EnumVariantId, FunctionId, StructId},
    item_tree::FunctionName,
};
use boltlang_hir_def::resolver::GenericDefId;
use boltlang_hir_ty::{
    AdtId, Ty,
    lang::{char_struct, string_struct},
    lower::{VariantDef, field_types, struct_args, subst_params},
};

/// The types that a function's scheme is used with, for its parameters and
/// then its bound variables: see [`boltlang_hir_ty::InferenceResult::type_args`].
/// They choose the impls of the class methods it uses (#236). Empty when
/// they aren't known, as for a function called from outside.
pub type TypeArgs<'db> = Rc<[Ty<'db>]>;

/// The storage of a local variable. It's shared, so that closures see and
/// make changes to the variables they capture. `None` until it's assigned.
pub(crate) type Slot<'db> = Rc<RefCell<Option<Value<'db>>>>;

/// The local variables of a call. Binding IDs are unique within a body, so
/// one flat map per call is enough.
pub(crate) type Locals<'db> = HashMap<BindingId, Slot<'db>>;

/// Something that every name for it sees the changes of, like an array or
/// a struct.
pub type Shared<T> = Rc<Object<T>>;

/// An array or struct: what it holds, and whether it was made `mut` or
/// `imm`. The type checker makes sure an `imm` one never changes; with
/// [`Evaluator::with_mutability_checks`](crate::Evaluator::with_mutability_checks),
/// the evaluator checks it too. It derefs to its contents.
#[derive(Debug)]
pub struct Object<T> {
    pub mutability: Mutability,
    contents: RefCell<T>,
}

impl<T> Object<T> {
    pub fn new(mutability: Mutability, contents: T) -> Rc<Self> {
        Rc::new(Object { mutability, contents: RefCell::new(contents) })
    }
}

impl<T> Deref for Object<T> {
    type Target = RefCell<T>;

    fn deref(&self) -> &RefCell<T> {
        &self.contents
    }
}

/// The fields of a struct or variant, in order.
pub type Fields<'db> = Vec<(Name, Value<'db>)>;

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
    /// The types the body it's in is run with.
    pub(crate) type_args: TypeArgs<'db>,
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
    /// `(a, b)`, or `()`, which is what statements evaluate to.
    Tuple(Vec<Value<'db>>),
    /// Shared: `let ys = xs` makes another name for the same array.
    Array(Shared<Vec<Value<'db>>>),
    Range { start: Option<Box<Value<'db>>>, end: Option<Box<Value<'db>>>, inclusive: bool },
    /// A struct or variant with its fields, in order. The fields of tuple
    /// structs and variants are named `0`, `1`, ... Shared, like an array.
    /// A variant can't be changed, so it being shared makes no difference.
    Adt { ctor: Ctor<'db>, fields: Shared<Fields<'db>> },
    /// A function, with the types it's used with.
    Function(FunctionId<'db>, TypeArgs<'db>),
    /// A tuple struct or variant used as a function, like `Some`.
    Constructor(Ctor<'db>),
    Closure(Rc<Closure<'db>>),
    /// An element of an array from `array_alloc` that isn't set yet. Only
    /// arrays hold it, and reading it is an error.
    Unset,
}

impl<'db> Value<'db> {
    pub fn unit() -> Self {
        Value::Tuple(Vec::new())
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Value::Tuple(items) if items.is_empty())
    }

    /// `text` as the prelude's `String`, as `file` sees it: a struct that
    /// holds an `imm` array of its bytes in UTF-8. `None` without one.
    pub fn string(db: &'db dyn Db, file: File, text: &str) -> Option<Self> {
        let id = string_struct(db, file)?;
        let bytes = text.bytes().map(|b| Value::Int(b as i128)).collect();
        let fields = vec![(Name::new("0"), Value::array(Mutability::Imm, bytes))];
        Some(Value::Adt { ctor: Ctor::Struct(id), fields: Object::new(Mutability::Imm, fields) })
    }

    /// `c` as the prelude's `Char`, as `file` sees it: a struct that holds
    /// its code point. `None` without one.
    pub fn char(db: &'db dyn Db, file: File, c: char) -> Option<Self> {
        let id = char_struct(db, file)?;
        let fields = vec![(Name::new("0"), Value::Int(c as i128))];
        Some(Value::Adt { ctor: Ctor::Struct(id), fields: Object::new(Mutability::Imm, fields) })
    }

    /// The text of a `String`, if this is one: its array of bytes, decoded
    /// as UTF-8.
    pub fn text(&self) -> Option<String> {
        let Value::Adt { fields, .. } = self else { return None };
        let fields = fields.borrow();
        let [(_, Value::Array(items))] = &fields[..] else { return None };
        let bytes = items
            .borrow()
            .iter()
            .map(|v| match v {
                Value::Int(b) => u8::try_from(*b).ok(),
                _ => None,
            })
            .collect::<Option<Vec<u8>>>()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// A new array, made `mut` or `imm`.
    pub fn array(mutability: Mutability, items: Vec<Value<'db>>) -> Self {
        Value::Array(Object::new(mutability, items))
    }

    /// A new struct, made `mut` or `imm`, or a variant, which can't be
    /// changed: it's `imm`.
    pub fn adt(ctor: Ctor<'db>, mutability: Mutability, fields: Fields<'db>) -> Self {
        let mutability = match ctor {
            Ctor::Struct(_) => mutability,
            Ctor::Variant(_) => Mutability::Imm,
        };
        Value::Adt { ctor, fields: Object::new(mutability, fields) }
    }

    /// Whether nothing that can be changed is reachable from this value
    /// without going through an `imm` array or struct, which only hold such
    /// values themselves. A variant or tuple is looked through, since they
    /// can hold `mut` things. Closures can't be checked yet (#244).
    pub(crate) fn holds_only_imm(&self) -> bool {
        match self {
            Value::Array(items) => items.mutability == Mutability::Imm,
            Value::Adt { ctor: Ctor::Struct(_), fields } => fields.mutability == Mutability::Imm,
            Value::Adt { ctor: Ctor::Variant(_), fields } => fields.borrow().iter().all(|(_, v)| v.holds_only_imm()),
            Value::Tuple(items) => items.iter().all(Value::holds_only_imm),
            Value::Range { start, end, .. } => {
                start.iter().chain(end.iter()).all(|v| v.holds_only_imm())
            }
            _ => true,
        }
    }

    /// `imm e` of this value: each `mut` array and struct it reaches is
    /// copied into an `imm` one, once, so that what it shares stays shared
    /// and a cycle stays a cycle. What's `imm` is shared as it is, since it
    /// holds only what's `imm`. A closure is kept: the type checker makes
    /// sure it's `imm`.
    pub fn freeze(&self) -> Value<'db> {
        self.freeze_with(&mut HashMap::new())
    }

    /// [`Value::freeze`], with the copies made so far, by the address of
    /// what they copy.
    fn freeze_with(&self, copies: &mut HashMap<*const (), Value<'db>>) -> Value<'db> {
        match self {
            Value::Array(items) if items.mutability == Mutability::Imm => self.clone(),
            Value::Array(items) => {
                let key = Rc::as_ptr(items) as *const ();
                if let Some(copy) = copies.get(&key) {
                    return copy.clone();
                }
                let copy = Object::new(Mutability::Imm, Vec::new());
                copies.insert(key, Value::Array(copy.clone()));
                let frozen = items.borrow().iter().map(|v| v.freeze_with(copies)).collect();
                *copy.borrow_mut() = frozen;
                Value::Array(copy)
            }
            Value::Adt { ctor: Ctor::Struct(_), fields }
                if fields.mutability == Mutability::Imm =>
            {
                self.clone()
            }
            Value::Adt { ctor: Ctor::Variant(_), .. } if self.holds_only_imm() => self.clone(),
            Value::Adt { ctor, fields } => {
                let key = Rc::as_ptr(fields) as *const ();
                if let Some(copy) = copies.get(&key) {
                    return copy.clone();
                }
                let copy = Object::new(Mutability::Imm, Vec::new());
                copies.insert(key, Value::Adt { ctor: *ctor, fields: copy.clone() });
                let frozen = fields
                    .borrow()
                    .iter()
                    .map(|(n, v)| (n.clone(), v.freeze_with(copies)))
                    .collect();
                *copy.borrow_mut() = frozen;
                Value::Adt { ctor: *ctor, fields: copy }
            }
            Value::Tuple(items) => {
                Value::Tuple(items.iter().map(|v| v.freeze_with(copies)).collect())
            }
            Value::Range { start, end, inclusive } => Value::Range {
                start: start.as_ref().map(|v| Box::new(v.freeze_with(copies))),
                end: end.as_ref().map(|v| Box::new(v.freeze_with(copies))),
                inclusive: *inclusive,
            },
            _ => self.clone(),
        }
    }

    /// `mut e` of this value, which has the type `from`, giving the type
    /// `to`: what the types say is `imm`, or of a mutability the code doesn't
    /// know, is copied into new `mut` objects, once each, and what they say
    /// is `mut` is kept as it is. The two types differ exactly where a copy
    /// is made, so they're compared rather than the objects, which keeps a
    /// generic function's `mut xs` a copy even when it's given a `mut` one.
    pub fn thaw(&self, db: &'db dyn Db, from: &Ty<'db>, to: &Ty<'db>) -> Value<'db> {
        self.thaw_with(db, from, to, &mut HashMap::new())
    }

    fn thaw_with(
        &self,
        db: &'db dyn Db,
        from: &Ty<'db>,
        to: &Ty<'db>,
        copies: &mut HashMap<*const (), Value<'db>>,
    ) -> Value<'db> {
        if from == to {
            return self.clone();
        }
        match (self, from, to) {
            (Value::Array(items), Ty::Array(_, from_elem), Ty::Array(_, to_elem)) => {
                let key = Rc::as_ptr(items) as *const ();
                if let Some(copy) = copies.get(&key) {
                    return copy.clone();
                }
                let copy = Object::new(Mutability::Mut, Vec::new());
                copies.insert(key, Value::Array(copy.clone()));
                let thawed = items.borrow().iter().map(|v| v.thaw_with(db, from_elem, to_elem, copies)).collect();
                *copy.borrow_mut() = thawed;
                Value::Array(copy)
            }
            (Value::Adt { ctor, fields }, Ty::Adt(id, ..), Ty::Adt(..)) => {
                let def = match *ctor {
                    Ctor::Struct(s) => VariantDef::Struct(s),
                    Ctor::Variant(v) => VariantDef::Variant(v),
                };
                let generic_def = match id {
                    AdtId::Struct(s) => GenericDefId::Struct(*s),
                    AdtId::Enum(e) => GenericDefId::Enum(*e),
                };
                let key = Rc::as_ptr(fields) as *const ();
                if let Some(copy) = copies.get(&key) {
                    return copy.clone();
                }
                let mutability = match ctor {
                    Ctor::Struct(_) => Mutability::Mut,
                    Ctor::Variant(_) => Mutability::Imm,
                };
                let copy = Object::new(mutability, Vec::new());
                copies.insert(key, Value::Adt { ctor: *ctor, fields: copy.clone() });
                let (from_args, to_args) = (adt_subst(from), adt_subst(to));
                let field_tys = field_types(db, def);
                let thawed = fields
                    .borrow()
                    .iter()
                    .map(|(name, v)| {
                        let v = match field_tys.field(name) {
                            Some((_, t)) => v.thaw_with(
                                db,
                                &subst_params(t, generic_def, &from_args),
                                &subst_params(t, generic_def, &to_args),
                                copies,
                            ),
                            None => v.clone(),
                        };
                        (name.clone(), v)
                    })
                    .collect();
                *copy.borrow_mut() = thawed;
                Value::Adt { ctor: *ctor, fields: copy }
            }
            (Value::Tuple(items), Ty::Tuple(from_tys), Ty::Tuple(to_tys)) => Value::Tuple(
                items.iter().zip(from_tys.iter().zip(to_tys.iter())).map(|(v, (f, t))| v.thaw_with(db, f, t, copies)).collect(),
            ),
            _ => self.clone(),
        }
    }

    /// The field called `name`, of a struct or variant or tuple.
    pub fn field(&self, name: &Name) -> Option<Value<'db>> {
        match self {
            Value::Adt { fields, .. } => fields.borrow().iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()),
            Value::Tuple(items) => items.get(name.as_str().parse::<usize>().ok()?).cloned(),
            _ => None,
        }
    }

    /// Structural equality. Functions and closures are only equal to
    /// themselves.
    pub fn equals(&self, other: &Value<'db>) -> bool {
        match (self, other) {
            (Value::Array(a), Value::Array(b)) if Rc::ptr_eq(a, b) => true,
            (Value::Adt { fields: a, .. }, Value::Adt { fields: b, .. }) if Rc::ptr_eq(a, b) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => equal_items(a, b),
            (Value::Array(a), Value::Array(b)) => equal_items(&a.borrow(), &b.borrow()),
            (Value::Adt { ctor: c1, fields: f1 }, Value::Adt { ctor: c2, fields: f2 }) => {
                let (f1, f2) = (f1.borrow(), f2.borrow());
                c1 == c2 && f1.len() == f2.len() && f1.iter().zip(f2.iter()).all(|((_, a), (_, b))| a.equals(b))
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
            (Value::Function(a, _), Value::Function(b, _)) => a == b,
            (Value::Constructor(a), Value::Constructor(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// The order of numbers, and of tuples, arrays and structs of them,
    /// field by field: a `Char` is ordered by its code point, for range
    /// patterns like `'a'..='z'`. `None` for values that can't be ordered.
    pub fn compare(&self, other: &Value<'db>) -> Option<Ordering> {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Tuple(a), Value::Tuple(b)) => compare_items(a, b),
            (Value::Array(a), Value::Array(b)) => compare_items(&a.borrow(), &b.borrow()),
            (Value::Adt { ctor: c1 @ Ctor::Struct(_), fields: f1 }, Value::Adt { ctor: c2, fields: f2 }) if c1 == c2 => {
                let values = |f: &Fields<'db>| f.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
                compare_items(&values(&f1.borrow()), &values(&f2.borrow()))
            }
            _ => None,
        }
    }

    /// Show the value like Bolt code, with the names of structs and
    /// variants from `db`. It doesn't run Bolt code, so it doesn't use the
    /// impls of `Debug`: [`Evaluator::show`](crate::Evaluator::show) does.
    pub fn display(&self, db: &'db dyn Db) -> impl fmt::Display + '_ {
        DisplayValue { value: self, db, outer: Rc::new(RefCell::new(Vec::new())), shown: None }
    }

    /// [`Value::display`], but showing the structs and variants in `shown`,
    /// by the address of their fields, as the text there.
    pub(crate) fn display_with<'a>(&'a self, db: &'db dyn Db, shown: &'a Shown) -> impl fmt::Display + 'a
    where
        'db: 'a,
    {
        DisplayValue { value: self, db, outer: Rc::new(RefCell::new(Vec::new())), shown: Some(shown) }
    }

    /// The address of the shared array or struct, if it's one.
    pub(crate) fn shared_ptr(&self) -> Option<*const ()> {
        match self {
            Value::Array(items) => Some(Rc::as_ptr(items) as *const ()),
            Value::Adt { fields, .. } => Some(Rc::as_ptr(fields) as *const ()),
            _ => None,
        }
    }
}

fn equal_items<'db>(a: &[Value<'db>], b: &[Value<'db>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.equals(b))
}

fn compare_items<'db>(a: &[Value<'db>], b: &[Value<'db>]) -> Option<Ordering> {
    for (a, b) in a.iter().zip(b) {
        match a.compare(b)? {
            Ordering::Equal => {}
            ord => return Some(ord),
        }
    }
    Some(a.len().cmp(&b.len()))
}

/// How structs and variants are shown instead, by the address of their
/// fields, like a `Char` as `'a'`: what the impls of `Debug` give.
pub(crate) type Shown = HashMap<*const (), String>;

struct DisplayValue<'a, 'db> {
    value: &'a Value<'db>,
    db: &'db dyn Db,
    shown: Option<&'a Shown>,
    /// The arrays and structs being shown, outermost first. A value can
    /// contain itself, like `xs[0] = S { xs }`, and is shown as `...` there.
    outer: Rc<RefCell<Vec<*const ()>>>,
}

impl<'a, 'db> DisplayValue<'a, 'db> {
    fn inner<'b>(&self, value: &'b Value<'db>) -> DisplayValue<'b, 'db>
    where
        'a: 'b,
    {
        DisplayValue { value, db: self.db, outer: self.outer.clone(), shown: self.shown }
    }
}

impl<'db> fmt::Display for DisplayValue<'_, 'db> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.value.shared_ptr();
        if let Some(text) = ptr.zip(self.shown).and_then(|(ptr, shown)| shown.get(&ptr)) {
            return f.write_str(text);
        }
        if let Some(ptr) = ptr {
            if self.outer.borrow().contains(&ptr) {
                return f.write_str("...");
            }
            self.outer.borrow_mut().push(ptr);
        }
        let result = self.fmt_value(f);
        if ptr.is_some() {
            self.outer.borrow_mut().pop();
        }
        result
    }
}

impl<'db> DisplayValue<'_, 'db> {
    fn fmt_value(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db;
        let list = |f: &mut fmt::Formatter<'_>, items: &[Value<'db>]| -> fmt::Result {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}", self.inner(item))?;
            }
            Ok(())
        };
        match self.value {
            Value::Bool(b) => f.write_str(if *b { "True" } else { "False" }),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x:?}"),
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
                list(f, &items.borrow())?;
                f.write_str("]")
            }
            Value::Range { start, end, inclusive } => {
                if let Some(start) = start {
                    write!(f, "{}", self.inner(start))?;
                }
                f.write_str(if *inclusive { "..=" } else { ".." })?;
                if let Some(end) = end {
                    write!(f, "{}", self.inner(end))?;
                }
                Ok(())
            }
            // The prelude's `String`, as its text, for a prelude without an
            // impl of `Debug` for it, like the small one of the tests.
            Value::Adt { ctor: Ctor::Struct(id), .. } if string_struct(db, id.file(db)) == Some(*id) => {
                match self.value.text() {
                    Some(text) => write!(f, "{text:?}"),
                    None => f.write_str("String(?)"),
                }
            }
            Value::Adt { ctor, fields } => {
                let fields = fields.borrow();
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
                        write!(f, "{name}: {}", self.inner(value))?;
                    }
                    f.write_str(" }")
                }
            }
            Value::Function(id, _) => match &id.item(db).name {
                Some(FunctionName::Name(name)) => write!(f, "<fn {name}>"),
                Some(FunctionName::Operator(name)) => write!(f, "<fn ({name})>"),
                None => write!(f, "<fn>"),
            },
            Value::Constructor(ctor) => write!(f, "<constructor {}>", ctor.name(db)),
            Value::Closure(_) => write!(f, "<closure>"),
            Value::Unset => write!(f, "_"),
        }
    }
}

/// What to substitute in the field types of a struct or enum of type `ty`:
/// its type arguments, and for a struct its mutability arguments and its
/// mutability.
fn adt_subst<'db>(ty: &Ty<'db>) -> Vec<Ty<'db>> {
    match ty {
        Ty::Adt(AdtId::Struct(_), args, mutabilities, mutability) => struct_args(args, mutabilities, mutability),
        Ty::Adt(AdtId::Enum(_), args, ..) => args.to_vec(),
        _ => Vec::new(),
    }
}
