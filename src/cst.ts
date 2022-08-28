
export type TextSpan = [number, number];

export class TextPosition {

  public constructor(
    public offset: number,
    public line: number,
    public column: number,
  ) {

  }

  public clone(): TextPosition {
    return new TextPosition(
      this.offset,
      this.line,
      this.column,
    );
  }

  public advance(text: string): void {
    for (const ch of text) {
      if (ch === '\n') {
        this.line++;
        this.column = 1;
      } else {
        this.column++;
      }
      this.offset += text.length;
    }

  }

}

export class TextRange {

  constructor(
    public start: TextPosition,
    public end: TextPosition,
  ) {

  }

  public clone(): TextRange {
    return new TextRange(
      this.start.clone(),
      this.end.clone(),
    );
  }

}

export class TextFile {

  public constructor(
    public origPath: string,
    public text: string,
  ) {

  }

}

export const enum SyntaxKind {

  // Tokens
  Identifier,
  CustomOperator,
  LParen,
  RParen,
  LBrace,
  RBrace,
  LBracket,
  RBracket,
  Dot,
  DotDot,
  Comma,
  Colon,
  Equals,
  Integer,
  StringLiteral,
  LetKeyword,
  PubKeyword,
  MutKeyword,
  ModKeyword,
  ImportKeyword,
  StructKeyword,
  TypeKeyword,
  LineFoldEnd,
  BlockEnd,
  BlockStart,
  EndOfFile,

  // Type expressions
  ReferenceTypeExpression,

  // Patterns
  BindPattern,
  TuplePattern,

  // Expressions
  ReferenceExpression,
  TupleExpression,
  NestedExpression,
  ConstantExpression,
  PrefixExpression,
  PostfixExpression,
  InfixExpression,

  // Statements
  ReturnStatement,
  ExpressionStatement,

  // Declarations
  VariableDeclaration,
  PrefixFuncDecl,
  SuffixFuncDecl,
  LetDeclaration,
  StructDeclaration,
  ImportDeclaration,
  TypeAliasDeclaration,

  // Other nodes
  StructDeclarationField,
  ExprBody,
  BlockBody,
  TypeAssert,
  Param,
  Module,
  SourceFile,

}

export type Syntax
  = SourceFile
  | Param
  | StructDeclarationField
  | Declaration
  | Statement
  | Expression
  | TypeExpression
  | Pattern

abstract class SyntaxBase {

  public abstract readonly kind: SyntaxKind;

}

abstract class TokenBase extends SyntaxBase {

  private endPos: TextPosition | null = null;

  constructor(
    private startPos: TextPosition,
  ) {
    super();
  }

  public getStartPosition(): TextPosition {
    return this.startPos;
  }

  public getStartLine(): number {
    return this.getStartPosition().line;
  }

  public getStartColumn(): number {
    return this.getStartPosition().column;
  }

  public getEndPosition(): TextPosition {
    if (this.endPos === null) {
      const endPos = this.getStartPosition().clone();
      endPos.advance(this.text);
      return this.endPos = endPos;
    }
    return this.endPos;
  }

  public getEndLine(): number {
    return this.getEndPosition().line;
  }

  public getEndColumn(): number {
    return this.getEndPosition().column;
  }

  public abstract readonly text: string;

}

abstract class VirtualTokenBase extends TokenBase {
  public get text(): string {
    return '';
  }
}

export class EndOfFile extends VirtualTokenBase {
  public readonly kind = SyntaxKind.EndOfFile;
}

export class BlockEnd extends VirtualTokenBase {
  public readonly kind = SyntaxKind.BlockEnd;
}

export class BlockStart extends VirtualTokenBase {
  public readonly kind = SyntaxKind.BlockStart;
}

export class LineFoldEnd extends VirtualTokenBase {
  public readonly kind = SyntaxKind.LineFoldEnd;
}

export class Integer extends TokenBase {

  public readonly kind = SyntaxKind.Integer;

  public constructor(
    public value: bigint,
    public radix: number,
    private startPos: TextPosition,
  ) {
    super(startPos);
  }

  public get text(): string {
    switch (this.radix) {
      case 16:
        return '0x' + this.value.toString(16);
      case 10:
        return this.value.toString(10)
      case 8:
        return '0o' + this.value.toString(8)
      case 2:
        return '0b' + this.value.toString(2);
      default:
        throw new Error(`Radix ${this.radix} of Integer not recognised.`)
    }
  }

}

export class StringLiteral extends TokenBase {

  public readonly kind = SyntaxKind.StringLiteral;

  public constructor(
    public contents: string,
    private startPos: TextPosition,
  ) {
    super(startPos);
  }

  public get text(): string {
    let out = '"';
    for (const ch of this.contents) {
      const code = ch.charCodeAt(0);
      if (code >= 32 && code <= 127) {
        out += ch;
      } else if (code <= 127) {
        out += '\\x' + code.toString(16).padStart(2, '0');
      } else {
        out += '\\u' + code.toString(17).padStart(4, '0');
      }
    }
    out += '"';
    return out;
  }

}

export class Identifier extends TokenBase {

  public readonly kind = SyntaxKind.Identifier;

  public constructor(
    public text: string,
    private startPos: TextPosition,
  ) {
    super(startPos);
  }

}

export class CustomOperator extends TokenBase {

  public readonly kind = SyntaxKind.CustomOperator;

  public constructor(
    public text: string,
    private startPos: TextPosition,
  ) {
    super(startPos);
  }

}

export class LParen extends TokenBase {

  public readonly kind = SyntaxKind.LParen;

  public get text(): string {
    return '(';
  }

}

export class RParen extends TokenBase {

  public readonly kind = SyntaxKind.RParen;

  public get text(): string {
    return ')';
  }

}

export class LBrace extends TokenBase {

  public readonly kind = SyntaxKind.LBrace;

  public get text(): string {
    return '{';
  }

}

export class RBrace extends TokenBase {

  public readonly kind = SyntaxKind.RBrace;

  public get text(): string {
    return '}';
  }

}

export class LBracket extends TokenBase {

  public readonly kind = SyntaxKind.LBracket;

  public get text(): string {
    return '[';
  }

}

export class RBracket extends TokenBase {

  public readonly kind = SyntaxKind.RBracket;

  public get text(): string {
    return ']';
  }

}

export class Dot extends TokenBase {

  public readonly kind = SyntaxKind.Dot;

  public get text(): string {
    return '.';
  }

}

export class Comma extends TokenBase {

  public readonly kind = SyntaxKind.Comma;

  public get text(): string {
    return ',';
  }

}

export class DotDot extends TokenBase {

  public readonly kind = SyntaxKind.DotDot;

  public get text(): string {
    return '..';
  }

}

export class Colon extends TokenBase {

  public readonly kind = SyntaxKind.Colon;

  public get text(): string {
    return ':';
  }

}

export class Equals extends TokenBase {

  public readonly kind = SyntaxKind.Equals;

  public get text(): string {
    return '=';
  }

}

export class StructKeyword extends TokenBase {

  public readonly kind = SyntaxKind.StructKeyword;

  public get text(): string {
    return 'struct';
  }

}

export class ModKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ModKeyword;

  public get text(): string {
    return 'mod';
  }

}

export class MutKeyword extends TokenBase {

  public readonly kind = SyntaxKind.MutKeyword;

  public get text(): string {
    return 'mut';
  }

}

export class ImportKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ImportKeyword;

  public get text(): string {
    return 'import'
  }

}

export class TypeKeyword extends TokenBase {

  public readonly kind = SyntaxKind.TypeKeyword;

  public get text(): string {
    return 'type';
  }

}

export class PubKeyword extends TokenBase {

  public readonly kind = SyntaxKind.PubKeyword;

  public get text(): string {
    return 'pub';
  }

}

export class LetKeyword extends TokenBase {

  public readonly kind = SyntaxKind.LetKeyword;

  public get text(): string {
    return 'let';
  }

}

export type Token
  = LParen
  | RParen
  | LBrace
  | RBrace
  | LBracket
  | RBracket
  | Identifier
  | CustomOperator
  | Integer
  | StringLiteral
  | Comma
  | Dot
  | DotDot
  | Colon
  | Equals
  | LetKeyword
  | PubKeyword
  | MutKeyword
  | ModKeyword
  | ImportKeyword
  | TypeKeyword
  | StructKeyword
  | EndOfFile
  | BlockStart
  | BlockEnd
  | LineFoldEnd

export type TokenKind
  = Token['kind']

export class ReferenceTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ReferenceTypeExpression;

  public constructor(
    public modulePath: Array<[Identifier, Dot]>,
    public name: Identifier,
  ) {
    super();
  }

}

export type TypeExpression
  = ReferenceTypeExpression

export class BindPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.BindPattern;

  public constructor(
    public name: Identifier,
  ) {
    super();
  }

  public get isHole(): boolean {
    return this.name.text == '_';
  }

}

export class TuplePattern extends SyntaxBase {

  public readonly kind = SyntaxKind.TuplePattern;

  public constructor(
    public elements: Pattern[],
  ) {
    super();
  }

}

export type Pattern
  = BindPattern
  | TuplePattern

export class TupleExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.TupleExpression;

  public constructor(
    public lparen: LParen,
    public elements: Expression[],
    public rparen: RParen,
  ) {
    super();
  }

}

export class NestedExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.NestedExpression;

  public constructor(
    public lparen: LParen,
    public expression: Expression,
    public rparen: RParen,
  ) {
    super();
  }

}

export class ConstantExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ConstantExpression;

  public constructor(
    public token: Integer | StringLiteral,
  ) {
    super();
  }

}

export class ReferenceExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ReferenceExpression;

  public constructor(
    public modulePath: Array<[Identifier, Dot]>,
    public name: Identifier
  ) {
    super();
  }

}

export class PrefixExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.PrefixExpression;

  public constructor(
    public operator: Token,
    public expression: Expression,
  ) {
    super();
  }

}

export class PostfixExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.PostfixExpression;

  public constructor(
    public expression: Expression,
    public operator: Token,
  ) {
    super();
  }

}

export class InfixExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.InfixExpression;

  public constructor(
    public left: Expression,
    public operator: Token,
    public right: Expression,
  ) {
    super();
  }

}

export type Expression
  = ReferenceExpression
  | ConstantExpression
  | TupleExpression
  | NestedExpression
  | PrefixExpression
  | InfixExpression
  | PostfixExpression

export class ReturnStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ReturnStatement;

  public constructor(
    public expr: Expression
  ) {
    super();
  }

}

export class ExpressionStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ExpressionStatement;

  public constructor(
    public expresion: Expression,
  ) {
    super();
  }

}

export type Statement
  = ReturnStatement
  | ExpressionStatement

export class Param extends SyntaxBase {

  public readonly kind = SyntaxKind.Param;

  public constructor(
    public pattern: Pattern,
  ) {
    super();
  }

}

export class StructDeclarationField extends SyntaxBase {

  public readonly kind = SyntaxKind.StructDeclarationField;

  public constructor(
    public name: Identifier,
    public colon: Colon,
    public typeExpr: TypeExpression,
  ) {
    super();
  }

}

export class StructDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.StructDeclaration;

  public constructor(
    public structKeyword: StructKeyword,
    public name: Identifier,
    public members: StructDeclarationField[] | null,
  ) {
    super();
  }

}

export class TypeAssert extends SyntaxBase {

  public readonly kind = SyntaxKind.TypeAssert;

  public constructor(
    public colon: Colon,
    public typeExpression: TypeExpression,
  ) {
    super();
  }

}

export type Body 
  = ExprBody
  | BlockBody

export class ExprBody extends SyntaxBase {

  public readonly kind = SyntaxKind.ExprBody;

  public constructor(
    public equals: Equals,
    public expression: Expression,
  ) {
    super();
  }

}

export type LetBodyElement 
  = LetDeclaration
  | Statement

export class BlockBody extends SyntaxBase {

  public readonly kind = SyntaxKind.BlockBody;

  public constructor(
    public blockStart: BlockStart,
    public elements: LetBodyElement[],
  ) {
    super();
  }

}
export class LetDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.LetDeclaration;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public mutKeyword: MutKeyword | null,
    public pattern: Pattern,
    public params: Param[],
    public typeAssert: TypeAssert | null,
    public body: Body | null,
  ) {
    super();
  }

}

export class ImportDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.ImportDeclaration;

  public constructor(
    public importKeyword: ImportKeyword,
    public importSource: StringLiteral,
  ) {
    super();
  }

}

export type Declaration
  = LetDeclaration
  | ImportDeclaration
  | StructDeclaration

export class Module extends SyntaxBase {

  public readonly kind = SyntaxKind.Module;

  public constructor(
    public modKeyword: ModKeyword,
    public name: Identifier,
    public body: Body,
  ) {
    super();
  }

}

export type SourceFileElement
  = Statement
  | Declaration
  | Module

export class SourceFile extends SyntaxBase {

  public readonly kind = SyntaxKind.SourceFile;

  public constructor(
    public elements: SourceFileElement[]
  ) {
    super();
  }

  public *getChildNodes(): Iterable<Syntax> {
    for (const element in this.elements) {
      yield element;
    }
  }

}
