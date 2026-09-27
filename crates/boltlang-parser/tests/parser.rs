#![feature(string_from_utf8_lossy_owned)]

#[cfg(test)]
mod test {

    use std::{io::{Cursor, Write}};
    use serde::ser::{SerializeSeq, SerializeTuple};
    use insta::assert_snapshot;

    use boltlang_syntax::{NodeOrToken, SyntaxElement, SyntaxNode};
    use boltlang_parser::{Parser, grammar::*, parse};

    struct N(SyntaxElement);

//     impl serde::Serialize for N {
//         fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//         where
//             S: serde::Serializer
//         {
//             match &self.0 {
//                 NodeOrToken::Node(node) => {
//                     let mut s = serializer.serialize_seq(Some(node.children_with_tokens().count()))?;
//                     s.serialize_element(&format!("{:?}", self.0.kind()))?;
//                     for child in node.children_with_tokens() {
//                         s.serialize_element(&N(child))?;
//                     }
//                     s.end()
//                 }
//                 NodeOrToken::Token(token) => {
//                     let mut tup = serializer.serialize_tuple(2)?;
//                     tup.serialize_element(&format!("{:?}", self.0.kind()))?;
//                     tup.serialize_element(token.text())?;
//                     tup.end()
//                 }
//             }
//         }
//     }

    fn write_element<W: Write>(writer: &mut W, element: &SyntaxElement, indent: usize) -> std::io::Result<()> {
        match element {
            NodeOrToken::Node(node) => {
                writeln!(writer, "{:indent$}- {name:?}", "", indent = indent, name = element.kind())?;
                for child in node.children_with_tokens() {
                    write_element(writer, &child, indent+2)?;
                }
                Ok(())
            }
            NodeOrToken::Token(token)  => {
                writeln!(writer, "{:indent$}- {name:?} {text:?}", "", indent = indent, name = element.kind(), text = token.text())
            }
        }
    }

    fn output<R, F: Fn(&mut Parser) -> R>(lit: &str, proc: F) -> String {
        let (node, diags) = parse(lit, proc);
        let node = SyntaxNode::new_root(node).into();
        let mut out = Cursor::new(Vec::new());
        write_element(&mut out, &node, 0).unwrap();
        // yaml_serde::to_writer(&mut cursor, &N(node)).unwrap();
        for d in diags {
            write!(out, "---\n").unwrap();
            write!(out, "{}", yaml_serde::to_string(&d).unwrap()).unwrap();
        }
        String::from_utf8_lossy_owned(out.into_inner())
    }

    #[test]
    fn test_a_string_is_some_text_wrapped_in_two_double_quotes() {
        assert_snapshot!(&output("\"foobar\"", parse_expr));
    }

    #[test]
    fn test_a_string_is_some_text_wrapped_in_two_double_quotes_missing_right_quote() {
        assert_snapshot!(&output("\"foobar", parse_expr));
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
