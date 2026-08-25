
use std::num::NonZeroU32;

use rowan::{GreenNode, GreenNodeBuilder};

use crate::{
    Diagnostic, SyntaxKind::TOMBSTONE, diagnostic::SyntaxDiagnostic, parser::lexer::LexResult, syntax::SyntaxKind
};

/// Intermediate error structure generated during parsing.
pub type ParseError = String;

/// `Parser` produces a flat list of `Event`s.
/// They are converted to a tree-structure in
/// a separate pass, via `TreeBuilder`.
///
/// Kept to 8 bytes: error messages live in a side table on the `Parser`
/// (the `errors` vec) and `Event::Error` only stores an index into it.
/// `forward_parent` uses `NonZeroU32` so `Option` is niche-optimised away
/// (the offset is always ≥ 1 because the forward parent sits later in the
/// event stream).
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Event {

    /// This event signifies the start of the node.
    /// It should be either abandoned (in which case the
    /// `kind` is `TOMBSTONE`, and the event is ignored),
    /// or completed via a `Finish` event.
    ///
    /// All tokens between a `Start` and a `Finish` would
    /// become the children of the respective node.
    Start {
        kind: SyntaxKind,
        forward_parent: Option<NonZeroU32>,
    },

    /// Complete previous `Start` event
    Finish,

    /// Produce a single leaf-element.
    Token {
        kind: SyntaxKind,
    },

    /// Produce an error at the given syntax level.
    Error {
        // TODO actually store the errors in a separate vec
        msg: ParseError,
    }
}

impl Event {

    pub(crate) fn tombstone() -> Self {
        Event::Start { kind: TOMBSTONE, forward_parent: None }
    }

}

pub fn process_events<I: Iterator<Item = Event>>(
    events: I,
    lexed: &LexResult,
    text: &str
) -> (GreenNode, Vec<Diagnostic>) {
    let mut processor = EventProcessor::new(lexed, text);
    for event in events {
        processor.feed_event(event);
    }
    debug_assert!(processor.pos == lexed.len());
    (processor.builder.finish(), processor.errors)
}

struct EventProcessor<'lex, 'text, 'cache> {
    lexed: &'lex LexResult,
    text: &'text str,
    /// Kept to store on any diagnostics
    errors: Vec<Diagnostic>,
    builder: GreenNodeBuilder<'cache>,
    /// Which token is being inspected
    pos: u32,
    /// Where in the text the token is
    text_pos: usize,
}

impl <'lex, 'text, 'cache> EventProcessor<'lex, 'text, 'cache> {

    fn new(lexed: &'lex LexResult, text: &'text str) -> Self {
        Self {
            lexed,
            text,
            errors: Vec::new(),
            builder: GreenNodeBuilder::new(),
            pos: 0,
            text_pos: 0,
        }
    }

    fn push_token(&mut self) {
        let kind = self.lexed.token_kind(self.pos);
        let len = self.lexed.token_len(self.pos) as usize;
        let start = self.text_pos as usize;
        let end = start + len;
        self.builder.token(kind.into(), &self.text[start..end]);
        self.text_pos += len;
        self.pos += 1;
    }

    fn feed_event(&mut self, event: Event) {
        match event {
            Event::Start { kind: TOMBSTONE, .. } => {},
            Event::Start { kind, ..  } => {
                self.builder.start_node(kind.into());
            }
            Event::Token { kind: expected } => {
                #[cfg(debug_assertions)]
                {
                    let actual = self.lexed.token_kind(self.pos);
                    assert!(actual == expected, "{actual:?} == {expected:?}");
                }
                self.push_token();
            }
            Event::Error { msg } => {
                let start  = self.text_pos;
                self.errors.push(SyntaxDiagnostic::new(msg, start..start).into());
            }
            Event::Finish => {
                self.builder.finish_node();
            }
        }
    }

}

fn with_text<'text, 'event, I: Iterator<Item = &'event Event>>(iter: I, lexed: &LexResult, text: &'text str) -> impl Iterator<Item = &'text str> {
    let mut text_pos = 0;
    let mut pos = 0;
    iter
        .filter(|ev| matches!(ev, Event::Token { .. }))
        .map(move |_ev| {
            let start = text_pos;
            let len = lexed.token_len(pos) as usize;
            let end = start + len;
            let text = &text[start..end];
            text_pos += len;
            pos += 1;
            text
        })
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum IntersperseState {
    PendingEnter,
    Normal,
    PendingExit,
}

pub(crate) struct IntersperseTrivia<'a> {
    lexed: &'a LexResult,
    pos: u32,
    text_pos: usize,
    state: IntersperseState,
    output: Vec<Event>,
}

pub(crate) fn intersperse_trivia<'a, I: Iterator<Item = Event>>(events: I, lexed: &'a LexResult) -> Vec<Event> {
    let mut builder = IntersperseTrivia::new(lexed);
    for event in events {
        builder.feed_event(event);
    }
    match std::mem::replace(&mut builder.state, IntersperseState::Normal) {
        IntersperseState::PendingExit => {
            builder.eat_trivias();
            builder.output.push(Event::Finish);
        }
        IntersperseState::PendingEnter | IntersperseState::Normal => unreachable!(),
    }
    builder.output
}

impl <'a> IntersperseTrivia<'a> {

    pub fn new(lexed: &'a LexResult) -> Self {
        Self {
            lexed,
            pos: 0,
            text_pos: 0,
            state: IntersperseState::PendingEnter,
            output: Vec::new(),
        }
    }

    fn eat_trivias(&mut self) {
        while self.pos < self.lexed.len() {
            let kind = self.lexed.token_kind(self.pos);
            if !kind.is_trivia() {
                break;
            }
            self.do_token(kind);
        }
    }

    fn do_token(&mut self, kind: SyntaxKind) {
        self.output.push(Event::Token { kind });
        self.text_pos += self.lexed.token_len(self.pos) as usize;
        self.pos += 1
    }

    fn feed_event(&mut self, event: Event) {
        // FIXME must be placed somewhere else
        match event {
            Event::Token { kind } => {
                match std::mem::replace(&mut self.state, IntersperseState::Normal) {
                    IntersperseState::PendingEnter => unreachable!(),
                    IntersperseState::PendingExit => self.output.push(Event::Finish),
                    IntersperseState::Normal => (),
                }
                self.eat_trivias();
                self.do_token(kind);
            }
            Event::Start { kind: TOMBSTONE, .. } => {},
            Event::Start { kind, .. } => {
                match std::mem::replace(&mut self.state, IntersperseState::Normal) {
                    IntersperseState::PendingEnter => {
                        self.output.push(Event::Start { kind, forward_parent: None });
                        // No need to attach trivias to previous node: there is no
                        // previous node.
                        return;
                    }
                    IntersperseState::PendingExit => self.output.push(Event::Finish),
                    IntersperseState::Normal => (),
                }
                self.eat_trivias();
                self.output.push(Event::Start { kind, forward_parent: None });
                // TODO add trivias attached to node here
                // https://github.com/rust-lang/rust-analyzer/blob/137eee2f3d9acbabe677b07e221686d38f233ce9/crates/parser/src/shortcuts.rs#L156
            }
            Event::Finish => {
                match std::mem::replace(&mut self.state, IntersperseState::PendingExit) {
                    IntersperseState::PendingEnter => unreachable!(),
                    IntersperseState::PendingExit => self.output.push(Event::Finish),
                    IntersperseState::Normal => (),
                }
            }
            e@Event::Error { .. } => {
                self.eat_trivias();
                self.output.push(e)
            }
        }
    }

}

