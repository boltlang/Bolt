//! Bodies built by hand, and how they print: the examples of #324.

use boltlang_base_db::{File, TestDb};
use boltlang_hir_def::{
    Name,
    arena::{Arena, Idx},
    builtin_type::{BuiltinInt, BuiltinType},
    hir::Mutability,
    ids::{ClassId, EnumId, EnumVariantId, FunctionId, ItemContainerId},
    item_tree::item_tree,
};
use boltlang_hir_ty::{
    AdtId, Ty,
    builtin::{Builtin, PrimOp},
};

use crate::{
    Block, Body, Callee, Constant, ConstructKind, Label, LabelId, Local, LocalId, Operand, Place,
    Rvalue, Stmt, StmtKind, Switch, SwitchArm, SwitchTest,
};

const SOURCE: &str = "
fn add(a: i32, b: i32) -> i32 = a;
class Index[C] { fn index(c: C, i: i32) -> i32; }
enum Option[T] { Some(T), None }
";

/// A body under construction.
struct Builder<'db> {
    locals: Arena<Local<'db>>,
    labels: Arena<Label>,
}

impl<'db> Builder<'db> {
    fn new() -> Self {
        Builder { locals: Arena::default(), labels: Arena::default() }
    }

    fn var(&mut self, name: &str, ty: Ty<'db>) -> LocalId<'db> {
        self.locals.alloc(Local { name: Some(Name::new(name)), ty, mutable: false, cell: false })
    }

    fn temp(&mut self, ty: Ty<'db>) -> LocalId<'db> {
        self.locals.alloc(Local { name: None, ty, mutable: false, cell: false })
    }

    fn label(&mut self, name: Option<&str>) -> LabelId {
        self.labels.alloc(Label { name: name.map(Name::new) })
    }

    fn finish(self, params: Vec<LocalId<'db>>, block: Block<'db>) -> Body<'db> {
        Body { locals: self.locals, labels: self.labels, params, block }
    }
}

fn stmt(kind: StmtKind<'_>) -> Stmt<'_> {
    Stmt { kind, source: None }
}

fn block<'db>(stmts: Vec<Stmt<'db>>, value: Operand<'db>) -> Block<'db> {
    Block { label: None, stmts, value }
}

fn local(l: LocalId<'_>) -> Operand<'_> {
    Operand::Local(l)
}

fn i32_ty<'db>() -> Ty<'db> {
    Ty::Builtin(BuiltinType::Int(BuiltinInt::I32))
}

fn int<'db>(value: i64) -> Operand<'db> {
    Operand::Const(Constant::Int {
        value: value.unsigned_abs() as u128,
        negative: value < 0,
        ty: i32_ty(),
    })
}

fn option_ty<'db>(db: &'db TestDb, file: File, arg: Ty<'db>) -> Ty<'db> {
    Ty::Adt(
        AdtId::Enum(option(db, file)),
        Box::new([arg]),
        Box::new([]),
        Box::new(Ty::Mutability(Mutability::Imm)),
    )
}

fn function<'db>(
    db: &'db TestDb,
    file: File,
    container: ItemContainerId<'db>,
    name: &str,
) -> FunctionId<'db> {
    let index = item_tree(db, file)
        .functions
        .iter()
        .position(|f| f.name.as_ref().is_some_and(|n| n.name().as_str() == name))
        .unwrap();
    FunctionId::new(db, container, Idx::new(index))
}

fn option(db: &TestDb, file: File) -> EnumId<'_> {
    EnumId::new(db, file, Idx::new(0))
}

fn variant<'db>(db: &'db TestDb, file: File, name: &str) -> EnumVariantId<'db> {
    option(db, file).variant(db, &Name::new(name)).unwrap()
}

#[test]
fn index_and_add() {
    // `xs[i] += 1` in generic code: the impl of `Index` isn't chosen yet.
    let db = TestDb::new();
    let file = db.file("/main.bolt", SOURCE);
    let class = ClassId::new(&db, file, Idx::new(0));
    let index = function(&db, file, ItemContainerId::Class(class), "index");
    let add = function(&db, file, ItemContainerId::File(file), "add");
    let array = Ty::Array(Box::new(Ty::Mutability(Mutability::Mut)), Box::new(i32_ty()));

    let mut b = Builder::new();
    let xs = b.var("xs", array.clone());
    let i = b.var("i", i32_ty());
    let t0 = b.temp(i32_ty());
    let t1 = b.temp(i32_ty());
    let get = Callee::ClassMethod { class, class_args: vec![array], method: index };
    let body = b.finish(
        vec![xs, i],
        block(
            vec![
                stmt(StmtKind::Let {
                    local: t0,
                    value: Rvalue::Call { callee: get.clone(), args: vec![local(xs), local(i)] },
                }),
                stmt(StmtKind::Let {
                    local: t1,
                    value: Rvalue::Call {
                        callee: Callee::Function { id: add, type_args: vec![] },
                        args: vec![local(t0), int(1)],
                    },
                }),
                stmt(StmtKind::Eval(Rvalue::Call {
                    callee: get,
                    args: vec![local(xs), local(t1)],
                })),
            ],
            Operand::unit(),
        ),
    );
    assert_eq!(
        body.display(&db).to_string(),
        "\
body(xs: mut [i32], i: i32) {
  let %0: i32 = call Index[mut [i32]].index(xs, i)
  let %1: i32 = call add(%0, 1i32)
  call Index[mut [i32]].index(xs, %1)
}
"
    );
}

#[test]
fn match_with_join_points() {
    // `match o { Some(Some(n)) if n > 0 => n, Some(_) => 0, None => -1 }`:
    // `Some(_)` is reached two ways, so its arm comes after the block
    // `'some`, which both of them break to.
    let db = TestDb::new();
    let file = db.file("/main.bolt", SOURCE);
    let some = variant(&db, file, "Some");
    let none = variant(&db, file, "None");
    let bool_ty = Ty::Builtin(BuiltinType::Bool);

    let mut b = Builder::new();
    let o = b.var("o", option_ty(&db, file, option_ty(&db, file, i32_ty())));
    let inner = b.temp(option_ty(&db, file, i32_ty()));
    let n = b.var("n", i32_ty());
    let positive = b.temp(bool_ty.clone());
    let result = b.temp(i32_ty());
    let l_match = b.label(Some("match"));
    let l_some = b.label(Some("some"));
    let gt = Callee::Builtin(Builtin::Prim(PrimOp::Gt, BuiltinType::Int(BuiltinInt::I32)));

    let inner_switch = Switch {
        scrutinee: local(inner),
        arms: vec![
            SwitchArm {
                test: SwitchTest::Variant { variant: some, fields: vec![Some(n)] },
                body: block(
                    vec![
                        stmt(StmtKind::Let {
                            local: positive,
                            value: Rvalue::Call { callee: gt, args: vec![local(n), int(0)] },
                        }),
                        stmt(StmtKind::Eval(Rvalue::If {
                            cond: local(positive),
                            then_block: block(
                                vec![stmt(StmtKind::Break {
                                    label: l_match,
                                    value: Some(local(n)),
                                })],
                                Operand::unit(),
                            ),
                            else_block: block(vec![], Operand::unit()),
                        })),
                        stmt(StmtKind::Break { label: l_some, value: None }),
                    ],
                    Operand::unit(),
                ),
            },
            SwitchArm {
                test: SwitchTest::Variant { variant: none, fields: vec![] },
                body: block(
                    vec![stmt(StmtKind::Break { label: l_some, value: None })],
                    Operand::unit(),
                ),
            },
        ],
        default: None,
    };
    let outer_switch = Switch {
        scrutinee: local(o),
        arms: vec![
            SwitchArm {
                test: SwitchTest::Variant { variant: some, fields: vec![Some(inner)] },
                body: block(
                    vec![stmt(StmtKind::Eval(Rvalue::Switch(inner_switch)))],
                    Operand::unit(),
                ),
            },
            SwitchArm {
                test: SwitchTest::Variant { variant: none, fields: vec![] },
                body: block(
                    vec![stmt(StmtKind::Break { label: l_match, value: Some(int(-1)) })],
                    Operand::unit(),
                ),
            },
        ],
        default: None,
    };
    let some_block = Block {
        label: Some(l_some),
        stmts: vec![stmt(StmtKind::Eval(Rvalue::Switch(outer_switch)))],
        value: Operand::unit(),
    };
    let match_block = Block {
        label: Some(l_match),
        stmts: vec![stmt(StmtKind::Eval(Rvalue::Block(some_block)))],
        value: int(0),
    };
    let body = b.finish(
        vec![o],
        block(
            vec![stmt(StmtKind::Let { local: result, value: Rvalue::Block(match_block) })],
            local(result),
        ),
    );
    assert_eq!(
        body.display(&db).to_string(),
        "\
body(o: Option[Option[i32]]) {
  let %2: i32 = block 'match {
    block 'some {
      switch o {
        Option.Some(%0) => {
          switch %0 {
            Option.Some(n) => {
              let %1: Bool = call i32_gt(n, 0i32)
              if %1 {
                break 'match n
              } else {}
              break 'some
            }
            Option.None => {
              break 'some
            }
          }
        }
        Option.None => {
          break 'match -1i32
        }
      }
    }
    0i32
  }
  %2
}
"
    );
}

#[test]
fn closure_cell_and_places() {
    // `let mut n = 0; let f = |x| { n = x; n }; let mut t = (n, True); t.0 = 2;`
    // and a struct-like object whose field is set, which every name sees.
    let db = TestDb::new();
    let pair = Ty::Tuple(Box::new([i32_ty(), Ty::Builtin(BuiltinType::Bool)]));

    let mut b = Builder::new();
    let n = b.locals.alloc(Local {
        name: Some(Name::new("n")),
        ty: i32_ty(),
        mutable: true,
        cell: true,
    });
    let x = b.var("x", i32_ty());
    let f = b.var(
        "f",
        Ty::Fn(Box::new([i32_ty()]), Box::new(i32_ty()), Box::new(Ty::Mutability(Mutability::Mut))),
    );
    let t =
        b.locals.alloc(Local { name: Some(Name::new("t")), ty: pair, mutable: true, cell: false });
    let shadow = b.var("x", i32_ty());
    let l_loop = b.label(None);

    let closure = Rvalue::Closure {
        captures: vec![n],
        params: vec![x],
        body: block(
            vec![stmt(StmtKind::Assign { place: Place::local(n), value: local(x) })],
            local(n),
        ),
    };
    let body = b.finish(
        vec![],
        block(
            vec![
                stmt(StmtKind::Let { local: n, value: Rvalue::Use(int(0)) }),
                stmt(StmtKind::Let { local: f, value: closure }),
                stmt(StmtKind::Let {
                    local: t,
                    value: Rvalue::Construct {
                        kind: ConstructKind::Tuple,
                        args: vec![local(n), Operand::Const(Constant::Bool(true))],
                    },
                }),
                stmt(StmtKind::Assign {
                    place: Place { local: t, fields: vec![0] },
                    value: int(2),
                }),
                stmt(StmtKind::Let {
                    local: shadow,
                    value: Rvalue::Project { base: local(t), field: 0 },
                }),
                stmt(StmtKind::SetField {
                    object: local(t),
                    field: 1,
                    value: Operand::Const(Constant::Char('b')),
                }),
                stmt(StmtKind::Eval(Rvalue::Loop {
                    label: l_loop,
                    body: block(
                        vec![
                            stmt(StmtKind::Eval(Rvalue::Call {
                                callee: Callee::Value(local(f)),
                                args: vec![local(shadow)],
                            })),
                            stmt(StmtKind::Continue { label: l_loop }),
                        ],
                        Operand::unit(),
                    ),
                })),
            ],
            Operand::Const(Constant::String("done".into())),
        ),
    );
    assert_eq!(
        body.display(&db).to_string(),
        "\
body() {
  let cell n: i32 = 0i32
  let f: mut fn(i32) -> i32 = closure [n] (x#1: i32) {
    n = x#1
    n
  }
  let mut t: (i32, Bool) = construct (n, True)
  t.0 = 2i32
  let x#4: i32 = project t.0
  set t.1 = 'b'
  loop 'l0 {
    call f(x#4)
    continue 'l0
  }
  \"done\"
}
"
    );
}
