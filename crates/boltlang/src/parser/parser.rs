
use std::{collections::HashMap, num::NonZeroU32};

use crate::{parser::{event::Event, token_set::TokenSet}, syntax::SyntaxKind, util::DropBomb};

use SyntaxKind::*;

pub(crate) struct Marker {
    pos: u32,
    bomb: DropBomb,
}

/// Build a forward-parent offset. The offset is always ≥ 1 because the
/// forward-parent event is created *after* the event it forwards to, so
/// `NonZeroU32` is always valid here. Panics only on a parser bug.
#[inline]
fn fwd_parent(offset: u32) -> NonZeroU32 {
    NonZeroU32::new(offset).expect("forward-parent offset must be non-zero")
}

impl Marker {

    pub(crate) fn new(pos: u32) -> Self {
        Marker {
            pos,
            bomb: DropBomb::new("Marker must be either completed or abandoned"),
        }
    }

    /// Abandon this node and apppend all children of this node directly to the parent node.
    pub(crate) fn abandon(mut self, p: &mut Parser) {
        self.bomb.defuse();
        let idx = self.pos as usize;
        if idx == p.events.len() - 1 {
            assert!(matches!(
                p.events.pop(),
                Some(Event::Start { kind: TOMBSTONE, forward_parent: None })
            ));
        }}

    /// Will create a node with children between [Parer::start] and the current position.
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
        let end_pos = p.events.len() as u32;
        CompletedMarker::new(self.pos, end_pos, kind)
    }

}

pub(crate) struct CompletedMarker {
    start_pos: u32,
    end_pos: u32,
    kind: SyntaxKind,
}

impl CompletedMarker {

    pub(crate) fn new(start_pos: u32, end_pos: u32, kind: SyntaxKind) -> Self {
        Self {
            start_pos,
            end_pos,
            kind
        }
    }

    /// This method allows to create a new node which starts
    /// *before* the current one. That is, parser could start
    /// node `A`, then complete it, and then after parsing the
    /// whole `A`, decide that it should have started some node
    /// `B` before starting `A`. `precede` allows to do exactly
    /// that. See also docs about
    /// [`Event::Start::forward_parent`](crate::event::Event::Start::forward_parent).
    ///
    /// Given completed events `[START, FINISH]` and its corresponding
    /// `CompletedMarker(pos: 0, _)`.
    /// Append a new `START` events as `[START, FINISH, NEWSTART]`,
    /// then mark `NEWSTART` as `START`'s parent with saving its relative
    /// distance to `NEWSTART` into forward_parent(=2 in this case);
    pub(crate) fn precede(self, p: &mut Parser<'_>) -> Marker {
        let new_pos = p.start();
        let idx = self.start_pos as usize;
        match &mut p.events[idx] {
            Event::Start { forward_parent, .. } => {
                *forward_parent = Some(fwd_parent(new_pos.pos - self.start_pos));
            }
            _ => unreachable!(),
        }
        new_pos
    }

    /// Extends this completed marker *to the left* up to `m`.
    pub(crate) fn extend_to(self, p: &mut Parser<'_>, mut m: Marker) -> CompletedMarker {
        m.bomb.defuse();
        let idx = m.pos as usize;
        match &mut p.events[idx] {
            Event::Start { forward_parent, .. } => {
                *forward_parent = Some(fwd_parent(self.start_pos - m.pos));
            }
            _ => unreachable!(),
        }
        self
    }

    pub(crate) fn last_token(&self, p: &Parser<'_>) -> Option<SyntaxKind> {
        let end_pos = self.end_pos as usize;
        debug_assert_eq!(p.events[end_pos - 1], Event::Finish);
        p.events[..end_pos].iter().rev().find_map(|event| match event {
            Event::Token { kind, .. } => Some(*kind),
            _ => None,
        })
    }

}

pub struct Input {
    kinds: Vec<SyntaxKind>,
    values: Vec<Option<String>>,
}

impl Input {

    pub fn new(kinds: Vec<SyntaxKind>, values: Vec<Option<String>>) -> Self {
        Self { kinds, values }
    }

    pub fn kind(&self, idx: usize) -> SyntaxKind {
        self.kinds.iter().nth(idx).copied().unwrap_or(EOF)
    }

}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Associativity {
    Left,
    Right,
}

#[derive(Clone)]
struct OpDesc {
    pub assoc: Associativity,
    pub prec: u8,
}

const EMPTY_OP_DESC: OpDesc = OpDesc { prec: 0, assoc: Associativity::Left };

impl OpDesc {

    pub(crate) fn is_rassoc(&self) -> bool {
        self.assoc == Associativity::Right
    }

    pub(crate) fn is_lassoc(&self) -> bool {
        self.assoc == Associativity::Left
    }

}

pub struct Parser<'t> {
    inp: &'t Input,
    pos: usize,
    pub events: Vec<Event>,
    expr_op_table: HashMap<String, OpDesc>,
}

impl <'t> Parser<'t> {

    pub fn new(inp: &'t Input) -> Self {
        Self {
            inp,
            pos: 0,
            events: Vec::with_capacity(2 * inp.kinds.len()),
            expr_op_table: HashMap::new(),
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

    /// Tries to consume a specific token.
    ///
    /// If the token isn't present, this function returns `false`.
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
        if kind == EOF {
            return;
        }
        self.do_bump(kind);
    }

    /// Starts a new node in the syntax tree. All nodes and tokens
    /// consumed between the `start` and the corresponding `Marker::complete`
    /// belong to the same node.
    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len() as u32;
        self.push_event(Event::tombstone());
        Marker::new(pos)
    }

    /// Advances the parser by one token, remapping its kind.
    /// This is useful to create contextual keywords from
    /// identifiers. For example, the lexer creates a `union`
    /// *identifier* token, but the parser remaps it to the
    /// `union` keyword, and keyword is what ends up in the
    /// final tree.
    pub(crate) fn bump_remap(&mut self, kind: SyntaxKind) {
        if self.nth(0) == EOF {
            // FIXME: panic!?
            return;
        }
        self.do_bump(kind);
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
        self.error(format!("expected {}", kind.pretty()));
        false
    }

    /// Create an error node and consume the next token.
    pub(crate) fn err_and_bump(&mut self, message: &str) {
        let m = self.start();
        self.error(message);
        self.bump_any();
        m.complete(self, ERROR);
    }

    pub(crate) fn value(&self) -> Option<&String> {
        self.inp.values[self.pos].as_ref()
    }

    pub(crate) fn op_text(&self) -> Option<&str> {
        match self.current() {
            OPERATOR => Some(self.value().unwrap().as_str()),
            LT => Some("<"),
            GT => Some(">"),
            _ => None,
        }
    }

    pub fn is_operator(&self, text: &str) -> bool {
        self.op_text().is_some_and(|t| t == text)
    }

    pub fn is_expr_prefix_operator(&self) -> bool {
        self.op_text().is_some_and(|t| matches!(t, "-" | "+" | "!"))
    }

    pub(crate) fn expr_operator(&self) -> (u8, &str, Associativity) {
        let text = self.op_text().expect("current token is not an operator");
        let desc = self.expr_op_table.get(text).unwrap_or(&EMPTY_OP_DESC);
        (desc.prec, text, desc.assoc)
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

