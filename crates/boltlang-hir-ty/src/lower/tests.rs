use boltlang_base_db::{File, TestDb};
use boltlang_hir_def::{
    ids::{ClassId, ItemContainerId},
    item_tree::{ModItem, item_tree},
};

use super::*;

fn item(db: &TestDb, file: File, index: usize) -> ModItem {
    item_tree(db, file).top_level[index]
}

fn function<'db>(db: &'db TestDb, file: File, index: usize) -> FunctionId<'db> {
    let ModItem::Function(idx) = item(db, file, index) else { panic!() };
    FunctionId::new(db, ItemContainerId::File(file), idx)
}

/// The signature of a function as `fn(A, ?) -> R`, with `?` for a missing
/// type.
fn show_sig(db: &TestDb, sig: &FnSig<'_>) -> String {
    let show = |t: &Option<Ty<'_>>| t.as_ref().map_or("?".to_string(), |t| t.display(db).to_string());
    format!("fn({}) -> {}", sig.params.iter().map(show).collect::<Vec<_>>().join(", "), show(&sig.ret))
}

const SOURCE: &str = "\
struct Pair[A, B](A, B);
enum List[T] { Nil, Cons(T, List[T]) }
type IntPair = Pair[i32, i32];
type Swap[X, Y] = Pair[Y, X];
class Show[T] { fn show(x: T) -> String; }
fn f[T: Show](x: T, p: Swap[Bool, T], l: List[u8]) -> (T, [IntPair], !) where Show[List[T]] {}
fn g(x, y: i32) {}
fn h(x: Missing, y: Show, z: Pair[i32]) -> fn(i32) -> Bool {}
type Loop = Loop;
fn k(l: Loop) -> i32 {}
impl[T] Show[List[T]] where T: Show {}
impl[T] List[T] {}
const N: Swap[u8, Bool] = 1;
";

#[test]
fn function_signatures() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", SOURCE);

    let f = function(&db, file, 5);
    let sig = fn_sig(&db, f);
    // The body decides the mutabilities that a return type doesn't say, so
    // the signature isn't complete.
    assert_eq!(show_sig(&db, sig), "fn(T, Pair[T, Bool], List[u8]) -> (T, [Pair[i32, i32]], !)");
    assert!(!sig.is_complete());
    assert!(sig.diagnostics.is_empty(), "{:?}", sig.diagnostics);
    let preds = &generic_predicates(&db, GenericDefId::Function(f)).predicates;
    let shown: Vec<_> = preds
        .iter()
        .map(|p| format!("{}[{}]", p.class.item(&db).name.as_ref().unwrap(), p.args[0].display(&db)))
        .collect();
    assert_eq!(shown, ["Show[T]", "Show[List[T]]"]);

    let g = fn_sig(&db, function(&db, file, 6));
    assert_eq!(show_sig(&db, g), "fn(?, i32) -> ?");
    assert!(!g.is_complete());

    let h = fn_sig(&db, function(&db, file, 7));
    assert_eq!(show_sig(&db, h), "fn({error}, {error}, Pair[i32, {error}]) -> fn(i32) -> Bool");
    assert!(matches!(
        &h.diagnostics[..],
        [
            LowerDiagnostic::UnresolvedType(_),
            LowerDiagnostic::NotAType(_),
            LowerDiagnostic::WrongArgCount { expected: 2, found: 1, .. }
        ]
    ));

    let k = fn_sig(&db, function(&db, file, 9));
    assert_eq!(show_sig(&db, k), "fn({error}) -> i32");
    assert!(matches!(&k.diagnostics[..], [LowerDiagnostic::RecursiveAlias(_)]));
}

#[test]
fn class_methods_return_unit_or_their_type() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "class C[T] { fn a(x: T); fn b(x: T) -> T; }\n");
    let ModItem::Class(idx) = item(&db, file, 0) else { panic!() };
    let class = ClassId::new(&db, file, idx);
    let sigs: Vec<_> = class
        .item(&db)
        .items
        .iter()
        .map(|item| {
            let boltlang_hir_def::item_tree::AssocItem::Function(idx) = *item else { panic!() };
            show_sig(&db, fn_sig(&db, FunctionId::new(&db, ItemContainerId::Class(class), idx)))
        })
        .collect();
    assert_eq!(sigs, ["fn(T) -> ()", "fn(T) -> T"]);
}

#[test]
fn fields_impls_and_consts() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", SOURCE);
    let ModItem::Enum(idx) = item(&db, file, 1) else { panic!() };
    let list = EnumId::new(&db, file, idx);
    let cons = &enum_field_types(&db, list)[1];
    assert_eq!(cons.kind, FieldsKind::Tuple);
    let tys: Vec<_> = cons.fields.iter().map(|(_, t)| t.display(&db).to_string()).collect();
    assert_eq!(tys, ["T", "List[T]"]);

    let ModItem::Impl(idx) = item(&db, file, 10) else { panic!() };
    let imp = ImplId::new(&db, file, idx);
    let c = impl_header(&db, imp).class.as_ref().unwrap();
    assert_eq!(c.args[0].display(&db).to_string(), "List[T]");
    assert_eq!(generic_predicates(&db, GenericDefId::Impl(imp)).predicates.len(), 1);

    // Only classes have impls.
    let ModItem::Impl(idx) = item(&db, file, 11) else { panic!() };
    let header = impl_header(&db, ImplId::new(&db, file, idx));
    assert!(header.class.is_none());
    assert!(matches!(&header.diagnostics[..], [LowerDiagnostic::NotAClass(_)]));

    let ModItem::Const(idx) = item(&db, file, 12) else { panic!() };
    let n = const_ty(&db, ConstId::new(&db, ItemContainerId::File(file), idx));
    assert_eq!(n.ty.display(&db).to_string(), "imm Pair[Bool, u8]");
}
