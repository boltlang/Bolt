
import type stream from "stream";
import path from "path"

import { assert, deserializable, implementationLimitation, IndentWriter, JSONObject, JSONValue, nonenumerable, unreachable } from "./util";
import { isNodeWithScope, Scope } from "./scope"
import type { InferContext, Kind, KindEnv, Scheme, TypeEnv } from "./checker"
import type { Type } from "./types";
import { Emitter } from "./emitter";

export type TextSpan = [number, number];

export type Value
  = bigint
  | string

@deserializable()
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
    }
    this.offset += text.length;
  }

}

@deserializable()
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

@deserializable()
export class TextFile {

  public constructor(
    public origPath: string,
    public text: string,
  ) {

  }

  public getFullPath(): string {
    return path.resolve(this.origPath);
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
  RArrowAlt,
  VBar,
  Dot,
  DotDot,
  Comma,
  Colon,
  Equals,
  Backslash,
  Integer,
  StringLiteral,
  LetKeyword,
  PubKeyword,
  MutKeyword,
  ModKeyword,
  ImportKeyword,
  ClassKeyword,
  InstanceKeyword,
  StructKeyword,
  EnumKeyword,
  TypeKeyword,
  ReturnKeyword,
  MatchKeyword,
  ForeignKeyword,
  IfKeyword,
  ElifKeyword,
  ElseKeyword,
  ForallKeyword,
  LineFoldEnd,
  BlockEnd,
  BlockStart,
  EndOfFile,

  // Type expressions
  ReferenceTypeExpression,
  ArrowTypeExpression,
  VarTypeExpression,
  AppTypeExpression,
  NestedTypeExpression,
  TupleTypeExpression,
  ForallTypeExpression,
  TypeExpressionWithConstraints,

  // Patterns
  NamedPattern,
  TuplePattern,
  StructPattern,
  NestedPattern,
  NamedTuplePattern,
  LiteralPattern,
  DisjunctivePattern,

  // Struct expression elements
  StructExpressionField,
  PunnedStructExpressionField,

  // Struct pattern elements
  StructPatternField,
  PunnedStructPatternField,
  VariadicStructPatternElement,

  // Expressions
  MatchExpression,
  MemberExpression,
  CallExpression,
  ReferenceExpression,
  StructExpression,
  TupleExpression,
  NestedExpression,
  ConstantExpression,
  PrefixExpression,
  PostfixExpression,
  InfixExpression,
  FunctionExpression,

  // Statements
  ReturnStatement,
  ExpressionStatement,
  IfStatement,
  AssignStatement,

  // If statement elements
  IfStatementCase,

  // Declarations
  LetDeclaration,
  StructDeclaration,
  EnumDeclaration,
  ImportDeclaration,
  TypeDeclaration,
  ClassDeclaration,
  InstanceDeclaration,
  ModuleDeclaration,

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
  MatchArm,
  Initializer,
  TypeAssert,
  Param,
  SourceFile,
  ClassConstraint,
  ClassConstraintClause,
}

export type Syntax
  = SourceFile
  | ModuleDeclaration
  | ClassDeclaration
  | InstanceDeclaration
  | ClassConstraint
  | ClassConstraintClause
  | Token
  | Param
  | Body
  | MatchArm
  | WrappedOperator
  | Initializer
  | IfStatementCase
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

function isnonenumerabledProperty(key: string): boolean {
  return key === 'kind' || key === 'parent';
}

abstract class SyntaxBase {

  @nonenumerable
  public abstract readonly kind: SyntaxKind;

  @nonenumerable
  public parent: Syntax | null = null;

  @nonenumerable
  public inferredKind?: Kind;
  @nonenumerable
  public inferredType?: Type;

  public abstract getFirstToken(): Token;

  public abstract getLastToken(): Token;

  public abstract clone(): Syntax;

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

  public getEnclosingModule(): ModuleDeclaration | SourceFile {
    let curr = this.parent!;
    while (curr !== null) {
      if (curr.kind === SyntaxKind.SourceFile || curr.kind === SyntaxKind.ModuleDeclaration) {
        return curr;
      }
      curr = curr.parent!;
    }
    throw new Error(`Unable to find an enclosing module for ${this.constructor.name}. Perhaps the parent-links are not set?`);
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
      if (isnonenumerabledProperty(key)) {
        continue;
      }
      visit((this as any)[key]);
    }

  }

  public *getTokens(): Iterable<Token> {
    for (const [_, value] of this.getFields()) {
      yield* filter(value);
    }
    function* filter(value: any): Iterable<Token> {
      if (isToken(value)) {
        yield value;
      } else if (Array.isArray(value)) {
        for (const element of value) {
          yield* filter(element);
        }
      } else if (isSyntax(value)) {
        yield* value.getTokens();
      }
    }
  }

  public *getFields(): Iterable<[string, any]> {
    for (const key of Object.getOwnPropertyNames(this)) {
      if (!isnonenumerabledProperty(key)) {
        yield [key, (this as any)[key]];
      }
    }
  }

  public *getChildNodes(): Iterable<Syntax> {
    function* visit(value: any): Iterable<Syntax> {
      if (value === null) {
        return;
      }
      if (Array.isArray(value)) {
        for (const element of value) {
          yield* visit(element);
        }
      } else if (isSyntax(value)) {
        yield value;
      }
    }
    for (const [_key, value] of this.getFields()) {
      yield* visit(value);
    }
  }

  public emit(file: stream.Writable): void {
    const emitter = new Emitter(new IndentWriter(file));
    emitter.emit(this as any);
  }

  public toJSON(): JSONObject {

    const obj: JSONObject = {};
    obj['type'] = this.constructor.name;
    for (const [key, value] of this.getFields()) {
      if (isnonenumerabledProperty(key)) {
        continue;
      }
      obj[key] = encode(value);
    }
    return obj;

    function encode(value: any): JSONValue {
      if (value === null) {
        return null;
      } else if (Array.isArray(value)) {
        return value.map(encode);
      } else if (isSyntax(value)) {
        return value.toJSON();
      } else {
        return value;
      }
    }

  }

  public resolveModule(name: string): ModuleDeclaration | null {
    const node = this as unknown as Syntax;
    assert(node.kind === SyntaxKind.ModuleDeclaration || node.kind === SyntaxKind.SourceFile);
    for (const element of node.elements) {
      if (element.kind === SyntaxKind.ModuleDeclaration && element.name.text === name) {
        return element;
      }
    }
    return null;
  }

  public getModulePath(): string[] {
    let curr = this.parent;
    const modulePath = [];
    while (curr !== null) {
      if (curr.kind === SyntaxKind.ModuleDeclaration) {
        modulePath.unshift(curr.name.text);
      }
      curr = curr.parent;
    }
    return modulePath;
  }

}

export function forEachChild(node: Syntax, callback: (node: Syntax) => void): void {

  for (const key of Object.getOwnPropertyNames(node)) {
    if (isnonenumerabledProperty(key)) {
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

  @nonenumerable
  private endPos: TextPosition | null = null;

  public constructor(
    public startPos: TextPosition | null = null,
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
    assert(this.startPos !== null);
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

@deserializable()
export class EndOfFile extends VirtualTokenBase {

  public readonly kind = SyntaxKind.EndOfFile;

  public clone(): EndOfFile {
    return new EndOfFile(this.startPos);
  }

}

@deserializable()
export class BlockEnd extends VirtualTokenBase {

  public readonly kind = SyntaxKind.BlockEnd;

  public clone(): BlockEnd {
    return new BlockEnd(this.startPos);
  }

}

@deserializable()
export class BlockStart extends VirtualTokenBase {

  public readonly kind = SyntaxKind.BlockStart;

  public clone(): BlockStart {
    return new BlockStart(this.startPos);
  }

}

@deserializable()
export class LineFoldEnd extends VirtualTokenBase {

  public readonly kind = SyntaxKind.LineFoldEnd;

  public clone(): LineFoldEnd {
    return new LineFoldEnd(this.startPos);
  }

}

@deserializable()
export class Integer extends TokenBase {

  public readonly kind = SyntaxKind.Integer;

  public constructor(
    startPos: TextPosition | null = null,
    public value: bigint,
    public radix: number,
  ) {
    super(startPos);
  }

  public getValue(): Value {
    return this.value;
  }

  public clone(): Integer {
    return new Integer(
      this.startPos,
      this.value,
      this.radix,
    );
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

@deserializable()
export class StringLiteral extends TokenBase {

  public readonly kind = SyntaxKind.StringLiteral;

  public constructor(
    startPos: TextPosition | null = null,
    public contents: string,
  ) {
    super(startPos);
  }

  public getValue(): Value {
    return this.contents;
  }

  public clone(): StringLiteral {
    return new StringLiteral(
      this.startPos,
      this.contents,
    );
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
        out += '\\u' + code.toString(16).padStart(4, '0');
      }
    }
    out += '"';
    return out;
  }

}

@deserializable()
export class IdentifierAlt extends TokenBase {

  public readonly kind = SyntaxKind.IdentifierAlt;

  public constructor(
    startPos: TextPosition | null = null,
    public text: string,
  ) {
    super(startPos);
  }

  public clone(): IdentifierAlt {
    return new IdentifierAlt(
      this.startPos,
      this.text,
    );
  }

}

@deserializable()
export class Identifier extends TokenBase {

  public readonly kind = SyntaxKind.Identifier;

  public constructor(
    startPos: TextPosition | null = null,
    public text: string,
  ) {
    super(startPos);
  }

  public clone(): Identifier {
    return new Identifier(
      this.startPos,
      this.text,
    );
  }

}

@deserializable()
export class CustomOperator extends TokenBase {

  public readonly kind = SyntaxKind.CustomOperator;

  public constructor(
    startPos: TextPosition | null = null,
    public text: string,
  ) {
    super(startPos);
  }

  public clone(): CustomOperator {
    return new CustomOperator(
      this.startPos,
      this.text,
    );
  }

}

export type ExprOperator
  = CustomOperator
  | VBar

export function isExprOperator(node: Syntax): node is ExprOperator {
  return node.kind === SyntaxKind.CustomOperator
      || node.kind === SyntaxKind.VBar
}

@deserializable()
export class Assignment extends TokenBase {

  public readonly kind = SyntaxKind.Assignment;

  public constructor(
    startPos: TextPosition | null = null,
    public text: string,
  ) {
    super(startPos);
  }

  public clone(): Assignment {
    return new Assignment(
      this.startPos,
      this.text,
    );
  }

}

@deserializable()
export class LParen extends TokenBase {

  public readonly kind = SyntaxKind.LParen;

  public get text(): string {
    return '(';
  }

  public clone(): LParen {
    return new LParen(this.startPos);
  }

}

@deserializable()
export class RParen extends TokenBase {

  public readonly kind = SyntaxKind.RParen;

  public get text(): string {
    return ')';
  }

  public clone(): RParen {
    return new RParen(this.startPos);
  }

}

@deserializable()
export class LBrace extends TokenBase {

  public readonly kind = SyntaxKind.LBrace;

  public get text(): string {
    return '{';
  }

  public clone(): LBrace {
    return new LBrace(this.startPos);
  }

}

@deserializable()
export class RBrace extends TokenBase {

  public readonly kind = SyntaxKind.RBrace;

  public get text(): string {
    return '}';
  }

  public clone(): RBrace {
    return new RBrace(this.startPos);
  }

}

@deserializable()
export class LBracket extends TokenBase {

  public readonly kind = SyntaxKind.LBracket;

  public get text(): string {
    return '[';
  }

  public clone(): LBracket {
    return new LBracket(this.startPos);
  }

}

@deserializable()
export class RBracket extends TokenBase {

  public readonly kind = SyntaxKind.RBracket;

  public get text(): string {
    return ']';
  }

  public clone(): RBracket {
    return new RBracket(this.startPos);
  }

}

@deserializable()
export class Dot extends TokenBase {

  public readonly kind = SyntaxKind.Dot;

  public get text(): string {
    return '.';
  }

  public clone(): Dot {
    return new Dot(this.startPos);
  }

}

@deserializable()
export class Comma extends TokenBase {

  public readonly kind = SyntaxKind.Comma;

  public get text(): string {
    return ',';
  }

  public clone(): Comma {
    return new Comma(this.startPos);
  }

}

@deserializable()
export class DotDot extends TokenBase {

  public readonly kind = SyntaxKind.DotDot;

  public get text(): string {
    return '..';
  }

  public clone(): DotDot {
    return new DotDot(this.startPos);
  }

}

@deserializable()
export class Colon extends TokenBase {

  public readonly kind = SyntaxKind.Colon;

  public get text(): string {
    return ':';
  }

  public clone(): Colon {
    return new Colon(this.startPos);
  }

}

@deserializable()
export class Equals extends TokenBase {

  public readonly kind = SyntaxKind.Equals;

  public get text(): string {
    return '=';
  }

  public clone(): Equals {
    return new Equals(this.startPos);
  }

}

@deserializable()
export class Backslash extends TokenBase {

  public readonly kind = SyntaxKind.Equals;

  public get text(): string {
    return '\\';
  }

  public clone(): Backslash {
    return new Backslash(this.startPos);
  }

}

@deserializable()
export class IfKeyword extends TokenBase {

  public readonly kind = SyntaxKind.IfKeyword;

  public get text(): string {
    return 'if';
  }

  public clone(): IfKeyword {
    return new IfKeyword(this.startPos);
  }

}

@deserializable()
export class ElseKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ElseKeyword;

  public get text(): string {
    return 'else';
  }

  public clone(): ElseKeyword {
    return new ElseKeyword(this.startPos);
  }

}

@deserializable()
export class ElifKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ElifKeyword;

  public get text(): string {
    return 'elif';
  }

  public clone(): ElifKeyword {
    return new ElifKeyword(this.startPos);
  }

}

@deserializable()
export class StructKeyword extends TokenBase {

  public readonly kind = SyntaxKind.StructKeyword;

  public get text(): string {
    return 'struct';
  }

  public clone(): StructKeyword {
    return new StructKeyword(this.startPos);
  }

}

@deserializable()
export class EnumKeyword extends TokenBase {

  public readonly kind = SyntaxKind.EnumKeyword;

  public get text(): string {
    return 'enum';
  }

  public clone(): EnumKeyword {
    return new EnumKeyword(this.startPos);
  }

}

@deserializable()
export class ReturnKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ReturnKeyword;

  public get text(): string {
    return 'return';
  }

  public clone(): ReturnKeyword {
    return new ReturnKeyword(this.startPos);
  }

}

@deserializable()
export class MatchKeyword extends TokenBase {

  public readonly kind = SyntaxKind.MatchKeyword;

  public get text(): string {
    return 'match';
  }

  public clone(): MatchKeyword {
    return new MatchKeyword(this.startPos);
  }

}

@deserializable()
export class ForeignKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ForeignKeyword;

  public get text(): string {
    return 'foreign';
  }

  public clone(): ForeignKeyword {
    return new ForeignKeyword(this.startPos);
  }

}

@deserializable()
export class ModKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ModKeyword;

  public get text(): string {
    return 'mod';
  }

  public clone(): ModKeyword {
    return new ModKeyword(this.startPos);
  }

}

@deserializable()
export class MutKeyword extends TokenBase {

  public readonly kind = SyntaxKind.MutKeyword;

  public get text(): string {
    return 'mut';
  }

  public clone(): MutKeyword {
    return new MutKeyword(this.startPos);
  }

}

@deserializable()
export class ImportKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ImportKeyword;

  public get text(): string {
    return 'import'
  }

  public clone(): ImportKeyword {
    return new ImportKeyword(this.startPos);
  }

}

@deserializable()
export class ClassKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ClassKeyword;

  public get text(): string {
    return 'trait';
  }

  public clone(): ClassKeyword {
    return new ClassKeyword(this.startPos);
  }

}

@deserializable()
export class InstanceKeyword extends TokenBase {

  public readonly kind = SyntaxKind.InstanceKeyword;

  public get text(): string {
    return 'impl';
  }

  public clone(): InstanceKeyword {
    return new InstanceKeyword(this.startPos);
  }

}


@deserializable()
export class TypeKeyword extends TokenBase {

  public readonly kind = SyntaxKind.TypeKeyword;

  public get text(): string {
    return 'type';
  }

  public clone(): TypeKeyword {
    return new TypeKeyword(this.startPos);
  }

}

@deserializable()
export class PubKeyword extends TokenBase {

  public readonly kind = SyntaxKind.PubKeyword;

  public get text(): string {
    return 'pub';
  }

  public clone(): PubKeyword {
    return new PubKeyword();
  }

}

@deserializable()
export class LetKeyword extends TokenBase {

  public readonly kind = SyntaxKind.LetKeyword;

  public get text(): string {
    return 'let';
  }

  public clone(): LetKeyword {
    return new LetKeyword();
  }

}

@deserializable()
export class RArrow extends TokenBase {

  public readonly kind = SyntaxKind.RArrow;

  public get text(): string {
    return '->';
  }

  public clone(): RArrow {
    return new RArrow(this.startPos);
  }

}

@deserializable()
export class RArrowAlt extends TokenBase {

  public readonly kind = SyntaxKind.RArrowAlt;

  public get text(): string {
    return '=>';
  }

  public clone(): RArrowAlt {
    return new RArrowAlt(this.startPos);
  }

}

@deserializable()
export class VBar extends TokenBase {

  public readonly kind = SyntaxKind.VBar;

  public get text(): string {
    return '|';
  }

  public clone(): VBar {
    return new VBar(this.startPos);
  }

}

@deserializable()
export class ForallKeyword extends TokenBase {

  public readonly kind = SyntaxKind.ForallKeyword;
  
  public get text(): string {
    return 'forall';
  }

  public clone(): ForallKeyword {
     return new ForallKeyword(this.startPos);
  }

}

export type Token
  = RArrow
  | RArrowAlt
  | VBar
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
  | Backslash
  | LetKeyword
  | PubKeyword
  | MutKeyword
  | ModKeyword
  | ImportKeyword
  | ClassKeyword
  | InstanceKeyword
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
  | ForeignKeyword
  | ForallKeyword

export type TokenKind
  = Token['kind']

@deserializable()
export class ForallTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ForallTypeExpression;

  public constructor(
    public forallKeyword: ForallKeyword,
    public varTypeExps: VarTypeExpression[],
    public dot: Dot,
    public typeExpr: TypeExpression,
  ) {
    super();
  }

  public clone(): ForallTypeExpression {
    return new ForallTypeExpression(
      this.forallKeyword.clone(),
      this.varTypeExps.map(e => e.clone()),
      this.dot.clone(),
      this.typeExpr.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.forallKeyword;
  }

  public getLastToken(): Token {
    return this.typeExpr.getLastToken();
  }

}

@deserializable()
export class TypeExpressionWithConstraints extends SyntaxBase {

  public readonly kind = SyntaxKind.TypeExpressionWithConstraints;

  public constructor(
    public constraints: ClassConstraint[],
    public rarrowAlt: RArrowAlt,
    public typeExpr: TypeExpression,
  ) {
    super();
  }

  public clone(): TypeExpressionWithConstraints {
    return new TypeExpressionWithConstraints(
      this.constraints.map(c => c.clone()),
      this.rarrowAlt.clone(),
      this.typeExpr.clone(),
    );
  }

  public getFirstToken(): Token {
    if (this.constraints.length > 0) {
      return this.constraints[0].getFirstToken();
    }
    return this.rarrowAlt;
  }

  public getLastToken(): Token {
    return this.typeExpr.getLastToken();
  }

}

@deserializable()
export class ArrowTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ArrowTypeExpression;

  public constructor(
    public paramTypeExprs: TypeExpression[],
    public returnTypeExpr: TypeExpression
  ) {
    super();
  }

  public clone(): ArrowTypeExpression {
    return new ArrowTypeExpression(
      this.paramTypeExprs.map(te => te.clone()),
      this.returnTypeExpr.clone(),
    );
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

@deserializable()
export class TupleTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.TupleTypeExpression;

  public constructor(
    public lparen: LParen,
    public elements: TypeExpression[],
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): TupleTypeExpression {
    return new TupleTypeExpression(
      this.lparen.clone(),
      this.elements.map(element => element.clone()),
      this.rparen.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

@deserializable()
export class ReferenceTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ReferenceTypeExpression;

  public constructor(
    public modulePath: Array<[IdentifierAlt, Dot]>,
    public name: IdentifierAlt,
  ) {
    super();
  }

  public clone(): ReferenceTypeExpression {
    return new ReferenceTypeExpression(
      this.modulePath.map(([name, dot]) => [name.clone(), dot.clone()]),
      this.name.clone(),
    );
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

@deserializable()
export class AppTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.AppTypeExpression;

  public constructor(
    public operator: TypeExpression,
    public args: TypeExpression[],
  ) {
    super();
  }

  public clone(): AppTypeExpression {
    return new AppTypeExpression(
      this.operator.clone(),
      this.args.map(arg => arg.clone()),
    );
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

@deserializable()
export class VarTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.VarTypeExpression;

  public constructor(
    public name: Identifier
  ) {
    super();
  }

  public clone(): VarTypeExpression {
    return new VarTypeExpression( this.name.clone() );
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

@deserializable()
export class NestedTypeExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.NestedTypeExpression;

  public constructor(
    public lparen: LParen,
    public typeExpr: TypeExpression,
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): NestedTypeExpression {
    return new NestedTypeExpression(
      this.lparen.clone(),
      this.typeExpr.clone(),
      this.rparen.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export type TypeExpression
  = ReferenceTypeExpression
  | ArrowTypeExpression
  | VarTypeExpression
  | AppTypeExpression
  | NestedTypeExpression
  | TupleTypeExpression
  | ForallTypeExpression
  | TypeExpressionWithConstraints

@deserializable()
export class NamedPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.NamedPattern;

  public constructor(
    public name: Identifier | CustomOperator,
  ) {
    super();
  }

  public clone(): NamedPattern {
    return new NamedPattern( this.name.clone() );
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

@deserializable()
export class TuplePattern extends SyntaxBase {

  public readonly kind = SyntaxKind.TuplePattern;

  public constructor(
    public lparen: LParen,
    public elements: Pattern[],
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): TuplePattern {
    return new TuplePattern(
      this.lparen.clone(),
      this.elements.map(element => element.clone()),
      this.rparen.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

@deserializable()
export class NamedTuplePattern extends SyntaxBase {

  public readonly kind = SyntaxKind.NamedTuplePattern;

  public constructor(
    public name: IdentifierAlt,
    public elements: Pattern[],
  ) {
    super();
  }

  public clone(): NamedTuplePattern {
    return new NamedTuplePattern(
      this.name.clone(),
      this.elements.map(element => element.clone()),
    );
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

@deserializable()
export class StructPatternField extends SyntaxBase {

  public readonly kind = SyntaxKind.StructPatternField;

  public constructor(
    public name: Identifier,
    public equals: Equals,
    public pattern: Pattern,
  ) {
    super();
  }

  public clone(): StructPatternField {
    return new StructPatternField(
      this.name.clone(),
      this.equals.clone(),
      this.pattern.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.pattern.getLastToken();
  }

}

@deserializable()
export class VariadicStructPatternElement extends SyntaxBase {

  public readonly kind = SyntaxKind.VariadicStructPatternElement;

  public constructor(
    public dotdot: DotDot,
    public pattern: Pattern | null,
  ) {
    super();
  }

  public clone(): VariadicStructPatternElement {
    return new VariadicStructPatternElement(
      this.dotdot.clone(),
      this.pattern !== null ? this.pattern.clone() : null,
    );
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

@deserializable()
export class PunnedStructPatternField extends SyntaxBase {

  public readonly kind = SyntaxKind.PunnedStructPatternField;

  public constructor(
    public name: Identifier,
  ) {
    super();
  }

  public clone(): PunnedStructPatternField {
    return new PunnedStructPatternField( this.name.clone() );
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

@deserializable()
export class StructPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.StructPattern;

  public constructor(
    public lbrace: LBrace,
    public members: StructPatternElement[],
    public rbrace: RBrace,
  ) {
    super();
  }

  public clone(): StructPattern {
    return new StructPattern(
      this.lbrace.clone(),
      this.members.map(member => member.clone()),
      this.rbrace.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lbrace;
  }

  public getLastToken(): Token {
    return this.rbrace;
  }

}

@deserializable()
export class NestedPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.NestedPattern;

  public constructor(
    public lparen: LParen,
    public pattern: Pattern,
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): NestedPattern {
    return new NestedPattern(
      this.lparen.clone(),
      this.pattern.clone(),
      this.rparen.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

@deserializable()
export class DisjunctivePattern extends SyntaxBase {

  public readonly kind = SyntaxKind.DisjunctivePattern;

  public constructor(
    public left: Pattern,
    public operator: VBar,
    public right: Pattern,
  ) {
    super();
  }

  public clone(): DisjunctivePattern {
    return new DisjunctivePattern(
      this.left.clone(),
      this.operator.clone(),
      this.right.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.left.getFirstToken();
  }

  public getLastToken(): Token {
    return this.right.getLastToken();
  }

}


@deserializable()
export class LiteralPattern extends SyntaxBase {

  public readonly kind = SyntaxKind.LiteralPattern;

  public constructor(
    public token: StringLiteral | Integer
  ) {
    super();
  }

  public clone(): LiteralPattern {
    return new LiteralPattern( this.token.clone() );
  }

  public getFirstToken(): Token {
    return this.token;
  }

  public getLastToken(): Token {
    return this.token;
  }

}

export type Pattern
  = NamedPattern
  | NestedPattern
  | StructPattern
  | NamedTuplePattern
  | TuplePattern
  | DisjunctivePattern
  | LiteralPattern

@deserializable()
export class TupleExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.TupleExpression;

  public constructor(
    public lparen: LParen,
    public elements: Expression[],
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): TupleExpression {
    return new TupleExpression(
      this.lparen.clone(),
      this.elements.map(element => element.clone()),
      this.rparen.clone()
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

@deserializable()
export class NestedExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.NestedExpression;

  public constructor(
    public lparen: LParen,
    public expression: Expression,
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): NestedExpression {
    return new NestedExpression(
      this.lparen.clone(),
      this.expression.clone(),
      this.rparen.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

@deserializable()
export class ConstantExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ConstantExpression;

  public constructor(
    public token: Integer | StringLiteral,
  ) {
    super();
  }

  public clone(): ConstantExpression {
    return new ConstantExpression( this.token.clone() );
  }

  public getFirstToken(): Token {
    return this.token;
  }

  public getLastToken(): Token {
    return this.token;
  }

}

@deserializable()
export class CallExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.CallExpression;

  public constructor(
    public func: Expression,
    public args: Expression[],
  ) {
    super();
  }

  public clone(): CallExpression {
    return new CallExpression(
      this.func.clone(),
      this.args.map(arg => arg.clone()),
    );
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

@deserializable()
export class StructExpressionField extends SyntaxBase {

  public readonly kind = SyntaxKind.StructExpressionField;

  public constructor(
    public name: Identifier,
    public equals: Equals,
    public expression: Expression,
  ) {
    super();
  }

  public clone(): StructExpressionField {
    return new StructExpressionField(
      this.name.clone(),
      this.equals.clone(),
      this.expression.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

@deserializable()
export class PunnedStructExpressionField extends SyntaxBase {

  public readonly kind = SyntaxKind.PunnedStructExpressionField;

  public constructor(
    public name: Identifier,
  ) {
    super();
  }

  public clone(): PunnedStructExpressionField {
    return new PunnedStructExpressionField( this.name.clone() );
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

@deserializable()
export class StructExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.StructExpression;

  public constructor(
    public lbrace: LBrace,
    public members: StructExpressionElement[],
    public rbrace: RBrace,
  ) {
    super();
  }

  public clone(): StructExpression {
    return new StructExpression(
      this.lbrace.clone(),
      this.members.map(member => member.clone()),
      this.rbrace.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lbrace;
  }

  public getLastToken(): Token {
    return this.rbrace;
  }

}

@deserializable()
export class MatchArm extends SyntaxBase {

  public readonly kind = SyntaxKind.MatchArm;

  public constructor(
    public pattern: Pattern,
    public rarrowAlt: RArrowAlt,
    public expression: Expression,
  ) {
    super();
  }

  public clone(): MatchArm {
    return new MatchArm(
      this.pattern.clone(),
      this.rarrowAlt.clone(),
      this.expression.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

@deserializable()
export class FunctionExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.FunctionExpression;

  public constructor(
    public backslash: Backslash,
    public params: Param[],
    public body: Body,
  ) {
    super();
  }

  public clone(): FunctionExpression {
    return new FunctionExpression(
      this.backslash.clone(),
      this.params.map(param => param.clone()),
      this.body.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.backslash;
  }

  public getLastToken(): Token {
    return this.body.getLastToken();
  }


}

@deserializable()
export class MatchExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.MatchExpression;

  public constructor(
    public matchKeyword: MatchKeyword,
    public expression: Expression | null,
    public arms: MatchArm[],
  ) {
    super();
  }

  public clone(): MatchExpression {
    return new MatchExpression(
      this.matchKeyword.clone(),
      this.expression !== null ? this.expression.clone() : null,
      this.arms.map(arm => arm.clone()),
    );
  }

  public getFirstToken(): Token {
    return this.matchKeyword;
  }

  public getLastToken(): Token {
    if (this.arms.length > 0) {
      return this.arms[this.arms.length-1].getLastToken();
    }
    if (this.expression !== null) {
      return this.expression.getLastToken();
    }
    return this.matchKeyword;
  }

}

@deserializable()
export class ReferenceExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.ReferenceExpression;

  public constructor(
    public modulePath: Array<[IdentifierAlt, Dot]>,
    public name: Identifier | IdentifierAlt,
  ) {
    super();
  }

  public clone(): ReferenceExpression {
    return new ReferenceExpression(
      this.modulePath.map(([name, dot]) => [name.clone(), dot.clone()]),
      this.name.clone(),
    );
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

@deserializable()
export class MemberExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.MemberExpression;

  public constructor(
    public expression: Expression,
    public path: [Dot, Identifier][],
  ) {
    super();
  }

  public clone(): MemberExpression {
    return new MemberExpression(
      this.expression.clone(),
      this.path.map(([dot, name]) => [dot.clone(), name.clone()]),
    );
  }

  public getFirstToken(): Token {
    return this.expression.getFirstToken();
  }

  public getLastToken(): Token {
    return this.path[this.path.length-1][1];
  }

}

@deserializable()
export class PrefixExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.PrefixExpression;

  public constructor(
    public operator: ExprOperator,
    public expression: Expression,
  ) {
    super();
  }

  public clone(): PrefixExpression {
    return new PrefixExpression(
      this.operator.clone(),
      this.expression.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.operator;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

@deserializable()
export class PostfixExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.PostfixExpression;

  public constructor(
    public expression: Expression,
    public operator: ExprOperator,
  ) {
    super();
  }

  public clone(): PostfixExpression {
    return new PostfixExpression(
      this.expression.clone(),
      this.operator.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.expression.getFirstToken();
  }

  public getLastToken(): Token {
    return this.operator;
  }

}

@deserializable()
export class InfixExpression extends SyntaxBase {

  public readonly kind = SyntaxKind.InfixExpression;

  public constructor(
    public left: Expression,
    public operator: ExprOperator,
    public right: Expression,
  ) {
    super();
  }

  public clone(): InfixExpression {
    return new InfixExpression(
      this.left.clone(),
      this.operator.clone(),
      this.right.clone(),
    );
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
  | ReferenceExpression
  | ConstantExpression
  | TupleExpression
  | MatchExpression
  | NestedExpression
  | PrefixExpression
  | InfixExpression
  | PostfixExpression
  | FunctionExpression

@deserializable()
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

  public clone(): IfStatementCase {
    return new IfStatementCase(
      this.keyword.clone(),
      this.test !== null ? this.test.clone() : null,
      this.blockStart.clone(),
      this.elements.map(element => element.clone()),
    );
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

@deserializable()
export class IfStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.IfStatement;

  public constructor(
    public cases: IfStatementCase[],
  ) {
    super();
  }

  public clone(): IfStatement {
    return new IfStatement(
      this.cases.map(caze => caze.clone()),
    );
  }

  public getFirstToken(): Token {
    return this.cases[0].getFirstToken();
  }

  public getLastToken(): Token {
    return this.cases[this.cases.length-1].getLastToken();
  }

}

@deserializable()
export class ReturnStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ReturnStatement;

  public constructor(
    public returnKeyword: ReturnKeyword,
    public expression: Expression | null
  ) {
    super();
  }

  public clone(): ReturnStatement {
    return new ReturnStatement(
      this.returnKeyword.clone(),
      this.expression !== null ? this.expression.clone() : null,
    );
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

@deserializable()
export class AssignStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.AssignStatement;

  public constructor(
    public pattern: Pattern,
    public operator: Assignment,
    public expression: Expression,
  ) {
    super();
  }

  public clone(): AssignStatement {
    return new AssignStatement(
      this.pattern.clone(),
      this.operator.clone(),
      this.expression.clone()
    );
  }

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

@deserializable()
export class ExpressionStatement extends SyntaxBase {

  public readonly kind = SyntaxKind.ExpressionStatement;

  public constructor(
    public expression: Expression,
  ) {
    super();
  }

  public clone(): ExpressionStatement { 
    return new ExpressionStatement( this.expression.clone() );
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
  | AssignStatement

@deserializable()
export class Param extends SyntaxBase {

  public readonly kind = SyntaxKind.Param;

  public constructor(
    public pattern: Pattern,
  ) {
    super();
  }

  public clone(): Param {
    return new Param(
      this.pattern.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.pattern.getLastToken();
  }

}

@deserializable()
export class EnumDeclarationStructElement extends SyntaxBase {

  public readonly kind = SyntaxKind.EnumDeclarationStructElement;

  public scheme?: Scheme;

  public constructor(
    public name: IdentifierAlt,
    public blockStart: BlockStart,
    public fields: StructDeclarationField[],
  ) {
    super();
  }

  public clone(): EnumDeclarationStructElement {
    return new EnumDeclarationStructElement(
      this.name.clone(),
      this.blockStart.clone(),
      this.fields.map(field => field.clone()),
    );
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    if (this.fields.length > 0) {
      return this.fields[this.fields.length-1].getLastToken();
    }
    return this.blockStart;
  }

}

@deserializable()
export class EnumDeclarationTupleElement extends SyntaxBase {

  public readonly kind = SyntaxKind.EnumDeclarationTupleElement;

  public constructor(
    public name: IdentifierAlt,
    public elements: TypeExpression[],
  ) {
    super();
  }

  public clone(): EnumDeclarationElement {
    return new EnumDeclarationTupleElement(
      this.name.clone(),
      this.elements.map(element => element.clone()),
    );
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

@deserializable()
export class EnumDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.EnumDeclaration;

  @nonenumerable
  public typeEnv?: TypeEnv;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public enumKeyword: EnumKeyword,
    public name: IdentifierAlt,
    public varExps: Identifier[],
    public members: EnumDeclarationElement[] | null,
  ) {
    super();
  }

  public clone(): EnumDeclaration {
    return new EnumDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.enumKeyword.clone(),
      this.name.clone(),
      this.varExps.map(ve => ve.clone()),
      this.members !== null ? this.members.map(member => member.clone()) : null,
    );
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

@deserializable()
export class StructDeclarationField extends SyntaxBase {

  public readonly kind = SyntaxKind.StructDeclarationField;

  public constructor(
    public name: Identifier,
    public colon: Colon,
    public typeExpr: TypeExpression,
  ) {
    super();
  }

  public clone(): StructDeclarationField {
    return new StructDeclarationField(
      this.name.clone(),
      this.colon.clone(),
      this.typeExpr.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.typeExpr.getLastToken();
  }

}

@deserializable()
export class StructDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.StructDeclaration;

  @nonenumerable
  public typeEnv?: TypeEnv;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public structKeyword: StructKeyword,
    public name: IdentifierAlt,
    public varExps: Identifier[],
    public fields: StructDeclarationField[] | null,
  ) {
    super();
  }

  public clone(): StructDeclaration {
    return new StructDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.structKeyword.clone(),
      this.name.clone(),
      this.varExps.map(ve => ve.clone()),
      this.fields !== null ? this.fields.map(field => field.clone()) : null,
    );
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.structKeyword;
  }

  public getLastToken(): Token {
    if (this.fields && this.fields.length > 0) {
      return this.fields[this.fields.length-1].getLastToken();
    }
    return this.name;
  }

}

@deserializable()
export class TypeAssert extends SyntaxBase {

  public readonly kind = SyntaxKind.TypeAssert;

  public constructor(
    public colon: Colon,
    public typeExpression: TypeExpression,
  ) {
    super();
  }

  public clone(): TypeAssert {
    return new TypeAssert(
      this.colon.clone(),
      this.typeExpression.clone(),
    );
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

@deserializable()
export class ExprBody extends SyntaxBase {

  public readonly kind = SyntaxKind.ExprBody;

  public constructor(
    public equals: Equals | RArrow,
    public expression: Expression,
  ) {
    super();
  }

  public clone(): ExprBody {
    return new ExprBody(
      this.equals.clone(),
      this.expression.clone(),
    );
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

@deserializable()
export class BlockBody extends SyntaxBase {

  public readonly kind = SyntaxKind.BlockBody;

  public constructor(
    public blockStart: BlockStart,
    public elements: LetBodyElement[],
  ) {
    super();
  }

  public clone(): BlockBody {
    return new BlockBody(
      this.blockStart.clone(),
      this.elements.map(element => element.clone()),
    );
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

@deserializable()
export class WrappedOperator extends SyntaxBase {

  public readonly kind = SyntaxKind.WrappedOperator;

  public constructor(
    public lparen: LParen,
    public operator: CustomOperator,
    public rparen: RParen,
  ) {
    super();
  }

  public clone(): WrappedOperator {
    return new WrappedOperator(
      this.lparen.clone(),
      this.operator.clone(),
      this.rparen.clone(),
    );
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

@deserializable()
export class TypeDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.TypeDeclaration;

  @nonenumerable
  public typeEnv?: TypeEnv;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public typeKeyword: TypeKeyword,
    public name: IdentifierAlt,
    public varExps: Identifier[],
    public equals: Equals,
    public typeExpression: TypeExpression
  ) {
    super();
  }

  public clone(): TypeDeclaration {
    return new TypeDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.typeKeyword.clone(),
      this.name.clone(),
      this.varExps.map(ve => ve.clone()),
      this.equals.clone(),
      this.typeExpression.clone(),
    );
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

@deserializable()
export class LetDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.LetDeclaration;

  @nonenumerable
  public scope?: Scope;

  @nonenumerable
  public typeEnv?: TypeEnv;

  @nonenumerable
  public activeCycle?: boolean;
  @nonenumerable
  public visited?: boolean;

  @nonenumerable
  public context?: InferContext;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public foreignKeyword: ForeignKeyword | null,
    public mutKeyword: MutKeyword | null,
    public pattern: Pattern,
    public params: Param[],
    public typeAssert: TypeAssert | null,
    public body: Body | null,
  ) {
    super();
  }

  public get name(): Identifier | CustomOperator {
    switch (this.pattern.kind) {
      case SyntaxKind.NamedPattern:
        return this.pattern.name;
      case SyntaxKind.NestedPattern:
        assert(this.pattern.pattern.kind === SyntaxKind.NamedPattern);
        return this.pattern.pattern.name;
      default:
        unreachable();
    }
  }

  public clone(): LetDeclaration {
    return new LetDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.letKeyword.clone(),
      this.foreignKeyword !== null ? this.foreignKeyword.clone() : null,
      this.mutKeyword !== null ? this.mutKeyword.clone() : null,
      this.pattern.clone(),
      this.params.map(param => param.clone()),
      this.typeAssert !== null ? this.typeAssert.clone() : null,
      this.body !== null ? this.body.clone() : null,
    );
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

@deserializable()
export class ImportDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.ImportDeclaration;

  public constructor(
    public importKeyword: ImportKeyword,
    public importSource: StringLiteral,
  ) {
    super();
  }

  public clone(): ImportDeclaration {
    return new ImportDeclaration(
      this.importKeyword.clone(),
      this.importSource.clone(),
    );
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

@deserializable()
export class Initializer extends SyntaxBase {

  public readonly kind = SyntaxKind.Initializer;

  public constructor(
    public equals: Equals,
    public expression: Expression
  ) {
    super();
  }

  public clone(): Initializer {
    return new Initializer(
      this.equals.clone(),
      this.expression.clone()
    );
  }

  public getFirstToken(): Token {
    return this.equals;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

@deserializable()
export class ClassConstraint extends SyntaxBase {

  public readonly kind = SyntaxKind.ClassConstraint;

  public constructor(
    public name: IdentifierAlt,
    public types: TypeExpression[],
  ) {
    super();
  }

  public clone(): ClassConstraint {
    return new ClassConstraint(
      this.name.clone(),
      this.types.map(ty => ty.clone()),
    );
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.types[this.types.length-1].getLastToken();
  }

}

@deserializable()
export class ClassConstraintClause extends SyntaxBase {

  public readonly kind = SyntaxKind.ClassConstraintClause;

  public constructor(
    public constraints: ClassConstraint[],
    public rarrowAlt: RArrowAlt,
  ) {
    super();
  }

  public clone(): ClassConstraintClause {
    return new ClassConstraintClause(
      this.constraints.map(constraint => constraint.clone()),
      this.rarrowAlt.clone(),
    );
  }

  public getFirstToken(): Token {
    if (this.constraints.length > 0) {
      return this.constraints[0].getFirstToken();
    }
    return this.rarrowAlt;
  }

  public getLastToken(): Token {
    return this.rarrowAlt;
  }

}

export type ClassDeclarationElement
  = LetDeclaration
  | TypeDeclaration

@deserializable()
export class ClassDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.ClassDeclaration;

  @nonenumerable
  public typeEnv?: TypeEnv;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public classKeyword: ClassKeyword,
    public constraintClause: ClassConstraintClause | null,
    public name: IdentifierAlt,
    public types: VarTypeExpression[],
    public elements: ClassDeclarationElement[],
  ) {
    super();
  }

  public *getSupers(): Iterable<IdentifierAlt> {
    if (this.constraintClause !== null) {
      for (const constraint of this.constraintClause.constraints) {
        yield constraint.name;
      }
    }
  }

  public lookup(element: InstanceDeclarationElement): ClassDeclarationElement | null {

    switch (element.kind) {

      case SyntaxKind.LetDeclaration:
        implementationLimitation(element.pattern.kind === SyntaxKind.NamedPattern);
        for (const other of this.elements) {
          if (other.kind === SyntaxKind.LetDeclaration
              && other.pattern.kind === SyntaxKind.NamedPattern
              && other.pattern.name.text === element.pattern.name.text) {
            return other;
          }
        }
        break;

      case SyntaxKind.TypeDeclaration:
        for (const other of this.elements) {
          if (other.kind === SyntaxKind.TypeDeclaration
              && other.name.text === element.name.text) {
            return other;
          }
        }
        break;

    }

    return null;

  }

  public *getInstances(): Iterable<InstanceDeclaration> {
    let curr = this.parent!;
    for (;;) {
      if (!canHaveInstanceDeclaration(curr)) {
        curr = curr.parent!;
      }
      for (const element of getElements(curr)) {
        if (element.kind === SyntaxKind.InstanceDeclaration && element.name.text === this.name.text) {
          yield element;
        }
      }
    }
  }

  public clone(): ClassDeclaration {
    return new ClassDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.classKeyword.clone(),
      this.constraintClause !== null ? this.constraintClause.clone() : null,
      this.name.clone(),
      this.types.map(t => t.clone()),
      this.elements.map(element => element.clone()),
    );
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.classKeyword;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    if (this.types.length > 0) {
      return this.types[this.types.length-1].getLastToken();
    }
    return this.name;
  }

}

export type InstanceDeclarationElement
  = LetDeclaration
  | TypeDeclaration

@deserializable()
export class InstanceDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.InstanceDeclaration;

  @nonenumerable
  public typeEnv?: TypeEnv;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public classKeyword: InstanceKeyword,
    public constraintClause: ClassConstraintClause | null,
    public name: IdentifierAlt,
    public types: TypeExpression[],
    public elements: InstanceDeclarationElement[],
  ) {
    super();
  }

  public clone(): InstanceDeclaration {
    return new InstanceDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.classKeyword.clone(),
      this.constraintClause !== null ? this.constraintClause.clone() : null,
      this.name.clone(),
      this.types.map(t => t.clone()),
      this.elements.map(element => element.clone()),
    );
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.classKeyword;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    if (this.types.length > 0) {
      return this.types[this.types.length-1].getLastToken();
    }
    return this.name;
  }

}
@deserializable()
export class ModuleDeclaration extends SyntaxBase {

  public readonly kind = SyntaxKind.ModuleDeclaration;

  @nonenumerable
  public typeEnv?: TypeEnv;
  @nonenumerable
  public kindEnv?: KindEnv;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public modKeyword: ModKeyword,
    public name: IdentifierAlt,
    public blockStart: BlockStart,
    public elements: SourceFileElement[],
  ) {
    super();
  }

  public clone(): ModuleDeclaration {
    return new ModuleDeclaration(
      this.pubKeyword !== null ? this.pubKeyword.clone() : null,
      this.modKeyword.clone(),
      this.name.clone(),
      this.blockStart.clone(),
      this.elements.map(element => element.clone())
    );
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.modKeyword;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.blockStart;
  }

}

export type SourceFileElement
  = Statement
  | Declaration
  | ClassDeclaration
  | InstanceDeclaration
  | ModuleDeclaration

@deserializable()
export class SourceFile extends SyntaxBase {

  public readonly kind = SyntaxKind.SourceFile;

  @nonenumerable
  public scope?: Scope;
  @nonenumerable
  public typeEnv?: TypeEnv;
  @nonenumerable
  public kindEnv?: KindEnv;

  public constructor(
    private file: TextFile,
    public elements: SourceFileElement[],
    public eof: EndOfFile,
  ) {
    super();
  }

  public clone(): SourceFile {
    return new SourceFile(
      this.file,
      this.elements.map(element => element.clone()),
      this.eof,
    );
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

export function isSyntax(value: any): value is Syntax {
  return typeof value === 'object'
      && value !== null 
      && value instanceof SyntaxBase;
}

export function isToken(value: any): value is Token {
  return typeof value === 'object'
      && value !== null 
      && value instanceof TokenBase;
}

export function vistEachChild<T extends Syntax>(node: T, proc: (node: Syntax) => Syntax | undefined): Syntax {

  const newArgs = [];
  let changed = false;

  const traverse = (value: any): any => {
    if (Array.isArray(value)) {
      const newElements = [];
      let changed = false;
      for (const element of value) {
        const newElement = traverse(element);
        if (newElement !== element) {
          changed = true;
        }
        newElements.push(newElement);
      }
      return changed ? newElements : value;
    } else if (isSyntax(value)) {
      let newValue = proc(value);
      if (newValue === undefined) {
        newValue = value;
      }
      if (newValue !== value) {
        changed = true;
      }
      return newValue;
    } else {
      return value;
    }
  }

  for (const [_key, value] of node.getFields()) {
    newArgs.push(traverse(value));
  }

  if (!changed) {
    return node;
  }
  return new (node as any).constructor(...newArgs);
}

export function canHaveInstanceDeclaration(node: Syntax): boolean {
  return node.kind === SyntaxKind.SourceFile
      || node.kind === SyntaxKind.ModuleDeclaration
      || node.kind === SyntaxKind.LetDeclaration;
}

export function getElements(node: Syntax): Iterable<Syntax> {
  switch (node.kind) {
    case SyntaxKind.SourceFile:
    case SyntaxKind.ModuleDeclaration:
      return node.elements;
    case SyntaxKind.LetDeclaration:
      if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
        return node.body.elements;
      }
      // falls through
    default:
      return [];
  }
}
