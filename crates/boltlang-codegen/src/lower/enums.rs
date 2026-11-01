//! Enums with fields (#178): making a variant, like `Some(x)` or
//! `Shape.Rect { w, h }`, and matching one. How they're laid out is in
//! [`crate::layout`]: the index of the variant, then its fields.

use boltlang_hir_def::{
    Name,
    hir::{ExprId, PatId, Path},
    ids::EnumVariantId,
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
};
use boltlang_hir_ty::Ty;

use super::{Compiler, Frame, Result};
use crate::{
    ir::{BlockTy, Inst, Op, ValTy},
    layout,
};

impl<'db> Compiler<'db> {
    /// Where the fields of the variant `v` of an enum with fields of type
    /// `ty` are.
    fn variant_layout(&self, ty: &Ty<'db>, v: EnumVariantId<'db>, at: ExprId) -> Result<'db, layout::Layout<'db>> {
        layout::variant_layout(self.db, ty, v)
            .ok_or_else(|| self.unsupported(at, format!("a variant of `{}` with these fields", ty.display(self.db))))
    }

    /// A new value of the variant `v` of the enum with fields of the type of
    /// `expr`, with the values of `given` for its fields.
    pub(super) fn construct_variant(&mut self, expr: ExprId, v: EnumVariantId<'db>, given: &[(Name, ExprId)]) -> Result<'db, ()> {
        let ty = self.ty(expr);
        let layout = self.variant_layout(&ty, v, expr)?;
        let address = self.alloc(layout.size.max(4));
        self.push(Inst::LocalGet(address));
        self.push(Inst::I32Const(v.index as i32));
        self.push(Inst::Store(ValTy::I32, 0));
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

    /// The variant that the path of a pattern, like the `Some` of `Some(x)`,
    /// names.
    pub(super) fn pattern_variant(&self, path: &Path, at: ExprId) -> Result<'db, EnumVariantId<'db>> {
        match Resolver::for_body(self.db, self.def).resolve_path_in_value_ns(self.db, path) {
            Some(
                ValueResolution::Value(ValueNs::Variant(v))
                | ValueResolution::Type(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }),
            ) => Ok(v),
            _ => Err(self.unsupported(at, "this pattern of an enum")),
        }
    }

    /// Test whether the value in `local`, of the enum with fields `ty`, is
    /// the variant `v` with fields that match `fields`, leaving 1 or 0. The
    /// fields are only read when it's that variant: those of another may
    /// not be there.
    pub(super) fn variant_pattern(
        &mut self,
        v: EnumVariantId<'db>,
        fields: &[(Name, PatId)],
        local: Option<u32>,
        ty: &Ty<'db>,
        at: ExprId,
    ) -> Result<'db, ()> {
        let local = local.expect("an enum with fields is an address");
        self.push(Inst::LocalGet(local));
        self.push(Inst::Load(ValTy::I32, 0));
        self.push(Inst::I32Const(v.index as i32));
        self.op(Op::I32Eq);
        if fields.is_empty() {
            return Ok(());
        }
        let layout = self.variant_layout(ty, v, at)?;
        self.open(Inst::If(BlockTy::Value(ValTy::I32)), Frame::Plain);
        self.slot_patterns(fields, Some(local), &layout, at)?;
        self.push(Inst::Else);
        self.push(Inst::I32Const(0));
        self.close();
        Ok(())
    }
}
