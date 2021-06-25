
export interface TextPosition {
  offset: number;
  line: number;
  column: number;
}

export type TextRange = [TextPosition, TextPosition];

export type TextSpan = [number, number];

export enum SyntaxKind {

  // Other nodes
  MacroInvocation,
  SourceFile,

  // Purely syntactic nodes
  QualName,
  Paramater,
  TypeParameter,
  RecordDeclarationField,

  // Keywords
  ReturnKeyword,
  StructKeyword,
  TypeKeyword,
  ImportKeyword,
  PubKeyword,
  MutKeyword,
  LetKeyword,
  PerformKeyword,
  YieldKeyword,
  ResumeKeyword,

  // Other tokens
  EndOfFile,
  EndOfIndent,
  EndOfLine,
  Identifier,
  CustomOperator,
  DotSign,
  DotDotSign,
  ColonSign,
  EqualSign,
  LBracket,
  RBracket,
  LBrace,
  RBrace,
  LParen,
  RParen,

  // Expressions
  ReferenceExpression,

  // Statements
  ReturnStatement,
  ExpressionStatement,

  // Type expressions
  TypeReferenceExpression,

  // Declarations
  RecordDeclaration,

}

export type Syntax
  = SourceFile
  | ReturnKeyword
  | StructKeyword
  | TypeKeyword
  | ImportKeyword
  | PubKeyword
  | MutKeyword
  | LetKeyword
  | PerformKeyword
  | YieldKeyword
  | ResumeKeyword
  | Identifier
  | ReferenceExpression
  | ReturnStatement
  | ExpressionStatement

export interface SyntaxBase {
  readonly id: number;
  readonly kind: SyntaxKind;
  span: TextSpan | null;
}

let nextNodeId = 0;

function createNodeObject(kind: SyntaxKind, span: TextSpan | null): SyntaxBase {
  const obj = {} as SyntaxBase;
  Object.defineProperties(obj, {
    id: { value: nextNodeId++ },
    kind: { value: kind },
    span: {
      value: span,
      writable: true,
    },
  });
  return obj;
}

export type Token
  = EndOfFile
  | EndOfIndent
  | EndOfLine
  | Identifier
  | StructKeyword
  | ReturnKeyword
  | ImportKeyword
  | PubKeyword
  | LetKeyword
  | MutKeyword
  | TypeKeyword
  | PerformKeyword
  | ResumeKeyword
  | YieldKeyword
  | DotSign
  | DotDotSign
  | ColonSign
  | EqualSign
  | LBracket
  | RBracket
  | LBrace
  | RBrace
  | LParen
  | RParen

export type TokenSyntaxKind
  = Token['kind'];

interface TokenBase {
  readonly id: number;
  readonly kind: SyntaxKind;
  indentLevel: number;
  range: TextRange | null;
  getStartPos(): TextPosition;
  getStartLine(): number;
  getStartColumn(): number;
  getEndPos(): TextPosition;
  getEndLine(): number;
  getEndColumn(): number;
}

const tokenPrototype = {

  getStartPos(this: TokenBase): TextPosition {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range[0];
  },

  getStartLine(this: TokenBase) {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range[0].line;
  },

  getStartColumn(this: TokenBase) {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range[0].column;
  },

  getEndPos(this: TokenBase): TextPosition {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range[1];
  },

  getEndLine(this: TokenBase) {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range[1].line;
  },

  getEndColumn(this: TokenBase) {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range[1].column;
  },

}

function createTokenObject(kind: TokenSyntaxKind): TokenBase {
  const token = Object.create(tokenPrototype) as TokenBase;
  Object.defineProperties(token, {
    id: { value: nextNodeId++ },
    kind: { value: kind },
  });
  return token;
}

const TOKEN_TEXT: Partial<Record<SyntaxKind, string>> = {
  [SyntaxKind.LetKeyword]: 'let',
  [SyntaxKind.PubKeyword]: 'pub',
  [SyntaxKind.MutKeyword]: 'mut',
  [SyntaxKind.PerformKeyword]: 'perform',
  [SyntaxKind.YieldKeyword]: 'yield',
  [SyntaxKind.ResumeKeyword]: 'resume',
  [SyntaxKind.StructKeyword]: 'struct',
  [SyntaxKind.DotSign]: '.',
  [SyntaxKind.DotDotSign]: '..',
  [SyntaxKind.ColonSign]: ':',
  [SyntaxKind.EqualSign]: '=',
  [SyntaxKind.LBracket]: '{',
  [SyntaxKind.RBracket]: '}',
  [SyntaxKind.LBrace]: '{',
  [SyntaxKind.RBrace]: '}',
  [SyntaxKind.LParen]: '(',
  [SyntaxKind.RParen]: ')',
}

export function getTokenText(token: Token) {
  if (token.kind in TOKEN_TEXT) {
    return TOKEN_TEXT[token.kind];
  }
  switch (token.kind) {
    case SyntaxKind.Identifier:
      return token.text;
    case SyntaxKind.EndOfFile:
      return '';
    default:
      throw new Error(`Unhandled Token.kind value ${token.kind}`);
  }
}

export function describeToken(kind: SyntaxKind) {
  if (kind in TOKEN_TEXT) {
    return `'${TOKEN_TEXT[kind]}'`
  }
  switch (kind) {
    case SyntaxKind.Identifier: return 'an identifier';
    case SyntaxKind.CustomOperator: return 'an operator';
    case SyntaxKind.EndOfLine: return 'end-of-line';
    case SyntaxKind.EndOfFile : return 'end-of-file';
    case SyntaxKind.EndOfIndent: return 'the ending of an indented block';
    default:
      throw new Error(`Unhandled SyntaxKind value ${kind}`);
  }
}

export interface Identifier extends TokenBase {
  readonly kind: SyntaxKind.Identifier;
  text: string;
}

export function createIdentifier(
  text: string,
  indentLevel: number,
  range: TextRange | null = null,
): Identifier {
  const token = createTokenObject(SyntaxKind.Identifier) as Identifier;
  token.text = text;
  token.indentLevel = indentLevel;
  token.range = range;
  Object.seal(token);
  return token;
}

interface SimpleToken<K extends SyntaxKind> extends TokenBase {
  kind: K;
}

export function createSimpleToken<K extends TokenSyntaxKind>(
  kind: K,
  indentLevel: number,
  range: TextRange | null = null,
): SimpleToken<K> {
  const token = createTokenObject(kind) as SimpleToken<K>;
  token.indentLevel = indentLevel;
  token.range = range;
  Object.seal(token);
  return token;
}

export type EndOfFile = SimpleToken<SyntaxKind.EndOfFile>;
export type EndOfLine = SimpleToken<SyntaxKind.EndOfLine>;
export type EndOfIndent = SimpleToken<SyntaxKind.EndOfIndent>;

export type DotSign = SimpleToken<SyntaxKind.DotSign>;
export type DotDotSign = SimpleToken<SyntaxKind.DotDotSign>;
export type ColonSign = SimpleToken<SyntaxKind.ColonSign>;
export type EqualSign = SimpleToken<SyntaxKind.EqualSign>;
export type LBracket = SimpleToken<SyntaxKind.LBracket>;
export type RBracket = SimpleToken<SyntaxKind.RBracket>;
export type LParen = SimpleToken<SyntaxKind.LParen>;
export type RParen = SimpleToken<SyntaxKind.RParen>;
export type LBrace = SimpleToken<SyntaxKind.LBrace>;
export type RBrace = SimpleToken<SyntaxKind.RBrace>;

export type ReturnKeyword = SimpleToken<SyntaxKind.ReturnKeyword>;
export type StructKeyword = SimpleToken<SyntaxKind.StructKeyword>;
export type ImportKeyword = SimpleToken<SyntaxKind.ImportKeyword>;
export type PubKeyword = SimpleToken<SyntaxKind.PubKeyword>;
export type LetKeyword = SimpleToken<SyntaxKind.LetKeyword>;
export type MutKeyword = SimpleToken<SyntaxKind.MutKeyword>;
export type TypeKeyword = SimpleToken<SyntaxKind.TypeKeyword>;
export type PerformKeyword = SimpleToken<SyntaxKind.PerformKeyword>;
export type ResumeKeyword = SimpleToken<SyntaxKind.ResumeKeyword>;
export type YieldKeyword = SimpleToken<SyntaxKind.YieldKeyword>;

export interface QualName extends SyntaxBase {
  readonly kind: SyntaxKind.QualName;
  isAbsolute: boolean;
  modulePath: Identifier[];
  name: Identifier;
}

export function createQualName(
  isAbsolute: boolean = false,
  modulePath: Identifier[] = [],
  name: Identifier,
  span: TextSpan | null = null,
): QualName {
  const obj = createNodeObject(SyntaxKind.QualName, span) as QualName;
  obj.isAbsolute = isAbsolute;
  obj.modulePath = modulePath;
  obj.name = name;
  Object.seal(obj);
  return obj;
}

export type Expression
  = ReferenceExpression

export interface ReferenceExpression extends SyntaxBase {
  readonly kind: SyntaxKind.ReferenceExpression;
  name: Identifier;
}

export interface ReturnStatement extends SyntaxBase {
  readonly kind: SyntaxKind.ReturnStatement;
  returnKeyword: ReturnKeyword;
  expression: Expression;
}

export interface ExpressionStatement extends SyntaxBase {
  readonly kind: SyntaxKind.ExpressionStatement;
  expression: Expression;
}

export type Statement
  = ReturnStatement

export type TypeExpression
  = TypeReferenceExpression

export interface TypeReferenceExpression extends SyntaxBase {
  readonly kind: SyntaxKind.TypeReferenceExpression;
  name: QualName;
}

export interface TypeParameter extends SyntaxBase {
  readonly kind: SyntaxKind.TypeParameter;
  name: Identifier;
}

export function createTypeParameter(
  name: Identifier,
  span: TextSpan | null = null
): TypeParameter {
  const obj = createNodeObject(SyntaxKind.TypeParameter, span) as TypeParameter;
  obj.name = name;
  Object.seal(obj);
  return obj;
}

export interface Block<T> {
  dotSign: DotSign;
  elements: T[];
}

export interface RecordDeclaration extends SyntaxBase {
  readonly kind: SyntaxKind.RecordDeclaration;
  pubKeyword: PubKeyword | null;
  structKeyword: StructKeyword;
  name: Identifier;
  typeParams: TypeParameter[];
  body: Block<RecordDeclarationElement> | null;
}

export function createRecordDeclaration(
  pubKeyword: PubKeyword | null = null,
  structKeyword: StructKeyword,
  name: Identifier,
  typeParams: TypeParameter[] = [],
  body: Block<RecordDeclarationElement> | null = null,
  span: TextSpan | null = null
): RecordDeclaration {
  const obj = createNodeObject(SyntaxKind.RecordDeclaration, span) as RecordDeclaration;
  obj.pubKeyword = pubKeyword;
  obj.structKeyword = structKeyword;
  obj.name = name;
  obj.typeParams = typeParams;
  obj.body = body;
  Object.seal(obj);
  return obj;
}

export type RecordDeclarationElement
  = RecordDeclarationField
  | MacroInvocation

export interface RecordDeclarationField extends SyntaxBase {
  readonly kind: SyntaxKind.RecordDeclarationField;
  name: Identifier;
  colonSign: ColonSign;
  typeExpr: TypeExpression;
}

export function createRecordDeclarationField(
  name: Identifier,
  colonSign: ColonSign,
  typeExpr: TypeExpression,
  span: TextSpan | null = null,
): RecordDeclarationField {
  const obj = createNodeObject(SyntaxKind.RecordDeclarationField, span) as RecordDeclarationField;
  obj.name = name;
  obj.colonSign = colonSign;
  obj.typeExpr = typeExpr;
  Object.seal(obj);
  return obj;
}

export interface MacroInvocation extends SyntaxBase {
  readonly kind: SyntaxKind.MacroInvocation;
  text: string;
}

export type SourceElement
  = Statement
  | RecordDeclaration

export interface SourceFile extends SyntaxBase {
  elements: SourceElement[];
}

export function createSourceFile(
  elements: SourceElement[],
  span: TextSpan | null = null
): SourceFile {
  const obj = createNodeObject(SyntaxKind.SourceFile, span) as SourceFile;
  obj.elements = elements;
  Object.seal(obj);
  return obj;
}

