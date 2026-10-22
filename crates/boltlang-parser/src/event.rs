
use std::num::NonZeroU32;

use boltlang_syntax::SyntaxKind::{self, TOMBSTONE};
use crate::{diagnostic::{Diagnostics, ExpectedTokenDiagnostic}, lexer::LexResult, rowan::{GreenNode, GreenNodeBuilder}};

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
    text: &str,
    diags: &mut Diagnostics,
) -> GreenNode {
    let mut processor = EventProcessor::new(lexed, text, diags);
    for event in events {
        processor.feed_event(event);
    }
    debug_assert!(processor.pos == lexed.len());
    processor.builder.finish()
}

struct EventProcessor<'lex, 'text, 'cache, 'diag> {
    lexed: &'lex LexResult,
    text: &'text str,
    diags: &'diag mut Diagnostics,
    builder: GreenNodeBuilder<'cache>,
    /// Which token is being inspected
    pos: u32,
    /// Where in the text the token is
    text_pos: usize,
}

impl <'lex, 'text, 'cache, 'diag> EventProcessor<'lex, 'text, 'cache, 'diag> {

    fn new(lexed: &'lex LexResult, text: &'text str, diags: &'diag mut Diagnostics) -> Self {
        Self {
            lexed,
            text,
            diags,
            builder: GreenNodeBuilder::new(),
            pos: 0,
            text_pos: 0,
        }
    }

    /// Add the next token from the source text to the tree, as `kind`. This is
    /// usually the kind the lexer gave it, but the parser can remap it, e.g. to
    /// turn the identifier `default` into a keyword.
    fn push_token(&mut self, kind: SyntaxKind) {
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
            Event::Token { kind } => {
                self.push_token(kind);
            }
            Event::Error { msg } => {
                let start  = self.text_pos;
                self.diags.push(ExpectedTokenDiagnostic::new(msg, start..start).into());
            }
            Event::Finish => {
                self.builder.finish_node();
            }
        }
    }

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

/// Resolve `forward_parent` links, so that every `Start` event is in the
/// position where its node actually starts.
///
/// [`CompletedMarker::precede`](crate::parser::CompletedMarker::precede) and
/// [`CompletedMarker::extend_to`](crate::parser::CompletedMarker::extend_to)
/// can't insert a `Start` event before an earlier one, so they append it and
/// link to it from the earlier `Start` instead. Here, each chain of links is
/// followed, the linked events are replaced with tombstones, and the chain's
/// nodes are started outermost-first in place of the first event.
fn resolve_forward_parents(mut events: Vec<Event>) -> Vec<Event> {
    let mut output = Vec::with_capacity(events.len());
    let mut chain = Vec::new();
    for i in 0..events.len() {
        match std::mem::replace(&mut events[i], Event::tombstone()) {
            Event::Start { kind, forward_parent } => {
                chain.push(kind);
                let mut idx = i;
                let mut next = forward_parent;
                while let Some(offset) = next {
                    idx += offset.get() as usize;
                    next = match std::mem::replace(&mut events[idx], Event::tombstone()) {
                        Event::Start { kind, forward_parent } => {
                            chain.push(kind);
                            forward_parent
                        }
                        _ => unreachable!("forward_parent must point to a Start event"),
                    };
                }
                for kind in chain.drain(..).rev() {
                    if kind != TOMBSTONE {
                        output.push(Event::Start { kind, forward_parent: None });
                    }
                }
            }
            event => output.push(event),
        }
    }
    output
}

pub(crate) fn intersperse_trivia(events: Vec<Event>, lexed: &LexResult) -> Vec<Event> {
    let mut builder = IntersperseTrivia::new(lexed);
    for event in resolve_forward_parents(events) {
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

