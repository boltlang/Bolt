
import { Syntax, SyntaxKind, BoltQualName, BoltExpression, kindToString, BoltSyntax, isBoltStatement } from "./ast"
import { FastStringMap, assert } from "./util"
import { emitNode } from "./emitter";

export class Record {

  private fields: Map<string, Value>;

  constructor(fields: Iterable<[string, Value]>) {
    this.fields = new Map(fields);
  }

  public getFields(): IterableIterator<[string, Value]> {
    return this.fields[Symbol.iterator]();
  }

  public clone(): Record {
    return new Record(this.fields);
  }

  public getFieldValue(name: string): Value {
    if (!this.fields.has(name)) {
      throw new Error(`Trying to access non-existent field ${name} of a record.`);
    }
    return this.fields.get(name);
  }

  public addField(name: string, value: Value): void {
    this.fields.set(name, value);
  }

  public deleteField(name: string): void {
    this.fields.delete(name);
  }

  public clear(): void {
    this.fields.clear();
  }

}

export type Value
  = string
  | undefined
  | boolean
  | number
  | bigint
  | object

class Environment {

  private symbols = new FastStringMap<string, Value>();

  constructor(public parentEnv: Environment | null = null) {

  }

  public setValue(name: string, value: Value) {
    if (name in this.symbols) {
      throw new Error(`A variable with the name '${name}' already exists.`);
    }
    this.symbols.set(name, value);
  }

  public updateValue(name: string, newValue: Value) {
    if (!this.symbols.has(name)) {
      throw new Error(`Trying to update a variable '${name}' that has not been declared.`);
    }
    this.symbols.delete(name);
    this.symbols.set(name, newValue); 
  }

  public lookup(name: string) {
    let curr = this as Environment;
    while (true) {
      if (this.symbols.has(name)) {
        return curr.symbols.get(name);
      }
      if (curr.parentEnv === null) {
        break;
      }
      curr = curr.parentEnv;
    }
    throw new Error(`A variable named '${name}' was not found.`);
  }

}

function mangle(node: BoltSyntax) {
  switch (node.kind) {
    case SyntaxKind.BoltIdentifier:
      return emitNode(node);
    default:
      throw new Error(`Could not mangle ${kindToString(node.kind)} to a symbol name.`)
  }
}

class EvaluationError extends Error {

}

export class Evaluator {

  constructor(public checker: TypeChecker) {

  }

  private performPatternMatch(value: Value, node: Syntax, env: Environment): boolean {

    switch (node.kind) {

      case SyntaxKind.BoltBindPattern:
      {
        env.setValue(node.name.text, value);
        return true;
      }

      case SyntaxKind.BoltRecordPattern:
      {
        if (!(value instanceof Record)) {
          throw new EvaluationError(`A deconstructing record pattern received a value that is not a record.`);
        }
        const record = value.clone();
        for (const fieldPatt of node.fields) {
          if (fieldPatt.isRest) {
            if (fieldPatt.name !== null) {
              env.setValue(fieldPatt.name.text, { data: record.clone() });
            }
            record.clear();
          } else {
            assert(fieldPatt.name !== null);
            let isMatch = true;
            if (fieldPatt.pattern !== null) {
              isMatch = this.performPatternMatch(value.getFieldValue(fieldPatt.name!.text), fieldPatt.pattern, env);
            }
            if (!isMatch) {
              return false;
            }
            record.deleteField(fieldPatt.name!.text);
          }
        }
        return true;
      }

      case SyntaxKind.BoltTypePattern:
      {
        const expectedType = this.checker.getTypeOfNode(node.type);
        if (!this.checker.isTypeAssignableTo(expectedType, this.checker.createTypeForValue(value))) {
          return false;
        }
        return false;
      }

      default:
        throw new Error(`I did not know how to match on pattern ${kindToString(node.kind)}`)

    }

  }

  public eval(node: Syntax, env: Environment = new Environment()): Value { 

    switch (node.kind) {

      case SyntaxKind.BoltSourceFile:
      case SyntaxKind.BoltModule:
        for (const element of node.elements) {
          if (isBoltStatement(element)) {
            this.eval(element, env);
          }
        }
        return { data: undefined }

      case SyntaxKind.BoltReferenceExpression:
        return env.lookup(mangle(node.name));

      case SyntaxKind.BoltMatchExpression:
        const value = this.eval(node.value, env);
        for (const matchArm of node.arms) {
          const matchArmEnv = new Environment(env);
          const isMatch = this.performPatternMatch(value, matchArm.pattern, matchArmEnv);
          if (isMatch) {
            return this.eval(matchArm.body, env)
          }
        }
        return { data: undefined };

      case SyntaxKind.BoltConstantExpression:
        return node.value;

      default:
        throw new Error(`Could not evaluate node ${kindToString(node.kind)}`)

    }

  }

}

