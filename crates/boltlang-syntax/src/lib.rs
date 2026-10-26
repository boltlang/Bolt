
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
    /// A lone `'`, which can appear in token trees
    QUOTE,
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


    // Keywords. `macro_rules` is contextual: the lexer produces an IDENT and
    // the parser remaps it where it acts as a keyword.
    AS_KEYWORD,
    ASYNC_KEYWORD,
    AWAIT_KEYWORD,
    BREAK_KEYWORD,
    CLASS_KEYWORD,
    CONST_KEYWORD,
    CONTINUE_KEYWORD,
    ELSE_KEYWORD,
    ENUM_KEYWORD,
    EXTERN_KEYWORD,
    FN_KEYWORD,
    FOR_KEYWORD,
    IF_KEYWORD,
    IMPL_KEYWORD,
    IMPORT_KEYWORD,
    IN_KEYWORD,
    LET_KEYWORD,
    LOOP_KEYWORD,
    MACRO_RULES_KEYWORD,
    MATCH_KEYWORD,
    MUT_KEYWORD,
    PACKAGE_KEYWORD,
    PUB_KEYWORD,
    RETURN_KEYWORD,
    STRUCT_KEYWORD,
    SUPER_KEYWORD,
    TYPE_KEYWORD,
    WHERE_KEYWORD,
    WHILE_KEYWORD,
    YIELD_KEYWORD,

    // Names and paths
    /// The name of something being declared, like the `f` in `fn f()`
    NAME,
    /// A reference to a name, like a segment of a path
    NAME_REF,
    /// `a.b.c`: an optional qualifier `PATH` (`a.b`) and a `PATH_SEGMENT` (`c`)
    PATH,
    PATH_SEGMENT,

    // Patterns
    /// A binding like `x`
    NAMED_PATT,
    /// `1`, `-1` or `"s"`
    LIT_PATT,
    /// `_`
    WILDCARD_PATT,
    /// A constant or variant like `Color.Red`
    PATH_PATT,
    /// `(a, b)`
    TUPLE_PATT,
    /// `Some(x)`
    TUPLE_STRUCT_PATT,
    /// `Point { x, y: 0, .. }`
    STRUCT_PATT,
    /// `x`, `mut x` or `x: pattern` in a struct pattern
    FIELD_PATT,
    /// `..` in a tuple, slice or struct pattern
    REST_PATT,
    /// `[a, .., b]`
    SLICE_PATT,
    /// `x @ pattern`
    CAPTURED_PATT,
    /// `mut x`
    MUT_PATT,
    /// `1..=5`, `a..` or `..=b`
    RANGE_PATT,
    /// `a | b`
    OR_PATT,
    /// `const { ... }`
    CONST_BLOCK_PATT,

    // Type expressions
    /// A type named by a path, like `Vec[T]` or `Shape.Circle`
    PATH_TYPE,
    /// `(A, B)`, or `()` for the unit type
    TUPLE_TYPE,
    /// `[T]` or `[T; N]`
    ARRAY_TYPE,
    /// `!`
    NEVER_TYPE,
    /// `A + B`
    BOUNDED_TYPE,
    /// `fn(A) -> B`, `extern "C" fn()` or `FnOnce(A) -> B`
    FN_TYPE,
    /// `-> T`
    RET_TYPE,
    /// `extern` or `extern "C"`
    ABI,
    /// `impl Trait` or `impl for[T] Trait`
    IMPL_TRAIT_TYPE,

    // Generic parameters
    /// `[T, const N: usize]` after the name of a function or type
    GENERIC_PARAM_LIST,
    /// `T`, `T: Bound` or `T = Default`
    TYPE_PARAM,
    /// `const N: usize` or `const N: usize = 3`
    CONST_PARAM,

    // Generic arguments
    /// `[A, B]` after a type
    GENERIC_ARG_LIST,
    /// A type as a generic argument
    TYPE_ARG,
    /// A binding like `Item = T` as a generic argument
    ASSOC_TYPE_ARG,
    /// A literal or block as a generic argument, like `3` or `{ N + 1 }`
    CONST_ARG,
    /// `: A + B`
    TRAIT_BOUNDS,

    // Macros
    /// `#[meta]` or `#![meta]`
    ATTR,
    /// `path`, `path = value` or `path(tokens)` in an attribute
    META,
    /// A macro invocation used as an expression
    MACRO_EXPR,
    /// `name!(tokens)`, `name![tokens]` or `name!{tokens}`
    MACRO_CALL,
    /// A macro invocation used as a pattern
    MACRO_PATT,
    /// A macro invocation used as a type
    MACRO_TYPE,
    /// `macro_rules! name { rules }`
    MACRO_RULES,
    /// `(pattern) => { body }` in a macro definition
    MACRO_RULE,
    /// `$( tokens ) sep op` in a macro definition, like `$($x),*`
    TOKEN_REPETITION,
    /// `$name:fragment` in the pattern of a macro rule, like `$e:expr`
    TOKEN_BINDING,
    TOKEN_TREE,

    // Expressions
    AWAIT_EXPR,
    /// `[a, b]` or `[x; n]`
    ARRAY_EXPR,
    /// `|a, b| a + b`
    CLOSURE_EXPR,
    /// `if cond { } else { }`
    IF_EXPR,
    /// `let pattern = value` in the condition of an `if` or `while`
    LET_EXPR,
    /// `while cond { }`
    WHILE_EXPR,
    /// `loop { }`
    LOOP_EXPR,
    /// `for pattern in iterable { }`
    FOR_EXPR,
    /// `break`, `break 'label` or `break value`
    BREAK_EXPR,
    /// `continue` or `continue 'label`
    CONTINUE_EXPR,
    /// `yield` or `yield value`
    YIELD_EXPR,
    /// `'outer`, before a loop or block or after `break` and `continue`
    LABEL,
    /// The `{ ... }` of a `match`
    MATCH_ARM_LIST,
    /// `pattern if guard => value`
    MATCH_ARM,
    /// `if condition` in a match arm
    MATCH_GUARD,
    BIN_EXPR,
    BLOCK_EXPR,
    CALL_EXPR,
    CAST_EXPR,
    FIELD_EXPR,
    FUN_EXPR,
    INDEX_EXPR,
    LIT_EXPR,
    MATCH_EXPR,
    NEST_EXPR,
    PATH_EXPR,
    PREFIX_EXPR,
    RANGE_EXPR,
    RECORD_EXPR,
    RET_EXPR,
    TRY_EXPR,
    TUPLE_EXPR,

    // Expression helpers
    RECORD_EXPR_FIELD,
    ARG_LIST,

    /// An expression followed by `;`, or one ending in a block, used as a
    /// statement.
    EXPR_STMT,

    // Declarations
    FUNC_DECL,
    EXTERN_BLOCK,
    LET_STMT,
    RECORD_EXPR_FIELD_LIST,
    /// `type A = B;`, or `type Item: Bound;` in a trait
    TYPE_ALIAS,

    // Impl helper syntax

    // Helper syntax
    BLOCK,
    /// `(a, b: T)`
    PARAM_LIST,
    PARAM,
    /// `...` or `args: ...`
    VARIADIC_PARAM,
    /// `pub`, `pub(super)` or `pub(package)`
    VISIBILITY,
    /// The `+` in `fn a + b`
    OPERATOR_NAME,
    /// `const NAME: Type = value;`
    CONST,
    /// `import "./path" (a, b as c);`
    IMPORT,
    /// `(a, b as c)` in an import
    IMPORT_LIST,
    /// `b as c` in an import list
    IMPORT_ALIAS,
    /// `class Show[T] { items }`
    CLASS,
    /// `impl Dog { items }` or `impl Show[Dog] { items }`
    IMPL,
    /// The `{ items }` of a class, impl or extern block
    ITEM_LIST,
    /// `struct Point { x: i32 }`, `struct Pair(A, B);` or `struct Unit;`
    STRUCT,
    /// `enum Option[T] { None, Some(T) }`
    ENUM,
    /// The `{ ... }` of an enum
    VARIANT_LIST,
    /// `None`, `Some(T)`, `Point { x: i32 }` or `A = 1` in an enum
    VARIANT,
    /// `{ x: i32, pub y: i32 }`
    RECORD_FIELD_LIST,
    /// `x: i32` in a struct or variant
    RECORD_FIELD,
    /// `(i32, pub String)`
    TUPLE_FIELD_LIST,
    /// `i32` or `pub String` in a tuple struct or variant
    TUPLE_FIELD,
    /// `where T: Ord, U: Hash`
    WHERE_CLAUSE,
    /// `T: Ord` in a where clause
    WHERE_PRED,

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
    ['\''] => { $crate::SyntaxKind::QUOTE };
    [_] => { $crate::SyntaxKind::UNDERSCORE };
    [as] => { $crate::SyntaxKind::AS_KEYWORD };
    [async] => { $crate::SyntaxKind::ASYNC_KEYWORD };
    [await] => { $crate::SyntaxKind::AWAIT_KEYWORD };
    [break] => { $crate::SyntaxKind::BREAK_KEYWORD };
    [const] => { $crate::SyntaxKind::CONST_KEYWORD };
    [continue] => { $crate::SyntaxKind::CONTINUE_KEYWORD };
    [else] => { $crate::SyntaxKind::ELSE_KEYWORD };
    [enum] => { $crate::SyntaxKind::ENUM_KEYWORD };
    [extern] => { $crate::SyntaxKind::EXTERN_KEYWORD };
    [fn] => { $crate::SyntaxKind::FN_KEYWORD };
    [for] => { $crate::SyntaxKind::FOR_KEYWORD };
    [if] => { $crate::SyntaxKind::IF_KEYWORD };
    [impl] => { $crate::SyntaxKind::IMPL_KEYWORD };
    [import] => { $crate::SyntaxKind::IMPORT_KEYWORD };
    [in] => { $crate::SyntaxKind::IN_KEYWORD };
    [let] => { $crate::SyntaxKind::LET_KEYWORD };
    [loop] => { $crate::SyntaxKind::LOOP_KEYWORD };
    [macro_rules] => { $crate::SyntaxKind::MACRO_RULES_KEYWORD };
    [match] => { $crate::SyntaxKind::MATCH_KEYWORD };
    [mut] => { $crate::SyntaxKind::MUT_KEYWORD };
    [package] => { $crate::SyntaxKind::PACKAGE_KEYWORD };
    [pub] => { $crate::SyntaxKind::PUB_KEYWORD };
    [return] => { $crate::SyntaxKind::RETURN_KEYWORD };
    [struct] => { $crate::SyntaxKind::STRUCT_KEYWORD };
    [super] => { $crate::SyntaxKind::SUPER_KEYWORD };
    [class] => { $crate::SyntaxKind::CLASS_KEYWORD };
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
            MACRO_RULES => "a macro definition",
            MACRO_RULE => "a macro rule",
            TOKEN_REPETITION => "a repetition in a macro",
            TOKEN_BINDING => "a metavariable binding",
            MACRO_PATT => "a macro invocation as a pattern",
            MACRO_TYPE => "a macro invocation as a type",
            ATTR => "an attribute",
            META => "the contents of an attribute",
            QUOTE => "'\''",
            IMPORT => "an import",
            IMPORT_LIST => "a list of imported names",
            IMPORT_ALIAS => "an imported name with an alias",
            CLASS => "a class",
            IMPL => "an impl",
            ITEM_LIST => "a list of items",
            CONST => "a constant",
            STRUCT => "a struct",
            ENUM => "an enum",
            VARIANT_LIST => "the variants of an enum",
            VARIANT => "an enum variant",
            RECORD_FIELD_LIST => "a list of named fields",
            RECORD_FIELD => "a named field",
            TUPLE_FIELD_LIST => "a list of tuple fields",
            TUPLE_FIELD => "a tuple field",
            OPERATOR_NAME => "an operator name",
            WHERE_CLAUSE => "a where clause",
            WHERE_PRED => "a where predicate",
            CLOSURE_EXPR => "a closure",
            ARRAY_EXPR => "an array expression",
            MATCH_ARM_LIST => "the arms of a match",
            MATCH_ARM => "a match arm",
            MATCH_GUARD => "a match guard",
            LOOP_EXPR => "a loop",
            FOR_EXPR => "a for loop",
            BREAK_EXPR => "a break expression",
            CONTINUE_EXPR => "a continue expression",
            YIELD_EXPR => "a yield expression",
            LABEL => "a label",
            IF_EXPR => "an if expression",
            LET_EXPR => "a let condition",
            WHILE_EXPR => "a while loop",
            EXPR_STMT => "an expression statement",
            WILDCARD_PATT => "a wildcard pattern",
            PATH_PATT => "a path pattern",
            TUPLE_STRUCT_PATT => "a tuple struct pattern",
            STRUCT_PATT => "a struct pattern",
            FIELD_PATT => "a field pattern",
            REST_PATT => "a rest pattern (..)",
            SLICE_PATT => "a slice pattern",
            CAPTURED_PATT => "a captured pattern",
            MUT_PATT => "a mutable pattern",
            RANGE_PATT => "a range pattern",
            OR_PATT => "an or-pattern",
            CONST_BLOCK_PATT => "a const block pattern",
            IMPL_TRAIT_TYPE => "an impl trait type",
            GENERIC_PARAM_LIST => "a list of generic parameters",
            TYPE_PARAM => "a type parameter",
            CONST_PARAM => "a const parameter",
            FN_TYPE => "a function type",
            RET_TYPE => "a return type",
            ABI => "an ABI (like extern \"C\")",
            PARAM_LIST => "a parameter list",
            VARIADIC_PARAM => "a variadic parameter",
            PATH_TYPE => "a type",
            TUPLE_TYPE => "a tuple type",
            ARRAY_TYPE => "an array type",
            NEVER_TYPE => "the never type",
            BOUNDED_TYPE => "a type with bounds",
            GENERIC_ARG_LIST => "a list of generic arguments",
            TYPE_ARG => "a type argument",
            ASSOC_TYPE_ARG => "an associated type binding",
            CONST_ARG => "a constant generic argument",
            TRAIT_BOUNDS => "a list of trait bounds",
            NAME => "a name",
            NAME_REF => "a reference to a name",
            PATH => "a path",
            PATH_SEGMENT => "a segment of a path",
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
            ELSE_KEYWORD => "'else'",
            ENUM_KEYWORD => "'enum'",
            EXTERN_KEYWORD => "'extern'",
            FN_KEYWORD => "'fn'",
            FOR_KEYWORD => "'for'",
            IF_KEYWORD => "'if'",
            IMPL_KEYWORD => "'impl'",
            IMPORT_KEYWORD => "'import'",
            IN_KEYWORD => "'in'",
            LET_KEYWORD => "'let'",
            LOOP_KEYWORD => "'loop'",
            MACRO_RULES_KEYWORD => "'macro_rules'",
            MATCH_KEYWORD => "'match'",
            MUT_KEYWORD => "'mut'",
            PACKAGE_KEYWORD => "'package'",
            PUB_KEYWORD => "'pub'",
            RETURN_KEYWORD => "'return'",
            STRUCT_KEYWORD => "'struct'",
            SUPER_KEYWORD => "'super'",
            CLASS_KEYWORD => "'class'",
            TYPE_KEYWORD => "'type'",
            WHERE_KEYWORD => "'where'",
            WHILE_KEYWORD => "'while'",
            YIELD_KEYWORD => "'yield'",
            AT => "'@'",
            ARG_LIST => "the argument list",
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
            INDEX_EXPR => "an index expression",
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
            MACRO_EXPR => "a macro invocation as an expression",
            MATCH_EXPR => "a match expression",
            MINUS => "'-'",
            NAMED_PATT => "a named pattern",
            NEST_EXPR => "a nested expression",
            OPERATOR => "an operator",
            PARAM => "a function parameter",
            PATH_EXPR => "a path expression",
            PLUS => "'+'",
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
            TYPE_ALIAS => "a type alias",
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
