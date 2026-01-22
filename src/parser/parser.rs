
use crate::{parser::{error::Error, event::Event, token_set::TokenSet}, syntax::SyntaxKind, util::DropBomb};

use SyntaxKind::*;

pub(crate) struct Marker {
    pos: u32,
    bomb: DropBomb,
}

impl Marker {

    pub(crate) fn new(pos: u32) -> Self {
        Marker {
            pos,
            bomb: DropBomb::new("Marker must be either completed or abandoned"),
        }
    }

    pub(crate) fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
        self.bomb.defuse();
        let idx = self.pos as usize;
        match &mut p.events[idx] {
            Event::Start { kind: slot, .. } => {
                *slot = kind;
            }
            _ => unreachable!(),
        }
        p.push_event(Event::Finish);
        CompletedMarker {
        }
    }

}

pub(crate) struct CompletedMarker {
}

pub struct Input {
    kinds: Vec<SyntaxKind>,
}

impl Input {

    pub fn new(kinds: Vec<SyntaxKind>) -> Self {
        Self { kinds }
    }

    pub fn kind(&self, idx: usize) -> SyntaxKind {
        self.kinds.iter().nth(idx).copied().unwrap_or(END_OF_FILE)
    }

}

pub struct Parser<'t> {
    inp: &'t Input,
    pos: usize,
    events: Vec<Event>,
    errors: Vec<Error>,
}

impl <'t> Parser<'t> {

    pub fn new(inp: &'t Input) -> Self {
        Self {
            inp,
            pos: 0,
            events: Vec::with_capacity(2 * inp.kinds.len()),
            errors: Vec::new(),
        }
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        self.inp.kind(self.pos + n)
    }

    /// Check whether the current token falls into the given [TokenSet].
    pub(crate) fn at_ts(&self, ts: TokenSet) -> bool {
        ts.contains(self.current())
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth_at(0, kind)
    }

    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        self.inp.kind(self.pos + n) == kind
    }

    pub (crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if !self.at(kind) {
            return false;
        }
        self.do_bump(kind);
        true
    }

    /// Consumes a specfic token.
    ///
    /// Panics if the token does not match the expected kind.
    pub(crate) fn bump(&mut self, kind: SyntaxKind) {
        assert!(self.eat(kind));
    }

    /// Advances the parser by one token
    pub(crate) fn bump_any(&mut self) {
        let kind = self.nth(0);
        if kind == END_OF_FILE {
            return;
        }
        self.do_bump(kind);
    }

    /// Starts a new node in the syntax tree. All nodes and tokens
    /// consumed between the `start` and the corresponding `Marker::complete`
    /// belong to the same node.
    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len() as u32;
        self.push_event(Event::Start { kind: TOMBSTONE });
        Marker::new(pos)
    }

    /// Emit an error.
    pub(crate) fn error<T: Into<String>>(&mut self, message: T) {
        let msg = message.into();
        self.push_event(Event::Error { msg });
    }

    /// Consume the next token if it is `kind` or emit an error
    /// otherwise.
    pub(crate) fn expect(&mut self, kind: SyntaxKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error(format!("expected {kind:?}"));
        false
    }

    /// Create an error node and consume the next token unless it is in the recovery set.
    ///
    /// Returns true if recovery kicked in.
    pub(crate) fn err_recover(&mut self, message: &str, recovery: TokenSet) -> bool {
        if self.at_ts(recovery) {
            self.error(message);
            return true;
        }

        let m = self.start();
        self.error(message);
        self.bump_any();
        m.complete(self, ERROR);
        false
    }

    /// Create an error node and consume the next token.
    pub(crate) fn error_and_bump(&mut self, message: &str) {
        let m = self.start();
        self.error(message);
        self.bump_any();
        m.complete(self, ERROR);
    }

    /// Perform the actual recording of a token.
    fn do_bump(&mut self, kind: SyntaxKind) {
        self.pos += 1;
        self.push_event(Event::Token { kind });
    }

    /// Helper function to add an event to the builder.
    fn push_event(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Finishes the builder and returns the events that were generated during the build.
    pub fn finish(self) -> Vec<Event> {
        self.events
    }

}

