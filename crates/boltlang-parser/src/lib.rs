mod token_set;
mod lexer;
mod event;
mod parser;
pub mod grammar;
mod diagnostic;

use boltlang_syntax::SyntaxKind::EOF;
pub use boltlang_syntax::rowan;

use rowan::GreenNode;

pub(crate) use event::{process_events, intersperse_trivia};

pub use diagnostic::Diagnostic;

pub use {
    lexer::LineColumn,
    parser::Parser,
};

use crate::{diagnostic::Diagnostics, lexer::tokenize};

pub fn parse_file(text: &str) -> (GreenNode, Vec<Diagnostic>) {
    parse(text, grammar::parse_source_file)
}

/// Parse `text` with a single grammar rule, e.g. `grammar::parse_expr`.
pub fn parse<R, F: Fn(&mut Parser) -> R>(text: &str, rule: F) -> (GreenNode, Vec<Diagnostic>) {
    let mut diags = Diagnostics::new();
    let mut lexed = tokenize(text, &mut diags);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    rule(&mut p);
    if !p.at(EOF) {
        p.error("parser not at end of file");
    }
    let (events, splits) = p.finish();
    for split in splits {
        lexed.split(split.index, split.len, split.rest_kind);
    }
    let interspersed = intersperse_trivia(events, &lexed);
    let node = process_events(
        interspersed.into_iter(),
        &lexed,
        &text,
        &mut diags
    );
    (node, diags.take_diagnostics())
}

#[cfg(test)]
mod tests {
    use boltlang_syntax::{SyntaxKind::*, SyntaxNode};

    use super::parse;

    /// The (kind, text) of every token in the tree.
    fn tokens(text: &str, rule: impl Fn(&mut crate::Parser)) -> Vec<(boltlang_syntax::SyntaxKind, String)> {
        let (green, diags) = parse(text, rule);
        assert!(diags.is_empty(), "{diags:?}");
        SyntaxNode::new_root(green)
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .map(|t| (t.kind(), t.text().to_string()))
            .collect()
    }

    #[test]
    fn contextual_keywords_and_operators() {
        let tokens = tokens("macro_rules <$> x", |p| {
            let m = p.start();
            assert!(p.at_contextual_kw(MACRO_RULES_KEYWORD));
            p.bump_remap(MACRO_RULES_KEYWORD);
            assert!(p.at_op("<$>"));
            assert!(!p.at_op("<"));
            p.bump_any();
            assert!(!p.at_contextual_kw(MACRO_RULES_KEYWORD));
            p.bump_any();
            m.complete(p, SOURCE_FILE);
        });
        let kinds: Vec<_> = tokens.iter().map(|(kind, _)| *kind).filter(|kind| *kind != WHITESPACE).collect();
        assert_eq!(kinds, [MACRO_RULES_KEYWORD, OPERATOR, IDENT]);
    }

    #[test]
    #[should_panic(expected = "the parser seems stuck")]
    fn a_stuck_parser_panics() {
        parse("x", |p| loop {
            p.at(IDENT);
        });
    }

    #[test]
    fn split_bump_splits_a_token_in_two() {
        let tokens = tokens(" >> ", |p| {
            let m = p.start();
            p.split_bump(GT, 1);
            assert_eq!(p.value().map(String::as_str), Some(">"));
            p.bump_any();
            m.complete(p, SOURCE_FILE);
        });
        assert_eq!(tokens, [
            (WHITESPACE, " ".to_string()),
            (GT, ">".to_string()),
            (GT, ">".to_string()),
            (WHITESPACE, " ".to_string()),
        ]);
    }
}
