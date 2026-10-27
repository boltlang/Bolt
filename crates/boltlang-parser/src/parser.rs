
use std::{cell::Cell, num::NonZeroU32};

use boltlang_syntax::{SyntaxKind, T};
use boltlang_common::DropBomb;
use crate::{event::Event, token_set::TokenSet};

use SyntaxKind::*;

/// Whether `kind` is a run of operator characters (see [`crate::lexer`]),
/// leaving out `=` and compound assignments like `+=`.
pub(crate) fn is_operator_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        OPERATOR | R_ARROW | FAT_ARROW | VBAR | DOT | DOTDOT | DOTDOTDOT | DOTDOTEQUALS
            | EXCL | QUEST | LT | GT | STAR | PLUS | MINUS | DOLLAR
    )
}

/// A token that the parser split in two. See [`Parser::split_bump`].
pub(crate) struct Split {
    /// The index of the token, counting only non-trivia tokens.
    pub(crate) index: usize,
    /// The length of the first part in bytes.
    pub(crate) len: u32,
    /// The kind of the second part.
    pub(crate) rest_kind: SyntaxKind,
}

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

    /// Whether everything since this marker is one node, with no tokens or
    /// errors outside of it, so that the node can stand in for this one.
    pub(crate) fn has_one_node_since(&self, p: &Parser) -> bool {
        is_one_node(&p.events[self.pos as usize + 1..])
    }

    /// Abandon this node and append all children of this node directly to the parent node.
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
        CompletedMarker::new(self.pos, kind)
    }

}

/// Whether `events` make one node, and nothing outside of it.
///
/// A node that [`CompletedMarker::precede`] wraps around another starts
/// after the other's `Finish`, so it's counted where the chain of
/// `forward_parent` links to it starts.
fn is_one_node(events: &[Event]) -> bool {
    let mut linked = vec![false; events.len()];
    let mut depth = 0usize;
    let mut nodes = 0;
    for (i, event) in events.iter().enumerate() {
        match event {
            Event::Start { .. } if linked[i] => {}
            Event::Start { kind, forward_parent } => {
                let mut started = usize::from(*kind != TOMBSTONE);
                let (mut idx, mut next) = (i, *forward_parent);
                while let Some(offset) = next {
                    idx += offset.get() as usize;
                    linked[idx] = true;
                    let Event::Start { kind, forward_parent } = &events[idx] else { return false };
                    started += usize::from(*kind != TOMBSTONE);
                    next = *forward_parent;
                }
                if started > 0 && depth == 0 {
                    nodes += 1;
                }
                depth += started;
            }
            Event::Finish => depth -= 1,
            Event::Token { .. } | Event::Error { .. } if depth == 0 => return false,
            Event::Token { .. } | Event::Error { .. } => {}
        }
    }
    nodes == 1
}

pub struct CompletedMarker {
    start_pos: u32,
    kind: SyntaxKind,
}

impl CompletedMarker {

    pub(crate) fn new(start_pos: u32, kind: SyntaxKind) -> Self {
        Self {
            start_pos,
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

    pub(crate) fn kind(&self) -> SyntaxKind {
        self.kind
    }

}

/// The tokens the parser reads, without trivia, as kinds and the ranges of
/// their text in the source.
pub struct Input<'t> {
    text: &'t str,
    kinds: Vec<SyntaxKind>,
    ranges: Vec<(u32, u32)>,
}

impl<'t> Input<'t> {

    pub fn new(text: &'t str, kinds: Vec<SyntaxKind>, ranges: Vec<(u32, u32)>) -> Self {
        Self { text, kinds, ranges }
    }

}

/// A token: its kind and the range of its text.
type Token = (SyntaxKind, u32, u32);

pub struct Parser<'t> {
    inp: Input<'t>,
    /// The index in `inp` of the next token, after `rest`.
    raw: usize,
    /// The second part of a token that [`Parser::split_bump`] split. It
    /// comes before `inp[raw]`, so the token list never has to shift.
    rest: Option<Token>,
    /// How many tokens were consumed, counting both parts of split tokens.
    pos: usize,
    events: Vec<Event>,
    /// Tokens that were split in two by [`Parser::split_bump`].
    splits: Vec<Split>,
    /// How often the parser looked at a token since it last consumed one.
    /// See [`PARSER_STEP_LIMIT`].
    steps: Cell<u32>,
}

/// If the parser looks at this many tokens without consuming any, it's
/// stuck in a loop, and [`Parser::nth`] panics instead of hanging.
const PARSER_STEP_LIMIT: u32 = if cfg!(debug_assertions) { 150_000 } else { 15_000_000 };

impl <'t> Parser<'t> {

    pub fn new(inp: Input<'t>) -> Self {
        Self {
            events: Vec::with_capacity(2 * inp.kinds.len()),
            inp,
            raw: 0,
            rest: None,
            pos: 0,
            splits: Vec::new(),
            steps: Cell::new(0),
        }
    }

    /// The token `n` tokens ahead, or `None` past the end.
    fn token(&self, n: usize) -> Option<Token> {
        let idx = match (self.rest, n) {
            (Some(rest), 0) => return Some(rest),
            (Some(_), n) => self.raw + n - 1,
            (None, n) => self.raw + n,
        };
        let (start, end) = *self.inp.ranges.get(idx)?;
        Some((self.inp.kinds[idx], start, end))
    }

    fn token_text(&self, (_, start, end): Token) -> &'t str {
        &self.inp.text[start as usize..end as usize]
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    /// How many tokens were consumed, to check that a loop makes progress.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        let steps = self.steps.get();
        assert!(steps < PARSER_STEP_LIMIT, "the parser seems stuck");
        self.steps.set(steps + 1);
        self.token(n).map_or(EOF, |(kind, ..)| kind)
    }

    /// The text of the token `n` tokens ahead, or `""` past the end.
    pub(crate) fn nth_text(&self, n: usize) -> &'t str {
        self.token(n).map_or("", |token| self.token_text(token))
    }

    /// Whether the current token is the contextual keyword `kw`, like
    /// `T![macro_rules]`. The lexer produces an `IDENT` for these.
    pub(crate) fn at_contextual_kw(&self, kw: SyntaxKind) -> bool {
        self.nth_at_contextual_kw(0, kw)
    }

    pub(crate) fn nth_at_contextual_kw(&self, n: usize, kw: SyntaxKind) -> bool {
        let text = match kw {
            MACRO_RULES_KEYWORD => "macro_rules",
            _ => panic!("{kw:?} is not a contextual keyword"),
        };
        self.nth(n) == IDENT && self.nth_text(n) == text
    }

    /// Whether the current token is the operator `op`, e.g. `"&&"`. Use this
    /// for operators that have no kind of their own (see [`is_operator_kind`]).
    pub(crate) fn at_op(&self, op: &str) -> bool {
        self.nth_at_op(0, op)
    }

    pub(crate) fn nth_at_op(&self, n: usize, op: &str) -> bool {
        is_operator_kind(self.nth(n)) && self.nth_text(n) == op
    }

    pub(crate) fn nth_at_ts(&self, n: usize, ts: TokenSet) -> bool {
        ts.contains(self.nth(n))
    }

    /// Check whether the current token falls into the given [TokenSet].
    pub(crate) fn at_ts(&self, ts: TokenSet) -> bool {
        ts.contains(self.current())
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth_at(0, kind)
    }

    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        self.nth(n) == kind
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


    /// The text of the current token, or `None` at the end.
    pub(crate) fn value(&self) -> Option<&'t str> {
        self.token(0).map(|token| self.token_text(token))
    }

    /// The text of the current token if it's an operator, including operators
    /// that have a kind of their own, like `|` and `<`. Leaves out `=` and
    /// compound assignments like `+=`.
    pub(crate) fn op_text(&self) -> Option<&str> {
        if is_operator_kind(self.current()) {
            self.value()
        } else {
            None
        }
    }




    /// Create an error node and consume the next token unless it is in the recovery set.
    ///
    /// Returns true if recovery kicked in.
    pub(crate) fn err_recover(&mut self, message: &str, recovery: TokenSet) -> bool {
        // Never skip braces: they're what the rest of the parser recovers on.
        if matches!(self.current(), T!['{'] | T!['}']) || self.at_ts(recovery) {
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

    /// Split the current token in two and consume the first `len` bytes as a
    /// token of kind `kind`. The rest stays as the current token, with the
    /// kind the lexer would give it on its own.
    ///
    /// The lexer joins runs of operator characters into one token, so the
    /// parser uses this where the grammar needs part of one, e.g. the first
    /// `|` of the `||` in the closure `|| x`.
    pub(crate) fn split_bump(&mut self, kind: SyntaxKind, len: usize) {
        let text = self.value().expect("no token to split");
        let rest_kind = crate::lexer::operator_kind(&text[len..]);
        self.split_bump_as(kind, len, rest_kind);
    }

    /// Like [`Parser::split_bump`], but the rest becomes a token of kind
    /// `rest_kind`. Used to split the float `0.1` in `t.0.1`.
    pub(crate) fn split_bump_as(&mut self, kind: SyntaxKind, len: usize, rest_kind: SyntaxKind) {
        let token = self.token(0).expect("no token to split");
        let (_, start, end) = token;
        let text = self.token_text(token);
        assert!(0 < len && len < text.len(), "can't split {text:?} at {len}");
        self.splits.push(Split { index: self.pos, len: len as u32, rest_kind });
        self.do_bump(kind);
        // `do_bump` consumed the whole token; the rest is the next one.
        self.rest = Some((rest_kind, start + len as u32, end));
    }

    /// Perform the actual recording of a token.
    fn do_bump(&mut self, kind: SyntaxKind) {
        if self.rest.take().is_none() {
            self.raw += 1;
        }
        self.pos += 1;
        self.steps.set(0);
        self.push_event(Event::Token { kind });
    }

    /// Helper function to add an event to the builder.
    fn push_event(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Finishes the builder and returns the events that were generated during
    /// the build, and the tokens that were split (see [`Parser::split_bump`]).
    pub(crate) fn finish(self) -> (Vec<Event>, Vec<Split>) {
        (self.events, self.splits)
    }

}

