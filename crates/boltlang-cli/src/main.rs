mod db;

use std::{
    io::{self, IsTerminal, Write},
    ops::Range,
    path::PathBuf,
    process::ExitCode,
};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind};
use boltlang_base_db::{
    DEFAULT_PRELUDE, Db, File, FilePath, SystemPath, SystemPathBuf, parse_file, source_text,
};
use boltlang_ide_diagnostics::{
    Diagnostic, Severity, dependency_diagnostics, diagnostics, syntax_diagnostics,
};
use boltlang_syntax::{
    SyntaxElement, SyntaxKind, SyntaxNode,
    rowan::{NodeOrToken, TextRange},
};
use clap::Parser;

use db::CliDatabase;

trait Exec {
    /// Run the command. `Ok(false)` means it ran, but found errors.
    fn exec(&self) -> anyhow::Result<bool>;
}

/// The Bolt compiler.
#[derive(clap::Parser)]
#[command(name = "bolt", version, long_version = long_version())]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Which prelude files get: the names of a package's `__prelude__.bolt`, or
/// `__prelude__/lib.bolt`, that can be used without importing them. It's `std`'s, unless a file's
/// `bolt.toml` says otherwise.
#[derive(clap::Args, Clone, Debug, Default)]
struct PreludeArgs {
    /// Don't use a prelude, whatever `bolt.toml` says.
    #[arg(long)]
    no_prelude: bool,
}

impl PreludeArgs {
    fn database(&self) -> anyhow::Result<CliDatabase> {
        // `std`'s prelude may be missing, as without `std`.
        let prelude = (!self.no_prelude).then(|| DEFAULT_PRELUDE.to_string());
        Ok(CliDatabase::new(&current_dir()?, prelude, false, std_dir()))
    }
}

/// The directory of the `std` that comes with the compiler, written out the
/// first time. Without one, `std` is looked up in `.bolt/packages`.
fn std_dir() -> Option<SystemPathBuf> {
    match boltlang_std::locate() {
        Ok((dir, _)) => Some(SystemPathBuf::from(dir)),
        Err(error) => {
            eprintln!("warning: {error}");
            None
        }
    }
}

/// `bolt --version`: the version, and where `std` is, so that it can be
/// read.
fn long_version() -> &'static str {
    let std = match boltlang_std::find() {
        Ok((dir, source)) => format!("{} ({source})", dir.display()),
        Err(error) => format!("not found: {error}"),
    };
    Box::leak(format!("{}\nstd: {std}", env!("CARGO_PKG_VERSION")).into_boxed_str())
}

#[derive(clap::Subcommand)]
enum Command {
    DumpAst(DumpAstCommand),
    Check(CheckCommand),
    Run(RunCommand),
    Test(TestCommand),
    Build(BuildCommand),
}

impl Exec for Command {
    fn exec(&self) -> anyhow::Result<bool> {
        match self {
            Self::Check(cmd) => cmd.exec(),
            Self::DumpAst(cmd) => cmd.exec(),
            Self::Run(cmd) => cmd.exec(),
            Self::Test(cmd) => cmd.exec(),
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

/// Run a file. Not available until the Conduit backend lands.
#[derive(clap::Parser)]
struct RunCommand {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

/// Run the tests of files. Not available until the Conduit backend lands.
#[derive(clap::Parser)]
struct TestCommand {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

/// Compile a file. Not available until the Conduit backend lands.
#[derive(clap::Parser)]
struct BuildCommand {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

/// What `bolt run`, `bolt test` and `bolt build` do until Bolt has a
/// backend again.
fn not_available(command: &str) -> anyhow::Result<bool> {
    eprintln!("error: `bolt {command}` is not available until the Conduit backend lands");
    Ok(false)
}

impl Exec for RunCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        not_available("run")
    }
}

impl Exec for TestCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        not_available("test")
    }
}

impl Exec for BuildCommand {
    fn exec(&self) -> anyhow::Result<bool> {
        not_available("build")
    }
}

/// The diagnostics of `file`, and one for each file it uses that has errors
/// (#249), unless it's one of `reported`, whose errors are shown anyway.
fn checked(db: &dyn Db, file: File, reported: &[File]) -> Vec<Diagnostic> {
    let mut found = diagnostics(db, file);
    let broken = dependency_diagnostics(db, file);
    found.extend(
        broken.into_iter().filter(|d| !d.labels.iter().any(|l| reported.contains(&l.file))),
    );
    found
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
        let sources: Vec<(String, String)> =
            files.iter().map(|&f| (display_path(db, f), source_text(db, f).clone())).collect();
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
                Label::new(span(label.file, label.range))
                    .with_message(&label.message)
                    .with_color(Color::Blue)
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
