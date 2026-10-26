
use std::{collections::{HashMap, VecDeque}, str::Chars};

use boltlang_common::Span;
use boltlang_syntax::SyntaxKind::{self, *};
use crate::{diagnostic::{Diagnostics, UnexpectedCharDiagnostic}, parser::Input};

use itertools::izip;
use lazy_static::lazy_static;
use unicode_ident::{is_xid_continue, is_xid_start};

const CHAR_EOF: char = '\u{FFFF}';

#[derive(Debug, Clone)]
pub struct Pos {
    /// Byte offset into the source text.
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

pub struct Lexer<'d, I> {
    iter: I,
    buffer: VecDeque<char>,
    pos: Pos,
    diags: &'d mut Diagnostics,
}

/// Unicode whitespace, like the grammar's `\s`.
fn is_whitespace(ch: char) -> bool {
    ch.is_whitespace()
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || is_xid_start(ch)
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
    matches!(ch, '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9')
}

fn is_hex_num(ch: char) -> bool {
    matches!(ch, '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'A' | 'B' | 'C' | 'D' | 'E' | 'F')
}

/// The kind of token for a run of operator characters, e.g. `->` or `+=`.
pub(crate) fn operator_kind(text: &str) -> SyntaxKind {
    match text {
        // Operators with a fixed role in the grammar get a kind of their own.
        "->" => R_ARROW,
        "=>" => FAT_ARROW,
        "|" => VBAR,
        "." => DOT,
        ".." => DOTDOT,
        "..." => DOTDOTDOT,
        "..=" => DOTDOTEQUALS,
        "=" => EQUALS,
        "!" => EXCL,
        "?" => QUEST,
        "<" => LT,
        ">" => GT,
        "*" => STAR,
        "+" => PLUS,
        "-" => MINUS,
        "$" => DOLLAR,
        "!=" | "<=" | ">=" => OPERATOR,
        // Things like +==% and -== should be parsed as a regular operator
        t if t.chars().filter(|c| *c == '=').count() > 1 => OPERATOR,
        // Previous check made sure there is no other '=' in text
        t if t.ends_with('=') => ASSIGNMENT,
        _ => OPERATOR,
    }
}

/// The numeric types a number literal can have as a suffix, like `10u8`.
const NUMERIC_SUFFIXES: &[&str] = &[
    "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "u128", "i128", "isize", "usize", "f32", "f64",
];

/// Characters that are joined into operators like `<$>`. `?` is not one of
/// them: it's always a token of its own, so that `x?.y` and `x??` lex as
/// postfix tries.
fn is_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '&' | '^' | '|' | '<' | '>' | '=' | '$' | '!' | '.')
}

lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, SyntaxKind> = {
        // `macro_rules` is a contextual keyword, so it's left out here; the
        // parser remaps it where needed.
        let mut m = HashMap::new();
        m.insert("as", AS_KEYWORD);
        m.insert("async", ASYNC_KEYWORD);
        m.insert("await", AWAIT_KEYWORD);
        m.insert("break", BREAK_KEYWORD);
        m.insert("class", CLASS_KEYWORD);
        m.insert("const", CONST_KEYWORD);
        m.insert("continue", CONTINUE_KEYWORD);
        m.insert("else", ELSE_KEYWORD);
        m.insert("enum", ENUM_KEYWORD);
        m.insert("extern", EXTERN_KEYWORD);
        m.insert("fn", FN_KEYWORD);
        m.insert("for", FOR_KEYWORD);
        m.insert("if", IF_KEYWORD);
        m.insert("imm", IMM_KEYWORD);
        m.insert("impl", IMPL_KEYWORD);
        m.insert("import", IMPORT_KEYWORD);
        m.insert("in", IN_KEYWORD);
        m.insert("let", LET_KEYWORD);
        m.insert("loop", LOOP_KEYWORD);
        m.insert("match", MATCH_KEYWORD);
        m.insert("mut", MUT_KEYWORD);
        m.insert("package", PACKAGE_KEYWORD);
        m.insert("pub", PUB_KEYWORD);
        m.insert("return", RETURN_KEYWORD);
        m.insert("struct", STRUCT_KEYWORD);
        m.insert("super", SUPER_KEYWORD);
        m.insert("type", TYPE_KEYWORD);
        m.insert("where", WHERE_KEYWORD);
        m.insert("while", WHILE_KEYWORD);
        m.insert("yield", YIELD_KEYWORD);
        m
    };
}

impl <'d, 'a> Lexer<'d, Chars<'a>> {

    pub fn new(iter: Chars<'a>, diags: &'d mut Diagnostics) -> Self {
        Self {
            iter,
            pos: Pos::default(),
            buffer: VecDeque::new(),
            diags,
        }
    }

}

impl <'d, I: Iterator<Item = char>> Lexer<'d, I> {

    fn read(&mut self) -> char {
        self.iter.next().unwrap_or(CHAR_EOF)
    }

    fn get(&mut self) -> char {
        let ch = match self.buffer.pop_front() {
            Some(ch) => ch,
            None => self.read(),
        };
        if ch != CHAR_EOF {
            self.pos.offset += ch.len_utf8();
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

    fn error(&mut self, msg: impl Into<String>, span: Span) {
        self.diags.push(UnexpectedCharDiagnostic { message: msg.into(), span }.into());
    }

    pub fn pos(&self) -> Pos {
        self.pos.clone()
    }

    /// Lex an escape sequence after its `\\`, reporting an invalid one.
    /// Like the grammar, any character can be escaped, but `\\x` must be
    /// followed by two hex digits and `\\u` by `{` hex digits `}` or four
    /// hex digits.
    fn scan_escape(&mut self) {
        let start = self.pos.offset - 1;
        match self.get() {
            'x' => {
                if is_hex_num(self.peek(0)) && is_hex_num(self.peek(1)) {
                    self.get();
                    self.get();
                } else {
                    self.error("expected two hex digits after `\\x`", start..self.pos.offset);
                }
            }
            'u' if self.peek(0) == '{' => {
                self.get();
                self.eat_while(is_hex_num);
                if self.peek(0) == '}' {
                    self.get();
                } else {
                    self.error("expected `}` to end the unicode escape", start..self.pos.offset);
                }
            }
            'u' => {
                for _ in 0..4 {
                    if !is_hex_num(self.peek(0)) {
                        self.error("expected `{` or four hex digits after `\\u`", start..self.pos.offset);
                        return;
                    }
                    self.get();
                }
            }
            CHAR_EOF => {}
            _ => {}
        }
    }

    /// Lex the rest of a string after its opening `"`.
    fn scan_string_body(&mut self) {
        loop {
            match self.get() {
                CHAR_EOF => {
                    self.error("expected '\"' to delimit the string literal but found end-of-file", self.pos.offset-1..self.pos.offset);
                    break;
                }
                '"' => break,
                '\\' => self.scan_escape(),
                _ => {}
            }
        }
    }

    /// Lex the rest of a character literal after its opening `'`.
    fn scan_char_body(&mut self, kind: SyntaxKind) -> SyntaxKind {
        let start = self.pos.offset - 1;
        match self.get() {
            '\\' => self.scan_escape(),
            '\'' | '\n' | CHAR_EOF => {
                self.error("expected a character", start..self.pos.offset);
                return kind;
            }
            _ => {}
        }
        if self.peek(0) == '\'' {
            self.get();
        } else {
            self.error("expected `'` to end the character literal", start..self.pos.offset);
        }
        kind
    }

    /// Whether a raw string starts at `peek(n)`: `"` or `#`s followed by `"`.
    fn at_raw_string(&mut self, n: usize) -> bool {
        let mut i = n;
        while self.peek(i) == '#' {
            i += 1;
        }
        self.peek(i) == '"'
    }

    /// Lex a raw string like `r#"..."#` after its `r`.
    fn scan_raw_string(&mut self) {
        let start = self.pos.offset - 1;
        let mut hashes = 0;
        while self.peek(0) == '#' {
            self.get();
            hashes += 1;
        }
        self.get(); // the opening `"`
        loop {
            match self.get() {
                CHAR_EOF => {
                    self.error("expected the raw string to be closed", start..self.pos.offset);
                    return;
                }
                '"' => {
                    let mut closing = 0;
                    while closing < hashes && self.peek(0) == '#' {
                        self.get();
                        closing += 1;
                    }
                    if closing == hashes {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    /// Whether `![` follows, skipping whitespace after the `!`.
    fn at_inner_attribute(&mut self) -> bool {
        let mut i = 1;
        while matches!(self.peek(i), ' ' | '\t' | '\r' | '\x0b' | '\x0c') {
            i += 1;
        }
        self.peek(i) == '['
    }

    /// Consume characters while `pred` holds for them.
    fn eat_while(&mut self, pred: impl Fn(char) -> bool) {
        while pred(self.peek(0)) {
            self.get();
        }
    }

    /// Lex an integer or float literal whose first digit `c0` was consumed,
    /// following the grammar's `integer_literal` and `float_literal`:
    /// `1_000`, `0xff`, `0b1010`, `0o17`, `1.5`, `1.`, `2.5e-3` and any of
    /// those with a type suffix like `u8` or `f64`.
    fn scan_number(&mut self, c0: char) -> SyntaxKind {
        if c0 == '0' && matches!(self.peek(0), 'b' | 'o' | 'x') {
            let is_digit: fn(char) -> bool = match self.peek(0) {
                'b' => is_bin_num,
                'o' => is_oct_num,
                _ => is_hex_num,
            };
            if is_digit(self.peek(1)) || self.peek(1) == '_' {
                self.get();
                self.eat_while(|ch| is_digit(ch) || ch == '_');
                self.scan_number_suffix(false);
                return INT_NUMBER;
            }
        }
        self.eat_while(|ch| is_dec_num(ch) || ch == '_');
        let mut is_float = false;
        // `1.5` and `1.` are floats, but `1..2` is a range and `1.foo()` a
        // method call.
        if self.peek(0) == '.' && self.peek(1) != '.' && !is_ident_start(self.peek(1)) {
            self.get();
            self.eat_while(|ch| is_dec_num(ch) || ch == '_');
            is_float = true;
        }
        if matches!(self.peek(0), 'e' | 'E')
            && (is_dec_num(self.peek(1)) || (matches!(self.peek(1), '+' | '-') && is_dec_num(self.peek(2))))
        {
            self.get();
            if matches!(self.peek(0), '+' | '-') {
                self.get();
            }
            self.eat_while(|ch| is_dec_num(ch) || ch == '_');
            is_float = true;
        }
        if self.scan_number_suffix(is_float) {
            is_float = true;
        }
        if is_float { FLOAT_NUMBER } else { INT_NUMBER }
    }

    /// Lex a type suffix like `u8` after a number, if there is one. Returns
    /// whether it was a float suffix.
    fn scan_number_suffix(&mut self, is_float: bool) -> bool {
        if !is_ident_start(self.peek(0)) {
            return false;
        }
        let suffix_start = self.pos.offset;
        let mut suffix = String::new();
        while is_ident_part(self.peek(0)) {
            suffix.push(self.get());
        }
        let valid = if is_float {
            matches!(suffix.as_str(), "f32" | "f64")
        } else {
            NUMERIC_SUFFIXES.contains(&suffix.as_str())
        };
        if !valid {
            self.error(format!("invalid suffix `{suffix}` for a number literal"), suffix_start..self.pos.offset);
        }
        matches!(suffix.as_str(), "f32" | "f64")
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
            ':' if self.peek(0) == ':' => {
                self.get();
                COLONCOLON
            }
            ':' => COLON,
            '@' => AT,
            '$' if is_ident_start(self.peek(0)) => {
                while is_ident_part(self.peek(0)) {
                    self.get();
                }
                METAVARIABLE
            }
            // A label like `'outer`. Char literals like `'a'` end with a quote.
            '\'' if is_ident_start(self.peek(0)) && self.peek(1) != '\'' => {
                while is_ident_part(self.peek(0)) {
                    self.get();
                }
                LABEL_IDENT
            }
            ';' => SEMI,
            '"' => {
                self.scan_string_body();
                STRING
            }
            // `'a'` or `'\n'` is a character, but a `'` that doesn't start one
            // is a token of its own, like in the grammar's token trees.
            '\'' if self.peek(0) == '\\' || (self.peek(0) != '\n' && self.peek(1) == '\'') => {
                self.scan_char_body(CHAR)
            }
            '\'' => QUOTE,
            // `#!` at the start of a file is a shebang, unless it starts an
            // inner attribute like `#![allow(unused)]`.
            '#' if self.pos.offset == 1 && self.peek(0) == '!' && !self.at_inner_attribute() => {
                self.eat_while(|ch| ch != '\n' && ch != CHAR_EOF);
                SHEBANG
            }
            '#' => HASHTAG,
            '/' if self.peek(0) == '*' => {
                // Block comments nest, like in Rust: `/* a /* b */ c */`.
                let start = self.pos.offset - 1;
                self.get();
                let mut depth = 1;
                loop {
                    match self.get() {
                        CHAR_EOF => {
                            self.error("unterminated block comment", start..start + 2);
                            break;
                        }
                        '*' if self.peek(0) == '/' => {
                            self.get();
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        '/' if self.peek(0) == '*' => {
                            self.get();
                            depth += 1;
                        }
                        _ => {}
                    }
                }
                BLOCK_COMMENT
            }
            '/' if self.peek(0) == '/' => {
                // The newline isn't part of the comment.
                while !matches!(self.peek(0), '\n' | CHAR_EOF) {
                    self.get();
                }
                LINE_COMMENT
            }
            // Prefixed literals: b"..", b'.', c"..", r".." and r#".."#, br"..",
            // cr"..", and raw identifiers like r#match.
            'b' if self.peek(0) == '"' => {
                self.get();
                self.scan_string_body();
                BYTE_STRING
            }
            'b' if self.peek(0) == '\'' => {
                self.get();
                self.scan_char_body(BYTE)
            }
            'c' if self.peek(0) == '"' => {
                self.get();
                self.scan_string_body();
                C_STRING
            }
            'r' if self.at_raw_string(0) => {
                self.scan_raw_string();
                STRING
            }
            'b' | 'c' if self.peek(0) == 'r' && self.at_raw_string(1) => {
                self.get();
                self.scan_raw_string();
                if c0 == 'b' { BYTE_STRING } else { C_STRING }
            }
            'r' if self.peek(0) == '#' && is_ident_start(self.peek(1)) => {
                self.get();
                self.eat_while(is_ident_part);
                IDENT
            }
            c0 if is_ident_start(c0) => {
                let mut name = String::new();
                name.push(c0);
                while is_ident_part(self.peek(0)) {
                    name.push(self.get());
                }
                if name == "_" {
                    return UNDERSCORE;
                }
                KEYWORDS.get(name.as_str()).copied().unwrap_or(IDENT)
            }
            c0 if is_dec_num(c0) => self.scan_number(c0),
            '?' => QUEST,
            c0 if is_operator(c0) => {
                let mut text = String::new();
                text.push(c0);
                // Stop before `//` and `/*`, so `a +// note` is `+` followed by a comment.
                while is_operator(self.peek(0)) && !(self.peek(0) == '/' && matches!(self.peek(1), '/' | '*')) {
                    text.push(self.get());
                }
                operator_kind(&text)
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

    /// Split the `index`th non-trivia token in two, the first part being
    /// `len` bytes long. See [`Parser::split_bump`](crate::Parser::split_bump).
    pub(crate) fn split(&mut self, index: usize, len: u32, rest_kind: SyntaxKind) {
        let raw = self
            .kinds
            .iter()
            .enumerate()
            .filter(|(_, kind)| !kind.is_trivia())
            .nth(index)
            .map(|(raw, _)| raw)
            .expect("split index out of range");
        let text = self.values[raw].take().unwrap();
        let (first, rest) = text.split_at(len as usize);
        self.kinds.insert(raw + 1, rest_kind);
        self.lens[raw] = len;
        self.lens.insert(raw + 1, rest.len() as u32);
        self.values[raw] = Some(first.to_string());
        self.values.insert(raw + 1, Some(rest.to_string()));
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

pub fn tokenize(text: impl Into<String>, diags: &mut Diagnostics) -> LexResult {

    // Input
    let text = text.into();

    // Output
    let mut kinds = Vec::new();
    let mut lens = Vec::new();
    let mut values = Vec::new();

    // State
    let mut lexer = Lexer::new(text.chars(), diags);

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
        values.push(Some(text[start.offset..end.offset].to_string()));
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

#[cfg(test)]
mod tests {
    use boltlang_syntax::SyntaxKind::{self, *};

    use crate::diagnostic::Diagnostics;

    /// The (kind, text) of every non-whitespace token in `text`.
    fn lex(text: &str) -> Vec<(SyntaxKind, &str)> {
        let mut diags = Diagnostics::new();
        let lexed = super::tokenize(text, &mut diags);
        assert!(diags.take_diagnostics().is_empty(), "{text:?} has lexer errors");
        let mut offset = 0;
        let mut tokens = Vec::new();
        for i in 0..lexed.len() {
            let len = lexed.token_len(i) as usize;
            let kind = lexed.token_kind(i);
            if kind != WHITESPACE {
                tokens.push((kind, &text[offset..offset + len]));
            }
            offset += len;
        }
        tokens
    }

    /// The diagnostics the lexer reports for `text`.
    fn lex_errors(text: &str) -> Vec<String> {
        let mut diags = Diagnostics::new();
        super::tokenize(text, &mut diags);
        diags.take_diagnostics().iter().map(|d| d.to_string()).collect()
    }

    #[test]
    fn integers() {
        for text in ["0", "42", "1_000", "0xff", "0xFF_FF", "0b1010", "0o17", "10u8", "0xffi64", "1usize"] {
            assert_eq!(lex(text), [(INT_NUMBER, text)], "{text}");
        }
    }

    #[test]
    fn floats() {
        for text in ["1.5", "1.", "1e10", "2.5E-3", "1_000.000_1", "1f32", "2.0f64", "1e+3"] {
            assert_eq!(lex(text), [(FLOAT_NUMBER, text)], "{text}");
        }
    }

    #[test]
    fn numbers_next_to_dots() {
        assert_eq!(lex("1..2"), [(INT_NUMBER, "1"), (DOTDOT, ".."), (INT_NUMBER, "2")]);
        assert_eq!(lex("1.max"), [(INT_NUMBER, "1"), (DOT, "."), (IDENT, "max")]);
        assert_eq!(lex("x.0"), [(IDENT, "x"), (DOT, "."), (INT_NUMBER, "0")]);
    }

    #[test]
    fn invalid_number_suffix() {
        assert_eq!(lex_errors("1abc").len(), 1);
        assert_eq!(lex_errors("1.5u8").len(), 1);
    }

    #[test]
    fn strings() {
        for (text, kind) in [
            (r#""""#, STRING),
            (r#""a \" b""#, STRING),
            (r#""\n\t\\\0\x7f\u{1F600}\u00e9""#, STRING),
            (r#"b"bytes""#, BYTE_STRING),
            (r#"c"c string""#, C_STRING),
            (r#"r"raw \ string""#, STRING),
            (r###"r##"a "# b"##"###, STRING),
            (r#"br"raw bytes""#, BYTE_STRING),
            (r##"cr#"raw c"#"##, C_STRING),
        ] {
            assert_eq!(lex(text), [(kind, text)], "{text}");
        }
    }

    #[test]
    fn chars() {
        for (text, kind) in [("'a'", CHAR), ("'\\''", CHAR), ("'\\u{e9}'", CHAR), ("'é'", CHAR), ("b'a'", BYTE)] {
            assert_eq!(lex(text), [(kind, text)], "{text}");
        }
    }

    #[test]
    fn lone_quote() {
        assert_eq!(lex("';"), [(QUOTE, "'"), (SEMI, ";")]);
        assert_eq!(lex("' "), [(QUOTE, "'")]);
    }

    #[test]
    fn raw_identifiers() {
        assert_eq!(lex("r#match r#abc"), [(IDENT, "r#match"), (IDENT, "r#abc")]);
        assert_eq!(lex("r b c"), [(IDENT, "r"), (IDENT, "b"), (IDENT, "c")]);
    }

    #[test]
    fn invalid_literals() {
        assert_eq!(lex_errors(r#""\x7""#).len(), 1);
        assert_eq!(lex_errors(r#""\u{12""#).len(), 1);
        assert!(!lex_errors("b'ab'").is_empty());
        assert_eq!(lex_errors(r##"r#"no end""##).len(), 1);
    }

    #[test]
    fn shebang() {
        assert_eq!(lex("#!/usr/bin/env bolt\nx"), [(SHEBANG, "#!/usr/bin/env bolt"), (IDENT, "x")]);
        assert_eq!(lex("#![attr]"), [(HASHTAG, "#"), (EXCL, "!"), (L_BRACKET, "["), (IDENT, "attr"), (R_BRACKET, "]")]);
        assert_eq!(lex("x #!y")[1], (HASHTAG, "#"));
    }

    #[test]
    fn unicode_whitespace() {
        assert_eq!(lex("a\u{a0}b\u{2028}c"), [(IDENT, "a"), (IDENT, "b"), (IDENT, "c")]);
    }

    #[test]
    fn punctuation() {
        assert_eq!(lex("a::b"), [(IDENT, "a"), (COLONCOLON, "::"), (IDENT, "b")]);
        assert_eq!(lex("a: b"), [(IDENT, "a"), (COLON, ":"), (IDENT, "b")]);
        assert_eq!(lex("x @ y"), [(IDENT, "x"), (AT, "@"), (IDENT, "y")]);
        assert_eq!(lex("$name $"), [(METAVARIABLE, "$name"), (DOLLAR, "$")]);
        assert_eq!(lex("'outer: loop"), [(LABEL_IDENT, "'outer"), (COLON, ":"), (LOOP_KEYWORD, "loop")]);
    }

    #[test]
    fn operators_with_a_role_of_their_own() {
        let ops = [
            ("->", R_ARROW), ("=>", FAT_ARROW), ("|", VBAR), (".", DOT), ("..", DOTDOT),
            ("...", DOTDOTDOT), ("..=", DOTDOTEQUALS), ("=", EQUALS), ("!", EXCL), ("?", QUEST),
            ("<", LT), (">", GT), ("*", STAR), ("+", PLUS), ("-", MINUS), ("$", DOLLAR),
        ];
        for (text, kind) in ops {
            assert_eq!(lex(text), [(kind, text)], "{text}");
        }
    }

    #[test]
    fn other_operators_are_joined() {
        assert_eq!(lex("a <$> b"), [(IDENT, "a"), (OPERATOR, "<$>"), (IDENT, "b")]);
        assert_eq!(lex(">>"), [(OPERATOR, ">>")]);
        assert_eq!(lex("||"), [(OPERATOR, "||")]);
        assert_eq!(lex("<="), [(OPERATOR, "<=")]);
        assert_eq!(lex("+="), [(ASSIGNMENT, "+=")]);
        assert_eq!(lex("x?.y??"), [(IDENT, "x"), (QUEST, "?"), (DOT, "."), (IDENT, "y"), (QUEST, "?"), (QUEST, "?")]);
    }
}
