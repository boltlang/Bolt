use std::num::NonZeroU32;

use crate::{
    diagnostic::{Diagnostics, ExpectedTokenDiagnostic},
    lexer::LexResult,
    rowan::{GreenNode, GreenNodeBuilder},
};
use boltlang_syntax::SyntaxKind::{self, TOMBSTONE};

/// Intermediate error structure generated during parsing.
pub type ParseError = String;

/// `Parser` produces a flat list of `Event`s. They are turned into a tree
/// in a separate pass, by [`intersperse_trivia`] and [`process_events`].
///
/// `Event::Error` holds its message. `forward_parent` uses `NonZeroU32`, so
/// that its `Option` takes no extra space: the offset is always ≥ 1 because
/// the forward parent sits later in the event stream.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Event {
    /// This event signifies the start of the node.
    /// It should be either abandoned (in which case the
    /// `kind` is `TOMBSTONE`, and the event is ignored),
    /// or completed via a `Finish` event.
    ///
    /// All tokens between a `Start` and a `Finish` would
    /// become the children of the respective node.
    Start { kind: SyntaxKind, forward_parent: Option<NonZeroU32> },

    /// Complete previous `Start` event
    Finish,

    /// Produce a single leaf-element.
    Token { kind: SyntaxKind },

    /// Produce an error at the given syntax level.
    Error { msg: ParseError },
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

struct EventProcessor<'lex, 'text, 'diag> {
    lexed: &'lex LexResult,
    text: &'text str,
    diags: &'diag mut Diagnostics,
    builder: GreenNodeBuilder<'static>,
    /// Which token is being inspected
    pos: u32,
    /// Where in the text the token is
    text_pos: usize,
}

impl<'lex, 'text, 'diag> EventProcessor<'lex, 'text, 'diag> {
    fn new(lexed: &'lex LexResult, text: &'text str, diags: &'diag mut Diagnostics) -> Self {
        Self { lexed, text, diags, builder: GreenNodeBuilder::new(), pos: 0, text_pos: 0 }
    }

    /// Add the next token from the source text to the tree, as `kind`. This is
    /// usually the kind the lexer gave it, but the parser can remap it, e.g. to
    /// turn the identifier `default` into a keyword.
    fn push_token(&mut self, kind: SyntaxKind) {
        let len = self.lexed.token_len(self.pos) as usize;
        let start = self.text_pos;
        let end = start + len;
        self.builder.token(kind.into(), &self.text[start..end]);
        self.text_pos += len;
        self.pos += 1;
    }

    fn feed_event(&mut self, event: Event) {
        match event {
            Event::Start { kind: TOMBSTONE, .. } => {}
            Event::Start { kind, .. } => {
                self.builder.start_node(kind.into());
            }
            Event::Token { kind } => {
                self.push_token(kind);
            }
            Event::Error { msg } => {
                let start = self.text_pos;
                self.diags.push(ExpectedTokenDiagnostic::new(msg, start..start).into());
            }
            Event::Finish => {
                self.builder.finish_node();
            }
        }
    }
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

/// The parser's events with events for the trivia (whitespace and comments)
/// added between them. Trivia between two nodes goes outside both, so a
/// node starts at its first token and ends at its last one.
pub(crate) fn intersperse_trivia(events: Vec<Event>, lexed: &LexResult) -> Vec<Event> {
    let mut builder =
        IntersperseTrivia { lexed, pos: 0, state: State::BeforeRoot, output: Vec::new() };
    for event in resolve_forward_parents(events) {
        builder.feed_event(event);
    }
    assert!(builder.state == State::AfterFinish, "the root node wasn't finished");
    builder.eat_trivia();
    builder.output.push(Event::Finish);
    builder.output
}

#[derive(PartialEq, Eq)]
enum State {
    /// No node has started yet. Trivia at the start of the file goes in the
    /// root node.
    BeforeRoot,
    Normal,
    /// A node just finished. Its `Finish` waits for the next event, so that
    /// trivia after the node can go outside it.
    AfterFinish,
}

struct IntersperseTrivia<'a> {
    lexed: &'a LexResult,
    /// The index of the next token in `lexed`.
    pos: u32,
    state: State,
    output: Vec<Event>,
}

impl IntersperseTrivia<'_> {
    fn eat_trivia(&mut self) {
        while self.pos < self.lexed.len() {
            let kind = self.lexed.token_kind(self.pos);
            if !kind.is_trivia() {
                break;
            }
            self.output.push(Event::Token { kind });
            self.pos += 1;
        }
    }

    /// Push the `Finish` of a node that just finished, if there is one.
    fn finish_pending(&mut self) {
        debug_assert!(self.state != State::BeforeRoot, "an event before the root node");
        if self.state == State::AfterFinish {
            self.output.push(Event::Finish);
        }
        self.state = State::Normal;
    }

    fn feed_event(&mut self, event: Event) {
        match event {
            Event::Start { kind: TOMBSTONE, .. } => {}
            Event::Start { kind, .. } if self.state == State::BeforeRoot => {
                self.state = State::Normal;
                self.output.push(Event::Start { kind, forward_parent: None });
            }
            Event::Finish => {
                self.finish_pending();
                self.state = State::AfterFinish;
            }
            // Close a finished node first, or the trivia before the event
            // would end up inside it, like the space in `g ` of `g x`.
            event => {
                self.finish_pending();
                self.eat_trivia();
                if let Event::Token { kind } = event {
                    self.output.push(Event::Token { kind });
                    self.pos += 1;
                } else {
                    self.output.push(event);
                }
            }
        }
    }
}
