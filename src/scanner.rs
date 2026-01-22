
use std::{collections::{HashMap, VecDeque}, str::Chars};

use crate::{parser::parser::Input, syntax::SyntaxKind};

use SyntaxKind::*;
use lazy_static::lazy_static;
use unicode_ident::{is_xid_continue, is_xid_start};

const EOF: char = '\u{FFFF}';

pub struct Scanner<I> {
    iter: I,
    buffer: VecDeque<char>,
    offset: u32,
    errors: Vec<Error>,
}

type Error = String;

const fn is_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\t' | '\r')
}

fn is_ident_start(ch: char) -> bool {
    is_xid_start(ch)
}

fn is_ident_part(ch: char) -> bool {
    is_xid_continue(ch)
}

fn is_bin_num(ch: char) -> bool {
    matches!(ch, '0' | '1')
}

fn is_oct_num(ch: char) -> bool {
    matches!(ch, '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7')
}

fn is_dec_num(ch: char) -> bool {
    matches!(ch, '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '9')
}

fn is_hex_num(ch: char) -> bool {
    matches!(ch, '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '9' | 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'A' | 'B' | 'C' | 'D' | 'E' | 'F')
}

lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, SyntaxKind> = {
        let mut m = HashMap::new();
        m.insert("fn", FN_KEYWORD);
        m.insert("let", LET_KEYWORD);
        m.insert("match", MATCH_KEYWORD);
        m.insert("mut", MUT_KEYWORD);
        m.insert("pub", PUB_KEYWORD);
        m.insert("return", RETURN_KEYWORD);
        m.insert("type", TYPE_KEYWORD);
        m
    };
}

impl <'a> Scanner<Chars<'a>> {

    pub fn new(iter: Chars<'a>) -> Self {
        Self {
            iter,
            offset: 0,
            buffer: VecDeque::new(),
            errors: Vec::new(),
        }
    }

}

impl <I: Iterator<Item = char>> Scanner<I> {

    fn read(&mut self) -> char {
        self.iter.next().unwrap_or(EOF)
    }

    fn get(&mut self) -> char {
        let ch = match self.buffer.pop_front() {
            Some(ch) => ch,
            None => self.read(),
        };
        if ch != EOF {
            self.offset += 1;
        }
        ch
    }

    fn peek(&mut self, n: usize) -> char {
        while self.buffer.len() <= n {
            let ch = self.read();
            self.buffer.push_back(ch);
        }
        self.buffer[n]
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }

    pub fn pos(&self) -> u32 {
        self.offset
    }

    pub fn scan(&mut self) -> SyntaxKind {
        let c0 = self.get();
        match c0 {
            EOF => END_OF_FILE,
            ch if is_whitespace(ch) => {
                while is_whitespace(self.peek(0)) {
                    self.get();
                }
                WHITESPACE
            }
            '(' => L_PAREN,
            ')' => R_PAREN,
            '{' => L_BRACE,
            '}' => R_BRACE,
            '[' => L_BRACKET,
            ']' => R_BRACKET,
            ',' => COMMA,
            ':' => COLON,
            '=' => EQUALS,
            '#' => {
                loop {
                    let c1 = self.get();
                    if c1 == '\n' || c1 == EOF {
                        break;
                    }
                }
                LINE_COMMENT
            }
            c0 if is_ident_start(c0) => {
                let mut name = String::new();
                name.push(c0);
                while is_ident_part(self.peek(0)) {
                    name.push(self.get());
                }
                KEYWORDS.get(name.as_str()).copied().unwrap_or(IDENTIFIER)
            }
            '0' => match self.get() {
                'b' => {
                    while is_bin_num(self.peek(0)) {
                        self.get();
                    }
                    BIN_INT
                }
                'o' => {
                    while is_oct_num(self.peek(0)) {
                        self.get();
                    }
                    OCT_INT
                }
                'x' => {
                    while is_hex_num(self.peek(0)) {
                        self.get();
                    }
                    HEX_INT
                }
                c1 if is_dec_num(c1) => {
                    while is_dec_num(c1) {
                        self.get();
                    }
                    DEC_INT
                }
                c1 if is_ident_part(c1) => {
                    self.error("identifiers may not begin with decimal digits");
                    ERROR
                }
                _ => DEC_INT
            }
            c0 if is_dec_num(c0) => {
                while is_dec_num(self.peek(0)) {
                    self.get();
                }
                DEC_INT
            }
            _ => ERROR,
        }
    }

}

#[derive(Debug)]
pub struct LexResult {
    kinds: Vec<SyntaxKind>,
    lens: Vec<u32>,
}

impl LexResult {

    pub fn token_kind(&self, n: u32) -> SyntaxKind {
        self.kinds[n as usize]
    }

    pub fn token_len(&self, n: u32) -> u32 {
        self.lens[n as usize]
    }

    pub fn len(&self) -> u32 {
        self.kinds.len() as u32
    }

    pub fn to_input(&self) -> Input {
        Input::new(self.kinds
            .iter()
            .filter(|k| !k.is_trivia())
            .copied()
            .collect())
    }

}

pub fn tokenize(text: impl Into<String>) -> LexResult {
    let text = text.into();
    let mut scanner = Scanner::new(text.chars());
    let mut kinds = Vec::new();
    let mut lens = Vec::new();
    loop {
        let start = scanner.pos();
        let kind = scanner.scan();
        let end = scanner.pos();
        if kind == END_OF_FILE {
            break;
        }
        kinds.push(kind);
        lens.push(end - start);
    }
    LexResult {
        kinds,
        lens,
    }
}
