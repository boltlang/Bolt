
import { Stream, StreamWrapper } from "./util"

interface JsonArray extends Array<Json> {  };
interface JsonObject { [key: string]: Json }
type Json = null | string | boolean | number | JsonArray | JsonObject;

export type TokenStream = Stream<Token>;

function jsonify(value: any): Json {
  if (value === null || typeof value === 'string' || typeof value === 'number') {
    return value;
  } else if (Array.isArray(value)) {
    return value.map(element => jsonify(element));
  } else if (typeof value === 'object' && 'toJSON' in value) {
    return value.toJSON();
  } else {
    throw new Error(`I don't know how to convert ${value} to a JSON representation`)
  }
}

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

  EOS,

  // Patterns

  BindPatt,
  ExprPatt,

  // Expressions

  ConstExpr,
  RefExpr,

  // Stmts

  RetStmt,

  // Type declarations

  TypeRef,

  // Declaration nodes

  VarDecl,
  FuncDecl,

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

  getSpan() {

    let curr: Syntax | null = this as any as Syntax;

    do {
      if (curr.span !== null ) {
        return curr.span;
      }
      curr = curr.origNode
    } while (curr !== null)

    throw new Error(`No TextSpan object found in this node or any of its originating nodes.`);

  }

}

export class Literal extends SyntaxBase {

  kind: SyntaxKind.Literal = SyntaxKind.Literal;

  static META = {
    value: EdgeType.Primitive,
  }

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
      value: typeof this.value === 'bigint' ? { type: 'bigint', value: this.value.toString() } : this.value,
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

}

export enum PunctType {
  Paren,
  Bracket,
  Brace,
}

class EOS extends SyntaxBase {

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

}

export class Parenthesized extends SyntaxBase {

  kind: SyntaxKind.Parenthesized = SyntaxKind.Parenthesized;

  static META = {
    elements: EdgeType.Node | EdgeType.List
  }

  constructor(
    public elements: Token[],
    public span: TextSpan,
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toStream() {
    return new StreamWrapper(
      this.elements,
      () => new EOS(new TextSpan(this.getSpan().file, this.getSpan().end.clone(), this.getSpan().end.clone()))
    );
  }

  toJSON(): Json {
    return {
      kind: 'Parenthesized',
      elements: this.elements.map(element => element.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
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
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toStream() {
    return new StreamWrapper(
      this.elements,
      () => new EOS(new TextSpan(this.getSpan().file, this.getSpan().end.clone(), this.getSpan().end.clone()))
    );
  }

  toJSON(): Json {
    return {
      kind: 'Braced',
      elements: this.elements.map(element => element.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
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
    public origNode: [Syntax, Syntax] | Syntax | null = null,
    public parentNode: Syntax | null = null
  ) {
    super();
  }

  toStream() {
    return new StreamWrapper(
      this.elements,
      () => new EOS(new TextSpan(this.getSpan().file, this.getSpan().end.clone(), this.getSpan().end.clone()))
    );
  }

  toJSON(): Json {
    return {
      kind: 'Bracketed',
      elements: this.elements.map(element => element.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
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

}

export class Operator extends SyntaxBase {

  kind: SyntaxKind.Operator = SyntaxKind.Operator;

  static META = {
    text: EdgeType.Primitive
  }

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
  | Literal
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

  toStream() {
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

}

export class ConstExpr extends SyntaxBase {

  kind: SyntaxKind.ConstExpr = SyntaxKind.ConstExpr;

  static META = {
    value: EdgeType.Primitive,
  }

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

}

export type Expr
  = ConstExpr
  | RefExpr

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

}

export type Stmt
  = RetStmt

export class TypeRef extends SyntaxBase {

  kind: SyntaxKind.TypeRef = SyntaxKind.TypeRef;

  static META = {
    name: EdgeType.Node,
    args: EdgeType.Node | EdgeType.List,
  }

  constructor(
    public name: QualName,
    public args: TypeDecl[],
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
      args: this.args.map(a => a.toJSON()),
      span: this.span !== null ? this.span.toJSON() : null,
    }
  }

}

export type TypeDecl
  = TypeRef

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

export class FuncDecl extends SyntaxBase {

  kind: SyntaxKind.FuncDecl = SyntaxKind.FuncDecl;

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
      name: this.name.toJSON(),
      params: this.params.map(p => p.toJSON()),
      returnType: this.returnType !== null ? this.returnType.toJSON() : null,
      body: this.body !== null ? this.body.map(s => s.toJSON()) : null,
      span: this.span !== null ? this.span.toJSON() : this.span,
    }
  }

}

export class VarDecl extends SyntaxBase {

  kind: SyntaxKind.VarDecl = SyntaxKind.VarDecl;

  static META = {
    bindings: EdgeType.Node,
    typeDecl: EdgeType.Node | EdgeType.Nullable,
    value: EdgeType.Node | EdgeType.Nullable,
  }

  constructor(
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
      type: 'VarDecl',
      bindings: this.bindings.toJSON(),
      typeDecl: this.typeDecl !== null ? this.typeDecl.toJSON() : null,
      value: this.value !== null ? this.value.toJSON() : null,
      span: this.span !== null ? this.span.toJSON() : this.span,
    }
  }

}

export type Decl
  = Sentence 
  | FuncDecl
  | VarDecl

export type Syntax
  = Decl
  | Expr
  | Token
  | SourceFile
  | QualName
  | Param
  | EOS

export class SourceFile extends SyntaxBase {

  kind: SyntaxKind.SourceFile = SyntaxKind.SourceFile;

  constructor(
    public elements: (Decl | Stmt)[],
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

}

