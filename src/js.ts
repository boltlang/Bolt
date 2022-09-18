
import type stream from "stream"
import { IndentWriter } from "./util";

export const enum JSNodeKind {

  // Patterns
  BindPattern,
  ArrayPattern,
  ObjectPattern,

  // Expressions
  ReferenceExpression,
  CallExpression,
  FunctionExpression,
  MemberExpression,
  IndexExpression,

  // Statements
  ExpressionStatement,
  ReturnStatement,

  // Declarations
  FunctionDeclaration,
  VariableDeclaration,

  // Other nodes
  Program,
}

abstract class JSNodeBase {

  public abstract readonly kind: JSNodeKind;

  public emit(out: stream.Writable): void {
    const emitter = new JSEmitter(out);
    emitter.emit(this as unknown as JSNode);
  }

}

export class JSBindPattern extends JSNodeBase {

  public readonly kind = JSNodeKind.BindPattern;

  public constructor(
    public name: string,
  ) {
    super();
  }

}

export type JSPattern
  = JSBindPattern

export class JSReferenceExpression extends JSNodeBase {

  public kind = JSNodeKind.ReferenceExpression;

  public constructor(
    public name: string,
  ) {
    super();
  }

}

export class JSCallExpression extends JSNodeBase {

  public readonly kind = JSNodeKind.CallExpression;

  public constructor(
    public operator: JSExpression,
    public args: JSExpression[],
  ) {
    super();
  }

}

export type JSExpression
  = JSReferenceExpression
  | JSCallExpression

export class JSExpressionStatement extends JSNodeBase {

  public readonly kind = JSNodeKind.ExpressionStatement;

  public constructor(
    public expression: JSExpression,
  ) {
    super();
  }

}

export class JSReturnStatement extends JSNodeBase {

  public readonly kind = JSNodeKind.ReturnStatement;

  public constructor(
    public value: JSExpression | null,
  ) {
    super();
  }

}

export type JSStatement
  = JSExpressionStatement
  | JSReturnStatement

export const enum JSDeclarationFlags {
  IsExported = 1,
}

export type JSFunctionElement
  = JSDeclaration
  | JSStatement

export class JSFunctionDeclaration extends JSNodeBase {

  public readonly kind = JSNodeKind.FunctionDeclaration;

  public constructor(
    public flags: JSDeclarationFlags,
    public name: string,
    public params: JSPattern[],
    public body: JSFunctionElement[],
  ) {
    super();
  }

}

export enum JSVarType {
  Var,
  Const,
  Let,
}

export class JSVariableDeclaration extends JSNodeBase {

  public readonly kind = JSNodeKind.VariableDeclaration;

  public constructor(
    public flags: JSDeclarationFlags,
    public varType: JSVarType,
    public pattern: JSPattern,
    public value: JSExpression | null,
  ) {
    super();
  }

}

export type JSDeclaration
  = JSFunctionDeclaration
  | JSVariableDeclaration

export type JSSourceElement
  = JSStatement
  | JSDeclaration

export class JSProgram extends JSNodeBase {
  
  public readonly kind = JSNodeKind.Program;

  public constructor(
    public elements: JSSourceElement[],
  ) {
    super();
  }

}

export type JSNode
  = JSStatement
  | JSDeclaration
  | JSExpression
  | JSPattern
  | JSProgram

export class JSEmitter {

  private writer: IndentWriter;

  public constructor(out: stream.Writable) {
    this.writer = new IndentWriter(out);
  }

  public emit(node: JSNode) {
  }

}
