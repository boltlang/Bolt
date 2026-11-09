//! Compiling patterns to decisions: which tests to make, in which order, to
//! find the arm of a `match` that a value takes.
//!
//! A [`Decision`] is a graph in an arena, not a tree: a node refers to
//! others by index, so the same rest of a decision, reached along several
//! paths, is one node, and arm bodies are never copied. An [`Node::Arm`]
//! only names the arm by its index; a backend makes each arm a join point
//! and jumps to it, and makes a join point for a node with several
//! predecessors too. Building and walking it never recurse on the Rust stack
//! per pattern.
//!
//! The graph is built from a pattern matrix, as in Maranget's "Compiling
//! Pattern Matching to Good Decision Trees", testing the first place that
//! the first row tests. Matrices that are the same are the same node.
//!
//! Which tests are needed depends only on the patterns, not on an
//! instance's types: a pattern can't test what a type parameter is. So a
//! body's decisions are made once ([`body_decisions`]), for every instance
//! and backend.

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    body::{Body, DefWithBodyId, body},
    builtin_type::BuiltinValue,
    hir::{BindingId, Expr, ExprId, Literal, Pat, PatId, Path, Statement},
    ids::{ConstId, EnumVariantId, FunctionId, StructId},
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
};
use boltlang_hir_ty::{
    Ty, infer_body,
    lower::{VariantDef, field_types, fn_sig},
};

pub type PlaceId = u32;
pub type NodeId = u32;

/// A part of the value being matched, as a path from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Place<'db> {
    /// The value itself.
    Scrutinee,
    /// Field `index` of the place `of`, which is a value made by `ctor`.
    /// For a variant, it's only valid where a [`Node::Switch`] on `of` took
    /// that variant.
    Field { of: PlaceId, ctor: Ctor<'db>, index: u32 },
}

/// What made a value whose fields are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Ctor<'db> {
    /// A tuple with this many elements.
    Tuple(u32),
    Struct(StructId<'db>),
    Variant(EnumVariantId<'db>),
}

/// What a [`Node::Switch`] branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Case<'db> {
    Variant(EnumVariantId<'db>),
    Bool(bool),
    /// An integer, with its sign.
    Int(i128),
    Char(char),
}

/// What a [`Node::Equals`] compares with: values that can't be switched on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Value<'db> {
    /// A string, compared with `String`'s `==`.
    Str(String),
    /// A float, as the text of its literal, with its sign.
    Float(String),
    /// A constant's value, compared with its type's `==`.
    Const(ConstId<'db>),
}

/// An end of a [`Node::Range`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Bound<'db> {
    Int(i128),
    Char(char),
    Float(String),
    Const(ConstId<'db>),
}

/// A test, or what's decided.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Node<'db> {
    /// Branch on what `place` is: an enum's variant, a bool, an integer or a
    /// char. `default` is taken when no case is, and is `None` when the
    /// cases cover every value.
    Switch { place: PlaceId, cases: Vec<(Case<'db>, NodeId)>, default: Option<NodeId> },
    /// Whether `place` equals `value`.
    Equals { place: PlaceId, value: Value<'db>, then: NodeId, otherwise: NodeId },
    /// Whether `place` is in a range. A missing end is open.
    Range {
        place: PlaceId,
        start: Option<Bound<'db>>,
        end: Option<Bound<'db>>,
        inclusive: bool,
        then: NodeId,
        otherwise: NodeId,
    },
    /// Run the guard of `arm`, with its bindings bound. `otherwise` tries
    /// the arms after it.
    Guard {
        arm: u32,
        guard: ExprId,
        bindings: Vec<(BindingId, PlaceId)>,
        then: NodeId,
        otherwise: NodeId,
    },
    /// This arm matches, with its bindings bound to these places.
    Arm { arm: u32, bindings: Vec<(BindingId, PlaceId)> },
    /// No arm matches: an error at run time.
    Fail,
}

/// The decision for a list of arms.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Decision<'db> {
    pub places: Vec<Place<'db>>,
    pub nodes: Vec<Node<'db>>,
    pub root: NodeId,
    /// How many arms there are.
    pub arms: u32,
}

/// Why a decision can't be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub enum DecisionError {
    /// A pattern that isn't supported yet, like `[a, .., b]`.
    Unsupported(PatId),
    /// A pattern whose path doesn't resolve, or whose tuple's length isn't
    /// known, after an error the checker reports.
    Unknown(PatId),
}

/// The decisions of a body: its `match`es, by the `match`, and its `let`s
/// with patterns, in statements and conditions, and the patterns of its
/// `for`s, by the pattern.
#[derive(Debug, Clone, Default, PartialEq, Eq, salsa::SalsaValue)]
pub struct BodyDecisions<'db> {
    pub matches: Vec<(ExprId, Result<Decision<'db>, DecisionError>)>,
    pub lets: Vec<(PatId, Result<Decision<'db>, DecisionError>)>,
    /// The decisions of the effect arms of the `match`es, one for each
    /// operation they handle, over its arguments.
    pub operations: Vec<OperationDecision<'db>>,
}

/// The decision of the arms of a `match` for one operation, like those for
/// `yield` of `effect yield(0), k` and `effect yield(n), k`: its arguments
/// are matched as a tuple.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct OperationDecision<'db> {
    pub expr: ExprId,
    pub op: FunctionId<'db>,
    /// The operation's patterns in the arms, like `yield(0)`, in order.
    pub pats: Vec<PatId>,
    pub decision: Result<Decision<'db>, DecisionError>,
}

/// The decisions of the body `def`.
#[salsa::tracked(returns(ref))]
pub fn body_decisions<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> BodyDecisions<'db> {
    let body = body(db, def);
    let mut out = BodyDecisions::default();
    for (id, expr) in body.exprs.iter() {
        match expr {
            Expr::Match { arms, effect_arms, .. } => {
                let arms: Vec<(PatId, Option<ExprId>)> =
                    arms.iter().map(|a| (a.pat, a.guard)).collect();
                out.matches.push((id, decide(db, def, body, &arms)));
                // The arms of each operation, in order. An arm for several
                // operations is one for each.
                let operations = &infer_body(db, def).operations;
                let mut groups: Vec<(FunctionId<'db>, Vec<(PatId, Option<ExprId>)>)> = Vec::new();
                for arm in effect_arms.iter() {
                    for &op in arm.ops.iter() {
                        let Some(&f) = operations.get(op) else { continue };
                        match groups.iter_mut().find(|(g, _)| *g == f) {
                            Some((_, arms)) => arms.push((op, arm.guard)),
                            None => groups.push((f, vec![(op, arm.guard)])),
                        }
                    }
                }
                for (op, arms) in groups {
                    let arity = fn_sig(db, op).params.len();
                    out.operations.push(OperationDecision {
                        expr: id,
                        op,
                        pats: arms.iter().map(|(p, _)| *p).collect(),
                        decision: decide_args(db, def, body, arity, &arms),
                    });
                }
            }
            Expr::Let { pat, .. } | Expr::For { pat, .. } => {
                out.lets.push((*pat, decide(db, def, body, &[(*pat, None)])))
            }
            Expr::Block { statements, .. } => {
                for statement in statements.iter() {
                    if let Statement::Let { pat, initializer: Some(_), .. } = statement {
                        out.lets.push((*pat, decide(db, def, body, &[(*pat, None)])));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The decision for `arms`, each a pattern and a guard, in the body `def`.
/// The parameters of a function and the pattern of a `for` are one arm.
pub fn decide<'db>(
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    body: &Body,
    arms: &[(PatId, Option<ExprId>)],
) -> Result<Decision<'db>, DecisionError> {
    let mut b = Builder {
        db,
        def,
        body,
        arms,
        places: vec![Place::Scrutinee],
        place_ids: HashMap::new(),
        nodes: Vec::new(),
        memo: HashMap::new(),
    };
    let mut rows = Vec::new();
    for (arm, (pat, _)) in arms.iter().enumerate() {
        let row = Row { cols: vec![(0, *pat)], arm: arm as u32, bindings: Vec::new() };
        rows.extend(b.expand(row)?);
    }
    let root = b.compile(rows)?;
    Ok(Decision { places: b.places, nodes: b.nodes, root, arms: arms.len() as u32 })
}

/// The decision for the arms of an operation, each a pattern like
/// `yield(x)` and a guard, over the `arity` arguments of the operation: the
/// scrutinee is the tuple of the arguments.
pub fn decide_args<'db>(
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    body: &Body,
    arity: usize,
    arms: &[(PatId, Option<ExprId>)],
) -> Result<Decision<'db>, DecisionError> {
    let mut b = Builder {
        db,
        def,
        body,
        arms,
        places: vec![Place::Scrutinee],
        place_ids: HashMap::new(),
        nodes: Vec::new(),
        memo: HashMap::new(),
    };
    let ctor = Ctor::Tuple(arity as u32);
    let mut rows = Vec::new();
    for (arm, (op, _)) in arms.iter().enumerate() {
        let Pat::TupleStruct { args, rest, .. } = &body[*op] else {
            return Err(DecisionError::Unknown(*op));
        };
        // The checker reports the wrong number of arguments.
        let fits = match rest {
            None => args.len() == arity,
            Some(_) => args.len() <= arity,
        };
        if !fits {
            return Err(DecisionError::Unknown(*op));
        }
        let cols = positional(args, *rest, arity)
            .into_iter()
            .map(|(index, pat)| (b.place(Place::Field { of: 0, ctor, index }), pat))
            .collect();
        rows.extend(b.expand(Row { cols, arm: arm as u32, bindings: Vec::new() })?);
    }
    let root = b.compile(rows)?;
    Ok(Decision { places: b.places, nodes: b.nodes, root, arms: arms.len() as u32 })
}

/// A row of the matrix: what's left to test for an arm, as patterns at
/// places, and what it has bound so far.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Row {
    cols: Vec<(PlaceId, PatId)>,
    arm: u32,
    bindings: Vec<(BindingId, PlaceId)>,
}

/// What testing a pattern at a place is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Test<'db> {
    /// A case of a switch, with the patterns of its fields, if a variant.
    Case(Case<'db>, Vec<PatId>),
    Equals(Value<'db>),
    Range(Option<Bound<'db>>, Option<Bound<'db>>, bool),
}

struct Builder<'a, 'db> {
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    body: &'a Body,
    arms: &'a [(PatId, Option<ExprId>)],
    places: Vec<Place<'db>>,
    place_ids: HashMap<Place<'db>, PlaceId>,
    nodes: Vec<Node<'db>>,
    memo: HashMap<Vec<Row>, NodeId>,
}

impl<'db> Builder<'_, 'db> {
    fn place(&mut self, place: Place<'db>) -> PlaceId {
        if let Some(&id) = self.place_ids.get(&place) {
            return id;
        }
        let id = self.places.len() as PlaceId;
        self.places.push(place);
        self.place_ids.insert(place, id);
        id
    }

    fn node(&mut self, node: Node<'db>) -> NodeId {
        // Equal nodes are one, so that equal rests are shared.
        if let Some(i) = self.nodes.iter().position(|n| *n == node) {
            return i as NodeId;
        }
        self.nodes.push(node);
        (self.nodes.len() - 1) as NodeId
    }

    /// The row with the patterns that test nothing taken out: wildcards
    /// and bindings, and tuples and structs, whose fields are tested
    /// instead. An or-pattern makes a row for each of its alternatives.
    fn expand(&mut self, row: Row) -> Result<Vec<Row>, DecisionError> {
        let mut done = Vec::new();
        let mut todo = vec![row];
        // Rows are expanded in order, so that an or-pattern's alternatives
        // are tried in order.
        while let Some(mut row) = todo.pop() {
            let Some(i) = row.cols.iter().position(|&(_, p)| !self.is_test(p)) else {
                done.push(row);
                continue;
            };
            let (place, pat) = row.cols.remove(i);
            match &self.body[pat] {
                Pat::Wild | Pat::Missing => {}
                Pat::Bind { id, subpat } => {
                    row.bindings.push((*id, place));
                    if let Some(sub) = subpat {
                        row.cols.insert(i, (place, *sub));
                    }
                }
                Pat::Or(alts) => {
                    for alt in alts.iter().rev() {
                        let mut r = row.clone();
                        r.cols.insert(i, (place, *alt));
                        todo.push(r);
                    }
                    continue;
                }
                Pat::Tuple { args, rest } => {
                    let arity = self.tuple_arity(pat, args.len(), *rest)?;
                    let fields = positional(args, *rest, arity);
                    for (k, (index, sub)) in fields.into_iter().enumerate() {
                        let field = self.place(Place::Field {
                            of: place,
                            ctor: Ctor::Tuple(arity as u32),
                            index,
                        });
                        row.cols.insert(i + k, (field, sub));
                    }
                }
                Pat::TupleStruct { path, args, rest } => {
                    let VariantDef::Struct(s) = self.variant(pat, path)? else { unreachable!() };
                    let count = field_types(self.db, VariantDef::Struct(s)).fields.len();
                    for (k, (index, sub)) in positional(args, *rest, count).into_iter().enumerate()
                    {
                        let field =
                            self.place(Place::Field { of: place, ctor: Ctor::Struct(s), index });
                        row.cols.insert(i + k, (field, sub));
                    }
                }
                Pat::Record { path, fields, .. } => {
                    let VariantDef::Struct(s) = self.variant(pat, path)? else { unreachable!() };
                    let types = field_types(self.db, VariantDef::Struct(s));
                    for (k, f) in fields.iter().enumerate() {
                        let index = types.field(&f.name).ok_or(DecisionError::Unknown(pat))?.0;
                        let field = self.place(Place::Field {
                            of: place,
                            ctor: Ctor::Struct(s),
                            index: index as u32,
                        });
                        row.cols.insert(i + k, (field, f.pat));
                    }
                }
                // A unit struct, like `Unit`, has nothing to test.
                Pat::Path(_) => {}
                _ => return Err(DecisionError::Unsupported(pat)),
            }
            todo.push(row);
        }
        Ok(done)
    }

    /// Whether a pattern tests something, rather than always matching or
    /// only splitting a tuple or struct into its fields.
    fn is_test(&self, pat: PatId) -> bool {
        match &self.body[pat] {
            Pat::Wild | Pat::Missing | Pat::Bind { .. } | Pat::Or(_) | Pat::Tuple { .. } => false,
            Pat::TupleStruct { path, .. } | Pat::Record { path, .. } => {
                matches!(self.variant(pat, path), Ok(VariantDef::Variant(_)))
            }
            Pat::Path(path) => !matches!(
                self.resolve_value(path),
                Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(_), .. }))
            ),
            Pat::Lit { .. } | Pat::Range { .. } => true,
            // Reported when tested.
            Pat::Slice { .. }
            | Pat::ConstBlock(_)
            | Pat::MacroCall { .. }
            | Pat::NamedTuple { .. } => true,
        }
    }

    fn resolve_value(&self, path: &Path) -> Option<ValueResolution<'db>> {
        Resolver::for_body(self.db, self.def).resolve_path_in_value_ns(self.db, path)
    }

    /// The struct or variant of a pattern like `Some(x)` or `Point { x }`.
    fn variant(&self, pat: PatId, path: &Path) -> Result<VariantDef<'db>, DecisionError> {
        let resolver = Resolver::for_body(self.db, self.def);
        match resolver.resolve_path_in_type_ns(self.db, path) {
            Some(TypeResolution { ty: TypeNs::Struct(s), unresolved: None }) => {
                return Ok(VariantDef::Struct(s));
            }
            Some(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }) => {
                return Ok(VariantDef::Variant(v));
            }
            _ => {}
        }
        match resolver.resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(ValueNs::Variant(v))) => Ok(VariantDef::Variant(v)),
            _ => Err(DecisionError::Unknown(pat)),
        }
    }

    /// How many elements the tuple that a tuple pattern matches has: those
    /// it gives, or, with `..`, what its type says.
    fn tuple_arity(
        &self,
        pat: PatId,
        given: usize,
        rest: Option<u32>,
    ) -> Result<usize, DecisionError> {
        if rest.is_none() {
            return Ok(given);
        }
        match infer_body(self.db, self.def).pat_ty(pat) {
            Ty::Tuple(tys, _) => Ok(tys.len()),
            _ => Err(DecisionError::Unknown(pat)),
        }
    }

    /// What testing the pattern `pat` is.
    fn test(&self, pat: PatId) -> Result<Test<'db>, DecisionError> {
        let unknown = DecisionError::Unknown(pat);
        Ok(match &self.body[pat] {
            Pat::Lit { literal, negative } => match literal_test(literal, *negative) {
                Some(test) => test,
                None => return Err(DecisionError::Unsupported(pat)),
            },
            Pat::Path(path) => match self.resolve_value(path) {
                Some(ValueResolution::Value(ValueNs::Variant(v))) => {
                    Test::Case(Case::Variant(v), Vec::new())
                }
                Some(ValueResolution::Value(ValueNs::Builtin(b))) => {
                    Test::Case(Case::Bool(b == BuiltinValue::True), Vec::new())
                }
                Some(ValueResolution::Value(ValueNs::Const(c))) => Test::Equals(Value::Const(c)),
                _ => return Err(unknown),
            },
            Pat::TupleStruct { path, args, rest } => {
                let VariantDef::Variant(v) = self.variant(pat, path)? else { return Err(unknown) };
                let count = field_types(self.db, VariantDef::Variant(v)).fields.len();
                // The checker reports the wrong number of fields (#365).
                let fits = match rest {
                    None => args.len() == count,
                    Some(_) => args.len() <= count,
                };
                if !fits {
                    return Err(unknown);
                }
                let mut fields = vec![None; count];
                for (index, sub) in positional(args, *rest, count) {
                    fields[index as usize] = Some(sub);
                }
                Test::Case(Case::Variant(v), self.fill(fields))
            }
            Pat::Record { path, fields: given, .. } => {
                let VariantDef::Variant(v) = self.variant(pat, path)? else { return Err(unknown) };
                let types = field_types(self.db, VariantDef::Variant(v));
                let mut fields = vec![None; types.fields.len()];
                for f in given.iter() {
                    let index = types.field(&f.name).ok_or(unknown)?.0;
                    fields[index] = Some(f.pat);
                }
                Test::Case(Case::Variant(v), self.fill(fields))
            }
            Pat::Range { start, end, inclusive } => {
                let bound = |p: &Option<PatId>| -> Result<Option<Bound<'db>>, DecisionError> {
                    let Some(p) = p else { return Ok(None) };
                    Ok(Some(match self.test(*p)? {
                        Test::Case(Case::Int(n), _) => Bound::Int(n),
                        Test::Case(Case::Char(c), _) => Bound::Char(c),
                        Test::Equals(Value::Float(f)) => Bound::Float(f),
                        Test::Equals(Value::Const(c)) => Bound::Const(c),
                        _ => return Err(DecisionError::Unsupported(*p)),
                    }))
                };
                Test::Range(bound(start)?, bound(end)?, *inclusive)
            }
            _ => return Err(DecisionError::Unsupported(pat)),
        })
    }

    /// The patterns of a variant's fields that are given, in the order of
    /// its fields: [`Builder::field_index`] says which each is.
    fn fill(&self, fields: Vec<Option<PatId>>) -> Vec<PatId> {
        fields.into_iter().flatten().collect()
    }

    fn compile(&mut self, rows: Vec<Row>) -> Result<NodeId, DecisionError> {
        if let Some(&node) = self.memo.get(&rows) {
            return Ok(node);
        }
        let node = self.compile_new(&rows)?;
        self.memo.insert(rows, node);
        Ok(node)
    }

    fn compile_new(&mut self, rows: &[Row]) -> Result<NodeId, DecisionError> {
        let Some(first) = rows.first() else { return Ok(self.node(Node::Fail)) };
        let Some(&(place, pat)) = first.cols.first() else {
            // Nothing left to test: the arm matches, if its guard holds.
            let arm = Node::Arm { arm: first.arm, bindings: first.bindings.clone() };
            return Ok(match self.arms[first.arm as usize].1 {
                None => self.node(arm),
                Some(guard) => {
                    let then = self.node(arm);
                    let otherwise = self.compile(rows[1..].to_vec())?;
                    self.node(Node::Guard {
                        arm: first.arm,
                        guard,
                        bindings: first.bindings.clone(),
                        then,
                        otherwise,
                    })
                }
            });
        };
        match self.test(pat)? {
            Test::Case(..) => self.switch(rows, place),
            test @ (Test::Equals(_) | Test::Range(..)) => self.two_way(rows, place, test),
        }
    }

    /// The pattern a row has at `place`, if it tests it.
    fn at(row: &Row, place: PlaceId) -> Option<(usize, PatId)> {
        row.cols.iter().position(|&(p, _)| p == place).map(|i| (i, row.cols[i].1))
    }

    /// A switch on the cases that the rows test at `place`.
    fn switch(&mut self, rows: &[Row], place: PlaceId) -> Result<NodeId, DecisionError> {
        let mut cases: Vec<Case<'db>> = Vec::new();
        for row in rows {
            if let Some((_, pat)) = Self::at(row, place)
                && let Test::Case(case, _) = self.test(pat)?
                && !cases.contains(&case)
            {
                cases.push(case);
            }
        }
        let mut arms = Vec::new();
        for &case in &cases {
            let mut specialized = Vec::new();
            for row in rows {
                match Self::at(row, place) {
                    None => specialized.push(row.clone()),
                    Some((i, pat)) => match self.test(pat)? {
                        Test::Case(c, fields) if c == case => {
                            let mut row = row.clone();
                            row.cols.remove(i);
                            if let Case::Variant(v) = case {
                                for (k, sub) in fields.into_iter().enumerate() {
                                    let index = self.field_index(pat, k)?;
                                    let field = self.place(Place::Field {
                                        of: place,
                                        ctor: Ctor::Variant(v),
                                        index,
                                    });
                                    row.cols.insert(i + k, (field, sub));
                                }
                            }
                            specialized.extend(self.expand(row)?);
                        }
                        Test::Case(..) => {}
                        // Another kind of test of the place stays.
                        _ => specialized.push(row.clone()),
                    },
                }
            }
            arms.push((case, self.compile(specialized)?));
        }
        let complete = match cases.first() {
            Some(Case::Variant(v)) => {
                let all = v.parent.item(self.db).variants.len();
                cases.len() >= all
            }
            Some(Case::Bool(_)) => cases.len() >= 2,
            _ => false,
        };
        let default = if complete {
            None
        } else {
            let rest: Vec<Row> = rows
                .iter()
                .filter(|row| match Self::at(row, place) {
                    None => true,
                    Some((_, pat)) => !matches!(self.test(pat), Ok(Test::Case(..))),
                })
                .cloned()
                .collect();
            Some(self.compile(rest)?)
        };
        // A case that goes where the default goes needn't be tested, and a
        // switch that goes to one place is that place.
        if let Some(d) = default {
            arms.retain(|(_, to)| *to != d);
        }
        let targets: Vec<NodeId> = arms.iter().map(|(_, to)| *to).chain(default).collect();
        if let Some(&only) = targets.first()
            && targets.iter().all(|t| *t == only)
        {
            return Ok(only);
        }
        Ok(self.node(Node::Switch { place, cases: arms, default }))
    }

    /// The field index of the `k`th field pattern of a variant pattern.
    fn field_index(&self, pat: PatId, k: usize) -> Result<u32, DecisionError> {
        let unknown = DecisionError::Unknown(pat);
        let (path, given) = match &self.body[pat] {
            Pat::TupleStruct { path, args, rest } => {
                let v = self.variant(pat, path)?;
                let count = field_types(self.db, v).fields.len();
                let fields = positional(args, *rest, count);
                return fields.get(k).map(|(i, _)| *i).ok_or(unknown);
            }
            Pat::Record { path, fields, .. } => (path, fields),
            _ => return Err(unknown),
        };
        let v = self.variant(pat, path)?;
        let types = field_types(self.db, v);
        // `fill` keeps the fields in the order of the variant's.
        let mut indices: Vec<usize> = given
            .iter()
            .map(|f| types.field(&f.name).map(|(i, _)| i))
            .collect::<Option<_>>()
            .ok_or(unknown)?;
        indices.sort();
        indices.get(k).map(|i| *i as u32).ok_or(unknown)
    }

    /// A two-way test, `Equals` or `Range`, of `place`.
    fn two_way(
        &mut self,
        rows: &[Row],
        place: PlaceId,
        test: Test<'db>,
    ) -> Result<NodeId, DecisionError> {
        let mut then_rows = Vec::new();
        let mut otherwise_rows = Vec::new();
        for row in rows {
            let Some((i, pat)) = Self::at(row, place) else {
                then_rows.push(row.clone());
                otherwise_rows.push(row.clone());
                continue;
            };
            let other = self.test(pat)?;
            if other == test {
                let mut row = row.clone();
                row.cols.remove(i);
                then_rows.push(row);
            } else {
                // A value equal to one constant isn't equal to another of
                // the same kind; anything else is still to be tested.
                let excluded = matches!(
                    (&test, &other),
                    (Test::Equals(Value::Str(_)), Test::Equals(Value::Str(_)))
                        | (Test::Equals(Value::Float(_)), Test::Equals(Value::Float(_)))
                );
                if !excluded {
                    then_rows.push(row.clone());
                }
                otherwise_rows.push(row.clone());
            }
        }
        let then = self.compile(then_rows)?;
        let otherwise = self.compile(otherwise_rows)?;
        Ok(self.node(match test {
            Test::Equals(value) => Node::Equals { place, value, then, otherwise },
            Test::Range(start, end, inclusive) => {
                Node::Range { place, start, end, inclusive, then, otherwise }
            }
            Test::Case(..) => unreachable!("a switch"),
        }))
    }
}

/// The fields that positional patterns give, by index, with a `..` at
/// `rest` standing for those in between, of `count` in all.
fn positional(args: &[PatId], rest: Option<u32>, count: usize) -> Vec<(u32, PatId)> {
    match rest {
        None => args.iter().enumerate().map(|(i, &p)| (i as u32, p)).collect(),
        Some(at) => {
            let at = at as usize;
            let (before, after) = args.split_at(at.min(args.len()));
            let tail = count.saturating_sub(after.len());
            before
                .iter()
                .enumerate()
                .map(|(i, &p)| (i as u32, p))
                .chain(after.iter().enumerate().map(|(i, &p)| ((tail + i) as u32, p)))
                .collect()
        }
    }
}

fn literal_test<'db>(literal: &Literal, negative: bool) -> Option<Test<'db>> {
    let sign = |n: i128| if negative { -n } else { n };
    Some(match literal {
        Literal::Int(n, _) => Test::Case(Case::Int(sign(i128::try_from(*n).ok()?)), Vec::new()),
        Literal::Byte(b) => Test::Case(Case::Int(*b as i128), Vec::new()),
        Literal::Char(c) => Test::Case(Case::Char(*c), Vec::new()),
        Literal::String(s) => Test::Equals(Value::Str(s.clone())),
        Literal::Float(f, _) => {
            Test::Equals(Value::Float(if negative { format!("-{f}") } else { f.clone() }))
        }
        Literal::ByteString(_) | Literal::CString(_) => return None,
    })
}

impl<'db> Decision<'db> {
    /// Whether a value can match no arm.
    pub fn can_fail(&self) -> bool {
        self.reachable().iter().zip(&self.nodes).any(|(r, n)| *r && *n == Node::Fail)
    }

    /// Whether each arm can be taken: an arm that can't is never reached.
    /// One with a guard counts once its guard can run.
    pub fn reached_arms(&self) -> Vec<bool> {
        let mut out = vec![false; self.arms as usize];
        for (reached, node) in self.reachable().iter().zip(&self.nodes) {
            match node {
                Node::Arm { arm, .. } | Node::Guard { arm, .. } if *reached => {
                    out[*arm as usize] = true;
                }
                _ => {}
            }
        }
        out
    }

    /// The nodes the root reaches.
    fn reachable(&self) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![self.root];
        while let Some(n) = stack.pop() {
            if std::mem::replace(&mut seen[n as usize], true) {
                continue;
            }
            stack.extend(self.successors(n));
        }
        seen
    }

    fn successors(&self, n: NodeId) -> Vec<NodeId> {
        match &self.nodes[n as usize] {
            Node::Switch { cases, default, .. } => {
                cases.iter().map(|(_, n)| *n).chain(*default).collect()
            }
            Node::Equals { then, otherwise, .. }
            | Node::Range { then, otherwise, .. }
            | Node::Guard { then, otherwise, .. } => vec![*then, *otherwise],
            Node::Arm { .. } | Node::Fail => Vec::new(),
        }
    }

    /// What's known of the places along the first path to [`Node::Fail`],
    /// if there is one.
    fn missing_steps(&self) -> Option<HashMap<PlaceId, Step<'db>>> {
        // What each place was found to be on the path, by breadth-first
        // search from the root to a failure.
        let mut prev: Vec<Option<(NodeId, Step<'db>)>> = vec![None; self.nodes.len()];
        let mut seen = vec![false; self.nodes.len()];
        let mut queue = std::collections::VecDeque::from([self.root]);
        seen[self.root as usize] = true;
        let mut fail = None;
        while let Some(n) = queue.pop_front() {
            if self.nodes[n as usize] == Node::Fail {
                fail = Some(n);
                break;
            }
            let steps: Vec<(NodeId, Step<'db>)> = match &self.nodes[n as usize] {
                Node::Switch { place, cases, default } => {
                    let mut steps: Vec<(NodeId, Step<'db>)> =
                        cases.iter().map(|(c, to)| (*to, Step::Is(*place, *c))).collect();
                    if let Some(d) = default {
                        let taken = cases.iter().map(|(c, _)| *c).collect();
                        steps.push((*d, Step::Not(*place, taken)));
                    }
                    steps
                }
                Node::Equals { then, otherwise, .. }
                | Node::Range { then, otherwise, .. }
                | Node::Guard { then, otherwise, .. } => {
                    vec![(*then, Step::None), (*otherwise, Step::None)]
                }
                Node::Arm { .. } | Node::Fail => Vec::new(),
            };
            for (to, step) in steps {
                if !std::mem::replace(&mut seen[to as usize], true) {
                    prev[to as usize] = Some((n, step));
                    queue.push_back(to);
                }
            }
        }
        let mut at = fail?;
        let mut known: HashMap<PlaceId, Step<'db>> = HashMap::new();
        while let Some((from, step)) = prev[at as usize].clone() {
            match &step {
                Step::Is(p, _) | Step::Not(p, _) => {
                    known.entry(*p).or_insert(step);
                }
                Step::None => {}
            }
            at = from;
        }
        Some(known)
    }

    /// A pattern for a value that no arm matches, like `Option.None`, if
    /// there is one: the value along the first path to [`Node::Fail`].
    pub fn missing(&self, db: &'db dyn Db) -> Option<String> {
        let known = self.missing_steps()?;
        Some(self.show(db, 0, &known))
    }

    /// For a decision of [`decide_args`], arguments that no arm matches, as
    /// patterns, if there are some.
    pub fn missing_args(&self, db: &'db dyn Db, arity: usize) -> Option<Vec<String>> {
        let known = self.missing_steps()?;
        let ctor = Ctor::Tuple(arity as u32);
        let args = (0..arity as u32)
            .map(|index| {
                match self.places.iter().position(|p| *p == Place::Field { of: 0, ctor, index }) {
                    Some(at) => self.show(db, at as PlaceId, &known),
                    None => "_".to_string(),
                }
            })
            .collect();
        Some(args)
    }

    /// The pattern for `place`, from what's known of the places.
    fn show(&self, db: &'db dyn Db, place: PlaceId, known: &HashMap<PlaceId, Step<'db>>) -> String {
        let fields = |ctor: Ctor<'db>, count: usize| -> Vec<String> {
            (0..count)
                .map(|index| {
                    let at = self
                        .places
                        .iter()
                        .position(|p| *p == Place::Field { of: place, ctor, index: index as u32 });
                    match at {
                        Some(at) => self.show(db, at as PlaceId, known),
                        None => "_".to_string(),
                    }
                })
                .collect()
        };
        let variant = |v: EnumVariantId<'db>| -> String {
            let name = format!(
                "{}.{}",
                v.parent.item(db).name.as_ref().map_or("?", |n| n.as_str()),
                v.name(db).as_str()
            );
            let count = field_types(db, VariantDef::Variant(v)).fields.len();
            if count == 0 {
                name
            } else {
                format!("{name}({})", fields(Ctor::Variant(v), count).join(", "))
            }
        };
        match known.get(&place) {
            Some(Step::Is(_, case)) => show_case(*case, &variant),
            Some(Step::Not(_, taken)) => match taken.first() {
                Some(Case::Variant(v)) => {
                    let all = v.parent.item(db).variants.len() as u32;
                    (0..all)
                        .map(|index| EnumVariantId { parent: v.parent, index })
                        .find(|w| !taken.contains(&Case::Variant(*w)))
                        .map_or("_".to_string(), variant)
                }
                Some(Case::Bool(b)) => (if *b { "False" } else { "True" }).to_string(),
                _ => "_".to_string(),
            },
            _ => {
                // A tuple or struct whose fields were tested.
                let ctor =
                    self.places.iter().find_map(|p| match p {
                        Place::Field {
                            of, ctor: c @ (Ctor::Tuple(_) | Ctor::Struct(_)), ..
                        } if *of == place => Some(*c),
                        _ => None,
                    });
                match ctor {
                    Some(Ctor::Tuple(n)) => {
                        format!("({})", fields(Ctor::Tuple(n), n as usize).join(", "))
                    }
                    Some(Ctor::Struct(s)) => {
                        let count = field_types(db, VariantDef::Struct(s)).fields.len();
                        let name = s.item(db).name.as_ref().map_or("?", |n| n.as_str()).to_string();
                        format!("{name}({})", fields(Ctor::Struct(s), count).join(", "))
                    }
                    _ => "_".to_string(),
                }
            }
        }
    }
}

/// What a step from a node to the next says about a place.
#[derive(Debug, Clone)]
enum Step<'db> {
    Is(PlaceId, Case<'db>),
    Not(PlaceId, Vec<Case<'db>>),
    None,
}

fn show_case<'db>(case: Case<'db>, variant: &dyn Fn(EnumVariantId<'db>) -> String) -> String {
    match case {
        Case::Variant(v) => variant(v),
        Case::Bool(b) => (if b { "True" } else { "False" }).to_string(),
        Case::Int(n) => n.to_string(),
        Case::Char(c) => format!("{c:?}"),
    }
}

impl<'db> Decision<'db> {
    /// The decision as text, a node per line from the root, for tests and
    /// debugging. Places are `$0` for the value, and `$1 = $0.Some.0` for a
    /// field.
    pub fn display(&self, db: &'db dyn Db) -> String {
        let mut out = String::new();
        for (i, place) in self.places.iter().enumerate().skip(1) {
            if let Place::Field { of, ctor, index } = place {
                let ctor = match ctor {
                    Ctor::Tuple(_) => String::new(),
                    Ctor::Struct(s) => {
                        format!("{}.", s.item(db).name.as_ref().map_or("?", |n| n.as_str()))
                    }
                    Ctor::Variant(v) => format!("{}.", v.name(db).as_str()),
                };
                out.push_str(&format!("${i} = ${of}.{ctor}{index}\n"));
            }
        }
        // Each node once, numbered in the order they're first reached.
        let mut order = Vec::new();
        let mut stack = vec![self.root];
        while let Some(n) = stack.pop() {
            if order.contains(&n) {
                continue;
            }
            order.push(n);
            let mut next = self.successors(n);
            next.reverse();
            stack.extend(next);
        }
        let name = |n: &NodeId| format!("n{}", order.iter().position(|m| m == n).unwrap());
        let bindings = |bs: &[(BindingId, PlaceId)]| -> String {
            if bs.is_empty() {
                return String::new();
            }
            let bs: Vec<String> =
                bs.iter().map(|(b, p)| format!("b{} = ${p}", b.index())).collect();
            format!(" with {}", bs.join(", "))
        };
        for n in &order {
            let line = match &self.nodes[*n as usize] {
                Node::Switch { place, cases, default } => {
                    let mut arms: Vec<String> = cases
                        .iter()
                        .map(|(c, to)| {
                            let c = match c {
                                Case::Variant(v) => v.name(db).to_string(),
                                Case::Bool(b) => b.to_string(),
                                Case::Int(i) => i.to_string(),
                                Case::Char(c) => format!("{c:?}"),
                            };
                            format!("{c} => {}", name(to))
                        })
                        .collect();
                    if let Some(d) = default {
                        arms.push(format!("_ => {}", name(d)));
                    }
                    format!("switch ${place} {{ {} }}", arms.join(", "))
                }
                Node::Equals { place, value, then, otherwise } => {
                    let value = match value {
                        Value::Str(s) => format!("{s:?}"),
                        Value::Float(f) => f.clone(),
                        Value::Const(c) => {
                            c.item(db).name.as_ref().map_or("?".into(), |n| n.to_string())
                        }
                    };
                    format!("if ${place} == {value} then {} else {}", name(then), name(otherwise))
                }
                Node::Range { place, start, end, inclusive, then, otherwise } => {
                    let bound = |b: &Option<Bound<'db>>| match b {
                        None => String::new(),
                        Some(Bound::Int(i)) => i.to_string(),
                        Some(Bound::Char(c)) => format!("{c:?}"),
                        Some(Bound::Float(f)) => f.clone(),
                        Some(Bound::Const(c)) => {
                            c.item(db).name.as_ref().map_or("?".into(), |n| n.to_string())
                        }
                    };
                    let dots = if *inclusive { "..=" } else { ".." };
                    format!(
                        "if ${place} in {}{dots}{} then {} else {}",
                        bound(start),
                        bound(end),
                        name(then),
                        name(otherwise)
                    )
                }
                Node::Guard { arm, bindings: bs, then, otherwise, .. } => format!(
                    "guard {arm}{} then {} else {}",
                    bindings(bs),
                    name(then),
                    name(otherwise)
                ),
                Node::Arm { arm, bindings: bs } => format!("arm {arm}{}", bindings(bs)),
                Node::Fail => "fail".to_string(),
            };
            out.push_str(&format!("{} = {line}\n", name(n)));
        }
        out
    }
}
