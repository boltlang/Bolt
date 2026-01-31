
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types)]
#[repr(u16)]
pub enum SyntaxKind {
    // Special tokens
    ERROR,
    TOMBSTONE,
    END_OF_FILE,
    BLOCK_START,
    BLOCK_END,

    // Skippable tokens
    WHITESPACE,
    LINE_COMMENT,
    BLOCK_COMMENT,

    // Static tokens
    L_PAREN,
    R_PAREN,
    L_BRACKET,
    R_BRACKET,
    L_BRACE,
    R_BRACE,
    R_ARROW,
    COLON,
    EQUALS,
    COMMA,

    // Tokens which can have a value
    IDENTIFIER,
    OPERATOR,
    STRING,
    ASSIGNMENT,
    BIN_INT,
    OCT_INT,
    DEC_INT,
    HEX_INT,

    // Keywords
    DO_KEYWORD,
    FN_KEYWORD,
    LET_KEYWORD,
    MATCH_KEYWORD,
    MUT_KEYWORD,
    PUB_KEYWORD,
    RETURN_KEYWORD,
    TYPE_KEYWORD, // keep in sync with LAST_TOKEN_DISCRIMINANT

    // Patterns
    NAMED_PATT,
    LIT_PATT,
    TUPLE_PATT,
    NEST_PATT,

    // Type expressions
    ARROW_TYPE_EXPR,
    NAMED_TYPE_EXPR,

    // Expressions
    BLOCK_EXPR,
    CALL_EXPR,
    FUN_EXPR,
    INFIX_EXPR,
    LIT_EXPR,
    MATCH_EXPR,
    NEST_EXPR,
    POSTFIX_EXPR,
    PREFIX_EXPR,
    REF_EXPR,
    TUPLE_EXPR,

    // Declarations
    FUNC_DECL,
    VAR_DECL,

    // Helper syntax
    BLOCK,
    TYPE_SIGNATURE,
    INITIALIZER,
    PARAM,

    // Top-level
    SOURCE_FILE,
}
use SyntaxKind::*;

pub const LAST_TOKEN_KIND_DISCRIMINANT: usize = TYPE_KEYWORD as usize;

impl SyntaxKind {

    pub fn is_trivia(&self) -> bool {
        matches!(self, WHITESPACE | LINE_COMMENT | BLOCK_COMMENT)
    }

    pub fn pretty(&self) -> &str {
        match *self {
            TOMBSTONE => unreachable!(),
            ERROR => "an invalid node or token",
            END_OF_FILE => "end-of-file",
            WHITESPACE => "some whitespace",
            LINE_COMMENT => "a line comment",
            BLOCK_COMMENT => "a block comment",
            BLOCK_START => "the start of an indented block",
            BLOCK_END => "the end of an indented block",
            L_PAREN => "'('",
            R_PAREN => "')'",
            L_BRACKET => "'['",
            R_BRACKET => "']'",
            L_BRACE => "'{'",
            R_BRACE => "'}'",
            COLON => "':'",
            EQUALS => "'='",
            COMMA => "','",
            R_ARROW => "'->'",
            IDENTIFIER => "an identifier",
            STRING => "a string",
            OPERATOR => "an operator",
            ASSIGNMENT => "an assignment operator",
            BIN_INT | OCT_INT | DEC_INT | HEX_INT => "an integer literal",
            DO_KEYWORD => "'do'",
            FN_KEYWORD => "'fn'",
            LET_KEYWORD => "'let'",
            MATCH_KEYWORD => "'match'",
            MUT_KEYWORD => "'mut'",
            PUB_KEYWORD => "'pub'",
            RETURN_KEYWORD => "'return'",
            TYPE_KEYWORD => "'type'",
            ARROW_TYPE_EXPR => "a function type signature",
            NAMED_TYPE_EXPR => "a reference to another type",
            PARAM => "a function parameter",
            BLOCK => "a block of statement",
            TYPE_SIGNATURE => "a type ascription",
            INITIALIZER => "an initializer",
            NAMED_PATT => "a named pattern",
            LIT_PATT => "a literal pattern",
            TUPLE_PATT => "a tuple pattern",
            NEST_PATT => "a pattern between '(' and ')'",
            BLOCK_EXPR => "a block expression",
            CALL_EXPR => "a call expression",
            FUN_EXPR => "a function expression",
            INFIX_EXPR => "an infix-expression",
            LIT_EXPR => "a literal expression",
            MATCH_EXPR => "a match expression",
            NEST_EXPR => "a nested expression",
            POSTFIX_EXPR => "a postfix-expression",
            PREFIX_EXPR => "a prefix-expression",
            REF_EXPR => "a reference expression",
            TUPLE_EXPR => "a tuple expression",
            VAR_DECL => "a variable declaration",
            FUNC_DECL => "a function declaration",
            SOURCE_FILE => "a source file",
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
