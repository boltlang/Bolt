//! Checking patterns against the type of the value they match.

use boltlang_hir_def::{
    hir::{Pat, PatId, Path, RecordFieldPat},
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
};

use super::{InferenceContext, InferenceDiagnostic};
use crate::{
    lower::{FieldsKind, VariantDef, const_ty, field_types, identity_args, subst_params},
    solve::{ExprOrPat, Origin},
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
                self.result.binding_types.insert(*id, expected.clone());
                if let Some(subpat) = subpat {
                    self.check_pat(*subpat, expected);
                }
            }
            Pat::Lit { literal, .. } => {
                let ty = self.literal_ty(literal);
                self.solver.equal(expected, &ty, self.pat_origin(pat));
            }
            Pat::Path(path) => {
                let ty = self.path_pat_ty(pat, path);
                self.solver.equal(expected, &ty, self.pat_origin(pat));
            }
            Pat::Tuple { args, rest } => {
                let elems = match (self.solver.unifier.shallow_resolve(expected), rest) {
                    (Ty::Tuple(tys), Some(_)) if tys.len() >= args.len() => tys.into_vec(),
                    (_, Some(_)) => {
                        // The length isn't known, so neither is what `..` covers.
                        for &arg in args.iter() {
                            let var = self.solver.new_var(VarKind::General);
                            self.check_pat(arg, &var);
                        }
                        return;
                    }
                    (_, None) => {
                        let tys: Vec<_> = args.iter().map(|_| self.solver.new_var(VarKind::General)).collect();
                        self.expect_shape(pat, expected, &Ty::Tuple(tys.clone().into()), &tys);
                        tys
                    }
                };
                self.check_pat_list(args, *rest, &elems);
            }
            Pat::TupleStruct { path, args, rest } => {
                let Some((def, adt_ty, args_ty)) = self.variant_pat(pat, path, expected) else {
                    for &arg in args.iter() {
                        self.check_pat(arg, &Ty::Error);
                    }
                    return;
                };
                let (_, generic_def) = def.adt();
                let fields = field_types(self.db, def);
                if fields.kind != FieldsKind::Tuple {
                    self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Pat(pat) });
                }
                let field_tys: Vec<_> = fields.fields.iter().map(|(_, t)| subst_params(t, generic_def, &args_ty)).collect();
                if rest.is_none() && field_tys.len() != args.len() {
                    self.result.diagnostics.push(InferenceDiagnostic::WrongArgCount {
                        expr: ExprOrPat::Pat(pat),
                        expected: field_tys.len(),
                        found: args.len(),
                    });
                }
                self.check_pat_list(args, *rest, &field_tys);
                let _ = adt_ty;
            }
            Pat::Record { path, fields, .. } => {
                let Some((def, _, args_ty)) = self.variant_pat(pat, path, expected) else {
                    for field in fields.iter() {
                        self.check_pat(field.pat, &Ty::Error);
                    }
                    return;
                };
                self.check_record_fields(pat, def, &args_ty, fields);
            }
            Pat::Slice { args, rest } => {
                let elem = self.solver.new_var(VarKind::General);
                self.expect_shape(pat, expected, &Ty::Array(Box::new(elem.clone())), std::slice::from_ref(&elem));
                let _ = rest;
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

    /// Check the patterns of a tuple or tuple struct, where `..` at `rest`
    /// stands for the elements not matched.
    fn check_pat_list(&mut self, args: &[PatId], rest: Option<u32>, elems: &[Ty<'db>]) {
        let split = rest.map_or(args.len(), |r| r as usize);
        let after = args.len() - split;
        for (i, &arg) in args.iter().enumerate() {
            let index = if i < split { Some(i) } else { elems.len().checked_sub(after - (i - split)) };
            let ty = index.and_then(|i| elems.get(i)).cloned().unwrap_or(Ty::Error);
            self.check_pat(arg, &ty);
        }
    }

    fn check_record_fields(&mut self, pat: PatId, def: VariantDef<'db>, args: &[Ty<'db>], fields: &[RecordFieldPat]) {
        let (_, generic_def) = def.adt();
        let field_tys = field_types(self.db, def);
        for field in fields {
            let ty = match field_tys.field(&field.name) {
                Some((_, ty)) => subst_params(ty, generic_def, args),
                None => {
                    self.result
                        .diagnostics
                        .push(InferenceDiagnostic::NoSuchField { expr: ExprOrPat::Pat(pat), name: field.name.clone() });
                    Ty::Error
                }
            };
            self.check_pat(field.pat, &ty);
        }
    }

    /// Resolve the path of a struct or variant pattern, and check that its
    /// type is `expected`. Returns the struct or variant, its type and the
    /// type's arguments.
    fn variant_pat(
        &mut self,
        pat: PatId,
        path: &Path,
        expected: &Ty<'db>,
    ) -> Option<(VariantDef<'db>, Ty<'db>, Box<[Ty<'db>]>)> {
        let resolver = Resolver::for_body(self.db, self.def);
        let def = match resolver.resolve_path_in_type_ns(self.db, path) {
            Some(TypeResolution { ty: TypeNs::Struct(s), unresolved: None }) => VariantDef::Struct(s),
            Some(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }) => VariantDef::Variant(v),
            _ => {
                // A bare variant name like `Some`, imported into scope.
                match resolver.resolve_path_in_value_ns(self.db, path) {
                    Some(ValueResolution::Value(ValueNs::Variant(v))) => VariantDef::Variant(v),
                    _ => {
                        self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Pat(pat) });
                        return None;
                    }
                }
            }
        };
        let (adt, generic_def) = def.adt();
        let args: Box<[Ty<'db>]> =
            identity_args(self.db, generic_def).iter().map(|_| self.solver.new_var(VarKind::General)).collect();
        let ty = Ty::Adt(adt, args.clone());
        self.solver.equal(expected, &ty, self.pat_origin(pat));
        Some((def, ty, args))
    }

    /// The type of a path pattern: a unit variant or struct, or a const.
    fn path_pat_ty(&mut self, pat: PatId, path: &Path) -> Ty<'db> {
        let resolver = Resolver::for_body(self.db, self.def);
        let def = match resolver.resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(ValueNs::Variant(v))) => VariantDef::Variant(v),
            Some(ValueResolution::Value(ValueNs::Const(c))) => return const_ty(self.db, c).ty.clone(),
            Some(ValueResolution::Type(TypeResolution { ty: TypeNs::Struct(s), unresolved: None })) => {
                VariantDef::Struct(s)
            }
            _ => {
                self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Pat(pat) });
                return Ty::Error;
            }
        };
        let (adt, generic_def) = def.adt();
        let args = identity_args(self.db, generic_def).iter().map(|_| self.solver.new_var(VarKind::General)).collect();
        Ty::Adt(adt, args)
    }
}
