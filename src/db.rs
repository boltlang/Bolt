
#[cfg(test)]
use std::sync::{Arc, Mutex};

use rowan::GreenNode;
use salsa::Accumulator;

use crate::parser::{self, Parser, SyntaxError};

#[salsa::db]
#[derive(Clone)]
#[cfg_attr(not(test), derive(Default))]
pub struct DatabaseImpl {
    storage: salsa::Storage<Self>,

    // The logs are only used for testing and demonstrating reuse:
    #[cfg(test)]
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

#[cfg(test)]
impl Default for DatabaseImpl {
    fn default() -> Self {
        let logs = <Arc<Mutex<Option<Vec<String>>>>>::default();
        Self {
            storage: salsa::Storage::new(Some(Box::new({
                let logs = logs.clone();
                move |event| {
                    eprintln!("Event: {event:?}");
                    // Log interesting events, if logging is enabled
                    if let Some(logs) = &mut *logs.lock().unwrap() {
                        // only log interesting events
                        if let salsa::EventKind::WillExecute { .. } = event.kind {
                            logs.push(format!("Event: {event:?}"));
                        }
                    }
                }
            }))),
            logs,
        }
    }
}

#[salsa::db]
impl salsa::Database for DatabaseImpl {}

#[salsa::input(debug)]
pub struct SourceProgram {
    #[returns(ref)]
    pub text: String,
}

#[salsa::tracked(debug)]
pub struct Program<'db> {
    #[tracked]
    #[returns(ref)]
    pub node: GreenNode,
}

#[salsa::interned(debug)]
pub struct Name<'db> {
    #[returns(ref)]
    pub text: String,
}

pub type Span = std::ops::Range<usize>;

#[salsa::accumulator]
#[derive(Debug)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
}

#[salsa::tracked]
pub fn parse(db: &dyn salsa::Database, source: SourceProgram) -> Program<'_> {
    let text = source.text(db);
    let lexed = parser::lexer::tokenize(text);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    parser::parse_source_file(&mut p);
    let (interspersed, errors)= parser::intersperse_trivia(
        p.finish().into_iter(),
        &lexed
    );
    let (node, _) = parser::process_events(
        interspersed.into_iter(),
        &lexed,
        &text
    );
    for error in errors {
        Diagnostic {
            span: error.offset..error.offset,
            message: error.message,
        }.accumulate(db);
    }
    Program::new(db, node)
}
