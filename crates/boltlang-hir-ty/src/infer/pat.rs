//! Checking patterns against the type of the value they match.

use boltlang_hir_def::{
    Name,
    hir::{Pat, PatId, Path, RecordFieldPat, TupleItem},
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
};

use super::{InferenceContext, InferenceDiagnostic};
use crate::{
    lower::{FieldsKind, VariantDef, const_ty, field_types, subst_params},
    solve::{Constraint, ExprOrPat, Origin, TuplePatConstraint, TuplePatPart, TypeError},
    ty::{Ty, VarKind},
};

impl<'db> InferenceContext<'_, 'db> {
    fn pat_origin(&self, pat: PatId) -> Origin<'db> {
        Origin::new(self.def, ExprOrPat::Pat(pat))
    }

    /// Check that `pat` can match a value of type `expected`, and give its
    /// bindings their types.
    pub(crate) fn check_pat(&mut self, pat: PatId, expected: &Ty<'db>) {
        self.result.pat_types.insert(pat, expected.clone());
        let body = self.body;
        match &body[pat] {
            Pat::Missing | Pat::Wild | Pat::MacroCall { .. } => {}
            Pat::Bind { id, subpat } => {
                // The alternatives of an or-pattern share their bindings, so
                // a binding seen before has the same type here.
                match self.result.binding_types.get(*id).cloned() {
                    Some(ty) => self.solver.equal(&ty, expected, self.pat_origin(pat)),
                    None => {
                        self.result.binding_types.insert(*id, expected.clone());
                        self.declare(*id);
                    }
                }
                if let Some(subpat) = subpat {
                    self.check_pat(*subpat, expected);
                }
            }
            Pat::Lit { literal, .. } => {
                let ty = self.literal_ty(ExprOrPat::Pat(pat), literal);
                self.solver.equal(expected, &ty, self.pat_origin(pat));
            }
            Pat::Path(path) => {
                let ty = self.path_pat_ty(pat, path);
                self.solver.equal(expected, &ty, self.pat_origin(pat));
            }
            Pat::Tuple { args, rest } => {
                let resolved = self.solver.unifier.shallow_resolve(expected);
                let resolved = match resolved.has_spread() {
                    true => self.solver.unifier.resolve(&resolved),
                    false => resolved,
                };
                let elems = match (resolved.tuple_parts(), rest) {
                    // With a pack spread in it, only the fields before the
                    // first spread and after the last one are known.
                    (Some(parts), Some(r)) if parts.positional.len() >= args.len() => {
                        let split = *r as usize;
                        let spread = |t: &Ty<'db>| matches!(t, Ty::Spread(..));
                        let tys = &parts.positional;
                        let before = tys.iter().take_while(|t| !spread(t)).count();
                        let after = tys.iter().rev().take_while(|t| !spread(t)).count();
                        if split > before || args.len() - split > after {
                            let origin = self.pat_origin(pat);
                            self.solver.errors.push(TypeError::TypeNeeded { origin });
                            for &arg in args.iter() {
                                self.check_pat(arg, &Ty::Error);
                            }
                            return;
                        }
                        tys.clone()
                    }
                    (_, Some(_)) => {
                        // The length isn't known, so neither is what `..` covers.
                        for &arg in args.iter() {
                            let var = self.solver.new_var(VarKind::General);
                            self.check_pat(arg, &var);
                        }
                        return;
                    }
                    (_, None) => {
                        let tys: Vec<_> =
                            args.iter().map(|_| self.solver.new_var(VarKind::General)).collect();
                        let m = self.solver.new_var(VarKind::Mutability);
                        let shape = Ty::tuple(tys.clone().into(), m);
                        self.expect_shape(pat, expected, &shape, &tys);
                        tys
                    }
                };
                self.check_pat_list(args, *rest, &elems);
            }
            Pat::TupleStruct { path, args, rest } => {
                let Some((def, args_ty)) = self.variant_pat(pat, path, expected) else {
                    for &arg in args.iter() {
                        self.check_pat(arg, &Ty::Error);
                    }
                    return;
                };
                let (_, generic_def) = def.adt();
                let fields = field_types(self.db, def);
                if fields.kind != FieldsKind::Tuple {
                    self.unresolved(ExprOrPat::Pat(pat));
                }
                let field_tys: Vec<_> = fields
                    .fields
                    .iter()
                    .map(|(_, t)| subst_params(t, generic_def, &args_ty))
                    .collect();
                if rest.is_none() && field_tys.len() != args.len() {
                    self.result.diagnostics.push(InferenceDiagnostic::WrongArgCount {
                        expr: ExprOrPat::Pat(pat),
                        expected: field_tys.len(),
                        found: args.len(),
                    });
                }
                // The fields the pattern names must be visible; `..` can
                // stand for private ones.
                for (i, &arg) in args.iter().enumerate() {
                    if let Some(index) = list_index(i, args.len(), *rest, field_tys.len()) {
                        self.check_field_visible(ExprOrPat::Pat(arg), def, index);
                    }
                }
                self.check_pat_list(args, *rest, &field_tys);
            }
            Pat::Record { path, fields, .. } => {
                let Some((def, args_ty)) = self.variant_pat(pat, path, expected) else {
                    for field in fields.iter() {
                        self.check_pat(field.pat, &Ty::Error);
                    }
                    return;
                };
                self.check_record_fields(pat, def, &args_ty, fields);
            }
            // Lengths aren't checked yet, so `..` doesn't matter.
            Pat::Slice { args, .. } => {
                let elem = self.solver.new_var(VarKind::General);
                let array = Ty::array(self.solver.new_var(VarKind::Mutability), elem.clone());
                self.expect_shape(pat, expected, &array, std::slice::from_ref(&elem));
                for &arg in args.iter() {
                    self.check_pat(arg, &elem);
                }
            }
            Pat::Range { start, end, .. } => {
                for p in start.iter().chain(end.iter()) {
                    self.check_pat(*p, expected);
                }
            }
            Pat::Or(alternatives) => {
                for &alt in alternatives.iter() {
                    self.check_pat(alt, expected);
                }
            }
            Pat::ConstBlock(expr) => self.check_expr(*expr, expected),
            Pat::NamedTuple { items, rest } => {
                self.check_named_tuple_pat(pat, items, *rest, expected)
            }
        }
    }

    /// Check that `pat` can match `expected`, as `shape`: a tuple or array
    /// of the new variables `parts`. If it can't, like `(a, b)` for a
    /// struct, the parts met an error, so that the names in the pattern
    /// don't become type parameters.
    fn expect_shape(&mut self, pat: PatId, expected: &Ty<'db>, shape: &Ty<'db>, parts: &[Ty<'db>]) {
        let errors = self.solver.errors.len();
        self.solver.equal(expected, shape, self.pat_origin(pat));
        if self.solver.errors.len() > errors {
            for part in parts {
                self.solver.unifier.met_error(part);
            }
        }
    }

    /// A tuple pattern with named fields, like `(a, x: b)`. Without `..`,
    /// it says the whole type, so it's matched like a tuple pattern. With
    /// `..`, the fields it names are found once the type is known.
    fn check_named_tuple_pat(
        &mut self,
        pat: PatId,
        items: &[TupleItem<PatId>],
        rest: Option<u32>,
        expected: &Ty<'db>,
    ) {
        let origin = self.pat_origin(pat);
        // Each name once; a field given twice is matched against nothing.
        let mut names: Vec<&Name> = Vec::new();
        let mut parts = Vec::new();
        let mut positional = 0;
        for item in items {
            match item {
                TupleItem::Named(name, p) if names.contains(&name) => {
                    let name = name.clone();
                    self.solver.errors.push(TypeError::DuplicateField { origin, name });
                    self.check_pat(*p, &Ty::Error);
                }
                TupleItem::Named(name, p) => {
                    names.push(name);
                    parts.push((TuplePatPart::Named(name.clone()), *p));
                }
                TupleItem::Positional(p) => {
                    parts.push((TuplePatPart::Positional(positional), *p));
                    positional += 1;
                }
                TupleItem::Spread(_, p) => self.check_pat(*p, &Ty::Error),
            }
        }
        let vars: Vec<Ty<'db>> =
            parts.iter().map(|_| self.solver.new_var(VarKind::General)).collect();
        if rest.is_none() {
            let positional = parts
                .iter()
                .zip(&vars)
                .filter(|((part, _), _)| matches!(part, TuplePatPart::Positional(_)))
                .map(|(_, v)| v.clone())
                .collect();
            let named = parts
                .iter()
                .zip(&vars)
                .filter_map(|((part, _), v)| match part {
                    TuplePatPart::Named(name) => Some((name.clone(), v.clone())),
                    TuplePatPart::Positional(_) => None,
                })
                .collect();
            let m = self.solver.new_var(VarKind::Mutability);
            let shape = Ty::record(positional, named, m);
            self.expect_shape(pat, expected, &shape, &vars);
        } else {
            let fields = parts.iter().map(|(part, _)| part.clone()).zip(vars.clone()).collect();
            let constraint = TuplePatConstraint { ty: expected.clone(), fields, positional, rest };
            self.solver.add(Constraint::TuplePat { constraint, origin });
        }
        for ((_, p), ty) in parts.iter().zip(vars) {
            self.check_pat(*p, &ty);
        }
    }

    /// Check the patterns of a tuple or tuple struct, where `..` at `rest`
    /// stands for the elements not matched.
    pub(super) fn check_pat_list(&mut self, args: &[PatId], rest: Option<u32>, elems: &[Ty<'db>]) {
        for (i, &arg) in args.iter().enumerate() {
            let index = list_index(i, args.len(), rest, elems.len());
            let ty = index.map_or(Ty::Error, |i| elems[i].clone());
            self.check_pat(arg, &ty);
        }
    }

    fn check_record_fields(
        &mut self,
        pat: PatId,
        def: VariantDef<'db>,
        args: &[Ty<'db>],
        fields: &[RecordFieldPat],
    ) {
        let (_, generic_def) = def.adt();
        let field_tys = field_types(self.db, def);
        for field in fields {
            let ty = match field_tys.field(&field.name) {
                Some((index, ty)) => {
                    self.check_field_visible(ExprOrPat::Pat(field.pat), def, index);
                    subst_params(ty, generic_def, args)
                }
                None => {
                    self.result.diagnostics.push(InferenceDiagnostic::NoSuchField {
                        expr: ExprOrPat::Pat(pat),
                        name: field.name.clone(),
                    });
                    Ty::Error
                }
            };
            self.check_pat(field.pat, &ty);
        }
    }

    /// Resolve the path of a struct or variant pattern, and check that its
    /// type is `expected`. Returns the struct or variant, and the arguments
    /// of its type.
    fn variant_pat(
        &mut self,
        pat: PatId,
        path: &Path,
        expected: &Ty<'db>,
    ) -> Option<(VariantDef<'db>, Box<[Ty<'db>]>)> {
        let resolver = Resolver::for_body(self.db, self.def);
        let def = match resolver.resolve_path_in_type_ns(self.db, path) {
            Some(TypeResolution { ty: TypeNs::Struct(s), unresolved: None }) => {
                VariantDef::Struct(s)
            }
            Some(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }) => {
                VariantDef::Variant(v)
            }
            _ => {
                // A bare variant name like `Some`, imported into scope.
                match resolver.resolve_path_in_value_ns(self.db, path) {
                    Some(ValueResolution::Value(ValueNs::Variant(v))) => VariantDef::Variant(v),
                    _ => {
                        self.unresolved_path(ExprOrPat::Pat(pat), &resolver, path);
                        return None;
                    }
                }
            }
        };
        // A pattern matches a struct of either mutability.
        let mutability = self.new_mutability();
        let (ty, args) = self.fresh_adt(def, mutability);
        self.solver.equal(expected, &ty, self.pat_origin(pat));
        Some((def, args))
    }

    /// The type of a path pattern: a unit variant or struct, a const, or
    /// `True` or `False`.
    fn path_pat_ty(&mut self, pat: PatId, path: &Path) -> Ty<'db> {
        let resolver = Resolver::for_body(self.db, self.def);
        let def = match resolver.resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(ValueNs::Variant(v))) => VariantDef::Variant(v),
            Some(ValueResolution::Value(ValueNs::Const(c))) => {
                return const_ty(self.db, c).ty.clone();
            }
            Some(ValueResolution::Value(ValueNs::Builtin(v))) => return Ty::Builtin(v.ty()),
            Some(ValueResolution::Type(TypeResolution {
                ty: TypeNs::Struct(s),
                unresolved: None,
            })) => VariantDef::Struct(s),
            _ => {
                self.unresolved_path(ExprOrPat::Pat(pat), &resolver, path);
                return Ty::Error;
            }
        };
        let mutability = self.new_mutability();
        self.fresh_adt(def, mutability).0
    }
}

/// Which of `len` elements the `i`th of `count` patterns of a tuple or
/// tuple struct matches, where `..` at `rest` stands for the elements not
/// matched: those after it match from the end.
fn list_index(i: usize, count: usize, rest: Option<u32>, len: usize) -> Option<usize> {
    let split = rest.map_or(count, |r| r as usize);
    let index = if i < split { Some(i) } else { len.checked_sub(count - i) };
    index.filter(|&index| index < len)
}
