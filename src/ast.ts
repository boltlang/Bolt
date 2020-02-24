
import "reflect-metadata"

import { Stream } from "./util"

interface JsonArray extends Array<Json> {  };
interface JsonObject { [key: string]: Json }
type Json = null | string | boolean | number | JsonArray | JsonObject;

export type TokenStream = Stream<Token>;

export enum SyntaxKind {

  // Tokens

  Literal,
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

  // Special nodes

  SourceFile,

  QualName,

  Sentence,

  Param,

  // Patterns

  BindPatt,
  ExprPatt,

  // Expressions

  ConstExpr,
  ReferenceExpr,

  // Statements

  ReturnStatement,

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

abstract class SyntaxBase {
  abstract kind: SyntaxKind;
  abstract parentNode: Syntax | null;
}

export class Literal extends SyntaxBase {

  kind: SyntaxKind.Literal = SyntaxKind.Literal;

  static META = {
    value: EdgeType.Primitive,
  }

  constructor(
    public value: string | bigint,
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
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

export class Parenthesized extends SyntaxBase {

  kind: SyntaxKind.Parenthesized = SyntaxKind.Parenthesized;

  static META = {
    elements: EdgeType.Node | EdgeType.List
  }

  constructor(
    public elements: Token[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Parenthesized',
      elements: this.elements.map(element => element.toJSON()),
    }
  }

}


export class Braced extends SyntaxBase {

  kind: SyntaxKind.Braced = SyntaxKind.Braced;

  static META = {
    elements: EdgeType.Node | EdgeType.List
  }

  constructor(
    public elements: Token[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Braced',
      elements: this.elements.map(element => element.toJSON()),
    }
  }

}

export class Bracketed extends SyntaxBase {

  kind: SyntaxKind.Bracketed = SyntaxKind.Bracketed;

  static META = {
    elements: EdgeType.Node | EdgeType.List
  }

  constructor(
    public elements: Token[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Bracketed',
      elements: this.elements.map(element => element.toJSON()),
    }
  }

}

export class Identifier extends SyntaxBase {

  kind: SyntaxKind.Identifier = SyntaxKind.Identifier;

  static META = {
    text: EdgeType.Primitive
  }

  constructor(
    public text: string,
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Identifier',
      text: this.text,
      span: this.span.toJSON(),
    }
  }

}

export class Operator extends SyntaxBase {

  kind: SyntaxKind.Operator = SyntaxKind.Operator;

  static META = {
    text: EdgeType.Primitive
  }

  constructor(
    public text: string,
    public span: TextSpan,
    public origNode: Syntax | null = null,
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

}

export class Semi extends SyntaxBase {
  
  kind: SyntaxKind.Semi = SyntaxKind.Semi;

  constructor(
    public span: TextSpan | null = null,
    public origNode: Syntax | null = null,
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

}

export class Colon extends SyntaxBase {
  
  kind: SyntaxKind.Colon = SyntaxKind.Colon;

  constructor(
    public span: TextSpan,
    public origNode: Syntax | null = null,
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

}

export class Comma extends SyntaxBase {

  kind: SyntaxKind.Comma = SyntaxKind.Comma;

  constructor(
    public span: TextSpan | null = null,
    public origNode: Syntax | null = null,
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

}


export class RArrow extends SyntaxBase {

  kind: SyntaxKind.RArrow = SyntaxKind.RArrow;

  constructor(
    public span: TextSpan | null = null,
    public origNode: Syntax | null = null,
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

}



export class EqSign extends SyntaxBase {

  kind: SyntaxKind.EqSign = SyntaxKind.EqSign;

  constructor(
    public span: TextSpan | null = null,
    public origNode: Syntax | null = null,
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

}

export class Dot extends SyntaxBase {

  kind: SyntaxKind.Dot = SyntaxKind.Dot;

  constructor(
    public span: TextSpan | null = null,
    public origNode: Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON() {
    return {
      kind: 'Dot',
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

}

export type Token
  = Semi
  | Comma
  | Colon
  | EqSign
  | Dot
  | RArrow
  | Identifier
  | Operator
  | Literal
  | Parenthesized
  | Braced
  | Bracketed

export class Sentence extends SyntaxBase {

  kind = SyntaxKind.Sentence;

  constructor(
    public tokens: Token[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON(): Json {
    return {
      kind: 'Sentence',
      tokens: this.tokens.map(token => token.toJSON()),
      span: this.span.toJSON(),
    }
  }

}

export class QualName {

  kind: SyntaxKind.QualName = SyntaxKind.QualName;

  static META = {
    name: EdgeType.Node,
    path: EdgeType.Node | EdgeType.List,
  }

  constructor(
    public name: Identifier | Operator,
    public path: Identifier[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {

  }

}

export class Param extends SyntaxBase {

  kind: SyntaxKind.Param = SyntaxKind.Param;

  constructor(
    public bindings: Pattern,
    public typeDecl: TypeDecl | null,
    public defaultValue: Expr | null,
    public span: TextSpan | null = null,
    public parentNode: Syntax | null = null,
  ) {
    super();
  }

}

export class BindPatt extends SyntaxBase {

  kind: SyntaxKind.BindPatt = SyntaxKind.BindPatt;

  constructor(
    public name: Identifier,
    public span: TextSpan | null = null,
    public origNode: Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON() {
    return {
      kind: 'BindPatt',
      name: this.name.toJSON(),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

}

export class ConstExpr {

  kind = SyntaxKind.ConstExpr;

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
  = ConstExpr

class ReturnStatement extends SyntaxBase {

  kind: SyntaxKind.ReturnStatement = SyntaxKind.ReturnStatement;

  constructor(
    public value: Expr | null,
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }
}

export type Statement
  = ReturnStatement

export class TypeReference {

  kind: SyntaxKind.TypeReference = SyntaxKind.TypeReference;

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

// export class Unexpanded {
// 
//   static META = {
//     tokens: EdgeType.Node | EdgeType.List
//   }
// 
//   constructor(
//     public tokens: Token[],
//     public span: TextSpan,
//     public parentNode: Syntax | null = null
//   ) {
// 
//   }
// 
// }

export class FunctionDecl extends SyntaxBase {

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
    super();
  }


}

export class VariableDecl extends SyntaxBase {

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
    super();
  }

}

export type Decl
  = Sentence 
  | FunctionDecl
  | VariableDecl

export type Syntax
  = Decl
  | Expr
  | Token
  | SourceFile
  | QualName

export class SourceFile extends SyntaxBase {

  kind: SyntaxKind.SourceFile = SyntaxKind.SourceFile;

  constructor(
    public elements: (Decl | Statement)[],
    public span: TextSpan,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toJSON() {
    return {
      elements: this.elements.map(element => element.toJSON()),
      span: this.span.toJSON(),
    }
  }

}

