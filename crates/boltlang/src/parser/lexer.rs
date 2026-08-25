
use std::{collections::{HashMap, VecDeque}, str::Chars};

use crate::{parser::parser::Input, syntax::SyntaxKind};

use SyntaxKind::*;
use itertools::izip;
use lazy_static::lazy_static;
use unicode_ident::{is_xid_continue, is_xid_start};

const CHAR_EOF: char = '\u{FFFF}';

#[derive(Debug, Clone)]
pub struct Pos {
    offset: usize,
    line: usize,
    column: usize,
}

impl PartialEq for Pos {
    fn eq(&self, other: &Self) -> bool {
        self.offset.eq(&other.offset)
    }
}

impl Eq for Pos {}

impl Pos {
    pub fn new(offset: usize, line: usize, column: usize) -> Self {
        Self { offset, line, column }
    }
}

impl Default for Pos {
    fn default() -> Self {
        Self::new(0, 1, 1)
    }
}

pub struct Lexer<I> {
    iter: I,
    buffer: VecDeque<char>,
    pos: Pos,
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

fn is_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '&' | '^' | '|' | '<' | '>' | '=' | '$' | '?' | '!')
}

lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, SyntaxKind> = {
        let mut m = HashMap::new();
        m.insert("do", DO_KEYWORD);
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

impl <'a> Lexer<Chars<'a>> {

    pub fn new(iter: Chars<'a>) -> Self {
        Self {
            iter,
            pos: Pos::default(),
            buffer: VecDeque::new(),
            errors: Vec::new(),
        }
    }

}

impl <I: Iterator<Item = char>> Lexer<I> {

    fn read(&mut self) -> char {
        self.iter.next().unwrap_or(CHAR_EOF)
    }

    fn get(&mut self) -> char {
        let ch = match self.buffer.pop_front() {
            Some(ch) => ch,
            None => self.read(),
        };
        if ch != CHAR_EOF {
            self.pos.offset += 1;
            if ch == '\n' {
                self.pos.line += 1;
                self.pos.column = 1;
            } else {
                self.pos.column += 1;
            }
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

    pub fn pos(&self) -> Pos {
        self.pos.clone()
    }

    pub fn scan(&mut self) -> SyntaxKind {
        let c0 = self.get();
        match c0 {
            CHAR_EOF => EOF,
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
            ';' => SEMI,
            '=' => EQUALS,
            '"' => {
                loop {
                    let c1 = self.get();
                    if c1 == '"' {
                        break;
                    }
                }
                STRING
            }
            '#' => {
                loop {
                    let c1 = self.get();
                    if c1 == '\n' || c1 == CHAR_EOF {
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
                KEYWORDS.get(name.as_str()).copied().unwrap_or(IDENT)
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
            c0 if is_operator(c0) => {
                let mut text = String::new();
                text.push(c0);
                while is_operator(self.peek(0)) {
                    text.push(self.get());
                }
                if text == "->" {
                    R_ARROW
                // FIXME what if something like =+=?
                } else if text.ends_with("==") {
                    OPERATOR
                } else if text.ends_with('=') {
                    ASSIGNMENT
                } else {
                    OPERATOR
                }
            }
            _ => ERROR,
        }
    }

}

#[derive(Debug)]
pub struct LexResult {
    kinds: Vec<SyntaxKind>,
    lens: Vec<u32>,
    values: Vec<Option<String>>,
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
        let mut kinds = Vec::new();
        let mut lens = Vec::new();
        let mut values = Vec::new();
        for (kind, len, value) in izip!(self.kinds.iter().copied(), self.lens.iter(), self.values.iter().cloned()) {
            if kind.is_trivia() {
                continue;
            }
            kinds.push(kind);
            lens.push(len);
            values.push(value);
        }
        Input::new(kinds, values)
    }

}

#[derive(Debug, Clone)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

impl LineColumn {

    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
        }
    }

}

impl Default for LineColumn {
    fn default() -> Self {
        LineColumn { line: 1, column: 1 }
    }
}

impl From<&Pos> for LineColumn {
    fn from(value: &Pos) -> Self {
        LineColumn::new(value.line, value.column)
    }
}

fn needs_value(kind: SyntaxKind) -> bool {
    matches!(kind, OPERATOR)
}

pub fn tokenize(text: impl Into<String>) -> LexResult {

    // Input
    let text = text.into();

    // Output
    let mut kinds = Vec::new();
    let mut lens = Vec::new();
    let mut values = Vec::new();

    // State
    let mut lexer = Lexer::new(text.chars());

    loop {
        let start = lexer.pos();
        let kind = lexer.scan();
        let end = lexer.pos();
        if kind == EOF {
            break;
        }

        // Transfer the token from the input to the output vectors
        kinds.push(kind);
        lens.push((end.offset - start.offset) as u32);
        values.push(needs_value(kind).then(|| text[start.offset..end.offset].to_string()));
    }

    // For debugging only
    // for (i, (a, b)) in (&kinds).iter().zip(&lines).enumerate() {
    //     eprintln!("{i}. {a:?} = {b}");
    // }

    LexResult {
        kinds,
        values,
        lens,
    }
}
