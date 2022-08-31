import { JSONObject, JSONValue } from "./util";
import type { InferContext, Type } from "./checker"

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
  Constructor,
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
  ReturnKeyword,
  MatchKeyword,
  IfKeyword,
  ElifKeyword,
  ElseKeyword,
  LineFoldEnd,
  BlockEnd,
  BlockStart,
  EndOfFile,

  // Type expressions
  ReferenceTypeExpression,

  // Patterns
  BindPattern,
  TuplePattern,
  StructPattern,
  NestedPattern,
  NamedTuplePattern,

  // Struct pattern elements
  FieldStructPatternElement,
  PunnedFieldStructPatternElement,
  VariadicStructPatternElement,

  // Expressions
  CallExpression,
  ReferenceExpression,
  NamedTupleExpression,
  StructExpression,
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

  // Let declaration body members
  ExprBody,
  BlockBody,

  // Structure declaration members
  StructDeclarationField,

  // Other nodes
  Initializer,
  QualifiedName,
  TypeAssert,
  Param,
  Module,
  SourceFile,

}

export type Syntax
  = SourceFile
  | Module
  | Token
  | Param
  | Body
  | StructDeclarationField
  | Declaration
  | Statement
  | Expression
  | TypeExpression
  | Pattern

function isIgnoredProperty(key: string): boolean {
  return key === 'kind' || key === 'parent';
}

abstract class SyntaxBase {

  public parent: Syntax | null = null;

  public abstract readonly kind: SyntaxKind;

  public abstract getFirstToken(): Token;

  public abstract getLastToken(): Token;

  public getRange(): TextRange {
    return new TextRange(
      this.getFirstToken().getStartPosition(),
      this.getLastToken().getEndPosition(),
    );
  }

  public getSourceFile(): SourceFile {
    let curr = this as any;
    do {
      if (curr.kind === SyntaxKind.SourceFile) {
        return curr;
      }
      curr = curr.parent;
    } while (curr != null);
    throw new Error(`Could not find a SourceFile in any of the parent nodes of ${this}`);
  }

  public setParents(): void {

    const visit = (value: any) => {
      if (value === null) {
        return;
      }
      if (Array.isArray(value)) {
        value.forEach(visit);
        return;
      }
      if (value instanceof SyntaxBase) {
        value.parent = this as any;
        value.setParents();
        return;
      }
    }

    for (const key of Object.getOwnPropertyNames(this)) {
      if (isIgnoredProperty(key)) {
        continue;
      }
      visit((this as any)[key]);
    }

  }

  public toJSON(): JSONObject {

    const obj: JSONObject = {};

    obj['type'] = this.constructor.name;

    for (const key of Object.getOwnPropertyNames(this)) {
      if (isIgnoredProperty(key)) {
        continue;
      }
      obj[key] = encode((this as any)[key]);
    }

    return obj;

    function encode(value: any): JSONValue {
      if (value === null) {
        return null;
      } else if (Array.isArray(value)) {
        return value.map(encode);
      } else if (value instanceof SyntaxBase) {
        return value.toJSON();
      } else {
        return value;
      }
    }

  }

}

abstract class TokenBase extends SyntaxBase {

  private endPos: TextPosition | null = null;

  public constructor(
    private startPos: TextPosition,
  ) {
    super();
  }

  public getFirstToken(): Token {
    throw new Error(`Trying to get the first token of an object that is a token itself.`);
  }

  public getLastToken(): Token {
    throw new Error(`Trying to get the last token of an object that is a token itself.`);
  }

  public getRange(): TextRange {
    return new TextRange(
      this.getStartPosition(),
      this.getEndPosition(),
    );
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
    startPos: TextPosition,
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
    startPos: TextPosition,
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

export class Constructor extends TokenBase {

  public readonly kind = SyntaxKind.Constructor;

  public constructor(
    public text: string,
    startPos: TextPosition,
  ) {
    super(startPos);
  }

}

export class Identifier extends TokenBase {

  public readonly kind = SyntaxKind.Identifier;

  public constructor(
    public text: string,
    startPos: TextPosition,
  ) {
    super(startPos);
  }

}

export class CustomOperator extends TokenBase {

  public readonly kind = SyntaxKind.CustomOperator;

  public constructor(
    public text: string,
    startPos: TextPosition,
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

export class ReturnKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ReturnKeyword;

  public get text(): string {
    return 'return';
  }

}

export class MatchKeyword extends TokenBase {

  public readonly kind = SyntaxKind.MatchKeyword;

  public get text(): string {
    return 'match';
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
  | Constructor
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
  | ReturnKeyword
  | MatchKeyword
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

  public getFirstToken(): Token {
    if (this.modulePath.length > 0) {
      return this.modulePath[0][0];
    }
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
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

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export class TuplePattern extends SyntaxBase {

  public readonly kind = SyntaxKind.TuplePattern;

  public constructor(
    public lparen: LParen,
    public elements: Pattern[],
    public rparen: RParen,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export class NamedTuplePattern extends SyntaxBase {

  public readonly kind = SyntaxKind.NamedTuplePattern;

  public constructor(
    public name: Constructor,
    public elements: Pattern[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.name;
  }

}

export class FieldStructPatternElement extends SyntaxBase {

  public readonly kind = SyntaxKind.FieldStructPatternElement;

  public constructor(
    public name: Identifier,
    public equals: Equals,
    public pattern: Pattern,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.pattern.getLastToken();
  }

}

export class VariadicStructPatternElement extends SyntaxBase {

  public readonly kind = SyntaxKind.VariadicStructPatternElement;

  public constructor(
    public dotdot: DotDot,
    public pattern: Pattern | null,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.dotdot;
  }

  public getLastToken(): Token {
    if (this.pattern !== null) {
      return this.pattern.getLastToken();
    }
    return this.dotdot;
  }

}

export class PunnedFieldStructPatternElement extends SyntaxBase {

  public readonly kind = SyntaxKind.PunnedFieldStructPatternElement;

  public constructor(
    public name: Identifier,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export type StructPatternElement
  = VariadicStructPatternElement
  | PunnedFieldStructPatternElement
  | FieldStructPatternElement

export class StructPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.StructPattern;

  public constructor(
    public name: Constructor,
    public lbrace: LBrace,
    public elements: StructPatternElement[],
    public rbrace: RBrace,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.rbrace;
  }

}

export class NestedPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.NestedPattern;

  public constructor(
    public lparen: LParen,
    public pattern: Pattern,
    public rparen: RParen,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export type Pattern
  = BindPattern
  | NestedPattern
  | StructPattern
  | NamedTuplePattern
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

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
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

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export class ConstantExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ConstantExpression;

  public constructor(
    public token: Integer | StringLiteral,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.token;
  }

  public getLastToken(): Token {
    return this.token;
  }

}

export class QualifiedName extends SyntaxBase {

  public readonly kind = SyntaxKind.QualifiedName;

  public constructor(
    public modulePath: Array<[Identifier, Dot]>,
    public name: Identifier,
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.modulePath.length > 0) {
      return this.modulePath[0][0];
    }
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export class CallExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.CallExpression;

  public constructor(
    public func: Expression,
    public args: Expression[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.func.getFirstToken();
  }

  public getLastToken(): Token {
    if (this.args.length > 0) {
      return this.args[this.args.length-1].getLastToken();
    }
    return this.func.getLastToken();
  }

}

export class NamedTupleExpression extends SyntaxBase {
  
  public readonly kind = SyntaxKind.NamedTupleExpression;

  public constructor(
    public name: Constructor,
    public elements: Expression[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.name;
  }

}

export class ReferenceExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ReferenceExpression;

  public constructor(
    public name: QualifiedName,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name.getFirstToken();
  }

  public getLastToken(): Token {
     return this.name.getLastToken();
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

  public getFirstToken(): Token {
    return this.operator;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
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

  public getFirstToken(): Token {
    return this.expression.getFirstToken();
  }

  public getLastToken(): Token {
    return this.operator;
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

  public getFirstToken(): Token {
    return this.left.getFirstToken();
  }

  public getLastToken(): Token {
    return this.right.getLastToken();
  }

}

export type Expression
  = CallExpression
  | NamedTupleExpression
  | ReferenceExpression
  | ConstantExpression
  | TupleExpression
  | NestedExpression
  | PrefixExpression
  | InfixExpression
  | PostfixExpression

export class ReturnStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ReturnStatement;

  public constructor(
    public returnKeyword: ReturnKeyword,
    public expression: Expression
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.returnKeyword;
  }

  public getLastToken(): Token {
    if (this.expression !== null) {
      return this.expression.getLastToken();
    }
    return this.returnKeyword;
  }

}

export class ExpressionStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ExpressionStatement;

  public constructor(
    public expression: Expression,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.expression.getFirstToken();
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
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

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.pattern.getLastToken();
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

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.typeExpr.getLastToken();
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

  public getFirstToken(): Token {
    return this.structKeyword;
  }

  public getLastToken(): Token {
    if (this.members && this.members.length > 0) {
      return this.members[this.members.length-1].getLastToken();
    }
    return this.name;
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

  public getFirstToken(): Token {
    return this.colon;
  }

  public getLastToken(): Token {
    return this.typeExpression.getLastToken();
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

  public getFirstToken(): Token {
    return this.equals;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
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

  public getFirstToken(): Token {
    return this.blockStart;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.blockStart;
  }

}
export class LetDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.LetDeclaration;

  public type?: Type;
  public context?: InferContext;

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

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.letKeyword;
  }

  public getLastToken(): Token {
    if (this.body !== null) {
      return this.body.getLastToken();
    }
    if (this.typeAssert !== null) {
      return this.typeAssert.getLastToken();
    }
    if (this.params.length > 0) {
      return this.params[this.params.length-1].getLastToken();
    }
    return this.pattern.getLastToken();
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

  public getFirstToken(): Token {
     return this.importKeyword;
  }

  public getLastToken(): Token {
    return this.importSource;
  }

}

export type Declaration
  = LetDeclaration
  | ImportDeclaration
  | StructDeclaration

export class Initializer extends SyntaxBase {

  public readonly kind = SyntaxKind.Initializer;

  public constructor(
    public equals: Equals,
    public expression: Expression
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.equals;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

export class Module extends SyntaxBase {

  public readonly kind = SyntaxKind.Module;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public modKeyword: ModKeyword,
    public name: Identifier,
    public body: Body,
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.modKeyword;
  }

  public getLastToken(): Token {
    return this.body.getLastToken();
  }

}

export type SourceFileElement
  = Statement
  | Declaration
  | Module

export class SourceFile extends SyntaxBase {

  public readonly kind = SyntaxKind.SourceFile;

  public constructor(
    private file: TextFile,
    public elements: SourceFileElement[],
    public eof: EndOfFile,
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[0].getFirstToken();
    }
    return this.eof;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.eof;
  }

  public getFile() {
    return this.file;
  }

}
