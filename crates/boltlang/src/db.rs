
use std::char;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use rowan::GreenNode;
use salsa::Accumulator;

use crate::parser::{self, Parser, SyntaxError, lexer::LineColumn};

#[salsa::db]
#[derive(Clone)]
#[cfg_attr(not(test), derive(Default))]
pub struct BoltDatabaseImpl {
    storage: salsa::Storage<Self>,

    // The logs are only used for testing and demonstrating reuse:
    #[cfg(test)]
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

#[cfg(test)]
impl Default for BoltDatabaseImpl {
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
impl salsa::Database for BoltDatabaseImpl {}

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

const UNICODE_NEWLINE: u32 = 0x000A;
const UNICODE_INVALID: u32 = 0xFFFD;

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
        error.accumulate(db);
    }
    Program::new(db, node)
}

#[salsa::tracked(debug)]
pub struct DbLineIndex<'db> {
    #[tracked]
    #[returns(ref)]
    lines: Vec<usize>
}

pub fn start_offset_of_line(db: &dyn salsa::Database, index: DbLineIndex, line: usize) -> usize {
    match line.checked_sub(1) {
        None => 0,
        Some(idx) => index.lines(db).get(idx).copied().unwrap(),
    }
}

pub fn line_column_of_offset(db: &dyn salsa::Database, index: DbLineIndex, offset: usize) -> LineColumn {
    let newlines = index.lines(db);
    let line = newlines.partition_point(|&it| it <= offset);
    let start = start_offset_of_line(db, index, line);
    let col = offset - start;
    LineColumn::new(line, col.into())
}

pub fn end_offset_of_line(db: &dyn salsa::Database, index: DbLineIndex, line: usize) -> usize {
    index.lines(db).get(line).copied().unwrap()-1
}

#[salsa::tracked]
pub fn index_lines(db: &dyn salsa::Database, source: SourceProgram) -> DbLineIndex<'_> {

    let mut out = Vec::new();
    let mut char_offset = 0;
    let text = source.text(db);
    let mut iter = text.bytes();

    loop {

        macro_rules! unwrap_byte {
            () => {
                match iter.next() {
                    None => break,
                    Some(byte) => byte,
                }
            }
        }

        let code = match unwrap_byte!() {
            b0 if b0 < 0x80 => b0 as u32,
            b0 if (b0 & 0xe0) == 0xc0 => {
                let b1 = unwrap_byte!();
                  (((b0 & 0x1f) as u32) <<  6)
                | (((b1 & 0x3f) as u32) <<  0)
            },
            b0 if (b0 & 0xf0) == 0xe0 => {
                let b1 = unwrap_byte!();
                let b2 = unwrap_byte!();
                  (((b0 & 0x0f) as u32) << 12)
                | (((b1 & 0x3f) as u32) <<  6)
                | (((b2 & 0x3f) as u32) <<  0)
            },
            b0 if (b0 & 0xf8) == 0xf0 && (b0 <= 0xf4) => {
                let b1 = unwrap_byte!();
                let b2 = unwrap_byte!();
                let b3 = unwrap_byte!();
                  (((b0 & 0x07) as u32) << 18)
                | (((b1 & 0x3f) as u32) << 12)
                | (((b2 & 0x3f) as u32) <<  6)
                | (((b3 & 0x3f) as u32) <<  0)
            },
            _ => {
                // Invalid UTF-8 byte sequence
                UNICODE_INVALID
            }
        };

        if code == UNICODE_NEWLINE {
            out.push(char_offset);
        }
        char_offset += 1;

    }

    out.push(char_offset);

    DbLineIndex::new(db, out)

}
