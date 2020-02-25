
// FIXME SyntaxBase.getSpan() does not work then [n1, n2] is given as origNode

import { Stream, StreamWrapper } from "./util"
import { Scanner } from "./scanner"

interface JsonArray extends Array<Json> {  };
interface JsonObject { [key: string]: Json }
type Json = null | string | boolean | number | JsonArray | JsonObject;

export type TokenStream = Stream<Token>;

export enum SyntaxKind {

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

  FunctionKeyword,
  ForeignKeyword,
  LetKeyword,
  ImportKeyword,

  // Special nodes

  SourceFile,

  QualName,

  Sentence,

  Param,

  EOS,

  // Patterns

  BindPatt,
  ExprPatt,

  // Expressions

  CallExpr,
  ConstExpr,
  RefExpr,

  // Stmts

  RetStmt,

  // Type declarations

  TypeRef,

  // Declaration nodes

  VarDecl,
  FuncDecl,
  ImportDecl,

}

enum EdgeType {
  Primitive  = 1,
  Node       = 2,
  Nullable   = 4,
  List       = 8,
}

export class TextFile {

  constructor(public path: string) {
    
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

  toJSON(): Json {
    return {
      offset: this.offset,
      line: this.line,
      column: this.column
    }
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

  toJSON(): Json {
    return {
      file: this.file.path,
      start: this.start.toJSON(),
      end: this.end.toJSON(),
    }
  }

}

abstract class SyntaxBase {

  abstract kind: SyntaxKind;
  abstract origNode: [Syntax, Syntax] | Syntax | null;
  abstract parentNode: Syntax | null;
  abstract span: TextSpan | null;

  getSpan(): TextSpan {

    let curr: Syntax | null = this as any as Syntax;

    do {
      if (curr.span !== null ) {
        return curr.span;
      }
      curr = curr.origNode;
    } while (curr !== null)

    throw new Error(`No TextSpan object found in this node or any of its originating nodes.`);

  }

  getFile(): TextFile {
    return this.getSpan().file;
  }

  abstract getChildren(): IterableIterator<Syntax>;

}

export class StringLiteral extends SyntaxBase {

  kind: SyntaxKind.StringLiteral = SyntaxKind.StringLiteral;

  constructor(
    public value: string,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      value: this.value,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class IntegerLiteral extends SyntaxBase {

  kind: SyntaxKind.IntegerLiteral = SyntaxKind.IntegerLiteral;

  constructor(
    public value: string,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
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

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON() {
    return {
      kind: 'EOS'
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Parenthesized extends SyntaxBase {

  kind: SyntaxKind.Parenthesized = SyntaxKind.Parenthesized;

  protected buffered = null;

  constructor(
    public text: string,
    public span: TextSpan,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toTokenStream() {
    const span = this.getSpan();
    const startPos = span.start;
    return new Scanner(span.file, this.text, new TextPos(startPos.offset+1, startPos.line, startPos.column+1));
  }

  toJSON(): Json {
    return {
      kind: 'Parenthesized',
      text: this.text,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}


export class Braced extends SyntaxBase {

  kind: SyntaxKind.Braced = SyntaxKind.Braced;

  constructor(
    public text: string,
    public span: TextSpan,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toTokenStream() {
    const span = this.getSpan();
    const startPos = span.start;
    return new Scanner(span.file, this.text, new TextPos(startPos.offset+1, startPos.line, startPos.column+1));
  }

  toJSON(): Json {
    return {
      kind: 'Braced',
      text: this.text,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Bracketed extends SyntaxBase {

  kind: SyntaxKind.Bracketed = SyntaxKind.Bracketed;

  constructor(
    public text: string,
    public span: TextSpan,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toTokenStream() {
    const span = this.getSpan();
    const startPos = span.start;
    return new Scanner(span.file, this.text, new TextPos(startPos.offset+1, startPos.line, startPos.column+1));
  }

  toJSON(): Json {
    return {
      kind: 'Bracketed',
      text: this.text,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Identifier extends SyntaxBase {

  kind: SyntaxKind.Identifier = SyntaxKind.Identifier;

  constructor(
    public text: string,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Identifier',
      text: this.text,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Operator extends SyntaxBase {

  kind: SyntaxKind.Operator = SyntaxKind.Operator;

  constructor(
    public text: string,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Operator',
      text: this.text,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Semi extends SyntaxBase {
  
  kind: SyntaxKind.Semi = SyntaxKind.Semi;

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Semi',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Colon extends SyntaxBase {
  
  kind: SyntaxKind.Colon = SyntaxKind.Colon;

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Colon',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Comma extends SyntaxBase {

  kind: SyntaxKind.Comma = SyntaxKind.Comma;

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Comma',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}


export class RArrow extends SyntaxBase {

  kind: SyntaxKind.RArrow = SyntaxKind.RArrow;

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'RArrow',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}



export class EqSign extends SyntaxBase {

  kind: SyntaxKind.EqSign = SyntaxKind.EqSign;

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'EqSign',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export class Dot extends SyntaxBase {

  kind: SyntaxKind.Dot = SyntaxKind.Dot;

  constructor(
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Dot',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {

  }

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
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toTokenStream() {
    return new StreamWrapper(
      this.tokens,
      () => new EOS(new TextSpan(this.getSpan().file, this.getSpan().end.clone(), this.getSpan().end.clone()))
    );
  }

  toJSON(): Json {
    return {
      kind: 'Sentence',
      tokens: this.tokens.map(token => token.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    for (const token of this.tokens) {
      yield token;
    }
  }

}

export class QualName {

  kind: SyntaxKind.QualName = SyntaxKind.QualName;

  constructor(
    public name: Identifier | Operator,
    public path: Identifier[],
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {

  }

  toJSON(): Json {
    return {
      kind: 'QualName',
      name: this.name.toJSON(),
      path: this.path.map(p => p.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

    *getChildren(): IterableIterator<Syntax> {
      for (const chunk of this.path) {
        yield chunk
      }
      yield this.name
    }

}

export class Param extends SyntaxBase {

  kind: SyntaxKind.Param = SyntaxKind.Param;

  constructor(
    public bindings: Patt,
    public typeDecl: TypeDecl | null,
    public defaultValue: Expr | null,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null,
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Param',
      bindings: this.bindings.toJSON(),
      typeDecl: this.typeDecl !== null ? this.typeDecl.toJSON() : null,
      defaultValue: this.defaultValue !== null ? this.defaultValue.toJSON() : null,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren() { 
    yield this.bindings
    if (this.typeDecl !== null) {
      yield this.typeDecl
    }
    if (this.defaultValue !== null) {
      yield this.defaultValue
    }
  }

}

export class BindPatt extends SyntaxBase {

  kind: SyntaxKind.BindPatt = SyntaxKind.BindPatt;

  constructor(
    public name: Identifier,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'BindPatt',
      name: this.name.toJSON(),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    yield this.name
  }

}

export type Patt
  = BindPatt

export class RefExpr extends SyntaxBase {

  kind: SyntaxKind.RefExpr = SyntaxKind.RefExpr;

  constructor(
    public name: QualName,
    public span: TextSpan | null = null, 
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'RefExpr',
      name: this.name.toJSON(),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    yield this.name
  }

}

export class CallExpr extends SyntaxBase {

  kind: SyntaxKind.CallExpr = SyntaxKind.CallExpr;

  constructor(
    public operator: Expr,
    public args: Expr[],
    public span: TextSpan | null = null, 
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'CallExpr',
      operator: this.operator.toJSON(),
      args: this.args.map(a => a.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    yield this.operator
    for (const arg of this.args) {
      yield arg
    }
  }

}

export class ConstExpr extends SyntaxBase {

  kind: SyntaxKind.ConstExpr = SyntaxKind.ConstExpr;

  constructor(
    public value: string | bigint, 
    public span: TextSpan | null = null, 
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'ConstExpr',
      value: typeof this.value === 'bigint' ? { 'type': 'bigint', value: this.value.toString() } : this.value,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    
  }


}

export type Expr
  = ConstExpr
  | RefExpr
  | CallExpr

export class RetStmt extends SyntaxBase {

  kind: SyntaxKind.RetStmt = SyntaxKind.RetStmt;

  constructor(
    public value: Expr | null,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'RetStmt',
      value: this.value !== null ? this.value.toJSON() : null,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    if (this.value !== null) {
      yield this.value
    }
  }

}

export type Stmt
  = RetStmt

export class TypeRef extends SyntaxBase {

  kind: SyntaxKind.TypeRef = SyntaxKind.TypeRef;

  constructor(
    public name: QualName,
    public typeArgs: TypeDecl[],
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'TypeRef',
      name: this.name.toJSON(),
      args: this.typeArgs.map(a => a.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    yield this.name
    for (const arg of this.typeArgs) {
      yield arg
    }
  }

}

export type TypeDecl
  = TypeRef

export class FuncDecl extends SyntaxBase {

  kind: SyntaxKind.FuncDecl = SyntaxKind.FuncDecl;

  constructor(
    public target: string,
    public name: QualName, 
    public params: Param[],
    public returnType: TypeDecl | null,
    public body: Stmt[] | null,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'FuncDecl',
      target: this.target,
      name: this.name.toJSON(),
      params: this.params.map(p => p.toJSON()),
      returnType: this.returnType !== null ? this.returnType.toJSON() : null,
      body: this.body !== null ? this.body.map(s => s.toJSON()) : null,
      span: this.span !== null ? this.span.toJSON() : this.span,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    yield this.name
    for (const param of this.params) {
      yield param
    }
    if (this.returnType !== null) {
      yield this.returnType;
    }
    if (this.body !== null) {
      for (const stmt of this.body) {
        yield stmt
      }
    }
  }

}

export class VarDecl extends SyntaxBase {

  kind: SyntaxKind.VarDecl = SyntaxKind.VarDecl;

  constructor(
    public isMutable: boolean,
    public bindings: Patt, 
    public typeDecl: TypeDecl | null,
    public value: Expr | null,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'VarDecl',
      bindings: this.bindings.toJSON(),
      typeDecl: this.typeDecl !== null ? this.typeDecl.toJSON() : null,
      value: this.value !== null ? this.value.toJSON() : null,
      span: this.span !== null ? this.span.toJSON() : this.span,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    yield this.bindings
    if (this.typeDecl !== null) {
      yield this.typeDecl
    }
    if (this.value !== null) {
      yield this.value;
    }
  }

}

export class ImportDecl {

  kind: SyntaxKind.ImportDecl = SyntaxKind.ImportDecl;

  constructor(
    public file: string,
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {

  }

  *getChildren(): IterableIterator<Syntax> {

  }

}

export type Decl
  = Sentence 
  | FuncDecl
  | ImportDecl
  | VarDecl


export type Syntax
  = Decl
  | Expr
  | Token
  | Stmt
  | Patt
  | TypeDecl
  | SourceFile
  | QualName
  | Param
  | EOS

export class SourceFile extends SyntaxBase {

  kind: SyntaxKind.SourceFile = SyntaxKind.SourceFile;

  constructor(
    public elements: (Decl | Stmt | Expr)[],
    public span: TextSpan | null = null,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'SourceFile',
      elements: this.elements.map(element => element.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

  *getChildren(): IterableIterator<Syntax> {
    for (const element of this.elements) {
      yield element
    }
  }

}

export function isExpr(node: Syntax): node is Expr {
  return node.kind === SyntaxKind.ConstExpr || node.kind === SyntaxKind.CallExpr;
}

export function isJSNode(node: Syntax) {
  return typeof node.type === 'string'
}

export function setParents(node: Syntax) {
  if (isJSNode(node)) {
    return;
  }
  for (const child of node.getChildren()) {
    child.parentNode = node
    setParents(child)
  }
}

