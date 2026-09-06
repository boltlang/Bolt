
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

mod util;
mod codegen;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Codegen(CodegenCommand),
}

#[derive(Parser)]
struct CodegenCommand {
    name: String,
    #[clap(long, action = clap::ArgAction::SetTrue)]
    check: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum CodegenType {
    ParserTests,
}

impl std::fmt::Display for CodegenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

/// Returns the path to the root directory of `bolt` project.
fn project_root() -> PathBuf {
    let dir =
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(dir).parent().unwrap().to_owned()
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Codegen(codegen_cmd) => {
            match codegen_cmd.name.as_str() {
                "parser-tests" => {
                    codegen::parser_tests::generate(codegen_cmd.check);
                }
                name => panic!("undefined code generator {name}"),
            }
        }
    }
}
