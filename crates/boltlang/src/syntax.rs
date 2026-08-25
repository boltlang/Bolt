
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

    // Static tokens
    HASHTAG,
    L_PAREN,
    R_PAREN,
    L_BRACKET,
    R_BRACKET,
    L_BRACE,
    R_BRACE,
    R_ARROW,
    COLON,
    COLONCOLON,
    EQUALS,
    COMMA,
    SEMI,
    DOT,
    DOTDOT,
    DOTDOTEQUALS,
    LT,
    GT,
    MINUS,
    EXCL,
    QUEST,
    UNDERSCORE,

    // Tokens which can have a value
    IDENT,
    OPERATOR,
    STRING,
    ASSIGNMENT,
    FLOAT_NUMBER,
    INT_NUMBER,

    // TODO remove
    BIN_INT,
    OCT_INT,
    DEC_INT,
    HEX_INT,

    // Keywords
    AS_KEYWORD,
    ASYNC_KEYWORD,
    AWAIT_KEYWORD,
    DEFAULT_KEYWORD,
    DO_KEYWORD,
    FN_KEYWORD,
    IMPL_KEYWORD,
    LET_KEYWORD,
    MATCH_KEYWORD,
    MUT_KEYWORD,
    PUB_KEYWORD,
    RETURN_KEYWORD,
    TYPE_KEYWORD,
    UNSAFE_KEYWORD, // keep in sync with LAST_TOKEN_DISCRIMINANT

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
use SyntaxKind::*;
use rowan::GreenNode;

pub const LAST_TOKEN_KIND_DISCRIMINANT: usize = UNSAFE_KEYWORD as usize;

impl SyntaxKind {

    pub fn is_trivia(&self) -> bool {
        matches!(self, WHITESPACE | LINE_COMMENT | BLOCK_COMMENT)
    }

    pub fn pretty(&self) -> &str {
        match *self {
            TOMBSTONE => unreachable!(),
            ARG_LIST => "the argument list",
            ARROW_TYPE_EXPR => "a function type signature",
            ASSIGNMENT => "an assignment operator",
            ASYNC_KEYWORD => "'async'",
            AS_KEYWORD => "'as'",
            AWAIT_EXPR => "an await expression",
            AWAIT_KEYWORD => "'await'",
            BIN_EXPR => "an infix-expression",
            BIN_INT | OCT_INT | DEC_INT | HEX_INT => "an integer literal",
            BLOCK => "a block of statement",
            BLOCK_COMMENT => "a block comment",
            BLOCK_EXPR => "a block expression",
            CALL_EXPR => "a call expression",
            CAST_EXPR  => "a cast expression",
            COLON => "':'",
            COLONCOLON => "'::'",
            COMMA => "','",
            DEFAULT_KEYWORD => "'default'",
            DOT => "'.'",
            DOTDOT => "'..'",
            DOTDOTEQUALS => "'..='",
            DO_KEYWORD => "'do'",
            EOF => "end-of-file",
            EQUALS => "'='",
            ERROR => "an invalid node or token",
            EXCL => "'!'",
            EXTERN_BLOCK => "an 'extern' block",
            FIELD_EXPR => "a field expression",
            FLOAT_NUMBER => "a floating-point number",
            FN_KEYWORD => "'fn'",
            FUNC_DECL => "a function declaration",
            FUN_EXPR => "a function expression",
            GT => "'>'",
            HASHTAG => "'#'",
            IDENT => "an identifier",
            IMPL_KEYWORD => "'impl'",
            IMPL_RESTRICTION => "an restriction to an impl-block",
            INDEX_EXPR => "an index expression",
            INITIALIZER => "an initializer",
            INT_NUMBER => "an integral number",
            LET_KEYWORD => "'let'",
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
            MATCH_KEYWORD => "'match'",
            METHOD_CALL_EXPR => "a method call expression",
            MINUS => "'-'",
            MODULE => "a module",
            MUT_KEYWORD => "'mut'",
            NAMED_PATT => "a named pattern",
            NAMED_TYPE_EXPR => "a reference to another type",
            NEST_EXPR => "a nested expression",
            NEST_PATT => "a pattern between '(' and ')'",
            OPERATOR => "an operator",
            PARAM => "a function parameter",
            PATH_EXPR => "a path expression",
            POSTFIX_EXPR => "a postfix-expression",
            PREFIX_EXPR => "a prefix-expression",
            PUB_KEYWORD => "'pub'",
            QUEST => "'?'",
            RANGE_EXPR => "a range expression",
            RECORD_EXPR => "a record expression",
            RECORD_EXPR_FIELD => "a single field of a record expression",
            RECORD_EXPR_FIELD_LIST => "a list of fields inside a record expression",
            RETURN_KEYWORD => "'return'",
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
            TYPE_KEYWORD => "'type'",
            TYPE_SIGNATURE => "a type ascription",
            UNDERSCORE => "'_'",
            UNSAFE_KEYWORD => "'unsafe'",
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

