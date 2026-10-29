//! Linking: number the compiled instances and imports, and emit them as a
//! Wasm module. It's cheap, so it isn't a query: it runs every time, on top
//! of [`compile_instance`], which is.

use std::collections::HashMap;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    body::DefWithBody,
    ids::{FunctionId, ItemContainerId},
    item_tree::FunctionName,
};
use boltlang_hir_ty::{Ty, function_scheme};
use boltlang_hir_ty::builtin::Builtin;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, ImportSection, Instruction, MemorySection, MemoryType, Module, NameMap, NameSection,
    TypeSection, ValType,
};

use crate::{
    CodegenError, ErrorKind, Instance, compile_instance,
    ir::{BlockTy, Callee, CompiledFn, Inst, Op, ValTy},
    reachable_instances,
};

type Signature = (Vec<ValTy>, Option<ValTy>);

/// The Wasm module of the program whose entry is `file`, or why it can't be
/// compiled.
pub fn link<'db>(db: &'db dyn Db, file: File) -> Result<Vec<u8>, Vec<CodegenError<'db>>> {
    let instances = reachable_instances(db, file);
    let mut compiled: Vec<(Instance<'db>, &CompiledFn<'db>)> = Vec::new();
    let mut errors = Vec::new();
    for &instance in instances {
        match compile_instance(db, instance) {
            Ok(code) => compiled.push((instance, code)),
            Err(e) => errors.push(e.clone()),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // Imports come first in the index space of functions.
    let mut imports: Vec<Callee<'db>> = Vec::new();
    for (_, code) in &compiled {
        for callee in &code.callees {
            if matches!(callee, Callee::Import(_) | Callee::Builtin(_)) && !imports.contains(callee) {
                imports.push(*callee);
            }
        }
    }
    let mut index: HashMap<Callee<'db>, u32> = HashMap::new();
    for (i, callee) in imports.iter().enumerate() {
        index.insert(*callee, i as u32);
    }
    let strings = Strings::new(compiled.iter().flat_map(|(_, code)| &code.code));
    for (i, (instance, _)) in compiled.iter().enumerate() {
        index.insert(Callee::Instance(*instance), (imports.len() + i) as u32);
    }

    let mut signatures: Vec<Signature> = Vec::new();
    let mut type_of = |signature: Signature| -> u32 {
        match signatures.iter().position(|s| *s == signature) {
            Some(i) => i as u32,
            None => {
                signatures.push(signature);
                signatures.len() as u32 - 1
            }
        }
    };

    let mut import_section = ImportSection::new();
    let mut names = NameMap::new();
    for (i, callee) in imports.iter().enumerate() {
        let (module, name, signature) = match *callee {
            Callee::Import(f) => ("env", function_name(db, f), import_signature(db, f)?),
            Callee::Builtin(builtin) => ("bolt", builtin.name().to_string(), builtin_signature(builtin)),
            Callee::Instance(_) => unreachable!("an instance isn't imported"),
        };
        import_section.import(module, &name, EntityType::Function(type_of(signature)));
        names.append(i as u32, &name);
    }

    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let mut codes = CodeSection::new();
    let mut exported = vec!["main".to_string()];
    for (i, (instance, code)) in compiled.iter().enumerate() {
        let idx = (imports.len() + i) as u32;
        functions.function(type_of((code.params.clone(), code.result)));
        let name = instance_name(db, *instance);
        if i == 0 {
            exports.export("main", ExportKind::Func, idx);
        } else if let Some(name) = export_name(db, *instance)
            && !exported.contains(&name)
        {
            exports.export(&name, ExportKind::Func, idx);
            exported.push(name);
        }
        names.append(idx, &name);
        codes.function(&function(code, &index, &strings));
    }
    exports.export("memory", ExportKind::Memory, 0);

    // One memory, with the strings from `STRINGS_START` on.
    let mut memories = MemorySection::new();
    let pages = (strings.end as u64).div_ceil(65536).max(1);
    memories.memory(MemoryType { minimum: pages, maximum: None, memory64: false, shared: false, page_size_log2: None });
    let mut data = DataSection::new();
    if !strings.bytes.is_empty() {
        data.active(0, &ConstExpr::i32_const(STRINGS_START as i32), strings.bytes.iter().copied());
    }

    let mut types = TypeSection::new();
    for (params, result) in &signatures {
        types.ty().function(params.iter().map(|&t| val_type(t)), result.iter().map(|&t| val_type(t)));
    }
    let mut name_section = NameSection::new();
    name_section.functions(&names);

    let mut module = Module::new();
    module.section(&types);
    module.section(&import_section);
    module.section(&functions);
    module.section(&memories);
    module.section(&exports);
    module.section(&codes);
    module.section(&data);
    module.section(&name_section);
    let bytes = module.finish();

    wasmparser::Validator::new()
        .validate_all(&bytes)
        .map_err(|e| vec![CodegenError { kind: ErrorKind::Invalid(e.to_string()), location: None }])?;
    Ok(bytes)
}

/// Where the strings start in memory. Address 0 is left unused, so that it
/// can stand for no string later.
const STRINGS_START: u32 = 8;

/// The string literals of the module, each once, and their addresses.
struct Strings {
    addresses: HashMap<Box<str>, u32>,
    /// The memory from `STRINGS_START` on.
    bytes: Vec<u8>,
    end: u32,
}

impl Strings {
    fn new<'a, 'db: 'a>(code: impl Iterator<Item = &'a Inst<'db>>) -> Self {
        let mut strings = Strings { addresses: HashMap::new(), bytes: Vec::new(), end: STRINGS_START };
        for inst in code {
            let Inst::Str(s) = inst else { continue };
            if strings.addresses.contains_key(s) {
                continue;
            }
            // Lengths are aligned to 4 bytes.
            while strings.end % 4 != 0 {
                strings.bytes.push(0);
                strings.end += 1;
            }
            strings.addresses.insert(s.clone(), strings.end);
            strings.bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());
            strings.bytes.extend_from_slice(s.as_bytes());
            strings.end += 4 + s.len() as u32;
        }
        strings
    }
}

fn function<'db>(code: &CompiledFn<'db>, index: &HashMap<Callee<'db>, u32>, strings: &Strings) -> Function {
    // Consecutive locals of the same type are declared together.
    let mut locals: Vec<(u32, ValType)> = Vec::new();
    for &ty in &code.locals {
        match locals.last_mut() {
            Some((count, last)) if *last == val_type(ty) => *count += 1,
            _ => locals.push((1, val_type(ty))),
        }
    }
    let mut f = Function::new(locals);
    for inst in &code.code {
        f.instruction(&instruction(inst, index, strings));
    }
    f
}

fn val_type(ty: ValTy) -> ValType {
    match ty {
        ValTy::I32 => ValType::I32,
        ValTy::I64 => ValType::I64,
        ValTy::F32 => ValType::F32,
        ValTy::F64 => ValType::F64,
    }
}

fn block_type(ty: BlockTy) -> BlockType {
    match ty {
        BlockTy::Empty => BlockType::Empty,
        BlockTy::Value(ty) => BlockType::Result(val_type(ty)),
    }
}

fn instruction<'a, 'db>(inst: &Inst<'db>, index: &HashMap<Callee<'db>, u32>, strings: &Strings) -> Instruction<'a> {
    match inst {
        Inst::Str(s) => Instruction::I32Const(strings.addresses[s] as i32),
        Inst::I32Const(n) => Instruction::I32Const(*n),
        Inst::I64Const(n) => Instruction::I64Const(*n),
        Inst::F32Const(bits) => Instruction::F32Const(f32::from_bits(*bits).into()),
        Inst::F64Const(bits) => Instruction::F64Const(f64::from_bits(*bits).into()),
        Inst::LocalGet(i) => Instruction::LocalGet(*i),
        Inst::LocalSet(i) => Instruction::LocalSet(*i),
        Inst::LocalTee(i) => Instruction::LocalTee(*i),
        Inst::Call(callee) => Instruction::Call(index[callee]),
        Inst::Block(ty) => Instruction::Block(block_type(*ty)),
        Inst::Loop(ty) => Instruction::Loop(block_type(*ty)),
        Inst::If(ty) => Instruction::If(block_type(*ty)),
        Inst::Else => Instruction::Else,
        Inst::End => Instruction::End,
        Inst::Br(depth) => Instruction::Br(*depth),
        Inst::BrIf(depth) => Instruction::BrIf(*depth),
        Inst::Return => Instruction::Return,
        Inst::Unreachable => Instruction::Unreachable,
        Inst::Drop => Instruction::Drop,
        Inst::Op(op) => op_instruction(*op),
    }
}

fn op_instruction<'a>(op: Op) -> Instruction<'a> {
    use Instruction as I;
    match op {
        Op::I32Add => I::I32Add,
        Op::I32Sub => I::I32Sub,
        Op::I32Mul => I::I32Mul,
        Op::I32DivS => I::I32DivS,
        Op::I32DivU => I::I32DivU,
        Op::I32RemS => I::I32RemS,
        Op::I32RemU => I::I32RemU,
        Op::I32And => I::I32And,
        Op::I32Or => I::I32Or,
        Op::I32Xor => I::I32Xor,
        Op::I32Shl => I::I32Shl,
        Op::I32ShrS => I::I32ShrS,
        Op::I32ShrU => I::I32ShrU,
        Op::I32Eq => I::I32Eq,
        Op::I32Ne => I::I32Ne,
        Op::I32LtS => I::I32LtS,
        Op::I32LtU => I::I32LtU,
        Op::I32LeS => I::I32LeS,
        Op::I32LeU => I::I32LeU,
        Op::I32GtS => I::I32GtS,
        Op::I32GtU => I::I32GtU,
        Op::I32GeS => I::I32GeS,
        Op::I32GeU => I::I32GeU,
        Op::I32Eqz => I::I32Eqz,
        Op::I32Extend8S => I::I32Extend8S,
        Op::I32Extend16S => I::I32Extend16S,
        Op::I64Add => I::I64Add,
        Op::I64Sub => I::I64Sub,
        Op::I64Mul => I::I64Mul,
        Op::I64DivS => I::I64DivS,
        Op::I64DivU => I::I64DivU,
        Op::I64RemS => I::I64RemS,
        Op::I64RemU => I::I64RemU,
        Op::I64And => I::I64And,
        Op::I64Or => I::I64Or,
        Op::I64Xor => I::I64Xor,
        Op::I64Shl => I::I64Shl,
        Op::I64ShrS => I::I64ShrS,
        Op::I64ShrU => I::I64ShrU,
        Op::I64Eq => I::I64Eq,
        Op::I64Ne => I::I64Ne,
        Op::I64LtS => I::I64LtS,
        Op::I64LtU => I::I64LtU,
        Op::I64LeS => I::I64LeS,
        Op::I64LeU => I::I64LeU,
        Op::I64GtS => I::I64GtS,
        Op::I64GtU => I::I64GtU,
        Op::I64GeS => I::I64GeS,
        Op::I64GeU => I::I64GeU,
        Op::F32Add => I::F32Add,
        Op::F32Sub => I::F32Sub,
        Op::F32Mul => I::F32Mul,
        Op::F32Div => I::F32Div,
        Op::F32Neg => I::F32Neg,
        Op::F32Eq => I::F32Eq,
        Op::F32Ne => I::F32Ne,
        Op::F32Lt => I::F32Lt,
        Op::F32Le => I::F32Le,
        Op::F32Gt => I::F32Gt,
        Op::F32Ge => I::F32Ge,
        Op::F64Add => I::F64Add,
        Op::F64Sub => I::F64Sub,
        Op::F64Mul => I::F64Mul,
        Op::F64Div => I::F64Div,
        Op::F64Neg => I::F64Neg,
        Op::F64Eq => I::F64Eq,
        Op::F64Ne => I::F64Ne,
        Op::F64Lt => I::F64Lt,
        Op::F64Le => I::F64Le,
        Op::F64Gt => I::F64Gt,
        Op::F64Ge => I::F64Ge,
        Op::I32WrapI64 => I::I32WrapI64,
        Op::I64ExtendI32S => I::I64ExtendI32S,
        Op::I64ExtendI32U => I::I64ExtendI32U,
        Op::F32ConvertI32S => I::F32ConvertI32S,
        Op::F32ConvertI32U => I::F32ConvertI32U,
        Op::F32ConvertI64S => I::F32ConvertI64S,
        Op::F32ConvertI64U => I::F32ConvertI64U,
        Op::F64ConvertI32S => I::F64ConvertI32S,
        Op::F64ConvertI32U => I::F64ConvertI32U,
        Op::F64ConvertI64S => I::F64ConvertI64S,
        Op::F64ConvertI64U => I::F64ConvertI64U,
        Op::I32TruncSatF32S => I::I32TruncSatF32S,
        Op::I32TruncSatF32U => I::I32TruncSatF32U,
        Op::I32TruncSatF64S => I::I32TruncSatF64S,
        Op::I32TruncSatF64U => I::I32TruncSatF64U,
        Op::I64TruncSatF32S => I::I64TruncSatF32S,
        Op::I64TruncSatF32U => I::I64TruncSatF32U,
        Op::I64TruncSatF64S => I::I64TruncSatF64S,
        Op::I64TruncSatF64U => I::I64TruncSatF64U,
        Op::F32DemoteF64 => I::F32DemoteF64,
        Op::F64PromoteF32 => I::F64PromoteF32,
    }
}

/// The signature of the import of a builtin: strings are addresses.
fn builtin_signature(builtin: Builtin) -> Signature {
    match builtin {
        Builtin::Print | Builtin::Panic => (vec![ValTy::I32], None),
        Builtin::ArrayLen
        | Builtin::ArrayAlloc
        | Builtin::ArrayGet
        | Builtin::ArraySet
        | Builtin::ArrayCopy
        | Builtin::ArrayFreeze => {
            unreachable!("only the builtins of `Builtin::HOST` are called")
        }
    }
}

/// The signature of a function of an `extern` block.
fn import_signature<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Result<Signature, Vec<CodegenError<'db>>> {
    let unsupported = |what: String| vec![CodegenError { kind: ErrorKind::Unsupported(what), location: None }];
    let scheme = function_scheme(db, f).map_err(|_| unsupported("this extern function".into()))?;
    let Ty::Fn(params, ret, _) = &scheme.ty else { unreachable!("a function has a function type") };
    let val_ty = |ty: &Ty<'db>| -> Result<Option<ValTy>, Vec<CodegenError<'db>>> {
        use boltlang_hir_def::builtin_type::{BuiltinInt::*, BuiltinType::*, BuiltinUint::*, BuiltinFloat};
        match ty {
            Ty::Tuple(tys) if tys.is_empty() => Ok(None),
            Ty::Builtin(Bool | Char | String | Int(I8 | I16 | I32 | Isize) | Uint(U8 | U16 | U32 | Usize)) => {
                Ok(Some(ValTy::I32))
            }
            Ty::Builtin(Int(I64) | Uint(U64)) => Ok(Some(ValTy::I64)),
            Ty::Builtin(Float(BuiltinFloat::F32)) => Ok(Some(ValTy::F32)),
            Ty::Builtin(Float(BuiltinFloat::F64)) => Ok(Some(ValTy::F64)),
            ty => Err(unsupported(format!(
                "the type `{}` in the extern function `{}`",
                ty.display(db),
                function_name(db, f)
            ))),
        }
    };
    let mut wasm_params = Vec::new();
    for param in params.iter() {
        wasm_params.extend(val_ty(param)?);
    }
    Ok((wasm_params, val_ty(ret)?))
}

fn function_name(db: &dyn Db, f: FunctionId<'_>) -> String {
    match &f.item(db).name {
        // `Vec.new` for one in the namespace of a type.
        Some(FunctionName::Name(_)) => f.item(db).display_name().map_or("?".to_string(), |n| n.to_string()),
        Some(FunctionName::Operator(op)) => format!("({op})"),
        None => "?".to_string(),
    }
}

/// A name for the name section, like `id[i32]`.
fn instance_name<'db>(db: &'db dyn Db, instance: Instance<'db>) -> String {
    let DefWithBody::Function(f) = instance.body(db).def(db) else { return "main".to_string() };
    let args = instance.type_args(db);
    if args.is_empty() {
        return function_name(db, f);
    }
    let args: Vec<_> = args.iter().map(|t| t.display(db).to_string()).collect();
    format!("{}[{}]", function_name(db, f), args.join(", "))
}

/// The name a function is exported as: top-level functions that aren't
/// generic, under their own names.
fn export_name<'db>(db: &'db dyn Db, instance: Instance<'db>) -> Option<String> {
    let DefWithBody::Function(f) = instance.body(db).def(db) else { return None };
    let top_level = matches!(f.container(db), ItemContainerId::File(_));
    let named = matches!(f.item(db).name, Some(FunctionName::Name(_)));
    (top_level && named && instance.type_args(db).is_empty()).then(|| function_name(db, f))
}
