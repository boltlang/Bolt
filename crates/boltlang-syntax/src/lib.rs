
pub mod ast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types)]
#[repr(u16)]
pub enum SyntaxKind {
    // Special tokens
    ERROR,
    TOMBSTONE,
    EOF,

    // Skippable tokens
    WHITESPACE,
    LINE_COMMENT,
    BLOCK_COMMENT,

    /// A `#!/usr/bin/env bolt` line at the very start of a file.
    SHEBANG,

    // Static tokens
    AT,
    COLON,
    COLONCOLON,
    COMMA,
    DOT,
    DOLLAR,
    DOTDOT,
    DOTDOTDOT,
    DOTDOTEQUALS,
    EQUALS,
    EXCL,
    FAT_ARROW,
    GT,
    HASHTAG,
    LT,
    L_BRACE,
    L_BRACKET,
    L_PAREN,
    MINUS,
    PLUS,
    QUEST,
    R_ARROW,
    R_BRACE,
    R_BRACKET,
    R_PAREN,
    SEMI,
    STAR,
    UNDERSCORE,
    VBAR,

    // Tokens which can have a value
    IDENT,
    /// A loop label like `'outer`
    LABEL_IDENT,
    /// A macro metavariable like `$name`
    METAVARIABLE,
    OPERATOR,
    BYTE_STRING,
    BYTE,
    C_STRING,
    CHAR,
    STRING,
    ASSIGNMENT,
    FLOAT_NUMBER,
    INT_NUMBER,


    // Keywords. `default`, `gen` and `macro_rules` are contextual: the lexer
    // produces an IDENT and the parser remaps it where it acts as a keyword.
    AS_KEYWORD,
    ASYNC_KEYWORD,
    AWAIT_KEYWORD,
    BREAK_KEYWORD,
    CONST_KEYWORD,
    CONTINUE_KEYWORD,
    DEFAULT_KEYWORD,
    ELSE_KEYWORD,
    ENUM_KEYWORD,
    EXTERN_KEYWORD,
    FALSE_KEYWORD,
    FN_KEYWORD,
    FOR_KEYWORD,
    GEN_KEYWORD,
    IF_KEYWORD,
    IMPL_KEYWORD,
    IMPORT_KEYWORD,
    IN_KEYWORD,
    LET_KEYWORD,
    LOOP_KEYWORD,
    MACRO_RULES_KEYWORD,
    MATCH_KEYWORD,
    MOD_KEYWORD,
    MUT_KEYWORD,
    PACKAGE_KEYWORD,
    PUB_KEYWORD,
    RETURN_KEYWORD,
    SELF_KEYWORD,
    STATIC_KEYWORD,
    STRUCT_KEYWORD,
    SUPER_KEYWORD,
    TRAIT_KEYWORD,
    TRUE_KEYWORD,
    TYPE_KEYWORD,
    WHERE_KEYWORD,
    WHILE_KEYWORD,
    YIELD_KEYWORD,

    // Patterns
    NAMED_PATT,
    LIT_PATT,
    TUPLE_PATT,
    NEST_PATT,
    TYPED_PATT,

    // Type expressions
    ARROW_TYPE_EXPR,
    NAMED_TYPE_EXPR,

    // Macros
    MACRO_EXPR,
    MACRO_CALL,
    TOKEN_TREE,

    // Expressions
    AWAIT_EXPR,
    BIN_EXPR,
    BLOCK_EXPR,
    CALL_EXPR,
    CAST_EXPR,
    FIELD_EXPR,
    FUN_EXPR,
    INDEX_EXPR,
    LIT_EXPR,
    MATCH_EXPR,
    METHOD_CALL_EXPR,
    NEST_EXPR,
    PATH_EXPR,
    POSTFIX_EXPR,
    PREFIX_EXPR,
    RANGE_EXPR,
    RECORD_EXPR,
    RET_EXPR,
    TRY_EXPR,
    TUPLE_EXPR,

    // Expression helpers
    RECORD_EXPR_FIELD,
    ARG_LIST,

    // Declarations
    FUNC_DECL,
    EXTERN_BLOCK,
    LET_STMT,
    MODULE,
    RECORD_EXPR_FIELD_LIST,
    TYPE_ALIAS,

    // Impl helper syntax
    IMPL_RESTRICTION,

    // Helper syntax
    BLOCK,
    TYPE_SIGNATURE,
    INITIALIZER,
    PARAM,
    VISIBILITY,

    // Top-level
    SOURCE_FILE,
}

/// The [`SyntaxKind`] of a punctuation token or keyword, e.g. `T![;]` for
/// `SEMI` and `T![fn]` for `FN_KEYWORD`. Modelled after rust-analyzer's macro
/// of the same name.
///
/// Operators that are not in this list are lexed as `OPERATOR`; see
/// `boltlang_parser`'s lexer.
#[macro_export]
macro_rules! T {
    [;] => { $crate::SyntaxKind::SEMI };
    [,] => { $crate::SyntaxKind::COMMA };
    [:] => { $crate::SyntaxKind::COLON };
    [::] => { $crate::SyntaxKind::COLONCOLON };
    [.] => { $crate::SyntaxKind::DOT };
    [..] => { $crate::SyntaxKind::DOTDOT };
    [...] => { $crate::SyntaxKind::DOTDOTDOT };
    [..=] => { $crate::SyntaxKind::DOTDOTEQUALS };
    ['('] => { $crate::SyntaxKind::L_PAREN };
    [')'] => { $crate::SyntaxKind::R_PAREN };
    ['{'] => { $crate::SyntaxKind::L_BRACE };
    ['}'] => { $crate::SyntaxKind::R_BRACE };
    ['['] => { $crate::SyntaxKind::L_BRACKET };
    [']'] => { $crate::SyntaxKind::R_BRACKET };
    [=] => { $crate::SyntaxKind::EQUALS };
    [=>] => { $crate::SyntaxKind::FAT_ARROW };
    [->] => { $crate::SyntaxKind::R_ARROW };
    [|] => { $crate::SyntaxKind::VBAR };
    [!] => { $crate::SyntaxKind::EXCL };
    [?] => { $crate::SyntaxKind::QUEST };
    [<] => { $crate::SyntaxKind::LT };
    [>] => { $crate::SyntaxKind::GT };
    [*] => { $crate::SyntaxKind::STAR };
    [+] => { $crate::SyntaxKind::PLUS };
    [-] => { $crate::SyntaxKind::MINUS };
    [$] => { $crate::SyntaxKind::DOLLAR };
    [#] => { $crate::SyntaxKind::HASHTAG };
    [@] => { $crate::SyntaxKind::AT };
    [_] => { $crate::SyntaxKind::UNDERSCORE };
    [as] => { $crate::SyntaxKind::AS_KEYWORD };
    [async] => { $crate::SyntaxKind::ASYNC_KEYWORD };
    [await] => { $crate::SyntaxKind::AWAIT_KEYWORD };
    [break] => { $crate::SyntaxKind::BREAK_KEYWORD };
    [const] => { $crate::SyntaxKind::CONST_KEYWORD };
    [continue] => { $crate::SyntaxKind::CONTINUE_KEYWORD };
    [default] => { $crate::SyntaxKind::DEFAULT_KEYWORD };
    [else] => { $crate::SyntaxKind::ELSE_KEYWORD };
    [enum] => { $crate::SyntaxKind::ENUM_KEYWORD };
    [extern] => { $crate::SyntaxKind::EXTERN_KEYWORD };
    [false] => { $crate::SyntaxKind::FALSE_KEYWORD };
    [fn] => { $crate::SyntaxKind::FN_KEYWORD };
    [for] => { $crate::SyntaxKind::FOR_KEYWORD };
    [gen] => { $crate::SyntaxKind::GEN_KEYWORD };
    [if] => { $crate::SyntaxKind::IF_KEYWORD };
    [impl] => { $crate::SyntaxKind::IMPL_KEYWORD };
    [import] => { $crate::SyntaxKind::IMPORT_KEYWORD };
    [in] => { $crate::SyntaxKind::IN_KEYWORD };
    [let] => { $crate::SyntaxKind::LET_KEYWORD };
    [loop] => { $crate::SyntaxKind::LOOP_KEYWORD };
    [macro_rules] => { $crate::SyntaxKind::MACRO_RULES_KEYWORD };
    [match] => { $crate::SyntaxKind::MATCH_KEYWORD };
    [mod] => { $crate::SyntaxKind::MOD_KEYWORD };
    [mut] => { $crate::SyntaxKind::MUT_KEYWORD };
    [package] => { $crate::SyntaxKind::PACKAGE_KEYWORD };
    [pub] => { $crate::SyntaxKind::PUB_KEYWORD };
    [return] => { $crate::SyntaxKind::RETURN_KEYWORD };
    [self] => { $crate::SyntaxKind::SELF_KEYWORD };
    [static] => { $crate::SyntaxKind::STATIC_KEYWORD };
    [struct] => { $crate::SyntaxKind::STRUCT_KEYWORD };
    [super] => { $crate::SyntaxKind::SUPER_KEYWORD };
    [trait] => { $crate::SyntaxKind::TRAIT_KEYWORD };
    [true] => { $crate::SyntaxKind::TRUE_KEYWORD };
    [type] => { $crate::SyntaxKind::TYPE_KEYWORD };
    [where] => { $crate::SyntaxKind::WHERE_KEYWORD };
    [while] => { $crate::SyntaxKind::WHILE_KEYWORD };
    [yield] => { $crate::SyntaxKind::YIELD_KEYWORD };
    [ident] => { $crate::SyntaxKind::IDENT };
}

use SyntaxKind::*;
use rowan::GreenNode;

pub use rowan;

pub const LAST_TOKEN_KIND_DISCRIMINANT: usize = YIELD_KEYWORD as usize;

impl SyntaxKind {

    pub fn is_trivia(&self) -> bool {
        matches!(self, WHITESPACE | LINE_COMMENT | BLOCK_COMMENT)
    }

    pub fn pretty(&self) -> &str {
        match *self {
            TOMBSTONE => unreachable!(),
            SHEBANG => "a shebang line",
            C_STRING => "a C string (like c\"foo\")",
            DOLLAR => "'$'",
            DOTDOTDOT => "'...'",
            FAT_ARROW => "'=>'",
            STAR => "'*'",
            LABEL_IDENT => "a label (like 'outer)",
            METAVARIABLE => "a metavariable (like $name)",
            AS_KEYWORD => "'as'",
            ASYNC_KEYWORD => "'async'",
            AWAIT_KEYWORD => "'await'",
            BREAK_KEYWORD => "'break'",
            CONST_KEYWORD => "'const'",
            CONTINUE_KEYWORD => "'continue'",
            DEFAULT_KEYWORD => "'default'",
            ELSE_KEYWORD => "'else'",
            ENUM_KEYWORD => "'enum'",
            EXTERN_KEYWORD => "'extern'",
            FALSE_KEYWORD => "'false'",
            FN_KEYWORD => "'fn'",
            FOR_KEYWORD => "'for'",
            GEN_KEYWORD => "'gen'",
            IF_KEYWORD => "'if'",
            IMPL_KEYWORD => "'impl'",
            IMPORT_KEYWORD => "'import'",
            IN_KEYWORD => "'in'",
            LET_KEYWORD => "'let'",
            LOOP_KEYWORD => "'loop'",
            MACRO_RULES_KEYWORD => "'macro_rules'",
            MATCH_KEYWORD => "'match'",
            MOD_KEYWORD => "'mod'",
            MUT_KEYWORD => "'mut'",
            PACKAGE_KEYWORD => "'package'",
            PUB_KEYWORD => "'pub'",
            RETURN_KEYWORD => "'return'",
            SELF_KEYWORD => "'self'",
            STATIC_KEYWORD => "'static'",
            STRUCT_KEYWORD => "'struct'",
            SUPER_KEYWORD => "'super'",
            TRAIT_KEYWORD => "'trait'",
            TRUE_KEYWORD => "'true'",
            TYPE_KEYWORD => "'type'",
            WHERE_KEYWORD => "'where'",
            WHILE_KEYWORD => "'while'",
            YIELD_KEYWORD => "'yield'",
            AT => "'@'",
            ARG_LIST => "the argument list",
            ARROW_TYPE_EXPR => "a function type signature",
            ASSIGNMENT => "an assignment operator",
            AWAIT_EXPR => "an await expression",
            BIN_EXPR => "an infix-expression",
            BLOCK => "a block of statement",
            BLOCK_COMMENT => "a block comment",
            BLOCK_EXPR => "a block expression",
            BYTE => "a single byte (like b'a')",
            BYTE_STRING => "a byte string (like b\"foo\")",
            CALL_EXPR => "a call expression",
            CAST_EXPR  => "a cast expression",
            CHAR => "a single character (like 'f')",
            COLON => "':'",
            COLONCOLON => "'::'",
            COMMA => "','",
            DOT => "'.'",
            DOTDOT => "'..'",
            DOTDOTEQUALS => "'..='",
            EOF => "end-of-file",
            EQUALS => "'='",
            ERROR => "an invalid node or token",
            EXCL => "'!'",
            EXTERN_BLOCK => "an 'extern' block",
            FIELD_EXPR => "a field expression",
            FLOAT_NUMBER => "a floating-point number",
            FUNC_DECL => "a function declaration",
            FUN_EXPR => "a function expression",
            GT => "'>'",
            HASHTAG => "'#'",
            IDENT => "an identifier",
            IMPL_RESTRICTION => "an restriction to an impl-block",
            INDEX_EXPR => "an index expression",
            INITIALIZER => "an initializer",
            INT_NUMBER => "an integral number",
            LET_STMT => "a variable declaration",
            LINE_COMMENT => "a line comment",
            LIT_EXPR => "a literal expression",
            LIT_PATT => "a literal pattern",
            LT => "'<'",
            L_BRACE => "'{'",
            L_BRACKET => "'['",
            L_PAREN => "'('",
            MACRO_CALL => "a macro call",
            MACRO_EXPR => "a macro expression'",
            MATCH_EXPR => "a match expression",
            METHOD_CALL_EXPR => "a method call expression",
            MINUS => "'-'",
            MODULE => "a module",
            NAMED_PATT => "a named pattern",
            NAMED_TYPE_EXPR => "a reference to another type",
            NEST_EXPR => "a nested expression",
            NEST_PATT => "a pattern between '(' and ')'",
            OPERATOR => "an operator",
            PARAM => "a function parameter",
            PATH_EXPR => "a path expression",
            PLUS => "'+'",
            POSTFIX_EXPR => "a postfix-expression",
            PREFIX_EXPR => "a prefix-expression",
            QUEST => "'?'",
            RANGE_EXPR => "a range expression",
            RECORD_EXPR => "a record expression",
            RECORD_EXPR_FIELD => "a single field of a record expression",
            RECORD_EXPR_FIELD_LIST => "a list of fields inside a record expression",
            RET_EXPR => "a 'return' expression",
            R_ARROW => "'->'",
            R_BRACE => "'}'",
            R_BRACKET => "']'",
            R_PAREN => "')'",
            SEMI => "';'",
            SOURCE_FILE => "a source file",
            STRING => "a string",
            TOKEN_TREE => "a token tree",
            TRY_EXPR => "a try-expression",
            TUPLE_EXPR => "a tuple expression",
            TUPLE_PATT => "a tuple pattern",
            TYPED_PATT => "a pattern with a type ascription",
            TYPE_ALIAS => "a type alias",
            TYPE_SIGNATURE => "a type ascription",
            UNDERSCORE => "'_'",
            VBAR => "'|'",
            VISIBILITY => "a visibility modifier",
            WHITESPACE => "some whitespace",
        }
    }

}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(value: SyntaxKind) -> Self {
        Self(value as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {}
impl rowan::Language for Lang {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        assert!(raw.0 <= SOURCE_FILE as u16);
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

#[allow(unused)]
pub type SyntaxNode = rowan::SyntaxNode<Lang>;

#[allow(unused)]
pub type SyntaxToken = rowan::SyntaxToken<Lang>;

#[allow(unused)]
pub type NodeOrToken = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

#[allow(unused)]
pub type SyntaxNodeChildren = rowan::SyntaxNodeChildren<Lang>;

#[allow(unused)]
pub type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

#[salsa::tracked]
pub struct DbNode<'db> {
    #[tracked]
    #[returns(ref)]
    pub node: GreenNode,
}


#[cfg(test)]
mod tests {
    use super::SyntaxKind::*;

    #[test]
    fn t_macro() {
        assert_eq!(T![;], SEMI);
        assert_eq!(T![::], COLONCOLON);
        assert_eq!(T![..=], DOTDOTEQUALS);
        assert_eq!(T!['{'], L_BRACE);
        assert_eq!(T![$], DOLLAR);
        assert_eq!(T![fn], FN_KEYWORD);
        assert_eq!(T![macro_rules], MACRO_RULES_KEYWORD);
        assert_eq!(T![ident], IDENT);
    }
}
