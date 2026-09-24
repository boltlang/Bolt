#![feature(string_from_utf8_lossy_owned)]

#[cfg(test)]
mod test {

    use std::{io::{Cursor, Write}};
    use serde::ser::{SerializeSeq, SerializeTuple};

    use boltlang_syntax::{NodeOrToken, SyntaxElement, SyntaxNode};
    use boltlang_parser::{Diagnostic, grammar::*, parse};

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

    fn stringify(node: SyntaxElement, diags: Vec<Diagnostic>) -> String {
        let mut cursor = Cursor::new(Vec::new());
        yaml_serde::to_writer(&mut cursor, &N(node)).unwrap();
        for d in diags {
            write!(cursor, "---\n").unwrap();
            write!(cursor, "{}", yaml_serde::to_string(&d).unwrap()).unwrap();
        }
        String::from_utf8_lossy_owned(cursor.into_inner())
    }

    macro_rules! assert_parse_succeed {
        ($lit:literal, $expr:expr) => {
            let (node, diags) = parse($lit, $expr);
            let node = SyntaxNode::new_root(node).into();
            insta::assert_snapshot!(&stringify(node, diags));
        };
    }

    macro_rules! assert_parse_fail {
        ($lit:literal, $expr:expr) => {

        };
    }

    #[test]
    fn test_a_string_is_some_text_wrapped_in_two_double_quotes() {
        assert_parse_succeed!("\"foobar\"", parse_expr);
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
        assert_parse_succeed!("\"\"", parse_expr);
    }
    #[test]
    fn test_a_string_may_contain_spaces() {
        assert_parse_succeed!("\"Hello, world!\"", parse_expr);
    }
    #[test]
    fn test_a_string_may_contain_spaces_1() {
        assert_parse_succeed!("0b1100110", parse_expr);
    }
    #[test]
    fn test_a_string_may_contain_spaces_2() {
        assert_parse_succeed!("0o73651", parse_expr);
    }
    #[test]
    fn test_digits_are_valid_numbers() {
        assert_parse_succeed!("1", parse_expr);
    }
    #[test]
    fn test_digits_are_valid_numbers_1() {
        assert_parse_succeed!("2", parse_expr);
    }
    #[test]
    fn test_digits_are_valid_numbers_2() {
        assert_parse_succeed!("3", parse_expr);
    }
    #[test]
    fn test_digits_are_valid_numbers_3() {
        assert_parse_succeed!("42", parse_expr);
    }
    #[test]
    fn test_digits_are_valid_numbers_4() {
        assert_parse_succeed!("123456", parse_expr);
    }
    #[test]
    fn test_digits_are_valid_numbers_5() {
        assert_parse_succeed!("0xffab23", parse_expr);
    }
}
