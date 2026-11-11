use std::{collections::VecDeque, str::Chars};

use crate::{
    diagnostic::{Diagnostics, UnexpectedCharDiagnostic},
    parser::{Input, Split},
};
use boltlang_common::Span;
use boltlang_syntax::SyntaxKind::{self, *};

use unicode_ident::{is_xid_continue, is_xid_start};

pub struct Lexer<'a, 'd> {
    text: &'a str,
    chars: Chars<'a>,
    /// Characters that were peeked at but not consumed yet.
    /// `None` is the end of the input.
    buffer: VecDeque<Option<char>>,
    /// The byte offset in `text` of the next character.
    offset: usize,
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
    matches!(ch, '0'..='7')
}

fn is_dec_num(ch: char) -> bool {
    ch.is_ascii_digit()
}

fn is_hex_num(ch: char) -> bool {
    matches!(ch, '0'..='9' | 'a'..='f' | 'A'..='F')
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
    "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "u128", "i128", "isize", "usize", "f32",
    "f64",
];

/// Characters that are joined into operators like `<$>`. `?` is not one of
/// them: it's always a token of its own, so that `x?.y` and `x??` lex as
/// postfix tries.
fn is_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '&' | '^' | '|' | '<' | '>' | '=' | '$' | '!' | '.')
}

/// The keyword `name` is, if it's one. `macro_rules` is a contextual
/// keyword, so it's left out here; the parser remaps it where needed.
fn keyword_kind(name: &str) -> Option<SyntaxKind> {
    let kind = match name {
        "as" => AS_KEYWORD,
        "break" => BREAK_KEYWORD,
        "class" => CLASS_KEYWORD,
        "const" => CONST_KEYWORD,
        "continue" => CONTINUE_KEYWORD,
        "does" => DOES_KEYWORD,
        "effect" => EFFECT_KEYWORD,
        "else" => ELSE_KEYWORD,
        "enum" => ENUM_KEYWORD,
        "extern" => EXTERN_KEYWORD,
        "fn" => FN_KEYWORD,
        "for" => FOR_KEYWORD,
        "if" => IF_KEYWORD,
        "imm" => IMM_KEYWORD,
        "impl" => IMPL_KEYWORD,
        "import" => IMPORT_KEYWORD,
        "in" => IN_KEYWORD,
        "let" => LET_KEYWORD,
        "loop" => LOOP_KEYWORD,
        "match" => MATCH_KEYWORD,
        "mut" => MUT_KEYWORD,
        "package" => PACKAGE_KEYWORD,
        "pub" => PUB_KEYWORD,
        "return" => RETURN_KEYWORD,
        "struct" => STRUCT_KEYWORD,
        "super" => SUPER_KEYWORD,
        "type" => TYPE_KEYWORD,
        "where" => WHERE_KEYWORD,
        "while" => WHILE_KEYWORD,
        _ => return None,
    };
    Some(kind)
}

impl<'a, 'd> Lexer<'a, 'd> {
    pub fn new(text: &'a str, diags: &'d mut Diagnostics) -> Self {
        Self { text, chars: text.chars(), buffer: VecDeque::new(), offset: 0, diags }
    }

    /// The next character, or `None` at the end of the input.
    fn get(&mut self) -> Option<char> {
        let ch = match self.buffer.pop_front() {
            Some(ch) => ch,
            None => self.chars.next(),
        };
        if let Some(ch) = ch {
            self.offset += ch.len_utf8();
        }
        ch
    }

    /// The character `n` places ahead, or `None` past the end of the input.
    fn peek(&mut self, n: usize) -> Option<char> {
        while self.buffer.len() <= n {
            let ch = self.chars.next();
            self.buffer.push_back(ch);
        }
        self.buffer[n]
    }

    fn error(&mut self, msg: impl Into<String>, span: Span) {
        self.diags.push(UnexpectedCharDiagnostic { message: msg.into(), span }.into());
    }

    /// The text consumed since the byte offset `start`.
    fn text_since(&self, start: usize) -> &'a str {
        &self.text[start..self.offset]
    }

    /// Lex an escape sequence after its `\\`, reporting an invalid one.
    /// Like the grammar, any character can be escaped, but `\\x` must be
    /// followed by two hex digits and `\\u` by `{` hex digits `}` or four
    /// hex digits.
    fn scan_escape(&mut self) {
        let start = self.offset - 1;
        match self.get() {
            Some('x') => {
                if self.peek(0).is_some_and(is_hex_num) && self.peek(1).is_some_and(is_hex_num) {
                    self.get();
                    self.get();
                } else {
                    self.error("expected two hex digits after `\\x`", start..self.offset);
                }
            }
            Some('u') if self.peek(0) == Some('{') => {
                self.get();
                let digits_start = self.offset;
                self.eat_while(is_hex_num);
                let digits = self.text_since(digits_start);
                if self.peek(0) != Some('}') {
                    self.error("expected `}` to end the unicode escape", start..self.offset);
                    return;
                }
                self.get();
                if digits.is_empty() || digits.len() > 6 {
                    self.error(
                        "expected one to six hex digits in a unicode escape",
                        start..self.offset,
                    );
                    return;
                }
                self.check_code_point(digits, start);
            }
            Some('u') => {
                let digits_start = self.offset;
                for _ in 0..4 {
                    if !self.peek(0).is_some_and(is_hex_num) {
                        self.error(
                            "expected `{` or four hex digits after `\\u`",
                            start..self.offset,
                        );
                        return;
                    }
                    self.get();
                }
                self.check_code_point(self.text_since(digits_start), start);
            }
            _ => {}
        }
    }

    /// Report the hex digits of a unicode escape that started at `start` if
    /// they aren't a character: above `10FFFF`, or a surrogate.
    fn check_code_point(&mut self, digits: &str, start: usize) {
        let value = u32::from_str_radix(digits, 16).expect("hex digits");
        if char::from_u32(value).is_none() {
            self.error(format!("`{digits}` is not a unicode character"), start..self.offset);
        }
    }

    /// Lex the rest of a string after its opening `"`.
    fn scan_string_body(&mut self) {
        loop {
            match self.get() {
                None => {
                    self.error(
                        "expected '\"' to delimit the string literal but found end-of-file",
                        self.offset - 1..self.offset,
                    );
                    break;
                }
                Some('"') => break,
                Some('\\') => self.scan_escape(),
                _ => {}
            }
        }
    }

    /// Lex the rest of a character literal after its opening `'`.
    fn scan_char_body(&mut self) {
        let start = self.offset - 1;
        match self.get() {
            Some('\\') => self.scan_escape(),
            Some('\'' | '\n') | None => {
                self.error("expected a character", start..self.offset);
                return;
            }
            _ => {}
        }
        if self.peek(0) == Some('\'') {
            self.get();
        } else {
            self.error("expected `'` to end the character literal", start..self.offset);
        }
    }

    /// Whether a raw string starts at `peek(n)`: `"` or `#`s followed by `"`.
    fn at_raw_string(&mut self, n: usize) -> bool {
        let mut i = n;
        while self.peek(i) == Some('#') {
            i += 1;
        }
        self.peek(i) == Some('"')
    }

    /// Lex a raw string like `r#"..."#` after its `r`.
    fn scan_raw_string(&mut self) {
        let start = self.offset - 1;
        let mut hashes = 0;
        while self.peek(0) == Some('#') {
            self.get();
            hashes += 1;
        }
        self.get(); // the opening `"`
        loop {
            match self.get() {
                None => {
                    self.error("expected the raw string to be closed", start..self.offset);
                    return;
                }
                Some('"') => {
                    let mut closing = 0;
                    while closing < hashes && self.peek(0) == Some('#') {
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
        while matches!(self.peek(i), Some(' ' | '\t' | '\r' | '\x0b' | '\x0c')) {
            i += 1;
        }
        self.peek(i) == Some('[')
    }

    /// Consume characters while `pred` holds for them.
    fn eat_while(&mut self, pred: impl Fn(char) -> bool) {
        while self.peek(0).is_some_and(&pred) {
            self.get();
        }
    }

    /// Lex an integer or float literal whose first digit `c0` was consumed,
    /// following the grammar's `integer_literal` and `float_literal`:
    /// `1_000`, `0xff`, `0b1010`, `0o17`, `1.5`, `1.`, `2.5e-3` and any of
    /// those with a type suffix like `u8` or `f64`.
    fn scan_number(&mut self, c0: char) -> SyntaxKind {
        if c0 == '0' && matches!(self.peek(0), Some('b' | 'o' | 'x')) {
            let is_digit: fn(char) -> bool = match self.peek(0) {
                Some('b') => is_bin_num,
                Some('o') => is_oct_num,
                _ => is_hex_num,
            };
            if self.peek(1).is_some_and(|ch| is_digit(ch) || ch == '_') {
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
        if self.peek(0) == Some('.')
            && self.peek(1) != Some('.')
            && !self.peek(1).is_some_and(is_ident_start)
        {
            self.get();
            self.eat_while(|ch| is_dec_num(ch) || ch == '_');
            is_float = true;
        }
        if matches!(self.peek(0), Some('e' | 'E'))
            && (self.peek(1).is_some_and(is_dec_num)
                || (matches!(self.peek(1), Some('+' | '-'))
                    && self.peek(2).is_some_and(is_dec_num)))
        {
            self.get();
            if matches!(self.peek(0), Some('+' | '-')) {
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
        if !self.peek(0).is_some_and(is_ident_start) {
            return false;
        }
        let suffix_start = self.offset;
        self.eat_while(is_ident_part);
        let suffix = self.text_since(suffix_start);
        let is_float_suffix = matches!(suffix, "f32" | "f64");
        let valid = if is_float { is_float_suffix } else { NUMERIC_SUFFIXES.contains(&suffix) };
        if !valid {
            self.error(
                format!("invalid suffix `{suffix}` for a number literal"),
                suffix_start..self.offset,
            );
        }
        is_float_suffix
    }

    pub fn scan(&mut self) -> SyntaxKind {
        let start = self.offset;
        let Some(c0) = self.get() else {
            return EOF;
        };
        match c0 {
            ch if is_whitespace(ch) => {
                self.eat_while(is_whitespace);
                WHITESPACE
            }
            '(' => L_PAREN,
            ')' => R_PAREN,
            '{' => L_BRACE,
            '}' => R_BRACE,
            '[' => L_BRACKET,
            ']' => R_BRACKET,
            ',' => COMMA,
            ':' if self.peek(0) == Some(':') => {
                self.get();
                COLONCOLON
            }
            ':' => COLON,
            '@' => AT,
            '$' if self.peek(0).is_some_and(is_ident_start) => {
                self.eat_while(is_ident_part);
                METAVARIABLE
            }
            // A label like `'outer`. Char literals like `'a'` end with a quote.
            '\'' if self.peek(0).is_some_and(is_ident_start) && self.peek(1) != Some('\'') => {
                self.eat_while(is_ident_part);
                LABEL_IDENT
            }
            ';' => SEMI,
            '"' => {
                self.scan_string_body();
                STRING
            }
            // `'a'` or `'\n'` is a character, but a `'` that doesn't start one
            // is a token of its own, like in the grammar's token trees.
            '\'' if self.peek(0) == Some('\\')
                || (self.peek(0) != Some('\n') && self.peek(1) == Some('\'')) =>
            {
                self.scan_char_body();
                CHAR
            }
            '\'' => QUOTE,
            // `#!` at the start of a file is a shebang, unless it starts an
            // inner attribute like `#![allow(unused)]`.
            '#' if self.offset == 1 && self.peek(0) == Some('!') && !self.at_inner_attribute() => {
                self.eat_while(|ch| ch != '\n');
                SHEBANG
            }
            '#' => HASHTAG,
            '/' if self.peek(0) == Some('*') => {
                // Block comments nest, like in Rust: `/* a /* b */ c */`.
                self.get();
                let mut depth = 1;
                loop {
                    match self.get() {
                        None => {
                            self.error("unterminated block comment", start..start + 2);
                            break;
                        }
                        Some('*') if self.peek(0) == Some('/') => {
                            self.get();
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Some('/') if self.peek(0) == Some('*') => {
                            self.get();
                            depth += 1;
                        }
                        _ => {}
                    }
                }
                BLOCK_COMMENT
            }
            '/' if self.peek(0) == Some('/') => {
                // The newline isn't part of the comment.
                self.eat_while(|ch| ch != '\n');
                LINE_COMMENT
            }
            // Prefixed literals: b"..", b'.', c"..", r".." and r#".."#, br"..",
            // cr"..", and raw identifiers like r#match.
            'b' if self.peek(0) == Some('"') => {
                self.get();
                self.scan_string_body();
                BYTE_STRING
            }
            'b' if self.peek(0) == Some('\'') => {
                self.get();
                self.scan_char_body();
                BYTE
            }
            'c' if self.peek(0) == Some('"') => {
                self.get();
                self.scan_string_body();
                C_STRING
            }
            'r' if self.at_raw_string(0) => {
                self.scan_raw_string();
                STRING
            }
            'b' | 'c' if self.peek(0) == Some('r') && self.at_raw_string(1) => {
                self.get();
                self.scan_raw_string();
                if c0 == 'b' { BYTE_STRING } else { C_STRING }
            }
            'r' if self.peek(0) == Some('#') && self.peek(1).is_some_and(is_ident_start) => {
                self.get();
                self.eat_while(is_ident_part);
                IDENT
            }
            c0 if is_ident_start(c0) => {
                self.eat_while(is_ident_part);
                let name = self.text_since(start);
                if name == "_" {
                    return UNDERSCORE;
                }
                keyword_kind(name).unwrap_or(IDENT)
            }
            c0 if is_dec_num(c0) => self.scan_number(c0),
            '?' => QUEST,
            c0 if is_operator(c0) => {
                // Stop before `//` and `/*`, so `a +// note` is `+` followed by a comment.
                while self.peek(0).is_some_and(is_operator)
                    && !(self.peek(0) == Some('/') && matches!(self.peek(1), Some('/' | '*')))
                {
                    self.get();
                }
                operator_kind(self.text_since(start))
            }
            ch => {
                self.error(
                    format!("unexpected character `{}`", ch.escape_debug()),
                    start..self.offset,
                );
                ERROR
            }
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

    /// Split the tokens that the parser split, in one pass. The index of a
    /// split counts non-trivia tokens, including the parts of earlier
    /// splits, and the splits are in order. See
    /// [`Parser::split_bump`](crate::Parser::split_bump).
    pub(crate) fn apply_splits(&mut self, splits: &[Split]) {
        if splits.is_empty() {
            return;
        }
        let mut kinds = Vec::with_capacity(self.kinds.len() + splits.len());
        let mut lens = Vec::with_capacity(self.lens.len() + splits.len());
        let mut splits = splits.iter().peekable();
        let mut index = 0;
        for (kind, len) in self.kinds.iter().copied().zip(self.lens.iter().copied()) {
            if kind.is_trivia() {
                kinds.push(kind);
                lens.push(len);
                continue;
            }
            let (mut kind, mut len) = (kind, len);
            while let Some(split) = splits.next_if(|split| split.index == index) {
                kinds.push(kind);
                lens.push(split.len);
                kind = split.rest_kind;
                len -= split.len;
                index += 1;
            }
            kinds.push(kind);
            lens.push(len);
            index += 1;
        }
        assert!(splits.next().is_none(), "split index out of range");
        self.kinds = kinds;
        self.lens = lens;
    }

    /// The tokens without trivia, for the parser.
    pub fn to_input<'t>(&self, text: &'t str) -> Input<'t> {
        let mut kinds = Vec::new();
        let mut ranges = Vec::new();
        let mut offset = 0;
        for (kind, len) in self.kinds.iter().copied().zip(self.lens.iter().copied()) {
            if !kind.is_trivia() {
                kinds.push(kind);
                ranges.push((offset, offset + len));
            }
            offset += len;
        }
        Input::new(text, kinds, ranges)
    }
}

pub fn tokenize(text: &str, diags: &mut Diagnostics) -> LexResult {
    let mut kinds = Vec::new();
    let mut lens = Vec::new();
    let mut lexer = Lexer::new(text, diags);
    loop {
        let start = lexer.offset;
        let kind = lexer.scan();
        if kind == EOF {
            break;
        }
        kinds.push(kind);
        lens.push((lexer.offset - start) as u32);
    }
    LexResult { kinds, lens }
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
        for text in
            ["0", "42", "1_000", "0xff", "0xFF_FF", "0b1010", "0o17", "10u8", "0xffi64", "1usize"]
        {
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
        for (text, kind) in
            [("'a'", CHAR), ("'\\''", CHAR), ("'\\u{e9}'", CHAR), ("'é'", CHAR), ("b'a'", BYTE)]
        {
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
        assert_eq!(lex_errors(r#""\u{}""#), ["expected one to six hex digits in a unicode escape"]);
        assert_eq!(
            lex_errors(r#""\u{0000041}""#),
            ["expected one to six hex digits in a unicode escape"]
        );
        assert_eq!(lex_errors(r#""\u{FFFFFF}""#), ["`FFFFFF` is not a unicode character"]);
        assert_eq!(lex_errors(r#""\u{D800}""#), ["`D800` is not a unicode character"]);
        assert_eq!(lex_errors(r#""\uDFFF""#), ["`DFFF` is not a unicode character"]);
        assert_eq!(lex_errors(r#"'\u{110000}'"#), ["`110000` is not a unicode character"]);
        assert!(lex_errors(r#""\u{10FFFF}\u{41}\u0041""#).is_empty());
        assert!(!lex_errors("b'ab'").is_empty());
        assert_eq!(lex_errors(r##"r#"no end""##).len(), 1);
    }

    #[test]
    fn unexpected_characters() {
        assert_eq!(lex_errors("a ~ b"), ["unexpected character `~`"]);
        assert_eq!(lex_errors("\u{200b}"), ["unexpected character `\\u{200b}`"]);
    }

    /// U+FFFF is an ordinary character, not the end of the input (#428).
    #[test]
    fn u_ffff_is_not_the_end() {
        let text = "a \u{FFFF} b";
        let mut diags = Diagnostics::new();
        let lexed = super::tokenize(text, &mut diags);
        let errors: Vec<_> = diags.take_diagnostics().iter().map(|d| d.to_string()).collect();
        assert_eq!(errors, ["unexpected character `\\u{ffff}`"]);
        let kinds: Vec<_> = (0..lexed.len()).map(|i| lexed.token_kind(i)).collect();
        assert_eq!(kinds, [IDENT, WHITESPACE, ERROR, WHITESPACE, IDENT]);
        let len: u32 = (0..lexed.len()).map(|i| lexed.token_len(i)).sum();
        assert_eq!(len as usize, text.len());

        assert_eq!(lex("\"\u{FFFF}\""), [(STRING, "\"\u{FFFF}\"")]);
        assert_eq!(lex("// x \u{FFFF} y"), [(LINE_COMMENT, "// x \u{FFFF} y")]);
        assert_eq!(lex("/* \u{FFFF} */"), [(BLOCK_COMMENT, "/* \u{FFFF} */")]);
    }

    #[test]
    fn shebang() {
        assert_eq!(lex("#!/usr/bin/env bolt\nx"), [(SHEBANG, "#!/usr/bin/env bolt"), (IDENT, "x")]);
        assert_eq!(
            lex("#![attr]"),
            [(HASHTAG, "#"), (EXCL, "!"), (L_BRACKET, "["), (IDENT, "attr"), (R_BRACKET, "]")]
        );
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
        assert_eq!(
            lex("'outer: loop"),
            [(LABEL_IDENT, "'outer"), (COLON, ":"), (LOOP_KEYWORD, "loop")]
        );
    }

    #[test]
    fn operators_with_a_role_of_their_own() {
        let ops = [
            ("->", R_ARROW),
            ("=>", FAT_ARROW),
            ("|", VBAR),
            (".", DOT),
            ("..", DOTDOT),
            ("...", DOTDOTDOT),
            ("..=", DOTDOTEQUALS),
            ("=", EQUALS),
            ("!", EXCL),
            ("?", QUEST),
            ("<", LT),
            (">", GT),
            ("*", STAR),
            ("+", PLUS),
            ("-", MINUS),
            ("$", DOLLAR),
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
        assert_eq!(
            lex("x?.y??"),
            [(IDENT, "x"), (QUEST, "?"), (DOT, "."), (IDENT, "y"), (QUEST, "?"), (QUEST, "?")]
        );
    }
}
