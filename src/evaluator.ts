
import { Syntax, SyntaxKind, Expr, isNode } from "./ast"
import { TypeChecker, Type, RecordType, PrimType, boolType } from "./checker"
import { FastStringMap } from "./util"

export interface Value {
  type: Type;
}

export class PrimValue implements Value {

  constructor(
    public type: PrimType,
    public value: any
  ) {
    
  }

}

export const TRUE = new PrimValue(boolType, true);
export const FALSE = new PrimValue(boolType, false);

export abstract class RecordValue implements Value {

  abstract type: RecordType;

  abstract getValueOfField(name: string): Value;

}

export class NativeRecord implements Value {

  constructor(
    public type: RecordType,
    protected fields: FastStringMap<Value>,
  ) {
    
  }

  getValueOfField(name: string): Value {
    if (!this.type.hasField(name)) {
      throw new Error(`Field '${name}' does not exist on this record.`)
    }
    return this.fields[name]
  }

}

export class RecordWrapper extends RecordValue {

  constructor(
    public type: RecordType,
    protected data: any,
  ) {
    super();
  }

  getValueOfField(name: string): Value {
    if (!this.type.hasField(name)) {
      throw new Error(`Field '${name}' does not exist on this record.`)
    }
    return this.data[name]
  }

}

export class Evaluator {

  constructor(public checker: TypeChecker) {

  }

  match(value: Value, node: Syntax) {

    switch (node.kind) {

      case SyntaxKind.RecordPatt:
        for (const field of node.fields) {
          if (!this.match((value as RecordValue).getValueOfField(field.name.text), field.pattern)) {
            return false;
          }
        }
        return true;

      case SyntaxKind.TypePatt:
        return value.type === this.checker.getTypeOfNode(node)

      default:
        throw new Error(`I did not know how to match on pattern ${SyntaxKind[node.kind]}`)

    }

  }

  createValue(data: any) {
    if (isNode(data)) {
      return new RecordWrapper(this.checker.getTypeNamed(`Bolt.AST.${SyntaxKind[data.kind]}`)! as RecordType, data)
    }
  }

  eval(node: Syntax): Value { 

    switch (node.kind) {

      case SyntaxKind.MatchExpr:
        const value = this.eval(node.value);
        for (const [pattern, result] of node.arms) {
          if (this.match(value, pattern)) {
            return this.eval(result as Expr)
          }
        }
        return new PrimValue(this.checker.getTypeNamed('Void')!, null);

      case SyntaxKind.ConstExpr:
        return new PrimValue(this.checker.getTypeOfNode(node), node.value)

      default:
        throw new Error(`Could not evaluate node ${SyntaxKind[node.kind]}`)

    }

  }

}

