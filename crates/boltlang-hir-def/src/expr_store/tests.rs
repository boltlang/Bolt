use boltlang_syntax::{SyntaxNode, ast::{self, Node}};

use super::*;
use crate::hir::*;

/// Lower the tail expression of a block like `{ ...; expr }`.
fn lower(source: &str) -> (ExpressionStore, ExpressionStoreSourceMap, ExprId, SyntaxNode) {
    let (green, errors) = boltlang_parser::parse_file(source);
    assert!(errors.is_empty(), "{errors:?}");
    let root = SyntaxNode::new_root(green);
    let file = ast::SourceFile::cast(root.clone()).unwrap();
    let mut collector = ExprCollector::new();
    let statements = collector.lower_statements(file.elements());
    let block = collector.store.exprs.alloc(Expr::Block {
        statements: statements.into(),
        tail: None,
        label: None,
        kind: BlockKind::Plain,
    });
    let (store, source_map) = collector.finish();
    (store, source_map, block, root)
}

/// Render the statements of `source` as s-expressions, one per line.
fn render(source: &str) -> String {
    let (store, _, block, _) = lower(source);
    let Expr::Block { statements, .. } = &store[block] else { unreachable!() };
    statements.iter().map(|s| Printer(&store).stmt(s)).collect::<Vec<_>>().join("\n")
}

struct Printer<'a>(&'a ExpressionStore);

impl Printer<'_> {
    fn list<T: Copy>(&self, items: &[T], f: impl Fn(&Self, T) -> String) -> String {
        items.iter().map(|i| f(self, *i)).collect::<Vec<_>>().join(" ")
    }

    fn opt_expr(&self, e: Option<ExprId>) -> String {
        e.map_or("_".into(), |e| self.expr(e))
    }

    fn path(&self, p: &Path) -> String {
        p.segments
            .iter()
            .map(|s| match &s.generic_args {
                None => s.name.to_string(),
                Some(args) => format!(
                    "{}[{}]",
                    s.name,
                    args.iter()
                        .map(|a| match a {
                            GenericArg::Type { type_ref, bounds } if bounds.is_empty() => self.ty(*type_ref),
                            GenericArg::Type { type_ref, bounds } => {
                                format!("{}: {}", self.ty(*type_ref), self.list(bounds, Self::ty))
                            }
                            GenericArg::AssocType { name, type_ref, .. } => format!("{name} = {}", self.ty(*type_ref)),
                            GenericArg::Const(e) => self.expr(*e),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            })
            .collect::<Vec<_>>()
            .join(".")
    }

    fn lit(&self, l: &Literal) -> String {
        match l {
            Literal::Int(v, None) => v.to_string(),
            Literal::Int(v, Some(s)) => format!("{v}{s}"),
            Literal::Float(v, None) => v.clone(),
            Literal::Float(v, Some(s)) => format!("{v}{s}"),
            other => format!("{other:?}"),
        }
    }

    fn stmt(&self, s: &Statement) -> String {
        match s {
            Statement::Let { pat, type_ref, initializer, else_branch } => format!(
                "(let {}{}{}{})",
                self.pat(*pat),
                type_ref.map_or(String::new(), |t| format!(" : {}", self.ty(t))),
                initializer.map_or(String::new(), |e| format!(" = {}", self.expr(e))),
                else_branch.map_or(String::new(), |e| format!(" else {}", self.expr(e))),
            ),
            Statement::Expr { expr, has_semi } => format!("{}{}", self.expr(*expr), if *has_semi { ";" } else { "" }),
            Statement::Item => "item".into(),
        }
    }

    fn label(&self, l: Option<LabelId>) -> String {
        l.map_or(String::new(), |l| format!("'{} ", self.0[l].name))
    }

    fn expr(&self, e: ExprId) -> String {
        match &self.0[e] {
            Expr::Missing => "<missing>".into(),
            Expr::Path(p) => self.path(p),
            Expr::Literal(l) => self.lit(l),
            Expr::Block { statements, tail, label, kind } => {
                let mut parts: Vec<String> = statements.iter().map(|s| self.stmt(s)).collect();
                if let Some(tail) = tail {
                    parts.push(self.expr(*tail));
                }
                let kind = match kind {
                    BlockKind::Plain => "",
                    BlockKind::Async => "async ",
                    BlockKind::Const => "const ",
                };
                format!("{}{kind}{{{}}}", self.label(*label), parts.join(" "))
            }
            Expr::Call { callee, args, is_mut } => format!(
                "({}call {} {})",
                if *is_mut { "mut " } else { "" },
                self.expr(*callee),
                self.list(args, Self::expr)
            ),
            Expr::Field { expr, name } => format!("(. {} {name})", self.expr(*expr)),
            Expr::Await { expr } => format!("(await {})", self.expr(*expr)),
            Expr::Try { expr } => format!("(? {})", self.expr(*expr)),
            Expr::Cast { expr, type_ref } => format!("(as {} {})", self.expr(*expr), self.ty(*type_ref)),
            Expr::Index { base, index } => format!("(index {} {})", self.expr(*base), self.expr(*index)),
            Expr::Prefix { op, expr } => format!("({op:?} {})", self.expr(*expr)),
            Expr::Binary { op, lhs, rhs } => {
                let op = match op {
                    BinaryOp::Custom(name) => name.to_string(),
                    other => format!("{other:?}"),
                };
                format!("({op} {} {})", self.expr(*lhs), self.expr(*rhs))
            }
            Expr::Range { lhs, rhs, inclusive } => {
                format!("({} {} {})", if *inclusive { "..=" } else { ".." }, self.opt_expr(*lhs), self.opt_expr(*rhs))
            }
            Expr::Tuple { exprs } => format!("(tuple {})", self.list(exprs, Self::expr)),
            Expr::Array { exprs, is_mut } => {
                format!("{}[{}]", if *is_mut { "mut " } else { "" }, self.list(exprs, Self::expr))
            }
            Expr::ArrayRepeat { value, count, is_mut } => {
                format!("{}[{}; {}]", if *is_mut { "mut " } else { "" }, self.expr(*value), self.expr(*count))
            }
            Expr::Record { path, fields, spread, is_mut } => format!(
                "({}record {} {}{})",
                if *is_mut { "mut " } else { "" },
                path.as_ref().map_or("_".into(), |p| self.path(p)),
                fields.iter().map(|f| format!("{}: {}", f.name, self.expr(f.expr))).collect::<Vec<_>>().join(", "),
                spread.map_or(String::new(), |s| format!(" ..{}", self.expr(s))),
            ),
            Expr::If { condition, then_branch, else_branch } => format!(
                "(if {} {}{})",
                self.expr(*condition),
                self.expr(*then_branch),
                else_branch.map_or(String::new(), |e| format!(" {}", self.expr(e)))
            ),
            Expr::Let { pat, expr } => format!("(let {} {})", self.pat(*pat), self.expr(*expr)),
            Expr::Match { expr, arms } => format!(
                "(match {} {})",
                self.expr(*expr),
                arms.iter()
                    .map(|a| format!(
                        "[{}{} => {}]",
                        self.pat(a.pat),
                        a.guard.map_or(String::new(), |g| format!(" if {}", self.expr(g))),
                        self.expr(a.expr)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Expr::While { condition, body, label } => {
                format!("({}while {} {})", self.label(*label), self.expr(*condition), self.expr(*body))
            }
            Expr::Loop { body, label } => format!("({}loop {})", self.label(*label), self.expr(*body)),
            Expr::For { pat, iterable, body, label } => {
                format!("({}for {} {} {})", self.label(*label), self.pat(*pat), self.expr(*iterable), self.expr(*body))
            }
            Expr::Break { expr, label } => format!(
                "(break{}{})",
                label.as_ref().map_or(String::new(), |l| format!(" '{l}")),
                expr.map_or(String::new(), |e| format!(" {}", self.expr(e)))
            ),
            Expr::Continue { label } => format!("(continue{})", label.as_ref().map_or(String::new(), |l| format!(" '{l}"))),
            Expr::Return { expr } => format!("(return{})", expr.map_or(String::new(), |e| format!(" {}", self.expr(e)))),
            Expr::Yield { expr } => format!("(yield{})", expr.map_or(String::new(), |e| format!(" {}", self.expr(e)))),
            Expr::Lambda { params, param_types, ret_type, body, kind } => format!(
                "({kind:?} ({}){} {})",
                params
                    .iter()
                    .zip(param_types.iter())
                    .map(|(p, t)| match t {
                        Some(t) => format!("{}: {}", self.pat(*p), self.ty(*t)),
                        None => self.pat(*p),
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                ret_type.map_or(String::new(), |t| format!(" -> {}", self.ty(t))),
                self.expr(*body)
            ),
            Expr::MacroCall { name } => format!("({}!)", name.as_ref().map_or("?".into(), |n| n.to_string())),
        }
    }

    fn pat(&self, p: PatId) -> String {
        let rest = |args: &[PatId], rest: Option<u32>| {
            let mut parts: Vec<String> = args.iter().map(|a| self.pat(*a)).collect();
            if let Some(i) = rest {
                parts.insert(i as usize, "..".into());
            }
            parts.join(", ")
        };
        match &self.0[p] {
            Pat::Missing => "<missing>".into(),
            Pat::Wild => "_".into(),
            Pat::Bind { id, subpat } => {
                let b = &self.0[*id];
                let mut s = format!("{}{}", if b.mutable { "mut " } else { "" }, b.name);
                if let Some(sub) = subpat {
                    s = format!("{s} @ {}", self.pat(*sub));
                }
                s
            }
            Pat::Lit { literal, negative } => format!("{}{}", if *negative { "-" } else { "" }, self.lit(literal)),
            Pat::Path(path) => format!("<{}>", self.path(path)),
            Pat::Tuple { args, rest: r } => format!("({})", rest(args, *r)),
            Pat::TupleStruct { path, args, rest: r } => format!("{}({})", self.path(path), rest(args, *r)),
            Pat::Record { path, fields, rest } => format!(
                "{} {{ {}{} }}",
                self.path(path),
                fields.iter().map(|f| format!("{}: {}", f.name, self.pat(f.pat))).collect::<Vec<_>>().join(", "),
                if *rest { ", .." } else { "" }
            ),
            Pat::Slice { args, rest: r } => format!("[{}]", rest(args, *r)),
            Pat::Range { start, end, inclusive } => format!(
                "{}{}{}",
                start.map_or(String::new(), |s| self.pat(s)),
                if *inclusive { "..=" } else { ".." },
                end.map_or(String::new(), |s| self.pat(s))
            ),
            Pat::Or(pats) => pats.iter().map(|p| self.pat(*p)).collect::<Vec<_>>().join(" | "),
            Pat::ConstBlock(e) => format!("const {}", self.expr(*e)),
            Pat::MacroCall { name } => format!("({}!)", name.as_ref().map_or("?".into(), |n| n.to_string())),
        }
    }

    fn ty(&self, t: TypeRefId) -> String {
        match &self.0[t] {
            TypeRef::Error => "<error>".into(),
            TypeRef::Path(p) => self.path(p),
            TypeRef::Tuple(ts) => format!("({})", self.list(ts, Self::ty)),
            TypeRef::Array { element, len, mutability } => {
                let prefix = match mutability {
                    Some(Mutability::Mut) => "mut ",
                    Some(Mutability::Imm) => "imm ",
                    None => "",
                };
                match len {
                    Some(len) => format!("{prefix}[{}; {}]", self.ty(*element), self.expr(*len)),
                    None => format!("{prefix}[{}]", self.ty(*element)),
                }
            }
            TypeRef::Mutability(mutability) => match mutability {
                MutabilityRef::Written(Mutability::Mut) => "mut".to_string(),
                MutabilityRef::Written(Mutability::Imm) => "imm".to_string(),
                MutabilityRef::Named(name) => format!("'{name}"),
            },
            TypeRef::Mut { mutability, inner } => {
                let prefix = match mutability {
                    MutabilityRef::Written(Mutability::Mut) => "mut".to_string(),
                    MutabilityRef::Written(Mutability::Imm) => "imm".to_string(),
                    MutabilityRef::Named(name) => format!("'{name}"),
                };
                format!("{prefix} {}", self.ty(*inner))
            }
            TypeRef::Never => "!".into(),
            TypeRef::Fn { params, is_variadic, ret_type, abi, .. } => format!(
                "{}fn({}{}){}",
                abi.as_ref().map_or(String::new(), |a| format!("extern {} ", a.as_deref().unwrap_or("_"))),
                params
                    .iter()
                    .map(|(n, t)| match n {
                        Some(n) => format!("{n}: {}", self.ty(*t)),
                        None => self.ty(*t),
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                if *is_variadic { ", ..." } else { "" },
                ret_type.map_or(String::new(), |r| format!(" -> {}", self.ty(r)))
            ),
            TypeRef::ImplTrait { generic_params, bounds } => format!(
                "impl{} {}",
                if generic_params.is_empty() {
                    String::new()
                } else {
                    format!(" for[{}]", generic_params.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "))
                },
                self.list(bounds, Self::ty)
            ),
            TypeRef::Bounded(ts) => ts.iter().map(|t| self.ty(*t)).collect::<Vec<_>>().join(" + "),
            TypeRef::MacroCall { name } => format!("({}!)", name.as_ref().map_or("?".into(), |n| n.to_string())),
        }
    }
}

#[test]
fn operators() {
    assert_eq!(
        render("a + b * c;\nx += (1 - 2);\nf <$> x;\n-a == !b;\nx as u8 as i32;"),
        "(Arith(Add) a (Arith(Mul) b c));\n\
         (Assign(Some(Add)) x (Arith(Sub) 1 2));\n\
         (<$> f x);\n\
         (Cmp(Eq) (Neg a) (Not b));\n\
         (as (as x u8) i32);"
    );
}

#[test]
fn literals() {
    assert_eq!(
        render(r##"1_000u8; 0xff; 0xf32; 0b101; 1.5e3f64; 2.; 'a'; '\n'; b'x'; "a\tb\u{e9}"; r#"raw\n"#; b"hi"; c"c";"##),
        "1000u8;\n255;\n3890;\n5;\n1.5e3f64;\n2.;\nChar('a');\nChar('\\n');\nByte(120);\n\
         String(\"a\\tbé\");\nString(\"raw\\\\n\");\nByteString([104, 105]);\nCString(\"c\");"
    );
}

#[test]
fn postfix_expressions() {
    assert_eq!(
        render("t.0.1;\nx.f(1)?.await;\nxs[i];\nr#match.r#type;"),
        "(. (. t 0) 1);\n(await (? (call (. x f) 1)));\n(index xs i);\n(. match type);"
    );
}

#[test]
fn blocks_and_statements() {
    assert_eq!(
        render("let mut x: i32 = 1;\nlet Some(y) = o else { return; };\n{ f(); g() }\nfn item() {}\nasync { 1 };\n'l: { break 'l 2; };"),
        "(let mut x : i32 = 1)\n\
         (let Some(y) = o else {(return);})\n\
         {(call f ); (call g )}\n\
         item\n\
         async {1};\n\
         'l {(break 'l 2);};"
    );
}

#[test]
fn control_flow() {
    assert_eq!(
        render(
            "if let Some(x) = o && x > 0 { a } else if b { c } else { d }\n\
             match x { 0 | 1 => a, n if n > 1 => b, _ => {} }\n\
             'outer: while c { continue 'outer; }\n\
             for (i, x) in xs { yield x; }\n\
             loop { break; }"
        ),
        "(if (Logic(And) (let Some(x) o) (Cmp(Gt) x 0)) {a} (if b {c} {d}))\n\
         (match x [0 | 1 => a] [n if (Cmp(Gt) n 1) => b] [_ => {}])\n\
         ('outer while c {(continue 'outer);})\n\
         (for (i, x) xs {(yield x);})\n\
         (loop {(break);})"
    );
}

#[test]
fn compound_expressions() {
    assert_eq!(
        render(
            "(a, b);\n();\n[1, 2];\n[0; n + 1];\nPoint { x, y: 1, ..base };\nShape.Circle { radius: r };\n\
             a..b; ..=c; d..; ..;"
        ),
        "(tuple a b);\n(tuple );\n[1 2];\n[0; (Arith(Add) n 1)];\n\
         (record Point x: x, y: 1 ..base);\n(record Shape.Circle radius: r);\n\
         (.. a b);\n(..= _ c);\n(.. d _);\n(.. _ _);"
    );
}

#[test]
fn lambdas_and_macros() {
    assert_eq!(
        render("|x| x;\n|a, b: i32| -> i32 { a };\nasync || 1;\nfn (x) = x + 1;\nfn (x: i32) -> i32 = x;\nprintln!(\"{}\", x);"),
        "(Closure (x) x);\n\
         (Closure (a, b: i32) -> i32 {a});\n\
         (AsyncClosure () 1);\n\
         (Function (x) (Arith(Add) x 1));\n\
         (Function (x: i32) -> i32 x);\n\
         (println!);"
    );
}

#[test]
fn patterns() {
    assert_eq!(
        render(
            "let (a, .., b) = t;\nlet Point { x, mut y, z: (c, _), .. } = p;\nlet mut v @ Some(w) = o;\n\
             let [first, rest @ ..] = s;\nlet Color.Red | Color.Green = c;\nlet 1..=5 = n;\nlet -1 = n;\nlet mut (m, n) = t;"
        ),
        "(let (a, .., b) = t)\n\
         (let Point { x: x, y: mut y, z: (c, _), .. } = p)\n\
         (let mut v @ Some(w) = o)\n\
         (let [first, rest @ _] = s)\n\
         (let <Color.Red> | <Color.Green> = c)\n\
         (let 1..=5 = n)\n\
         (let -1 = n)\n\
         (let (mut m, mut n) = t)"
    );
}

#[test]
fn types() {
    assert_eq!(
        render(
            "let a: Vec[Vec[i32]];\nlet b: (A, B);\nlet c: [u8; N + 1];\nlet d: fn(x: i32, ...) -> !;\n\
             let e: extern \"C\" fn();\nlet f: Box[fn(T) -> U];\nlet g: impl for[T] Show[T] + Send;\n\
             let h: A + B + C;\nlet i: H[K = L, 3, { n }, T: Ord];\nlet j: M[N].O;"
        ),
        "(let a : Vec[Vec[i32]])\n\
         (let b : (A B))\n\
         (let c : [u8; (Arith(Add) N 1)])\n\
         (let d : fn(x: i32, ...) -> !)\n\
         (let e : extern C fn())\n\
         (let f : Box[fn(T) -> U])\n\
         (let g : impl for[T] Show[T] + Send)\n\
         (let h : A + B + C)\n\
         (let i : H[K = L, 3, {n}, T: Ord])\n\
         (let j : M[N].O)"
    );
}

#[test]
fn mutability_of_arrays() {
    assert_eq!(
        render(
            "let a: mut [i32] = mut [1, 2];\nlet b: imm [mut [u8; 4]] = [mut [0; 4]];\nlet c: [Bool] = mut [];\n\
             let d: fn(mut [i32]) -> imm [i32];"
        ),
        "(let a : mut [i32] = mut [1 2])\n\
         (let b : imm [mut [u8; 4]] = [mut [0; 4]])\n\
         (let c : [Bool] = mut [])\n\
         (let d : fn(mut [i32]) -> imm [i32])"
    );
}

#[test]
fn mutability_of_structs() {
    assert_eq!(
        render(
            "let a: mut Point = mut Point { x: 1 };\nlet b: imm Pair[A, B] = mut Pair(1, 2);\n\
             let c: Option[mut Point] = f(1);\nlet d: fn('m Point) -> 'm [i32];"
        ),
        "(let a : mut Point = (mut record Point x: 1))\n\
         (let b : imm Pair[A, B] = (mut call Pair 1 2))\n\
         (let c : Option[mut Point] = (call f 1))\n\
         (let d : fn('m Point) -> 'm [i32])"
    );
}

#[test]
fn source_map_points_back_to_the_syntax() {
    let (store, source_map, _, root) = lower("let x = (a + b) * c;");
    let bin = root.descendants().filter(|n| n.kind() == boltlang_syntax::SyntaxKind::BIN_EXPR).nth(1).unwrap();
    assert_eq!(bin.text().to_string(), "a + b");
    let id = source_map.node_expr(&bin).unwrap();
    assert!(matches!(store[id], Expr::Binary { .. }));
    assert_eq!(source_map.expr_syntax(id).unwrap().to_node(&root), Some(bin.clone()));
    // The parentheses map to the expression inside them.
    let nest = bin.parent().unwrap();
    assert_eq!(source_map.node_expr(&nest), Some(id));
}
