use std::{collections::{HashMap, hash_map}, iter::Peekable, path::Path};

use itertools::Itertools;
use markdown::mdast::Node;
use quote::{format_ident, quote};

use crate::{CodegenType, codegen::{ensure_file_contents, reformat}, project_root};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TestMode {
    Fail,
    Succeed,
    Skip,
}

#[derive(Clone, Debug)]
pub struct Test {
    title: String,
    mode: TestMode,
    contents: String,
    runner: String,
}

fn underscore(text: &str) -> String {
    text.chars()
        .filter_map(|mut c| {
            c.make_ascii_lowercase();
            if c.is_alphanumeric() || c == ' ' {
                Some(c)
            } else {
                None
            }
        })
        .collect::<String>()
        .split(" ")
        .intersperse("_")
        .collect()
}

pub fn generate(check: bool) {
    let grammar_file = project_root().join("grammar.md");
    let parser_tests_file = project_root().join("crates").join("boltlang").join("tests").join("parser.rs");
    let tests = collect_tests(&grammar_file);
    let fns: Vec<_> = tests.iter().filter_map(|t| {
        let name = format_ident!("test_{}", underscore(&t.title));
        let runner = format_ident!("{}", t.runner);
        let text = &t.contents;
        let assertion = match t.mode {
            TestMode::Succeed => quote! { assert_parse_succeed! },
            TestMode::Fail => quote! { assert_parse_fail! },
            TestMode::Skip => return None,
        };
        Some(quote::quote! {
            #[test]
            fn #name() {
                #assertion(#text, #runner);
            }
        })
    }).collect();
    let code = quote::quote! {
        #[cfg(test)]
        mod test {

            use boltlang::parser::tests::*;
            use boltlang::parser::grammar::*;

            #(#fns)*
        }
    };
    ensure_file_contents(CodegenType::ParserTests, &parser_tests_file, &reformat(code.to_string()), check);
}

fn collect_tests(path: &Path) -> Vec<Test> {
    let text = std::fs::read_to_string(path).expect("failed to read grammar.md");
    let ast = markdown::to_mdast(&text, &markdown::ParseOptions::default()).expect("failed to parse grammar.md as markdown");
    let mut map = HashMap::new();
    let mut tests = Vec::new();
    let mut mode = TestMode::Succeed;
    let mut runner = None;
    let mut title = None;
    for child in ast.children().unwrap() {
        match child {
            Node::Html(html) if is_html_comment(&html.value) => {
                let contents = html_comment_contents(&html.value);
                match parse_directive(contents) {
                    Err(ParseError::MissingDirective) => {},
                    Err(error) => panic!("error parsing directive: {}", error),
                    Ok(Directive::SetMode(new_mode)) => mode = new_mode,
                    Ok(Directive::SetParser(new_pname)) => runner = Some(new_pname),
                }
            }
            n @ Node::Paragraph(_) => {
                title = Some(to_text(n));
            }
            Node::Code(code) => {
                let new_title = match &title {
                    None => {
                        format!("Test {}", tests.len())
                    }
                    Some(title) => {
                        match map.entry(title.clone()) {
                            hash_map::Entry::Vacant(v) => {
                                v.insert_entry(1);
                                title.clone()
                            },
                            hash_map::Entry::Occupied(mut o) => {
                                let count = *o.get();
                                o.insert(count + 1);
                                format!("{} ({})", title, count)
                            }
                        }
                    }
                };
                tests.push(Test {
                    mode,
                    title: new_title,
                    contents: code.value.clone(),
                    runner: runner.clone().unwrap_or("toplevel".to_string()),
                });
                mode = TestMode::Succeed;
            },
            _ => {},
        }
    }
    tests
}

fn to_text(node: &Node) -> String {
    match node {
        Node::Text(text) =>
            text.value.clone(),
        Node::Paragraph(para) =>
            para.children.iter().map(|c| to_text(c)).collect(),
        Node::Heading(heading) =>
            heading.children.iter().map(|c| to_text(c)).collect(),
        Node::Break(_) =>
            "\n".to_string(),
        Node::Emphasis(emp) =>
            emp.children.iter().map(|c| to_text(c)).collect(),
        Node::Strong(strong) =>
            strong.children.iter().map(|c| to_text(c)).collect(),
        node => panic!("unexpected element {:?}", node),
    }
}

fn is_html_comment(text: &str) -> bool {
    text.starts_with("<!--") && text.ends_with("-->")
}

fn html_comment_contents(text: &str) -> &str {
    &text[4..text.len()-3]
}

#[derive(Debug)]
enum Directive {
    SetParser(String),
    SetMode(TestMode),
}

fn skip_ws<I: Iterator<Item = char>>(iter: &mut Peekable<I>) {
    loop {
        let ch = match iter.peek() {
            None => break,
            Some(ch) => ch,
        };
        if matches!(ch, '\n' | ' ' | '\r' | '\t') {
            iter.next();
            continue;
        }
        break;
    }
}

fn parse_ident<I: Iterator<Item = char>>(iter: &mut Peekable<I>) -> Result<String, ParseError> {
    let c0 = match iter.next() {
        None => return Err(ParseError::Unexpected(None)),
        Some(ch) => ch,
    };
    if !c0.is_alphabetic() {
        return Err(ParseError::Unexpected(Some(c0)));
    }
    let mut name = String::new();
    name.push(c0);
    loop {
        let c1 = match iter.peek() {
            None => break,
            Some(ch) => ch,
        };
        if !c1.is_alphanumeric() {
            break;
        }
        name.push(*c1);
        iter.next();
    };
    Ok(name)
}

fn parse_value<I: Iterator<Item = char>>(iter: &mut Peekable<I>) -> Result<String, ParseError> {
    parse_ident(iter)
}

fn parse_directive(text: &str) -> Result<Directive, ParseError> {
    let mut iter = text.chars().peekable();
    skip_ws(&mut iter);
    let c1 = match iter.next() {
        None => return Err(ParseError::MissingDirective),
        Some(ch) => ch,
    };
    if c1 != '@' {
        return Err(ParseError::MissingDirective);
    }
    let name = parse_ident(&mut iter)?;
    skip_ws(&mut iter);
    Ok(match name.as_str() {
        "parser" => {
            let runner = parse_value(&mut iter)?;
            Directive::SetParser(runner)
        },
        "fail" => Directive::SetMode(TestMode::Fail),
        "succeess" => Directive::SetMode(TestMode::Succeed),
        "skip" => Directive::SetMode(TestMode::Skip),
        name => return Err(ParseError::NameNotFound(name.to_string())),
    })
}

#[derive(thiserror::Error, Debug)]
enum ParseError {
    #[error("directive not present in HTML comment")]
    MissingDirective,
    #[error("the directive '{0}' is not defined")]
    NameNotFound(String),
    #[error("unexpected character encountered while parsing directive")]
    Unexpected(Option<char>),
}
