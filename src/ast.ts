
import "reflect-metadata"

interface JsonArray extends Array<Json> {  };
interface JsonObject { [key: string]: Json }
type Json = string | boolean | number | JsonArray | JsonObject;

export enum SyntaxKind {

  // Tokens

  Literal,
  Identifier,
  Operator,
  Punctuated,

  SourceFile,

  QualName,

  // Expressions

  ConstantExpr,
  ReferenceExpr,

  // Type declarations

  TypeReference,

  // Declaration nodes

  VariableDecl,
  FunctionDecl,

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

export class Literal {

  kind = SyntaxKind.Literal;

  static META = {
    value: EdgeType.Primitive,
  }

  constructor(
    public value: string | bigint,
    public span: TextSpan
  ) {

  }

  toJSON(): Json {
    return {
      value: typeof this.value === 'bigint' ? Number(this.value) : this.value,
      span: this.span.toJSON(),
    }
  }

}

export enum PunctType {
  Paren,
  Bracket,
  Brace,
}

export class Punctuated {

  kind = SyntaxKind.Punctuated

  static META = {
    punctuator: EdgeType.Primitive,
    elements: EdgeType.Node | EdgeType.List
  }

  constructor(
    public punctuator: PunctType,
    public elements: Token[],
    public span: TextSpan
  ) {

  }

  toJSON(): Json {
    return {
      kind: 'Punctuated',
      punctuator: this.punctuator,
      elements: this.elements.map(element => element.toJSON()),
    }
  }

}

export class Identifier {

  kind = SyntaxKind.Identifier;

  static META = {
    text: EdgeType.Primitive
  }

  constructor(
    public text: string,
    public span: TextSpan
  ) {

  }

  toJSON(): Json {
    return {
      kind: 'Identifier',
      text: this.text,
      span: this.span.toJSON(),
    }
  }

}

export class Operator {

  kind = SyntaxKind.Operator;

  static META = {
    text: EdgeType.Primitive
  }

  constructor(
    public text: string,
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {

  }

  toJSON(): Json {
    return {
      kind: 'Operator',
      text: this.text,
      span: this.span.toJSON(),
    }
  }

}

export type Token
  = Identifier
  | Operator
  | Literal
  | Punctuated

export class QualName {

  kind = SyntaxKind.QualName;

  static META = {
    name: EdgeType.Node,
    path: EdgeType.Node | EdgeType.List,
  }

  constructor(
    public name: string,
    public path: Identifier[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {

  }

}

export class ConstantExpr {

  kind = SyntaxKind.ConstantExpr;

  static META = {
    value: EdgeType.Primitive,
  }

  constructor(
    public value: string | bigint, 
    public span: TextSpan, 
    public parentNode: Syntax | null = null
  ) {

  }

}

export type Expr
  = ConstantExpr

export class TypeReference {

  kind = SyntaxKind.TypeReference;

  static META = {
    name: EdgeType.Node,
    args: EdgeType.Node | EdgeType.List,
  }

  constructor(
    public name: QualName,
    public args: TypeDecl[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {

  }

}

export type TypeDecl
  = TypeReference

export class Unexpanded {

  static META = {
    tokens: EdgeType.Node | EdgeType.List
  }

  constructor(
    public tokens: Token[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {

  }

}

export class FunctionDecl {

  kind = SyntaxKind.FunctionDecl;

  static META = {
    name: EdgeType.Node,
    params: EdgeType.Node | EdgeType.List,
    returnType: EdgeType.Node | EdgeType.Nullable,
    body: EdgeType.Node | EdgeType.List,
  }

  constructor(
    public name: QualName, 
    public params: Param[],
    public returnType: TypeDecl | null,
    public body: Statement[] | null,
    public span: TextSpan, 
    public parentNode: Syntax | null = null
  ) {

  }


}

export class VariableDecl {

  kind = SyntaxKind.VariableDecl;

  static META = {
    bindings: EdgeType.Node,
    typeDecl: EdgeType.Node | EdgeType.Nullable,
    value: EdgeType.Node | EdgeType.Nullable,
  }

  constructor(
    public bindings: Pattern, 
    public typeDecl: TypeDecl | null,
    public value: Expr | null,
    public span: TextSpan
  ) {

  }

}

export type Decl
  = Unexpanded
  | FunctionDecl
  | VariableDecl

export type Syntax
  = Decl
  | Expr
  | SourceFile
  | QualName

export class SourceFile {

  constructor(public elements: Decl[], public span: TextSpan) {

  } 

}

