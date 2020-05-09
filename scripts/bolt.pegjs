{
  const { 
    setParents,

    SourceFile,
    SyntaxKind, 
    TuplePattern, 
    ListPattern, 
    Identifier, 
    StructDefinition,
    StructField,
    EnumDefinition,
    EnumField,
    FunctionDefinition, 
    FunctionExpression, 
    FunctionCall,
    VariableDefinition, 
    Param, 
    MatchExpression, 
    MatchCase, 
    IntegerExpression, 
    CharLiteral, 
    StringLiteral,
    ReturnStatement,
    ExpressionStatement,
    ContinueStatement,
    BreakStatement,
    TypeReference,
    TypeAlias,
    MemberAccess,
    AssignExpression,
    FunctionBody
  } = require('./syntax')

  function loc() {
    const { start, end } = location()
    return { start, end, fileId: options.fileId }
  }

  function buildRightRecursive(e1, rest) {
    if (rest.length === 0) {
      return e1
    }
    const tail = rest.slice(0, rest.length-2)
    const head = rest[rest.length-1]
    const nested = buildRightRecursive(head[3], tail)
    return new FunctionCall(head[1], [e1, nested], null, null, null, loc())
  }

  function buildLeftRecursive(e1, rest) {
    if (rest.length === 0) {
      return e1
    }
    const [head, ...tail] = rest
    const nested = buildLeftRecursive(head[3], tail)
    return new FunctionCall(head[1], [e1, nested], null, null, null, loc())
  }

}

File
  = body:Statements {
      const sourceFile = new SourceFile(body, null, loc())
      setParents(sourceFile)
      return sourceFile
    }

Statements
  = @(__ @Statement)* __

EOS
  = __ ';'
  / _ LineComment? EOLF
  / _ &'}'
  / __ EOF

IdentifierStart = [a-zA-Z] 
IdentifierPart  = [a-zA-Z0-9]

Identifier "identifier"
  = !ReservedWord text:$(IdentifierStart IdentifierPart*) {
      return new Identifier(text, null, null, null, loc())
    }

FunctionKeyword  = 'func'     !IdentifierPart
ReturnKeyword    = 'return'   !IdentifierPart
BreakKeyword     = 'break'    !IdentifierPart
ContinueKeyword  = 'continue' !IdentifierPart
VariableKeyword  = 'var'      !IdentifierPart
ConstantKeyword  = 'const'    !IdentifierPart
MatchKeyword     = 'match'    !IdentifierPart
ClassKeyword     = 'class'    !IdentifierPart
TypeKeyword      = 'type'     !IdentifierPart
StructKeyword    = 'struct'   !IdentifierPart

Keyword
  = FunctionKeyword
  / ReturnKeyword
  / BreakKeyword
  / ContinueKeyword
  / VariableKeyword
  / ConstantKeyword
  / MatchKeyword

ReservedWord 
  = 'if' !IdentifierPart
  / 'else' !IdentifierPart
  / 'while' !IdentifierPart
  / 'loop' !IdentifierPart
  / Keyword

TypeDecl
  = TypeReference

TypeReference
  = name:Identifier {
      return new TypeReference(name, null, null, null, loc())
    }

Statement
  = ReturnStatement
  / BreakStatement
  / ContinueStatement
  / FunctionDefinition
  / VariableDefinition
  / TypeClassDefinition
  / StructDefinition
  / ExpressionStatement
  / TypeAlias

TypeAlias
  = TypeKeyword __ name:Identifier __ '=' __ typeDecl:TypeDecl EOS {
      return new TypeAlias(name, typeDecl, null,  null, null, loc())
    }

TypeClassDefinition
  = TypeKeyword __ ClassKeyword __ name:Identifier __ param:Identifier __ '{' __ members:(@FunctionSignature EOS __)* '}' EOS {
      return new TypeClass(name, param, members, null, null, null, loc())
    }

StructField 
  = name:Identifier __ ':' __ typeDecl:TypeDecl {
      return new StructField(name, typeDecl, null, null, null, loc())
    }

StructDefinition
  = StructKeyword __ name:Identifier __ '{' members:(__ @StructField EOS)* __ '}' {
      return new StructDefinition(name, members, null, null, null, loc())
    }

ExpressionStatement
  = expression:Expr EOS {
      return new ExpressionStatement(expression, null, null, null, loc())
    }

ReturnStatement 
  = ReturnKeyword EOS {
      return new ReturnStatement(expression, null, null, null, loc())
    }
  / ReturnKeyword __ expression:Expr EOS {
      return new ReturnStatement(expression, null, null, null, loc())
    }

BreakStatement
  = BreakKeyword EOS {
      return new BreakStatement(null, null, null, null, loc())
    }
  / keyword:BreakKeyword __ label:Identifier EOS {
      return BreakStatement(label, null, null, null, loc())
    }

ContinueStatement
  = ContinueKeyword EOS {
      return new ContinueStatement(null, null, null, null, loc())
    }
  / ContinueKeyword __ label:Identifier EOS {
      return new ContinueStatement(label, null, null, null, loc())
    }

Param
  = pattern:Pattern typeDecl:(__ ':' __ @TypeDecl)? defaultValue:(__ '=' @Expr)? {
      return new Param(pattern, typeDecl, defaultValue, null, null, null, loc())
  }

OpParam
  = pattern:Identifier {
      return new Param(pattern, null, null, null, null, null, loc())
    }
  / '(' pattern:Pattern __ typeDecl:(__ ':' __ @TypeDecl)? defaultValue:(__ '=' @Expr)? __ ')' {
      return new Param(pattern, typeDecl, defaultValue, null, null, null, loc())
    }

Pattern
  = TuplePattern
  / VarPattern
  / ConstPattern

VarPattern
  = Identifier

PatternList
  = list:(first:Pattern rest:(__ ',' __ @Pattern)* { return [first, ...rest] })? {
      if (list === null)
        return []
      return list
    }

TuplePattern
  = '(' elements:PatternList ')' {
      return new TuplePattern(elements, null, null, null, loc())
    }

ConstPattern
  = StringLiteral
  / CharLiteral
  / IntegerExpression

ParamList
  = params:(Param (__ ',' __ @Param)*)? {
      if (params === null)
        return []
      return [params[0], ...params[1]]
    }

ArrayAccessParams
  = '[' pn:ParamList ']' {
      return [new Identifier('[]', null, null, null, loc()), pn]
    }

FunctionSignature
  = p1:OpParam __ rest:ArrayAccessParams {
      const [name, pn] = rest
      return [name, [p1, ...pn]]
    }
  / name:Identifier __ '(' __ params:ParamList __ ')' {
      return [name, params]
    }
  / name:UnaryOperator __ p:OpParam {
      return [name, [p]]
    }
  / p:OpParam __ name:PostfixOperator  {
      return [name, [p]]
    }
  / p1:OpParam __ name:BinOperator __ p2:OpParam {
      return [name, [p1, p2]]
    }

FunctionBody
  = statements:Statements {
      return new FunctionBody(statements, null, null, null, loc())
    }

FunctionDefinition
  = FunctionKeyword __  sig:FunctionSignature __ ':' __ returnTypeDecl:TypeDecl __ '{' __ body:FunctionBody __ '}' EOS {
      const [name, params] = sig
      return new FunctionDefinition(name, params, returnTypeDecl, body, null, null, null, loc())
    }
  / FunctionKeyword __  sig:FunctionSignature __ ':' __ returnTypeDecl:TypeDecl EOS {
      const [name, params] = sig
      return new FunctionDefinition(name, params, returnTypeDecl, null, null, null, null, loc())
    }
  / FunctionKeyword __  sig:FunctionSignature __ '{' __ body:FunctionBody __ '}' EOS {
      const [name, params] = sig
      return new FunctionDefinition(name, params, null, body, null, null, null, loc())
    }
  / FunctionKeyword __  sig:FunctionSignature EOS {
      const [name, params] = sig
      return new FunctionDefinition(name, params, null, null, null, null, null, loc())
    }

VariableDefinition
  = VariableKeyword __ pattern:Pattern EOS {
      return new VariableDefinition(pattern, null, null, null, null, null, loc())
    }
  / VariableKeyword __ pattern:Pattern __ ':' __ typeDecl:TypeDecl EOS {
      return new VariableDefinition(pattern, pattern, null, null, null, null, loc())
    }
  / VariableKeyword __ pattern:Pattern __ '=' __ expression:Expr EOS {
      return new VariableDefinition(pattern, null, expression, null, null, null, loc())
    }
  / VariableKeyword __ pattern:Pattern __ ':' __ typeDecl:TypeDecl __ '=' __ expression:Expr EOS {
      return new VariableDefinition(pattern, typeDecl, expression, null, null, null, loc())
    }

Expr
  = DelimExpr

DelimExpr
  = first:ExprNoDelim rest:(__ ',' __ @ExprNoDelim)* {
      if (rest.length === 0)
        return first
      return new ExpressionSequence([first, ...rest], null, null, null, loc())
    }

ExprNoDelim
  = AssignExpr

LOrOperator  = '||'      { return new Identifier('||', null, null, null, loc()) }
LAndOperator = '&&'      { return new Identifier('&&', null, null, null, loc()) }
BOrOperator  = '|' ![|=] { return new Identifier('|', null, null, null, loc()) }
BXOrOperator = '^' !'='  { return new Identifier('^', null, null, null, loc()) }
BAndOperator = '&' !'&=' { return new Identifier('&', null, null, null, loc()) }
EqOperator   = '=='      { return new Identifier('==', null, null, null, loc()) }
DotOperator  = '.'       { return new Identifier('.', null, null, null, loc()) }
ExptOperator = '**'      { return new Identifier('**', null, null, null, loc()) }

BinOperator
  = LOrOperator
  / LAndOperator
  / BOrOperator
  / BXOrOperator
  / BAndOperator
  / EqOperator
  / RelOperator
  / ShiftOperator
  / AddOperator
  / MulOperator
  / DotOperator

PostfixOperator 
  = text:$("++" / "--") { 
      return new Identifier(text, null, null, null, loc())
    }

AssignOperator
  = text:$('=' !'=' / '+=' / '-=' / '**=' / '*=' / '/=' / '%=' / '<<=' / '>>=' / '>>>=' / '&=' / '^=' / '|=') {
      return new Identifier(text, null, null, null, loc())
    }

RelOperator 
  = text:$("<=" / ">=" / "<" !"<" / ">" !">") { 
      return new Identifier(text, null, null, null, loc())
    }

ShiftOperator
  = text:$("<<" !"=" / ">>>" !"=") { 
      return new Identifier(text, null, null, null, loc())
    }

AddOperator
  = text:$("+" ![+=] / "-" ![-=]) { 
      return new Identifier(text, null, null, null, loc())
    }

MulOperator
  = text:$("*" ![*=] / "/" !"=" / "%" !"=") {
      return new Identifier(text, null, null, null, loc())
    }

UnaryOperator
  = text:$("++" / "--" / "+" !"=" / "-" !"=" / "~" / "!") {
      return new Identifier(text, null, null, null, loc())
    }

AssignExpr
  = left:(@(MemberAccessExpr / Pattern) __ AssignOperator __)* right:LOrExpr {
      if (left.length === 0)
        return right
      let out = right
      for (let i = left.length-1; i >= 0; i--) {
        out = new AssignExpression(left[i], out, null, null, null, loc())
      }
      return out
    }

LOrExpr
  = first:LAndExpr rest:(__ LOrOperator __ LAndExpr)* {
      return buildLeftRecursive(first, rest)
    }

LAndExpr
  = first:BOrExpr rest:(__ LAndOperator __ BOrExpr)* {
      return buildLeftRecursive(first, rest)
    }

BOrExpr
  = first:BXOrExpr rest:(__ BOrOperator __ BXOrExpr)* {
      return buildLeftRecursive(first, rest)
    }

BXOrExpr
  = first:BAndExpr rest:(__ BXOrOperator __ BAndExpr)* {
      return buildLeftRecursive(first, rest)
    }

BAndExpr
  = first:EqExpr rest:(__ BAndOperator __ EqExpr)* {
      return buildLeftRecursive(first, rest)
    }

EqExpr
  = first:RelExpr rest:(__ EqOperator __ RelExpr)* {
      return buildLeftRecursive(first, rest)
    }

RelExpr
  = first:ShiftExpr rest:(__ RelOperator __ ShiftExpr)* {
      return buildLeftRecursive(first, rest)
    }

ShiftExpr
  = first:AddExpr rest:(__ ShiftOperator __ AddExpr)* {
      return buildLeftRecursive(first, rest)
    }

AddExpr
  = first:MulExpr rest:(__ AddOperator __ MulExpr)* {
      return buildLeftRecursive(first, rest)
    }

MulExpr
  = first:ExptExpr rest:(__ MulOperator __ ExptExpr)* {
      return buildLeftRecursive(first, rest)
    }

ExptExpr
  = first:UnaryExpr rest:(__ ExptOperator __ UnaryExpr)* {
      return buildRightRecursive(first, rest)
    }

PostfixExpr 
  = e:CallExpr _ operator:PostfixOperator {
      return new FunctionCall(operator, [e], null, null, null, loc())
    }

UnaryExpr
  = PostfixExpr
  / CallExpr
  / operator:UnaryOperator _ e:UnaryExpr {
      return new FunctionCall(operator, [e], null, null, null, loc())
    }

ArgList
  = args:(Expr (__ ',' __ @Expr)) {
      if (args === null)
        return args
      return [args[0], ...args[1]]
    }

CallExpr
  = operator:ExprNoCall args:(__ '(' __ @ArgList __ ')')? { 
      if (args === null)
        return operator
      return new FunctionCall(operator, args, null, null, null, loc())
    }

VarRefExpr
  = Identifier

ASCIISpecialChar
  = '\\' char:. {
      switch (char) {
        case 'v': return '\v'
        case 'b': return '\b'
        case 'f': return '\f'
        case 'r': return '\r'
        case 't': return '\t'
        case 'b': return '\b'
        case 'n': return '\n'
        case '0': return '\0'
        default: return char
      }
    }

EscapeSeq
  = ASCIISpecialChar

IntegerExpression
  = digits:$([0-9]+) { 
      return new IntegerExpression(Number(digits), null, null, null, loc())
    }


Char
  = EscapeSeq / .

StringLiteral 
  = '"' chars:(!'"' @Char)* '"' {
      return new StringLiteral(chars.join(''), null, null, null, loc())
    }

CharLiteral
  = '\'' value:Char '\'' {
      return new CharLiteral(value, null, null, null, loc())
    }

NestExpr
  = '(' __ @Expr __ ')'

MatchCase
  = pattern:Pattern __ '->' __ expression:ExprNoDelim {
      return new MatchCase(pattern, expression, null, null, null, loc())
    }

MatchExpr
  = MatchKeyword __ expression:Expr __ '{' __ first:MatchCase rest:(__ ',' __ @MatchCase)* (__ ',')? __ '}' {
      return new MatchExpression(expression, [first, ...rest], null, null, null, loc())
    }

ArrayAccessArgs
  = '[' args:ArgList ']' {
      return [new Identifier('[]', null, null, null, loc()), args]
    }

ExprNoCall
  = arg0:MemberAccessExpr rest:(__ ArrayAccessArgs)? {
      if (rest === null) 
        return arg0
      const [operator, argn] = rest
      return new FunctionCall(operator, [arg0, ...argn], null, null, null, loc())
    }

MemberAccessExpr
  = first:PrimExpr rest:(__ DotOperator __ @Identifier)* {
      if (rest.length === 0) {
        return first
      }
      return new MemberAccess(first, rest, null, null, null, loc())
    }

PrimExpr
  = NestExpr
  / MatchExpr
  / VarRefExpr
  / IntegerExpression
  / StringLiteral
  / CharLiteral

WS "whitespace" = [\t\r\n ]
BL "blank space" = [\t\r ]

BlockComment
  = $('/*' (!'*/' (BlockComment / .))* '*/')

LineComment
  = $('//' (!EOLF .)*)

EOF "end of file" = !.
EOL "end of line"
  = "\n"
  / "\r\n"
  / "\r"
  / "\u2028"
  / "\u2029"
EOLF = EOL / EOF

Comment 
  = BlockComment
  / LineComment

__  = (WS / Comment)*
_   = (BL / BlockComment)*

