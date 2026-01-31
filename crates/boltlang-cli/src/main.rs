
use std::path::PathBuf;

use ariadne::Report;
use boltlang::rowan::NodeOrToken;
use boltlang::salsa::{self, Database};

use boltlang::{
    DbDiagnostic,
    File,
    RootDatabase,
    SyntaxElement,
    SyntaxKind,
    SyntaxNode,
    check_file,
    parse_file
};
use clap::Parser;
use fluent_uri::Uri;

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
}

impl Exec for DumpAstCommand {
    fn exec(&self) -> anyhow::Result<()> {
        let text = std::fs::read_to_string(&self.file).expect(&format!("could not read {}", self.file.display()));
        let root_node = RootDatabase::new(None).attach(|db| {
            let file = File::new(db, format!("file://{}", std::fs::canonicalize(&self.file).unwrap().to_string_lossy()), text);
            let parsed = parse_file(db, file);
            let diagnostics = parse_file::accumulated::<DbDiagnostic>(db, file);
            for diagnostic in diagnostics {
                report_diagnostic(db, diagnostic);
            }
            SyntaxNode::new_root(parsed.node(db).clone())
        });
        match self.format {
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
        for file in &self.files {
            let text = std::fs::read_to_string(&file).expect(&format!("could not read {}", file.display()));
            RootDatabase::new(None).attach(|db| {
                let file = File::new(db, format!("file://{}", std::fs::canonicalize(file).unwrap().to_string_lossy()), text);
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

fn report_diagnostic(db: &dyn salsa::Database, diagnostic: &DbDiagnostic) {
    match diagnostic.source() {
        None => eprintln!("Error: {}", diagnostic),
        Some(source) => {
            let parsed = Uri::parse(source.file().uri(db)).unwrap();
            let fname = parsed.path();
            let contents = source.file().contents(db);
            Report::build(severity_to_report_kind(diagnostic.severity()), (fname.as_str(), source.span().clone()))
                .with_code(diagnostic.code())
                .with_message(diagnostic.message())
                .with_label(
                    ariadne::Label::new((fname.as_str(), source.span().clone()))
                        .with_message(diagnostic.message())
                )
                .finish()
                .print((fname.as_str(), ariadne::Source::from(contents)))
                .unwrap();
        }
    }
}

fn main() -> anyhow::Result<()> {
    Cli::parse().exec()
}
