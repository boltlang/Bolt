#![feature(string_from_utf8_lossy_owned)]

#[cfg(test)]
mod test {

    use std::{io::{Cursor, Write}};
    use serde::ser::{SerializeSeq, SerializeTuple};
    use insta::assert_snapshot;

    use boltlang_syntax::{NodeOrToken, SyntaxElement, SyntaxNode};
    use boltlang_parser::{Parser, grammar::*, parse};

    struct N(SyntaxElement);

    impl serde::Serialize for N {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer
        {
            match &self.0 {
                NodeOrToken::Node(node) => {
                    let mut s = serializer.serialize_seq(Some(node.children_with_tokens().count()))?;
                    s.serialize_element(&format!("{:?}", self.0.kind()))?;
                    for child in node.children_with_tokens() {
                        s.serialize_element(&N(child))?;
                    }
                    s.end()
                }
                NodeOrToken::Token(token) => {
                    let mut tup = serializer.serialize_tuple(2)?;
                    tup.serialize_element(&format!("{:?}", self.0.kind()))?;
                    tup.serialize_element(token.text())?;
                    tup.end()
                }
            }
        }
    }

    fn output<R, F: Fn(&mut Parser) -> R>(lit: &str, proc: F) -> String {
        let (node, diags) = parse(lit, proc);
        let node = SyntaxNode::new_root(node).into();
        let mut cursor = Cursor::new(Vec::new());
        yaml_serde::to_writer(&mut cursor, &N(node)).unwrap();
        for d in diags {
            write!(cursor, "---\n").unwrap();
            write!(cursor, "{}", yaml_serde::to_string(&d).unwrap()).unwrap();
        }
        String::from_utf8_lossy_owned(cursor.into_inner())
    }

    macro_rules! assert_parse_fail {
        ($lit:literal, $expr:expr) => {

        };
    }

    #[test]
    fn test_a_string_is_some_text_wrapped_in_two_double_quotes() {
        assert_snapshot!(&output("\"foobar\"", parse_expr));
    }
    #[test]
    fn test_a_string_is_some_text_wrapped_in_two_double_quotes_1() {
        assert_parse_fail!("\"foobar", parse_expr);
    }
    #[test]
    fn test_a_string_is_some_text_wrapped_in_two_double_quotes_2() {
        assert_parse_fail!("foobar\"", parse_expr);
    }
    #[test]
    fn test_the_empty_string_is_a_valid_string() {
        assert_snapshot!(&output("\"\"", parse_expr));
    }
    #[test]
    fn test_a_string_may_contain_spaces() {
        assert_snapshot!(&output("\"Hello, world!\"", parse_expr));
    }
    #[test]
    fn test_a_string_may_contain_spaces_1() {
        assert_snapshot!(&output("0b1100110", parse_expr));
    }
    #[test]
    fn test_a_string_may_contain_spaces_2() {
        assert_snapshot!(&output("0o73651", parse_expr));
    }
    #[test]
    fn test_digits_are_valid_numbers() {
        assert_snapshot!(&output("1", parse_expr));
    }
    #[test]
    fn test_digits_are_valid_numbers_1() {
        assert_snapshot!(&output("2", parse_expr));
    }
    #[test]
    fn test_digits_are_valid_numbers_2() {
        assert_snapshot!(&output("3", parse_expr));
    }
    #[test]
    fn test_digits_are_valid_numbers_3() {
        assert_snapshot!(&output("42", parse_expr));
    }
    #[test]
    fn test_digits_are_valid_numbers_4() {
        assert_snapshot!(&output("123456", parse_expr));
    }
    #[test]
    fn test_digits_are_valid_numbers_5() {
        assert_snapshot!(&output("0xffab23", parse_expr));
    }
}
