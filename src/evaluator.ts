
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

class Environment {

  private symbols: FastStringMap<Value> = Object.create(null);

  constructor(public parentEnv: Environment | null = null) {

  }

  setValue(name: string, value: Value) {
    if (name in this.symbols) {
      throw new Error(`A variable with the name '${name}' already exists.`);
    }
    this.symbols[name] = value;
  }

  updateValue(name: string, newValue: Value) {
    if (!(name in this.symbols)) {
      throw new Error(`Trying to update a variable '${name}' that has not been declared.`);
    }
  }

  lookup(name: string) {
    let curr = this as Environment;
    while (true) {
      if (name in curr.symbols) {
        return curr.symbols[name];
      }
      if (curr.parentEnv === null) {
        break;
      }
      curr = curr.parentEnv;
    }
    throw new Error(`A variable named '${name}' was not found.`);
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

  eval(node: Syntax, env: Environment = new Environment()): Value { 

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      case SyntaxKind.Module:
        for (const element of node.elements) {
          this.eval(element, env);
        }
        break;

      case SyntaxKind.RefExpr:
        return env.lookup(node.name.fullText);

      case SyntaxKind.NewTypeDecl:
      case SyntaxKind.RecordDecl:
      case SyntaxKind.FuncDecl:
        break;

      case SyntaxKind.MatchExpr:
        const value = this.eval(node.value, env);
        for (const [pattern, result] of node.arms) {
          if (this.match(value, pattern)) {
            return this.eval(result as Expr, env)
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

