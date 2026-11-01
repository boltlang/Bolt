//! Arrays in linear memory (#179): literals, the builtins that `std`'s
//! `array.bolt` declares, like `array_len`, and `c[i]` and `c[i] = x`, which
//! call the impls of `Index` and `IndexMut`, as the evaluator does. How an
//! array is laid out is in [`crate::layout`].
//!
//! An element that isn't there stops the program with the evaluator's error,
//! through the host: see [`Failure`].

use boltlang_hir_def::hir::{ExprId, Mutability};
use boltlang_hir_ty::{
    Ty,
    builtin::Builtin,
    instances::{impl_method, select_impl},
    lang::LangClass,
};

use super::{Compiler, Frame, Result};
use crate::{
    Instance,
    ir::{BlockTy, Callee, Failure, Inst, Op, ValTy},
    layout::{self, Element},
};

impl<'db> Compiler<'db> {
    /// How the elements of arrays of type `ty` are kept.
    fn element_of(&self, ty: &Ty<'db>, at: ExprId) -> Result<'db, Element> {
        match ty {
            Ty::Array(_, elem) => layout::element(self.db, elem)
                .ok_or_else(|| self.unsupported(at, format!("an array of `{}`", elem.display(self.db)))),
            _ => Err(self.unsupported(at, format!("a `{}` as an array", ty.display(self.db)))),
        }
    }

    /// `[a, b, c]`.
    pub(super) fn array_literal(&mut self, expr: ExprId, items: &[ExprId]) -> Result<'db, ()> {
        // `[]` is its length, whatever its elements would be, which the types
        // may not even say, as for `array_len([])`.
        if items.is_empty() {
            let address = self.alloc(4);
            self.push(Inst::LocalGet(address));
            self.push(Inst::I32Const(0));
            self.push(Inst::Store(ValTy::I32, 0));
            self.push(Inst::LocalGet(address));
            return Ok(());
        }
        let el = self.element_of(&self.ty(expr), expr)?;
        let address = self.alloc(el.start + items.len() as u32 * el.size);
        self.push(Inst::LocalGet(address));
        self.push(Inst::I32Const(items.len() as i32));
        self.push(Inst::Store(ValTy::I32, 0));
        for (i, &item) in items.iter().enumerate() {
            if el.val_ty.is_none() {
                self.expr_as(item, None)?;
                continue;
            }
            self.push(Inst::LocalGet(address));
            self.expr(item)?;
            self.store_element(el, el.start + i as u32 * el.size);
        }
        self.push(Inst::LocalGet(address));
        Ok(())
    }

    /// `[value; count]`: `value`, evaluated once, in each element, as in the
    /// evaluator.
    pub(super) fn array_repeat(&mut self, expr: ExprId, value: ExprId, count: ExprId) -> Result<'db, ()> {
        let el = self.element_of(&self.ty(expr), expr)?;
        let value = self.temp(value)?;
        let count = self.temp(count)?.expect("a `usize` is an `i32`");
        let address = self.new_array(el, count, false);
        if let Some(value) = value {
            let i = self.new_local(ValTy::I32);
            self.push(Inst::I32Const(0));
            self.push(Inst::LocalSet(i));
            self.open(Inst::Block(BlockTy::Empty), Frame::Plain);
            self.open(Inst::Loop(BlockTy::Empty), Frame::Plain);
            self.push(Inst::LocalGet(i));
            self.push(Inst::LocalGet(count));
            self.op(Op::I32GeU);
            self.push(Inst::BrIf(1));
            self.element_address(address, i, el);
            self.push(Inst::LocalGet(value));
            self.store_element(el, el.start);
            self.push(Inst::LocalGet(i));
            self.push(Inst::I32Const(1));
            self.op(Op::I32Add);
            self.push(Inst::LocalSet(i));
            self.push(Inst::Br(0));
            self.close();
            self.close();
        }
        self.push(Inst::LocalGet(address));
        Ok(())
    }

    /// A builtin of `std`'s `array.bolt`, like `array_len`, called with
    /// `args` at `expr`. The type of an array argument, or of the result,
    /// says how its elements are kept.
    pub(super) fn array_builtin(&mut self, expr: ExprId, builtin: Builtin, args: &[ExprId]) -> Result<'db, ()> {
        match (builtin, args) {
            (Builtin::ArrayLen, &[xs]) => {
                self.expr(xs)?;
                self.push(Inst::Load(ValTy::I32, 0));
            }
            (Builtin::ArrayGet, &[xs, i]) => {
                let el = self.element_of(&self.ty(xs), xs)?;
                let xs = self.temp(xs)?.expect("an array is an address");
                let i = self.temp(i)?.expect("an index is an `i32`");
                self.check_index(xs, i);
                self.element_address(xs, i, el);
                self.load_element(el, el.start);
            }
            (Builtin::ArraySet, &[xs, i, x]) => {
                let el = self.element_of(&self.ty(xs), xs)?;
                let xs = self.temp(xs)?.expect("an array is an address");
                let i = self.temp(i)?.expect("an index is an `i32`");
                let x = self.temp(x)?;
                self.check_index(xs, i);
                if let Some(x) = x {
                    self.element_address(xs, i, el);
                    self.push(Inst::LocalGet(x));
                    self.store_element(el, el.start);
                }
            }
            // The elements aren't set: memory is zero, and nothing is freed
            // yet. The evaluator stops at reading one, but only `std` can
            // make such an array, and keeps track of which are set.
            (Builtin::ArrayAlloc, &[len]) => {
                let el = self.element_of(&self.ty(expr), expr)?;
                let len = self.temp(len)?.expect("a length is an `i32`");
                let address = self.new_array(el, len, true);
                self.push(Inst::LocalGet(address));
            }
            (Builtin::ArrayCopy, &[dst, dst_start, src, src_start, count]) => {
                let el = self.element_of(&self.ty(src), src)?;
                let [dst, dst_start, src, src_start, count] = [dst, dst_start, src, src_start, count]
                    .map(|e| self.temp(e).map(|local| local.expect("an array or an `i32`")));
                let (dst, dst_start, src, src_start, count) = (dst?, dst_start?, src?, src_start?, count?);
                // The source's range first, like the evaluator.
                self.check_range(src, src_start, count);
                self.check_range(dst, dst_start, count);
                // `memory.copy` copies overlapping ranges as they were.
                self.element_address(dst, dst_start, el);
                self.push(Inst::I32Const(el.start as i32));
                self.op(Op::I32Add);
                self.element_address(src, src_start, el);
                self.push(Inst::I32Const(el.start as i32));
                self.op(Op::I32Add);
                self.push(Inst::LocalGet(count));
                self.push(Inst::I32Const(el.size as i32));
                self.op(Op::I32Mul);
                self.push(Inst::MemoryCopy);
            }
            // An `imm` array never changes, so it's its own copy; one that
            // may change is copied.
            (Builtin::ArrayFreeze, &[xs]) => {
                let ty = self.ty(xs);
                if let Ty::Array(m, _) = &ty
                    && **m == Ty::Mutability(Mutability::Imm)
                {
                    return self.expr(xs);
                }
                let el = self.element_of(&ty, xs)?;
                let xs = self.temp(xs)?.expect("an array is an address");
                let bytes = self.new_local(ValTy::I32);
                self.push(Inst::LocalGet(xs));
                self.push(Inst::Load(ValTy::I32, 0));
                self.push(Inst::I32Const(el.size as i32));
                self.op(Op::I32Mul);
                self.push(Inst::I32Const(el.start as i32));
                self.op(Op::I32Add);
                self.push(Inst::LocalSet(bytes));
                self.push(Inst::LocalGet(bytes));
                let copy = self.alloc_from_stack();
                self.push(Inst::LocalGet(copy));
                self.push(Inst::LocalGet(xs));
                self.push(Inst::LocalGet(bytes));
                self.push(Inst::MemoryCopy);
                self.push(Inst::LocalGet(copy));
            }
            _ => return Err(self.unsupported(expr, format!("the builtin `{}`", builtin.name()))),
        }
        Ok(())
    }

    /// `c[i]` at `expr`: the `index` of the impl of `Index` for `c`.
    pub(super) fn index(&mut self, expr: ExprId, base: ExprId, index: ExprId) -> Result<'db, ()> {
        let i32 = self.ty(index);
        let instance = self.lang_instance(expr, LangClass::Index, vec![self.ty(base), i32], self.ty(expr))?;
        self.expr(base)?;
        self.expr(index)?;
        self.call_callee(Callee::Instance(instance));
        Ok(())
    }

    /// `c[i] = x` at `place`, `c[i]`, with `x` on the stack, if it has a
    /// value: the `set_index` of the impl of `IndexMut` for `c`.
    pub(super) fn assign_index(&mut self, place: ExprId, base: ExprId, index: ExprId) -> Result<'db, ()> {
        let value = self.expr_val_ty(place)?.map(|ty| self.new_local(ty));
        if let Some(value) = value {
            self.push(Inst::LocalSet(value));
        }
        let params = vec![self.ty(base), self.ty(index), self.ty(place)];
        let instance = self.lang_instance(place, LangClass::IndexMut, params, Ty::unit())?;
        self.expr(base)?;
        self.expr(index)?;
        if let Some(value) = value {
            self.push(Inst::LocalGet(value));
        }
        self.call_callee(Callee::Instance(instance));
        Ok(())
    }

    /// The instance of the method of `lang` that syntax at `at` calls, with
    /// arguments of the types `params` and a result of type `ret`: of the impl
    /// the type checker chose, or in a generic function the one for the
    /// instance's types.
    fn lang_instance(&self, at: ExprId, lang: LangClass, params: Vec<Ty<'db>>, ret: Ty<'db>) -> Result<'db, Instance<'db>> {
        let db = self.db;
        let file = self.def.def(db).file(db);
        let unsupported = || self.unsupported(at, format!("`{}` without its class in the prelude", lang.name()));
        let class = lang.class(db, file).ok_or_else(unsupported)?;
        let method = lang.method(db, class).ok_or_else(unsupported)?;
        let imp = match self.infer.instance(at, class) {
            Some(imp) => imp,
            None => select_impl(db, &[self.root], class, &params[..1])
                .ok_or_else(|| self.unsupported(at, format!("`{}` of a `{}`", lang.name(), params[0].display(db))))?,
        };
        let target = impl_method(db, imp, method);
        self.instance_at(at, target, &Ty::func(params, ret))
    }

    /// A new array of `len` elements, the local `len`, which aren't set, in
    /// a new local that it gives. A negative `len` is an overflow when it's
    /// `signed`, as the evaluator has it.
    fn new_array(&mut self, el: Element, len: u32, signed: bool) -> u32 {
        if signed {
            self.push(Inst::LocalGet(len));
            self.push(Inst::I32Const(0));
            self.op(Op::I32LtS);
            self.fail_if(Failure::Overflow, &[]);
        }
        self.push(Inst::LocalGet(len));
        self.push(Inst::I32Const(el.size as i32));
        self.op(Op::I32Mul);
        self.push(Inst::I32Const(el.start as i32));
        self.op(Op::I32Add);
        let address = self.alloc_from_stack();
        self.push(Inst::LocalGet(address));
        self.push(Inst::LocalGet(len));
        self.push(Inst::Store(ValTy::I32, 0));
        address
    }

    /// Allocate as many bytes as the top of the stack says, into a new local
    /// that it gives.
    fn alloc_from_stack(&mut self) -> u32 {
        self.call_callee(Callee::Alloc);
        let local = self.new_local(ValTy::I32);
        self.push(Inst::LocalSet(local));
        local
    }

    /// The address of the element at the local `i` of the array at the local
    /// `xs`, without the offset of the first element.
    fn element_address(&mut self, xs: u32, i: u32, el: Element) {
        self.push(Inst::LocalGet(xs));
        self.push(Inst::LocalGet(i));
        if el.size != 1 {
            self.push(Inst::I32Const(el.size as i32));
            self.op(Op::I32Mul);
        }
        self.op(Op::I32Add);
    }

    /// Stop with [`Failure::IndexOutOfBounds`] unless the local `i` is an
    /// index of the array at the local `xs`. A negative one isn't, as an
    /// unsigned number that's too large.
    fn check_index(&mut self, xs: u32, i: u32) {
        self.push(Inst::LocalGet(i));
        self.push(Inst::LocalGet(xs));
        self.push(Inst::Load(ValTy::I32, 0));
        self.op(Op::I32GeU);
        self.push(Inst::LocalGet(xs));
        self.fail_if(Failure::IndexOutOfBounds, &[i]);
    }

    /// Stop with [`Failure::RangeOutOfBounds`] unless the `count` elements
    /// from `start` of the array at `xs`, all locals, are there.
    fn check_range(&mut self, xs: u32, start: u32, count: u32) {
        let len = |c: &mut Self| {
            c.push(Inst::LocalGet(xs));
            c.push(Inst::Load(ValTy::I32, 0));
        };
        self.push(Inst::LocalGet(start));
        len(self);
        self.op(Op::I32GtU);
        self.push(Inst::LocalGet(count));
        self.push(Inst::I32Const(0));
        self.op(Op::I32LtS);
        self.op(Op::I32Or);
        self.push(Inst::LocalGet(count));
        len(self);
        self.push(Inst::LocalGet(start));
        self.op(Op::I32Sub);
        self.op(Op::I32GtU);
        self.op(Op::I32Or);
        self.push(Inst::LocalGet(xs));
        self.fail_if(Failure::RangeOutOfBounds, &[start, count]);
    }

    /// With a condition and an array's address on the stack: stop with
    /// `failure` if the condition holds, passing the locals `args` and then
    /// the array's length, if `failure` takes it. The address is dropped.
    fn fail_if(&mut self, failure: Failure, args: &[u32]) {
        // The address, kept for the length.
        let xs = (failure.arity() > args.len()).then(|| self.new_local(ValTy::I32));
        match xs {
            Some(xs) => {
                // Under the condition: swap them through the local.
                let condition = self.new_local(ValTy::I32);
                self.push(Inst::LocalSet(xs));
                self.push(Inst::LocalSet(condition));
                self.push(Inst::LocalGet(condition));
            }
            None if failure.arity() > 0 => self.push(Inst::Drop),
            None => {}
        }
        self.open(Inst::If(BlockTy::Empty), Frame::Plain);
        for &arg in args {
            self.push(Inst::LocalGet(arg));
        }
        if let Some(xs) = xs {
            self.push(Inst::LocalGet(xs));
            self.push(Inst::Load(ValTy::I32, 0));
        }
        self.call_callee(Callee::Fail(failure));
        self.push(Inst::Unreachable);
        self.close();
    }

    fn store_element(&mut self, el: Element, offset: u32) {
        match (el.narrow, el.val_ty) {
            (Some(narrow), _) => self.push(Inst::StoreNarrow(narrow, offset)),
            (None, Some(val_ty)) => self.push(Inst::Store(val_ty, offset)),
            (None, None) => {}
        }
    }

    /// Load the element at the address on the stack plus `offset`, or drop
    /// the address for an element without a value.
    fn load_element(&mut self, el: Element, offset: u32) {
        match (el.narrow, el.val_ty) {
            (Some(narrow), _) => self.push(Inst::LoadNarrow(narrow, offset)),
            (None, Some(val_ty)) => self.push(Inst::Load(val_ty, offset)),
            (None, None) => self.push(Inst::Drop),
        }
    }
}
