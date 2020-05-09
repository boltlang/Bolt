
// FIXME SyntaxBase.getSpan() does not work then [n1, n2] is given as origNode

import * as path from "path"

import { Stream, StreamWrapper, FastStringMap } from "./util"
import { Scanner } from "./scanner"
import { RecordType, PrimType, OptionType, VariantType, stringType, intType, boolType } from "./checker"
import { bind } from "./bindings"
import { Value } from "./evaluator"

interface JsonArray extends Array<Json> {  };
interface JsonObject { [key: string]: Json }
type Json = null | string | boolean | number | JsonArray | JsonObject;

export type TokenStream = Stream<Token>;

export enum SyntaxKind {

  // Bolt language AST

  // Tokens

  StringLiteral,
  IntegerLiteral,
  Identifier,
  Operator,
  Parenthesized,
  Braced,
  Bracketed,
  Semi,
  Comma,
  Colon,
  Dot,
  RArrow,
  EqSign,

  // Keywords

  FnKeyword,
  ForeignKeyword,
  LetKeyword,
  ImportKeyword,
  PubKeyword,
  ModKeyword,
  EnumKeyword,
  StructKeyword,
  NewTypeKeyword,

  // Special nodes

  SourceFile,
  Module,
  QualName,
  Sentence,
  Param,
  EOS,

  // Patterns

  BindPatt,
  TypePatt,
  RecordPatt,
  TuplePatt,

  // Expressions

  CallExpr,
  ConstExpr,
  RefExpr,
  MatchExpr,

  // Statements

  RetStmt,
  CondStmt,

  // Type declarations

  TypeRef,

  // Declaration nodes

  VarDecl,
  FuncDecl,
  ImportDecl,
  RecordDecl,
  VariantDecl,
  NewTypeDecl,

  // JavaScript AST

  // Expressions

  JSReferenceExpression,
  JSCallExpression,
  JSYieldExpression,

  // Statements

  JSReturnStatement,

  // Special nodes

  JSSourceFile,

  // C language AST

  // Miscellaneous elements

  CSourceFile,

}

export class TextFile {

  constructor(public origPath: string) {

  }

  get fullPath() {
    return path.resolve(this.origPath)
  }

}

export class TextPos {

  constructor(
    public offset: number,
    public line: number,
    public column: number
  ) {

  }

  clone() {
    return new TextPos(this.offset, this.line, this.column)
  }

}

export class TextSpan {

  constructor(
    public file: TextFile,
    public start: TextPos,
    public end: TextPos
  ) {

  }

  clone() {
    return new TextSpan(this.file, this.start.clone(), this.end.clone());
  }

}

export type SyntaxRange = [Syntax, Syntax];

abstract class SyntaxBase {

  abstract kind: SyntaxKind;

  public parentNode: Syntax | null = null;

  constructor(
    public span: TextSpan | null,
    public origNode: SyntaxRange | null,
  ) {

  }

}

export const fileType = new PrimType();

@bind('Bolt.AST.StringLiteral')
export class StringLiteral extends SyntaxBase {

  kind: SyntaxKind.StringLiteral = SyntaxKind.StringLiteral;

  constructor(
    public value: string,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class IntegerLiteral extends SyntaxBase {

  kind: SyntaxKind.IntegerLiteral = SyntaxKind.IntegerLiteral;

  constructor(
    public value: bigint,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

  toJSON(): Json {
    return {
      value: ['bigint', this.value.toString()],
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}


export enum PunctType {
  Paren,
  Bracket,
  Brace,
}

export class EOS extends SyntaxBase {
  kind: SyntaxKind.EOS = SyntaxKind.EOS;
}

export abstract class Punctuated extends SyntaxBase {

  constructor(
    public text: string,
    span: TextSpan,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

  toSentences() {
    const scanner = new Scanner(this.span!.file, this.text)
    return scanner.scanTokens()
  }

  toTokenStream() {
    const startPos = this.span!.start;
    return new Scanner(this.span!.file, this.text, new TextPos(startPos.offset+1, startPos.line, startPos.column+1));
  }

}

export class Parenthesized extends Punctuated {
  kind: SyntaxKind.Parenthesized = SyntaxKind.Parenthesized;
}

export class Braced extends Punctuated {
  kind: SyntaxKind.Braced = SyntaxKind.Braced;
}

export class Bracketed extends Punctuated {
  kind: SyntaxKind.Bracketed = SyntaxKind.Bracketed;
}

export class Identifier extends SyntaxBase {

  kind: SyntaxKind.Identifier = SyntaxKind.Identifier;

  constructor(
    public text: string,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class Operator extends SyntaxBase {

  kind: SyntaxKind.Operator = SyntaxKind.Operator;

  constructor(
    public text: string,
    public span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
    parentNode: Syntax | null = null
  ) {
    super(span, origNode);
  }

}

export class Semi extends SyntaxBase {
  kind: SyntaxKind.Semi = SyntaxKind.Semi;
}

export class Colon extends SyntaxBase {
  kind: SyntaxKind.Colon = SyntaxKind.Colon;
}

export class Comma extends SyntaxBase {
  kind: SyntaxKind.Comma = SyntaxKind.Comma;
}


export class RArrow extends SyntaxBase {
  kind: SyntaxKind.RArrow = SyntaxKind.RArrow;
}

export class EqSign extends SyntaxBase {
  kind: SyntaxKind.EqSign = SyntaxKind.EqSign;
}

export class Dot extends SyntaxBase {
  kind: SyntaxKind.Dot = SyntaxKind.Dot;
}

export type Token
  = Semi
  | Comma
  | Colon
  | EqSign
  | Dot
  | RArrow
  | EOS
  | Identifier
  | Operator
  | StringLiteral
  | IntegerLiteral
  | Parenthesized
  | Braced
  | Bracketed

export class Sentence extends SyntaxBase {

  kind: SyntaxKind.Sentence = SyntaxKind.Sentence;

  constructor(
    public tokens: Token[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

  toTokenStream() {
    return new StreamWrapper(
      this.tokens,
      () => new EOS(new TextSpan(this.getSpan().file, this.getSpan().end.clone(), this.getSpan().end.clone()))
    );
  }

}

export class QualName extends SyntaxBase {

  kind: SyntaxKind.QualName = SyntaxKind.QualName;

  constructor(
    public name: Identifier | Operator,
    public path: Identifier[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

  get fullText() {
    let out = ''
    for (const chunk of this.path) {
      out += chunk.text + '.'
    }
    return out + this.name.text
  }

}

export class Param extends SyntaxBase {

  kind: SyntaxKind.Param = SyntaxKind.Param;

  constructor(
    public bindings: Patt,
    public typeDecl: TypeDecl | null,
    public defaultValue: Expr | null,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class BindPatt extends SyntaxBase {

  kind: SyntaxKind.BindPatt = SyntaxKind.BindPatt;

  constructor(
    public name: Identifier,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

// FIXME make this a node
export interface RecordPattField {
  name: Identifier;
  pattern: Patt,
}

export class RecordPatt extends SyntaxBase {

  kind: SyntaxKind.RecordPatt = SyntaxKind.RecordPatt;

  constructor(
    public typeDecl: TypeDecl,
    public fields: RecordPattField[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class TuplePatt extends SyntaxBase {

  kind: SyntaxKind.TuplePatt = SyntaxKind.TuplePatt;

  constructor(
    public elements: Patt[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class TypePatt extends SyntaxBase {

  kind: SyntaxKind.TypePatt = SyntaxKind.TypePatt;

  constructor(
    public typeDecl: TypeDecl,
    public pattern: Patt,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export type Patt
  = BindPatt
  | Expr
  | TypePatt
  | TuplePatt 
  | RecordPatt

export class RefExpr extends SyntaxBase {

  kind: SyntaxKind.RefExpr = SyntaxKind.RefExpr;

  constructor(
    public name: QualName,
    span: TextSpan | null = null, 
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class CallExpr extends SyntaxBase {

  kind: SyntaxKind.CallExpr = SyntaxKind.CallExpr;

  constructor(
    public operator: Expr,
    public args: Expr[],
    span: TextSpan | null = null, 
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

// FIXME make this a node
export type MatchArm = [Patt, Expr | Stmt[]];

export class MatchExpr extends SyntaxBase {

  kind: SyntaxKind.MatchExpr = SyntaxKind.MatchExpr;

  constructor(
    public value: Expr,
    public arms: MatchArm[],
    span: TextSpan | null = null, 
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class ConstExpr extends SyntaxBase {

  kind: SyntaxKind.ConstExpr = SyntaxKind.ConstExpr;

  constructor(
    public value: Value,
    span: TextSpan | null = null, 
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export type Expr
  = ConstExpr
  | RefExpr
  | CallExpr
  | MatchExpr

export class RetStmt extends SyntaxBase {

  kind: SyntaxKind.RetStmt = SyntaxKind.RetStmt;

  constructor(
    public value: Expr | null,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export interface Case {
  conditional: Expr,
  consequent: Stmt[]
}

export class CondStmt extends SyntaxBase {

  kind: SyntaxKind.CondStmt = SyntaxKind.CondStmt

  constructor(
    public cases: Case[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export type Stmt
  = RetStmt
  | CondStmt

export class TypeRef extends SyntaxBase {

  kind: SyntaxKind.TypeRef = SyntaxKind.TypeRef;

  constructor(
    public name: QualName,
    public typeArgs: TypeDecl[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export type TypeDecl
  = TypeRef

export class FuncDecl extends SyntaxBase {

  kind: SyntaxKind.FuncDecl = SyntaxKind.FuncDecl;

  constructor(
    public isPublic: boolean,
    public target: string,
    public name: QualName, 
    public params: Param[],
    public returnType: TypeDecl | null,
    public body: Body | null,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class VarDecl extends SyntaxBase {

  kind: SyntaxKind.VarDecl = SyntaxKind.VarDecl;

  constructor(
    public isMutable: boolean,
    public bindings: Patt, 
    public typeDecl: TypeDecl | null,
    public value: Expr | null,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class ImportDecl extends SyntaxBase {

  kind: SyntaxKind.ImportDecl = SyntaxKind.ImportDecl;

  constructor(
    public file: string,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class RecordDecl extends SyntaxBase {

  kind: SyntaxKind.RecordDecl = SyntaxKind.RecordDecl;

  fields: Map<Identifier, TypeDecl>;

  constructor(
    public isPublic: boolean,
    public name: QualName,
    fields: Iterable<[Identifier, TypeDecl]>,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode)
    this.fields = new Map(fields);
  }

}

export class NewTypeDecl extends SyntaxBase {

  kind: SyntaxKind.NewTypeDecl = SyntaxKind.NewTypeDecl;

  constructor(
    public isPublic: boolean,
    public name: Identifier,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export type Decl
  = Sentence 
  | FuncDecl
  | ImportDecl
  | NewTypeDecl
  | VarDecl
  | RecordDecl

export type Syntax
  = Decl
  | Expr
  | Token
  | Stmt
  | Patt
  | TypeDecl
  | Module
  | SourceFile
  | QualName
  | Param
  | EOS

export type SourceElement = (Module | Decl | Stmt | Expr);

export class Module extends SyntaxBase {

  kind: SyntaxKind.Module = SyntaxKind.Module;

  constructor(
    public isPublic: boolean,
    public name: QualName,
    public elements: SourceElement[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class SourceFile extends SyntaxBase {

  kind: SyntaxKind.SourceFile = SyntaxKind.SourceFile;

  constructor(
    public elements: SourceElement[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export function isExpr(node: Syntax): node is Expr {
  return node.kind === SyntaxKind.ConstExpr 
      || node.kind === SyntaxKind.CallExpr
      || node.kind === SyntaxKind.MatchExpr
      || node.kind === SyntaxKind.RefExpr;
}

export class JSYieldExpression extends SyntaxBase {

  kind: SyntaxKind.JSYieldExpression = SyntaxKind.JSYieldExpression;

  constructor(
    public value: JSExpression,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export class JSReferenceExpression extends SyntaxBase {

  kind: SyntaxKind.JSReferenceExpression = SyntaxKind.JSReferenceExpression;

  constructor(
    public name: string,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null
  ) {
    super(span, origNode);
  }

}

export class JSCallExpression extends SyntaxBase {

  kind: SyntaxKind.JSCallExpression = JSCallExpression;

  constructor(
    public operator: JSExpression,
    public operands: JSExpression[],
  ) {

  }

}

export type JSExpression
  = JSYieldExpression
  | JSReferenceExpression
  | JSCallExpression

export type JSStatement
  = JSReturnStatement

export type JSDeclaration
  = JSVariableDeclaration
  | JSFunctionDeclaration

export type JSSourceElement
  = JSStatement
  | JSDeclaration

export class JSSourceFile extends SyntaxBase {

  kind: SyntaxKind.JSSourceFile = SyntaxKind.JSSourceFile;

  constructor(
    public elements: JSSourceElement[],
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null,
  ) {
    super(span, origNode);
  }

}

export type JSSyntax
  = JSExpression
  | JSStatement
  | JSDeclaration
  | JSSourceFile

export type CSourceElement
  = never

export class CSourceFile extends SyntaxBase {

  kind: SyntaxKind.CSourceFile = SyntaxKind.CSourceFile;

  constructor(
    public elements: CSourceElement,
    span: TextSpan | null = null,
    origNode: SyntaxRange | null = null
  ) {
    super(span, origNode);
  }

}

export type CSyntax
  = CSourceFile

export type AnySyntax
  = Syntax
  | JSSyntax
  | CSyntax

export type AnySourceFile
  = SourceFile
  | JSSourceFile
  | CSourceFile

export function nodeToJSON(node: Syntax) {

  const obj: Json = {
    kind: SyntaxKind[node.kind],
  };

  for (const key of Object.keys(node)) {
    if (key !== 'kind' && key !== 'origNode' && key !== 'parentNode') {
      const value = (node as any)[key];
      if (Array.isArray(value)) {
        obj[key] = value.map(visit)
      } else {
        obj[key] = visit(value);
      }
    }
  }

  return obj;

  function visit(value: any) {
    if (isNode(value)) {
      return nodeToJSON(node);
    } else {
      return value;
    }
  }

}

export function isJSNode(node: Syntax): node is JSSyntax {
  return SyntaxKind[node.kind].startsWith('JS');
}

export function getChildren(node: Syntax) {

  const out: Syntax[] = [];

  for (const key of Object.keys(node)) {
    if (key !== 'kind' && key !== 'origNode' && key !== 'parentNode') {
      const value = (node as any)[key];
      if (Array.isArray(value)) {
        for (const element of value) {
          visit(element)
        }
      } else {
        visit(value)
      }
    }
  }

  return out;

  function visit(value: any) {
    if (isNode(value)) {
      out.push(value);
    }
  }

}

export function kindToString(kind: SyntaxKind) {
  return SyntaxKind[kind];
}

export function isNode(value: any): value is Syntax {
  return typeof value.kind === 'number'
      && Object.prototype.hasOwnProperty.call(SyntaxKind, value.kind)
}

export function getTargetLanguage(node: AnySyntax) {
  switch (node.kind) {
    case SyntaxKind.StringLiteral:
    case SyntaxKind.IntegerLiteral:
    case SyntaxKind.Identifier:
    case SyntaxKind.Operator:
    case SyntaxKind.Parenthesized:
    case SyntaxKind.Braced:
    case SyntaxKind.Bracketed:
    case SyntaxKind.Semi:
    case SyntaxKind.Comma:
    case SyntaxKind.Colon:
    case SyntaxKind.Dot:
    case SyntaxKind.RArrow:
    case SyntaxKind.EqSign:
    case SyntaxKind.FnKeyword:
    case SyntaxKind.ForeignKeyword:
    case SyntaxKind.LetKeyword:
    case SyntaxKind.ImportKeyword:
    case SyntaxKind.PubKeyword:
    case SyntaxKind.ModKeyword:
    case SyntaxKind.EnumKeyword:
    case SyntaxKind.StructKeyword:
    case SyntaxKind.NewTypeKeyword:
    case SyntaxKind.SourceFile:
    case SyntaxKind.Module:
    case SyntaxKind.QualName:
    case SyntaxKind.Sentence:
    case SyntaxKind.Param:
    case SyntaxKind.EOS:
    case SyntaxKind.BindPatt:
    case SyntaxKind.TypePatt:
    case SyntaxKind.RecordPatt:
    case SyntaxKind.TuplePatt:
    case SyntaxKind.CallExpr:
    case SyntaxKind.ConstExpr:
    case SyntaxKind.RefExpr:
    case SyntaxKind.MatchExpr:
    case SyntaxKind.RetStmt:
    case SyntaxKind.CondStmt:
    case SyntaxKind.TypeRef:
    case SyntaxKind.VarDecl:
    case SyntaxKind.FuncDecl:
    case SyntaxKind.ImportDecl:
    case SyntaxKind.RecordDecl:
    case SyntaxKind.VariantDecl:
    case SyntaxKind.NewTypeDecl:
        return "Bolt";
    case SyntaxKind.JSReferenceExpression:
    case SyntaxKind.JSCallExpression:
    case SyntaxKind.JSYieldExpression:
    case SyntaxKind.JSReturnStatement:
    case SyntaxKind.JSSourceFile:
      return "JS";
    case SyntaxKind.CSourceFile:
      return "C";
    default:
      throw new Error(`Could not determine language of ${SyntaxKind[node.kind]}.`);
  }
}

export function setParents(node: Syntax) {
  for (const child of getChildren(node)) {
    child.parentNode = node
    setParents(child)
  }
}

