
mod db;

use ariadne::Report;

use boltlang::{SystemPathBuf, rowan::NodeOrToken};
use boltlang::salsa::Database;

use db::CliDatabase;

use boltlang::{
    Db, DbDiagnostic, Emit, File, Formatter, Node, Path, PathBuf, SourceFile, SyntaxElement, SyntaxKind, SyntaxNode, check_file, parse_file
};

use clap::Parser;

trait Exec {
    fn exec(&self) -> anyhow::Result<()>;
}

#[derive(clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Exec for Cli {
    fn exec(&self) -> anyhow::Result<()> {
        self.command.exec()
    }
}

#[derive(clap::Subcommand)]
enum Command {
    DumpAst(DumpAstCommand),
    Check(CheckCommand),
}

impl Exec for Command {
    fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Check(cmd) => cmd.exec(),
            Self::DumpAst(cmd) => cmd.exec(),
        }
    }
}

#[derive(clap::Parser)]
struct DumpAstCommand {
    file: PathBuf,
    #[clap(long, default_value_t, value_enum)]
    format: OutputFormat,
}

#[derive(clap::ValueEnum, Default, Clone, Debug)]
enum OutputFormat {
    #[default]
    Pretty,
    Json,
    Native,
}

impl Exec for DumpAstCommand {
    fn exec(&self) -> anyhow::Result<()> {
        let cwd = PathBuf::from(std::env::current_dir()?);
        let path = Path::new(self.file.as_path());
        let root_node = CliDatabase::new(&cwd).attach(|db| {
            let file = File::new(db, path.to_system_path_buf().into());
            let parsed = parse_file(db, file);
            let diagnostics = parse_file::accumulated::<DbDiagnostic>(db, file);
            for diagnostic in diagnostics {
                report_diagnostic(db, diagnostic);
            }
            SyntaxNode::new_root(parsed.node(db).clone())
        });
        match self.format {
            OutputFormat::Native => {
                let mut stdout = std::io::stdout();
                let mut f= Formatter::new(&mut stdout);
                SourceFile::wrap(root_node).emit(&mut f)?;
            }
            OutputFormat::Pretty => print(0, root_node.into()),
            OutputFormat::Json => serde_json::to_writer_pretty(std::io::stdout(), &root_node)?,
        }
        Ok(())
    }
}

#[derive(clap::Parser)]
struct CheckCommand {
    files: Vec<PathBuf>,
}

impl Exec for CheckCommand {
    fn exec(&self) -> anyhow::Result<()> {
        let cwd = SystemPathBuf::from(std::env::current_dir()?);
        for raw_path in &self.files {
            CliDatabase::new(&cwd).attach(|db| {
                let file = File::new(db, raw_path.clone().into());
                let _ = check_file(db, file);
                let diagnostics = check_file::accumulated::<DbDiagnostic>(db, file);
                for diagnostic in diagnostics {
                    report_diagnostic(db, diagnostic);
                }
            });
        }
        Ok(())
    }
}

fn print(indent: usize, element: SyntaxElement) {
    let kind: SyntaxKind = element.kind();
    print!("{:indent$}", "", indent = indent);
    match element {
        NodeOrToken::Node(node) => {
            println!("- {:?}", kind);
            for child in node.children_with_tokens() {
                print(indent + 2, child);
            }
        }
        NodeOrToken::Token(token) => println!("- {:?} {:?}", token.text(), kind),
    }
}

fn severity_to_report_kind(sev: boltlang::Severity) -> ariadne::ReportKind<'static> {
    match sev {
        boltlang::Severity::Info => ariadne::ReportKind::Advice,
        boltlang::Severity::Warn => ariadne::ReportKind::Warning,
        boltlang::Severity::Error => ariadne::ReportKind::Error,
        boltlang::Severity::Fatal => ariadne::ReportKind::Error,
    }
}

fn report_diagnostic(db: &dyn Db, diagnostic: &DbDiagnostic) {
    let source = diagnostic.source();
    let file = source.file();
    let fname = file.path(db).as_system_path_buf().unwrap().as_str().unwrap(); // TODO handle None
    let contents = file.read_to_string(db).unwrap(); // TODO handle Err
    Report::build(severity_to_report_kind(diagnostic.severity()), (fname, source.span().clone()))
        .with_code(diagnostic.code())
        .with_message(diagnostic.message())
        .with_label(
            ariadne::Label::new((fname, source.span().clone()))
                .with_message(diagnostic.message())
        )
        .finish()
        .print((fname, ariadne::Source::from(contents)))
        .unwrap();
}

fn main() -> anyhow::Result<()> {
    Cli::parse().exec()
}
