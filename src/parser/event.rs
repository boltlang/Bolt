use std::collections::VecDeque;

use rowan::{GreenNode, GreenNodeBuilder};

use super::error::Error;
use crate::{parser::error::SyntaxError, scanner::LexResult, syntax::SyntaxKind};

#[derive(Debug)]
pub(crate) enum Event {

    /// This event signifies the start of the node.
    /// It should be either abandoned (in which case the
    /// `kind` is `TOMBSTONE`, and the event is ignored),
    /// or completed via a `Finish` event.
    ///
    /// All tokens between a `Start` and a `Finish` would
    /// become the children of the respective node.
    Start { kind: SyntaxKind },

    /// Complete previous `Start` event
    Finish,

    /// Produce a single leaf-element.
    Token {
        kind: SyntaxKind,
    },

    /// Produce an error at the given syntax level.
    Error {
        msg: String,
    }
}

pub fn process<I: Iterator<Item = Event>>(
    events: I,
    lexed: &LexResult,
    text: &str
) -> (GreenNode, Vec<Error>) {
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
    errors: Vec<Error>,
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
            Event::Start { kind  } => {
                self.builder.start_node(kind.into());
            }
            Event::Token { kind: expected } => {
                // let mut actual;
                // loop {
                //     actual = self.lexed.token_kind(self.pos);
                //     if !actual.is_trivia() {
                //         break;
                //     }
                //     self.push_token();
                // }
                // debug_assert!(actual == expected, "{actual:?} == {expected:?}");
                self.push_token();
            }
            Event::Error { msg } => {
                self.errors.push(msg);
            }
            Event::Finish => {
                self.builder.finish_node();
            }
        }
    }

}

#[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum State {
    PendingEnter,
    Normal,
    PendingExit,
}

struct WithText<'a, 'b, I> {
    text: &'a str,
    pos: u32,
    lexed: &'b LexResult,
    text_pos: usize,
    events: I,
}

impl <'a, 'b, I> WithText<'a, 'b, I> {

    pub fn new(events: I, lexed: &'b LexResult, text: &'a str) -> Self {
        Self {
            text,
            lexed,
            pos: 0,
            text_pos: 0,
            events,
        }
    }

}

impl <'a, 'b, I: Iterator<Item = Event>> Iterator for WithText<'a, 'b, I> {

    type Item = (Event, Option<&'a str>);

    fn next(&mut self) -> Option<Self::Item> {
        match self.events.next()? {
            e@Event::Token { .. } => {
                let start = self.text_pos;
                let len = self.lexed.token_len(self.pos) as usize;
                let end = start + len;
                let text = &self.text[start..end];
                self.text_pos += len;
                self.pos += 1;
                Some((e, Some(text)))
            }
            e => Some((e, None)),
        }
    }
}

pub(crate) struct IntersperseTrivia<'a> {
    lexed: &'a LexResult,
    pos: u32,
    text_pos: usize,
    state: State,
    output: Vec<Event>,
    errors: Vec<SyntaxError>,
}

pub(crate) fn intersperse_trivia<'a, I: Iterator<Item = Event>>(events: I, lexed: &'a LexResult) -> Vec<Event> {
    let mut builder = IntersperseTrivia::new(lexed);
    for event in events {
        builder.feed_event(event);
    }
    match std::mem::replace(&mut builder.state, State::Normal) {
        State::PendingExit => {
            builder.eat_trivias();
            builder.output.push(Event::Finish);
        }
        State::PendingEnter | State::Normal => unreachable!(),
    }
    builder.output
}

impl <'a> IntersperseTrivia<'a> {

    pub fn new(lexed: &'a LexResult) -> Self {
        Self {
            lexed,
            pos: 0,
            text_pos: 0,
            state: State::PendingEnter,
            output: Vec::new(),
            errors: Vec::new(),
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
                eprintln!("TOKEN");
                match std::mem::replace(&mut self.state, State::Normal) {
                    State::PendingEnter => unreachable!(),
                    State::PendingExit => self.output.push(Event::Finish),
                    State::Normal => (),
                }
                self.eat_trivias();
                self.do_token(kind);
            }
            Event::Start { kind } => {
                eprintln!("START");
                match std::mem::replace(&mut self.state, State::Normal) {
                    State::PendingEnter => {
                        self.output.push(Event::Start { kind });
                        // No need to attach trivias to previous node: there is no
                        // previous node.
                        return;
                    }
                    State::PendingExit => self.output.push(Event::Finish),
                    State::Normal => (),
                }
                self.eat_trivias();
                self.output.push(Event::Start { kind });
                // TODO add trivias attached to node here
            }
            Event::Finish => {
                match std::mem::replace(&mut self.state, State::PendingExit) {
                    State::PendingEnter => unreachable!(),
                    State::PendingExit => self.output.push(Event::Finish),
                    State::Normal => (),
                }
            }
            Event::Error { msg } => {
                eprintln!("ERROR");
                let start  = self.text_pos;
                self.errors.push(SyntaxError::new(msg, start));
            }
        }
    }

}

