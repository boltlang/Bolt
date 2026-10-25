//! Parser tests driven by the `.txt` files in `tests/parser` at the root of
//! the repository, which are shared with the tree-sitter grammar. Each case
//! is a separate test, named after its file and title, e.g.
//! `expressions::closures`.
//!
//! ```text
//! ================================================================================
//! Precedence of binary operators
//! :rule expr
//! ================================================================================
//! a + b * c
//! ```
//!
//! A case is a title between two lines of `=`, and the code to parse. The
//! format is that of the tree-sitter corpus and the type checker's tests,
//! but without the expected tree: the two parsers don't give the same trees
//! yet, so a case only says whether the code parses.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:rule <rule>`: parse the code with that rule instead of as a file.
//!   The rules are `file`, `item`, `expr`, `pattern` and `type_expr`.
//! - `:error`: the code must not parse: the parser must report a
//!   diagnostic. Without it, the code must parse without diagnostics and
//!   without `ERROR` nodes.
//! - `:todo`: the parser doesn't handle the case as described yet. It's
//!   reported as ignored; run the cases with `-- --ignored`.
//! - `:skip`: never run the case, not even with `--ignored`.
//!
//! Run a single case with `cargo test --test parser closures`.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::RecvTimeoutError,
    time::Duration,
};

use boltlang_parser::{Diagnostic, Parser, grammar, parse, rowan::GreenNode};
use boltlang_syntax::{NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode};
use libtest_mimic::{Arguments, Failed, Trial};

/// A rule the code of a case can be parsed with.
#[derive(Clone, Copy, Debug)]
enum Rule {
    File,
    Item,
    Expr,
    Pattern,
    TypeExpr,
}

impl Rule {
    const ALL: &[(&str, Rule)] = &[
        ("file", Rule::File),
        ("item", Rule::Item),
        ("expr", Rule::Expr),
        ("pattern", Rule::Pattern),
        ("type_expr", Rule::TypeExpr),
    ];

    fn parse(self, text: &str) -> (GreenNode, Vec<Diagnostic>) {
        let rule: fn(&mut Parser) = match self {
            Rule::File => grammar::parse_source_file,
            Rule::Item => grammar::parse_item,
            Rule::Expr => |p| {
                grammar::parse_expr(p);
            },
            Rule::Pattern => |p| {
                grammar::parse_pattern(p);
            },
            Rule::TypeExpr => grammar::type_,
        };
        parse(text, rule)
    }
}

/// One test case of a data file.
struct Case {
    title: String,
    rule: Rule,
    error: bool,
    todo: bool,
    skip: bool,
    input: String,
}

/// Whether `line` is made of at least three `c`s.
fn is_separator(line: &str, c: char) -> bool {
    let line = line.trim_end();
    line.len() >= 3 && line.chars().all(|x| x == c)
}

/// The cases in the text of a data file.
fn parse_cases(text: &str) -> Result<Vec<Case>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cases = Vec::new();
    let mut i = 0;
    while i < lines.len() && !is_separator(lines[i], '=') {
        i += 1;
    }
    while i < lines.len() {
        let start = i + 1;
        i = start;
        while i < lines.len() && !is_separator(lines[i], '=') {
            i += 1;
        }
        if i == lines.len() {
            return Err(format!("line {start}: the title isn't followed by a line of `=`"));
        }
        let (attributes, title): (Vec<&str>, Vec<&str>) =
            lines[start..i].iter().partition(|line| line.trim_start().starts_with(':'));
        let title = title.join(" ").trim().to_string();
        let mut case = Case { title, rule: Rule::File, error: false, todo: false, skip: false, input: String::new() };
        for attribute in attributes {
            let attribute = attribute.trim();
            match attribute.split_once(' ') {
                Some((":rule", rule)) => {
                    let Some((_, rule)) = Rule::ALL.iter().find(|(name, _)| *name == rule.trim()) else {
                        let names: Vec<_> = Rule::ALL.iter().map(|(name, _)| format!("`{name}`")).collect();
                        return Err(format!("`{}`: unknown rule `{rule}`; the rules are {}", case.title, names.join(", ")));
                    };
                    case.rule = *rule;
                }
                _ => match attribute {
                    ":error" => case.error = true,
                    ":todo" => case.todo = true,
                    ":skip" => case.skip = true,
                    _ => {
                        return Err(format!(
                            "`{}`: unknown attribute `{attribute}`; the attributes are `:rule <rule>`, \
                             `:error`, `:todo` and `:skip`",
                            case.title
                        ));
                    }
                },
            }
        }
        i += 1;
        let input_start = i;
        while i < lines.len() && !is_separator(lines[i], '=') {
            if is_separator(lines[i], '-') {
                return Err(format!(
                    "`{}`: expected trees aren't supported yet, so a case can't have a line of `-`",
                    case.title
                ));
            }
            i += 1;
        }
        case.input = lines[input_start..i].join("\n").trim_matches('\n').to_string() + "\n";
        cases.push(case);
    }
    Ok(cases)
}

/// The test name for a title: `Mutual recursion` is `mutual_recursion`.
fn slug(title: &str) -> String {
    let slug: String = title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    slug.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

/// Parse on another thread, so that a parser that hangs or panics fails
/// its case instead of the whole run.
fn parse_with_timeout(rule: Rule, text: &str) -> Result<(GreenNode, Vec<Diagnostic>), String> {
    let text = text.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(rule.parse(&text));
    });
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => Ok(result),
        Err(RecvTimeoutError::Timeout) => Err("the parser didn't finish within 5s".to_string()),
        Err(RecvTimeoutError::Disconnected) => Err("the parser panicked".to_string()),
    }
}

fn run(case: &Case) -> Result<(), Failed> {
    let (node, diagnostics) = parse_with_timeout(case.rule, &case.input)?;
    let has_error_node = SyntaxNode::new_root(node.clone()).descendants().any(|n| n.kind() == SyntaxKind::ERROR);
    match (case.error, diagnostics.is_empty() && !has_error_node) {
        (false, false) => Err(format!("expected the code to parse\n{}", dump(&node, &diagnostics)).into()),
        (true, true) => Err(format!("expected a syntax error (`:error`)\n{}", dump(&node, &diagnostics)).into()),
        _ => Ok(()),
    }
}

/// The tree and the diagnostics, to show when a case fails.
fn dump(node: &GreenNode, diagnostics: &[Diagnostic]) -> String {
    fn write(out: &mut String, element: &SyntaxElement, indent: usize) {
        match element {
            NodeOrToken::Node(node) => {
                out.push_str(&format!("{:indent$}{:?}\n", "", node.kind()));
                for child in node.children_with_tokens() {
                    write(out, &child, indent + 2);
                }
            }
            NodeOrToken::Token(token) => {
                out.push_str(&format!("{:indent$}{:?} {:?}\n", "", token.kind(), token.text()));
            }
        }
    }
    let mut out = String::new();
    write(&mut out, &SyntaxNode::new_root(node.clone()).into(), 0);
    for diagnostic in diagnostics {
        out.push_str(&format!("error: {}\n", diagnostic.message()));
    }
    out
}

/// The data files, in alphabetical order.
fn data_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();
    files
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parser");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let cases = match parse_cases(&text) {
            Ok(cases) => cases,
            Err(error) => {
                let message = format!("{}: {error}", path.display());
                trials.push(Trial::test(stem, move || Err(message.into())));
                continue;
            }
        };
        let mut seen = Vec::new();
        for case in cases {
            let name = format!("{stem}::{}", slug(&case.title));
            if seen.contains(&name) {
                let message = format!("{}: two cases are called `{}`", path.display(), case.title);
                trials.push(Trial::test(format!("{name}_again"), move || Err(message.into())));
                continue;
            }
            seen.push(name.clone());
            let ignored = case.todo || case.skip;
            let trial = if case.skip {
                Trial::test(name, || Ok(())).with_kind("skip")
            } else if case.todo {
                Trial::test(name, move || run(&case)).with_kind("todo")
            } else {
                Trial::test(name, move || run(&case))
            };
            trials.push(trial.with_ignored_flag(ignored));
        }
    }
    libtest_mimic::run(&args, trials).exit();
}
