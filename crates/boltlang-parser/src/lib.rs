mod token_set;
mod lexer;
mod event;
mod parser;
pub mod grammar;
mod diagnostic;

use boltlang_syntax::SyntaxKind::{EOF, ERROR};
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
    // The tree needs one root. A rule like `parse_expr` that stops before
    // the end, that finds nothing to parse, or that leaves more than one
    // node or an error outside of its node, is wrapped in an `ERROR` node.
    // Otherwise the marker is abandoned and the rule's node is the root.
    let m = p.start();
    rule(&mut p);
    if p.at(EOF) && m.has_one_node_since(&p) {
        m.abandon(&mut p);
    } else {
        if !p.at(EOF) {
            p.error("expected the end of the input");
            while !p.at(EOF) {
                p.bump_any();
            }
        }
        m.complete(&mut p, ERROR);
    }
    let (events, splits) = p.finish();
    for split in splits {
        lexed.split(split.index, split.len, split.rest_kind);
    }
    let interspersed = intersperse_trivia(events, &lexed);
    let node = process_events(
        interspersed.into_iter(),
        &lexed,
        text,
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

    /// The kind of the root, and how many diagnostics there are.
    fn root(text: &str, rule: impl Fn(&mut crate::Parser)) -> (boltlang_syntax::SyntaxKind, usize) {
        let (green, diags) = parse(text, rule);
        (SyntaxNode::new_root(green).kind(), diags.len())
    }

    #[test]
    fn an_error_after_the_rules_node_is_inside_the_root() {
        // `#[a]` is an `ATTR` node, then `parse_item` reports a missing item.
        let (kind, errors) = root("#[a]", crate::grammar::parse_item);
        assert_eq!(kind, ERROR);
        assert!(errors > 0);
    }

    #[test]
    fn two_nodes_from_a_rule_are_wrapped_in_one_root() {
        // `parse_expr` gives an `ATTR` node, and an `ERROR` node after it.
        let (kind, errors) = root("#[a]", |p| {
            crate::grammar::parse_expr(p);
        });
        assert_eq!(kind, ERROR);
        assert!(errors > 0);
    }

    #[test]
    fn one_node_from_a_rule_is_the_root() {
        let (kind, errors) = root("a + b", |p| {
            crate::grammar::parse_expr(p);
        });
        assert_eq!(kind, BIN_EXPR);
        assert_eq!(errors, 0);
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
