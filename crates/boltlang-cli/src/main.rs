
mod db;

use std::{
    io::{self, IsTerminal, Write},
    ops::Range,
    path::PathBuf,
    process::ExitCode,
};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind};
use boltlang_base_db::{DEFAULT_PRELUDE, Db, File, FilePath, SystemPath, SystemPathBuf, parse_file, source_text};
use boltlang_codegen::{CodegenError, link, run};
use boltlang_eval::{EvalError, Evaluator, Limits, Stdout};
use boltlang_ide_diagnostics::{
    Diagnostic, DiagnosticCode, Severity, dependency_diagnostics, diagnostics, syntax_diagnostics,
};
use boltlang_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, rowan::{NodeOrToken, TextRange}};
use clap::Parser;

use db::CliDatabase;

trait Exec {
    /// Run the command. `Ok(false)` means it ran, but found errors.
    fn exec(&self) -> anyhow::Result<bool>;
}

/// The Bolt compiler.
#[derive(clap::Parser)]
#[command(name = "bolt", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Which prelude files get: the names of a package's `__prelude__/lib.bolt`
/// that can be used without importing them.
#[derive(clap::Args, Clone, Debug, Default)]
struct PreludeArgs {
    /// The package whose prelude to use [default: std]
    #[arg(long, value_name = "PACKAGE", conflicts_with = "no_prelude")]
    prelude: Option<String>,
    /// Don't use a prelude.
    #[arg(long)]
    no_prelude: bool,
}

impl PreludeArgs {
    fn database(&self) -> anyhow::Result<CliDatabase> {
        // Only the default prelude may be missing, so that `std` isn't needed.
        let (prelude, required) = match (&self.prelude, self.no_prelude) {
            (_, true) => (None, false),
            (Some(package), false) => (Some(package.clone()), true),
            (None, false) => (Some(DEFAULT_PRELUDE.to_string()), false),
        };
        Ok(CliDatabase::new(&current_dir()?, prelude, required))
    }
}

#[derive(clap::Subcommand)]
enum Command {
    DumpAst(DumpAstCommand),
    Check(CheckCommand),
    Run(RunCommand),
    Build(BuildCommand),
}

impl Exec for Command {
    fn exec(&self) -> anyhow::Result<bool> {
        match self {
            Self::Check(cmd) => cmd.exec(),
            Self::DumpAst(cmd) => cmd.exec(),
            Self::Run(cmd) => cmd.exec(),
            Self::Build(cmd) => cmd.exec(),
        }
    }
}

/// Print the syntax tree of a file, after its syntax errors.
#[derive(clap::Parser)]
struct DumpAstCommand {
    file: PathBuf,
    #[command(flatten)]
    prelude: PreludeArgs,
    #[clap(long, default_value_t, value_enum)]
    format: OutputFormat,
}

#[derive(clap::ValueEnum, Default, Clone, Debug)]
enum OutputFormat {
    #[default]
    Pretty,
    Json,
}

impl Exec for DumpAstCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        let db = self.prelude.database()?;
        let Some(file) = open(&db, &self.file) else { return Ok(false) };
        let errors = syntax_diagnostics(&db, file);
        report(&db, &errors)?;
        let root = SyntaxNode::new_root(parse_file(&db, file).tree(&db).clone());
        let mut out = io::BufWriter::new(io::stdout().lock());
        match self.format {
            OutputFormat::Pretty => print(&mut out, 0, root.into())?,
            OutputFormat::Json => serde_json::to_writer_pretty(&mut out, &root)?,
        }
        out.flush()?;
        Ok(errors.is_empty())
    }
}

/// Report the problems in files, from syntax errors to type errors.
#[derive(clap::Parser)]
struct CheckCommand {
    files: Vec<PathBuf>,
    #[command(flatten)]
    prelude: PreludeArgs,
}

impl Exec for CheckCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        let db = self.prelude.database()?;
        let mut ok = true;
        let mut files = Vec::new();
        for path in &self.files {
            match open(&db, path) {
                Some(file) => files.push(file),
                None => ok = false,
            }
        }
        for &file in &files {
            let found = checked(&db, file, &files);
            report(&db, &found)?;
            ok &= found.iter().all(|d| d.severity != Severity::Error);
        }
        Ok(ok)
    }
}

/// Check a file, then run the statements at its top level. The value of
/// the last one is printed, if it's an expression with a value.
#[derive(clap::Parser)]
struct RunCommand {
    file: PathBuf,
    #[command(flatten)]
    prelude: PreludeArgs,
    /// Run the file even when the checker finds errors.
    #[arg(long)]
    no_check: bool,
    /// Stop after evaluating this many expressions, or with `--wasm`,
    /// after about this many WebAssembly instructions.
    #[arg(long, value_name = "STEPS")]
    fuel: Option<u64>,
    /// Compile the file to WebAssembly and run that, instead of evaluating it.
    #[arg(long)]
    wasm: bool,
}

impl Exec for RunCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        let db = self.prelude.database()?;
        let Some(file) = open(&db, &self.file) else { return Ok(false) };
        let found = if self.no_check { diagnostics(&db, file) } else { checked(&db, file, &[file]) };
        report(&db, &found)?;
        if !self.no_check && found.iter().any(|d| d.severity == Severity::Error) {
            return Ok(false);
        }
        let result = if self.wasm {
            let bytes = match link(&db, file) {
                Ok(bytes) => bytes,
                Err(errors) => {
                    report(&db, &codegen_diagnostics(&db, file, &errors))?;
                    return Ok(false);
                }
            };
            run(&db, file, &bytes, Stdout, self.fuel).map_err(|error| {
                located_diagnostic(file, None, DiagnosticCode::RUNTIME_ERROR, error.to_string())
            })
        } else {
            let limits = Limits { fuel: self.fuel, ..Limits::default() };
            let mut evaluator = Evaluator::with_limits(&db, limits).with_host(Stdout);
            evaluator.eval_file(file).map_err(|error| runtime_diagnostic(&db, file, &error))
        };
        match result {
            Ok(value) => {
                if !value.is_unit() {
                    println!("{}", value.display(&db));
                }
                Ok(true)
            }
            Err(diagnostic) => {
                report(&db, &[diagnostic])?;
                Ok(false)
            }
        }
    }
}

/// Check a file, then compile it to a WebAssembly module. The top-level
/// statements are its exported function `main`.
#[derive(clap::Parser)]
struct BuildCommand {
    file: PathBuf,
    /// Where to write the module [default: the file with the extension `.wasm`]
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
    #[command(flatten)]
    prelude: PreludeArgs,
}

impl Exec for BuildCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        let db = self.prelude.database()?;
        let Some(file) = open(&db, &self.file) else { return Ok(false) };
        let found = checked(&db, file, &[file]);
        report(&db, &found)?;
        if found.iter().any(|d| d.severity == Severity::Error) {
            return Ok(false);
        }
        match link(&db, file) {
            Ok(bytes) => {
                let output = self.output.clone().unwrap_or_else(|| self.file.with_extension("wasm"));
                std::fs::write(&output, bytes)?;
                Ok(true)
            }
            Err(errors) => {
                report(&db, &codegen_diagnostics(&db, file, &errors))?;
                Ok(false)
            }
        }
    }
}

/// The diagnostics of `file`, and one for each file it uses that has errors
/// (#249), unless it's one of `reported`, whose errors are shown anyway.
fn checked(db: &dyn Db, file: File, reported: &[File]) -> Vec<Diagnostic> {
    let mut found = diagnostics(db, file);
    let broken = dependency_diagnostics(db, file);
    found.extend(broken.into_iter().filter(|d| !d.labels.iter().any(|l| reported.contains(&l.file))));
    found
}

/// Why `file` couldn't be compiled, as diagnostics.
fn codegen_diagnostics<'db>(db: &'db dyn Db, file: File, errors: &[CodegenError<'db>]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| located_diagnostic(file, e.source_range(db), DiagnosticCode::CODEGEN_ERROR, e.to_string()))
        .collect()
}

/// An error of the evaluator as a diagnostic, at the expression where it
/// happened, or at the start of `file` when that isn't known.
fn runtime_diagnostic<'db>(db: &'db dyn Db, file: File, error: &EvalError<'db>) -> Diagnostic {
    located_diagnostic(file, error.source_range(db), DiagnosticCode::RUNTIME_ERROR, error.to_string())
}

/// A diagnostic at a byte range of a file, or at the start of `file` when
/// it's not known where.
fn located_diagnostic(
    file: File,
    at: Option<(File, std::ops::Range<u32>)>,
    code: DiagnosticCode,
    message: String,
) -> Diagnostic {
    let (file, range) = match at {
        Some((file, range)) => (file, TextRange::new(range.start.into(), range.end.into())),
        None => (file, TextRange::default()),
    };
    Diagnostic { code, severity: Severity::Error, file, range, message, labels: Vec::new() }
}

fn current_dir() -> anyhow::Result<SystemPathBuf> {
    Ok(SystemPathBuf::from(std::env::current_dir()?))
}

/// The file at `path`, or `None` after reporting that it can't be read.
fn open(db: &CliDatabase, path: &std::path::Path) -> Option<File> {
    let file = db.files().resolve(db, SystemPath::new(path));
    if !file.exists(db) {
        eprintln!("error: cannot read `{}`", path.display());
        return None;
    }
    Some(file)
}

fn print(out: &mut impl Write, indent: usize, element: SyntaxElement) -> io::Result<()> {
    let kind: SyntaxKind = element.kind();
    write!(out, "{:indent$}", "", indent = indent)?;
    match element {
        NodeOrToken::Node(node) => {
            writeln!(out, "- {:?}", kind)?;
            for child in node.children_with_tokens() {
                print(out, indent + 2, child)?;
            }
            Ok(())
        }
        NodeOrToken::Token(token) => writeln!(out, "- {:?} {:?}", token.text(), kind),
    }
}

/// Print diagnostics to stderr, with the source they're about.
fn report(db: &dyn Db, diagnostics: &[Diagnostic]) -> std::io::Result<()> {
    let color = std::io::stderr().is_terminal();
    let config = Config::default().with_index_type(IndexType::Byte).with_color(color);
    for diagnostic in diagnostics {
        let mut files = vec![diagnostic.file];
        for label in &diagnostic.labels {
            if !files.contains(&label.file) {
                files.push(label.file);
            }
        }
        let sources: Vec<(String, String)> = files.iter().map(|&f| (display_path(db, f), source_text(db, f).clone())).collect();
        let span = |file: File, range: TextRange| {
            let name = display_path(db, file);
            let text = &sources.iter().find(|(n, _)| *n == name).unwrap().1;
            let range = byte_range(text, range);
            (name, range)
        };
        let (kind, primary) = match diagnostic.severity {
            Severity::Error => (ReportKind::Error, Color::Red),
            Severity::Warning => (ReportKind::Warning, Color::Yellow),
        };
        Report::build(kind, span(diagnostic.file, diagnostic.range))
            .with_config(config)
            .with_code(diagnostic.code)
            .with_message(&diagnostic.message)
            // A label without a message isn't always drawn.
            .with_label(
                Label::new(span(diagnostic.file, diagnostic.range))
                    .with_message(&diagnostic.message)
                    .with_color(primary),
            )
            .with_labels(diagnostic.labels.iter().map(|label| {
                Label::new(span(label.file, label.range)).with_message(&label.message).with_color(Color::Blue)
            }))
            .finish()
            .eprint(ariadne::sources(sources.clone()))?;
    }
    Ok(())
}

/// The path of a file as users wrote it: relative to the current directory
/// when it's inside it.
fn display_path(db: &dyn Db, file: File) -> String {
    match file.path(db) {
        FilePath::System(path) => {
            let path = path.as_std_path();
            let cwd = db.system().current_directory().ok();
            let relative = cwd.as_ref().and_then(|cwd| path.strip_prefix(cwd.as_std_path()).ok());
            relative.unwrap_or(path).display().to_string()
        }
        FilePath::SystemVirtual(path) => path.clone(),
    }
}

/// The bytes of `range` in `text`. An empty range, like that of a missing
/// `;`, is shown as the character after it, so that it can be underlined.
fn byte_range(text: &str, range: TextRange) -> Range<usize> {
    let (start, end) = (usize::from(range.start()), usize::from(range.end()));
    if start < end {
        return start..end;
    }
    match text[start..].chars().next() {
        Some(c) if c != '\n' => start..start + c.len_utf8(),
        _ => start..start,
    }
}

fn main() -> anyhow::Result<ExitCode> {
    match Cli::parse().command.exec() {
        Ok(true) => Ok(ExitCode::SUCCESS),
        Ok(false) => Ok(ExitCode::FAILURE),
        // The output was closed, like by `bolt dump-ast f.bolt | head`.
        Err(e) if is_broken_pipe(&e) => Ok(ExitCode::SUCCESS),
        Err(e) => Err(e),
    }
}

fn is_broken_pipe(error: &anyhow::Error) -> bool {
    let kind = match error.downcast_ref::<io::Error>() {
        Some(e) => Some(e.kind()),
        None => error.downcast_ref::<serde_json::Error>().and_then(|e| e.io_error_kind()),
    };
    kind == Some(io::ErrorKind::BrokenPipe)
}
