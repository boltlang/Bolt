import { JSONObject, JSONValue } from "./util";
import type { InferContext, Type, TypeEnv } from "./checker"

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
  IdentifierAlt,
  CustomOperator,
  Assignment,
  LParen,
  RParen,
  LBrace,
  RBrace,
  LBracket,
  RBracket,
  RArrow,
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
  EnumKeyword,
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
  ArrowTypeExpression,
  VarTypeExpression,
  AppTypeExpression,

  // Patterns
  BindPattern,
  TuplePattern,
  StructPattern,
  NestedPattern,
  NamedTuplePattern,

  // Struct expression elements
  StructExpressionField,
  PunnedStructExpressionField,

  // Struct pattern elements
  StructPatternField,
  PunnedStructPatternField,
  VariadicStructPatternElement,

  // Expressions
  MemberExpression,
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
  IfStatement,

  // If statement elements
  IfStatementCase,

  // Declarations
  LetDeclaration,
  StructDeclaration,
  EnumDeclaration,
  ImportDeclaration,
  TypeDeclaration,

  // Let declaration body members
  ExprBody,
  BlockBody,

  // Structure declaration members
  StructDeclarationField,

  // Enum declaration elements
  EnumDeclarationStructElement,
  EnumDeclarationTupleElement,

  // Other nodes
  WrappedOperator,
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
  | EnumDeclarationElement
  | TypeAssert
  | Declaration
  | Statement
  | Expression
  | TypeExpression
  | Pattern
  | StructExpressionElement
  | StructPatternElement

function isIgnoredProperty(key: string): boolean {
  return key === 'kind' || key === 'parent';
}

type NodeWithScope
  = SourceFile
  | LetDeclaration

function isNodeWithScope(node: Syntax): node is NodeWithScope {
  return node.kind === SyntaxKind.SourceFile
      || node.kind === SyntaxKind.LetDeclaration;
}

export const enum Symkind {
  Var = 1,
  Type = 2,
  Constructor = 4,
  Any = Var | Type | Constructor
}

export class Scope {

  private mapping = new Map<string, [Symkind, Syntax]>();

  public constructor(
    public node: NodeWithScope,
  ) {
    this.scan(node);
  }

  private getParent(): Scope | null {
    let curr = this.node.parent;
    while (curr !== null) {
      if (isNodeWithScope(curr)) {
        return curr.getScope();
      }
      curr = curr.parent;
    }
    return null;
  }

  private add(name: string, node: Syntax, kind: Symkind): void {
    this.mapping.set(name, [kind, node]);
  }

  private scan(node: Syntax): void {
    switch (node.kind) {
      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.scan(element);
        }
        break;
      }
      case SyntaxKind.ExpressionStatement:
      case SyntaxKind.ReturnStatement:
      case SyntaxKind.IfStatement:
        break;
      case SyntaxKind.TypeDeclaration:
      {
        this.add(node.name.text, node, Symkind.Type);
        break;
      }
      case SyntaxKind.EnumDeclaration:
      case SyntaxKind.StructDeclaration:
      {
        this.add(node.name.text, node, Symkind.Constructor);
        break;
      }
      case SyntaxKind.LetDeclaration:
      {
        for (const param of node.params) {
          this.scanPattern(param.pattern, param);
        }
        if (node === this.node) {
          if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
            for (const element of node.body.elements) {
              this.scan(element);
            }
          }
        } else {
          if (node.pattern.kind === SyntaxKind.WrappedOperator) {
            this.add(node.pattern.operator.text, node, Symkind.Var);
          } else {
            this.scanPattern(node.pattern, node);
          }
        }
        break;
      }
      default:
        throw new Error(`Unexpected ${node.constructor.name}`);
    }
  }

  private scanPattern(node: Pattern, decl: Syntax): void {
    switch (node.kind) {
      case SyntaxKind.BindPattern:
      {
        this.add(node.name.text, decl, Symkind.Var);
        break;
      }
      case SyntaxKind.StructPattern:
      {
        for (const member of node.members) {
          switch (member.kind) {
            case SyntaxKind.StructPatternField:
            {
              this.scanPattern(member.pattern, decl);
              break;
            }
            case SyntaxKind.PunnedStructPatternField:
            {
              this.add(node.name.text, decl, Symkind.Var);
              break;
            }
          }
        }
        break;
      }
      default:
        throw new Error(`Unexpected ${node}`);
    }
  }

  public lookup(name: string, expectedKind = Symkind.Any): Syntax | null {
    let curr: Scope | null = this;
    do {
      const match = curr.mapping.get(name);
      if (match !== undefined) {
        const [kind, decl] = match;
        if (kind & expectedKind) {
          return decl;
        }
      }
      curr = curr.getParent();
    } while (curr !== null);
    return null;
  }

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

  public getScope(): Scope {
    let curr: Syntax | null = this as any;
    do {
      if (isNodeWithScope(curr!)) {
        if (curr.scope === undefined) {
          curr.scope = new Scope(curr);
        }
        return curr.scope;
      }
      curr = curr!.parent;
    } while (curr !== null);
    throw new Error(`Could not find a scope for ${this}. Maybe the parent links are not set?`);
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

export function forEachChild(node: Syntax, callback: (node: Syntax) => void): void {

  for (const key of Object.getOwnPropertyNames(node)) {
    if (isIgnoredProperty(key)) {
      continue;
    }
    visitField((node as any)[key]);
  }

  function visitField(field: any): void {
    if (field === null) {
      return;
    }
    if (Array.isArray(field)) {
      for (const element of field) {
        visitField(element);
      }
      return;
    }
    if (field instanceof SyntaxBase) {
      callback(field as Syntax);
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

export class IdentifierAlt extends TokenBase {

  public readonly kind = SyntaxKind.IdentifierAlt;

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

export class Assignment extends TokenBase {

  public readonly kind = SyntaxKind.Assignment;

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

export class IfKeyword extends TokenBase {

  public readonly kind = SyntaxKind.IfKeyword;

  public get text(): string {
    return 'if';
  }

}

export class ElseKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ElseKeyword;

  public get text(): string {
    return 'else';
  }

}

export class ElifKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ElifKeyword;

  public get text(): string {
    return 'elif';
  }

}

export class StructKeyword extends TokenBase {

  public readonly kind = SyntaxKind.StructKeyword;

  public get text(): string {
    return 'struct';
  }

}

export class EnumKeyword extends TokenBase {

  public readonly kind = SyntaxKind.EnumKeyword;

  public get text(): string {
    return 'enum';
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

export class RArrow extends TokenBase {

  public readonly kind = SyntaxKind.RArrow;

  public get text(): string {
    return '->';
  }

}

export type Token
  = RArrow
  | LParen
  | RParen
  | LBrace
  | RBrace
  | LBracket
  | RBracket
  | Identifier
  | IdentifierAlt
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
  | Assignment
  | IfKeyword
  | ElseKeyword
  | ElifKeyword
  | EnumKeyword

export type TokenKind
  = Token['kind']

export class ArrowTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ArrowTypeExpression;

  public constructor(
    public paramTypeExprs: TypeExpression[],
    public returnTypeExpr: TypeExpression
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.paramTypeExprs.length > 0) {
      return this.paramTypeExprs[0].getFirstToken();
    }
    return this.returnTypeExpr.getFirstToken();
  }

  public getLastToken(): Token {
    return this.returnTypeExpr.getLastToken();
  }

}

export class ReferenceTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ReferenceTypeExpression;

  public constructor(
    public modulePath: Array<[Identifier, Dot]>,
    public name: IdentifierAlt,
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

export class AppTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.AppTypeExpression;

  public constructor(
    public operator: TypeExpression,
    public args: TypeExpression[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.operator.getFirstToken();
  }

  public getLastToken(): Token {
    if (this.args.length > 0) {
      return this.args[this.args.length-1].getLastToken();
    }
    return this.operator.getLastToken();
  }

}

export class VarTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.VarTypeExpression;

  public constructor(
    public name: Identifier
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

export type TypeExpression
  = ReferenceTypeExpression
  | ArrowTypeExpression
  | VarTypeExpression
  | AppTypeExpression

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
    public name: IdentifierAlt,
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

export class StructPatternField extends SyntaxBase {

  public readonly kind = SyntaxKind.StructPatternField;

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

export class PunnedStructPatternField extends SyntaxBase {

  public readonly kind = SyntaxKind.PunnedStructPatternField;

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
  | PunnedStructPatternField
  | StructPatternField

export class StructPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.StructPattern;

  public constructor(
    public name: IdentifierAlt,
    public lbrace: LBrace,
    public members: StructPatternElement[],
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
    public modulePath: Array<[IdentifierAlt, Dot]>,
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

export class StructExpressionField extends SyntaxBase {

  public readonly kind = SyntaxKind.StructExpressionField;

  public constructor(
    public name: Identifier,
    public equals: Equals,
    public expression: Expression,
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

export class PunnedStructExpressionField extends SyntaxBase {

  public readonly kind = SyntaxKind.PunnedStructExpressionField;

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

export type StructExpressionElement
  = StructExpressionField
  | PunnedStructExpressionField;

export class StructExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.StructExpression;

  public constructor(
    public name: IdentifierAlt,
    public lbrace: LBrace,
    public members: StructExpressionElement[],
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

export class NamedTupleExpression extends SyntaxBase {
  
  public readonly kind = SyntaxKind.NamedTupleExpression;

  public constructor(
    public name: IdentifierAlt,
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

export class MemberExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.MemberExpression;

  public constructor(
    public expression: Expression,
    public path: [Dot, Identifier][],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.expression.getFirstToken();
  }

  public getLastToken(): Token {
    return this.path[this.path.length-1][1];
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
  = MemberExpression
  | CallExpression
  | StructExpression
  | NamedTupleExpression
  | ReferenceExpression
  | ConstantExpression
  | TupleExpression
  | NestedExpression
  | PrefixExpression
  | InfixExpression
  | PostfixExpression

export class IfStatementCase extends SyntaxBase {

  public readonly kind = SyntaxKind.IfStatementCase;

  public constructor(
    public keyword: IfKeyword | ElseKeyword | ElifKeyword,
    public test: Expression | null,
    public blockStart: BlockStart,
    public elements: LetBodyElement[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.keyword;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.blockStart;
  }

}

export class IfStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.IfStatement;

  public constructor(
    public cases: IfStatementCase[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.cases[0].getFirstToken();
  }

  public getLastToken(): Token {
    return this.cases[this.cases.length-1].getLastToken();
  }

}

export class ReturnStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ReturnStatement;

  public constructor(
    public returnKeyword: ReturnKeyword,
    public expression: Expression | null
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
  | IfStatement

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

export class EnumDeclarationStructElement extends SyntaxBase {

  public readonly kind = SyntaxKind.EnumDeclarationStructElement;

  public constructor(
    public name: IdentifierAlt,
    public blockStart: BlockStart,
    public members: StructDeclarationField[],
  ) {
    super();
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    if (this.members.length > 0) {
      return this.members[this.members.length-1].getLastToken();
    }
    return this.blockStart;
  }

}

export class EnumDeclarationTupleElement extends SyntaxBase {

  public readonly kind = SyntaxKind.EnumDeclarationTupleElement;

  public constructor(
    public name: IdentifierAlt,
    public elements: TypeExpression[],
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

export type EnumDeclarationElement
  = EnumDeclarationStructElement
  | EnumDeclarationTupleElement

export class EnumDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.EnumDeclaration;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public enumKeyword: EnumKeyword,
    public name: IdentifierAlt,
    public members: EnumDeclarationElement[] | null,
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.enumKeyword;
  }

  public getLastToken(): Token {
    if (this.members !== null && this.members.length > 0) {
      return this.members[this.members.length-1].getLastToken();
    }
    return this.name;
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
    public pubKeyword: PubKeyword | null,
    public structKeyword: StructKeyword,
    public name: IdentifierAlt,
    public typeVars: Identifier[],
    public members: StructDeclarationField[] | null,
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
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

export class WrappedOperator extends SyntaxBase {

  public readonly kind = SyntaxKind.WrappedOperator;

  public constructor(
    public lparen: LParen,
    public operator: CustomOperator,
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

export class TypeDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.TypeDeclaration;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public typeKeyword: TypeKeyword,
    public name: IdentifierAlt,
    public typeVars: Identifier[],
    public equals: Equals,
    public typeExpression: TypeExpression
  ) {
    super();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.typeKeyword;
  }

  public getLastToken(): Token {
    return this.typeExpression.getLastToken();
  }

}

export class LetDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.LetDeclaration;

  public scope?: Scope;
  public type?: Type;
  public active?: boolean;
  public typeEnv?: TypeEnv;
  public context?: InferContext;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public mutKeyword: MutKeyword | null,
    public pattern: Pattern | WrappedOperator,
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
  | EnumDeclaration
  | TypeDeclaration

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

  public scope?: Scope;
  public typeEnv?: TypeEnv;

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
