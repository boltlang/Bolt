
import { FastStringMap, assert, isPlainObject } from "./util";
import { SyntaxKind, Syntax, isBoltTypeExpression, BoltExpression, BoltFunctionDeclaration, BoltFunctionBodyElement, kindToString } from "./ast";
import { getSymbolPathFromNode, ScopeType, SymbolResolver, SymbolInfo } from "./resolver";
import { Value } from "./evaluator";

enum TypeKind {
  OpaqueType,
  AnyType,
  NeverType,
  FunctionType,
  RecordType,
  PlainRecordFieldType,
  VariantType,
  UnionType,
  TupleType,
}

export type Type
  = OpaqueType
  | AnyType
  | NeverType
  | FunctionType
  | RecordType
  | VariantType
  | TupleType

abstract class TypeBase {

  public abstract kind: TypeKind;

  constructor(public symbol?: SymbolInfo) {
    
  }

}

export class OpaqueType extends TypeBase {

  public kind: TypeKind.OpaqueType = TypeKind.OpaqueType;

}

export class AnyType extends TypeBase {
  public kind: TypeKind.AnyType = TypeKind.AnyType;
}

export class NeverType extends TypeBase {
  public kind: TypeKind.NeverType = TypeKind.NeverType;
}

export class FunctionType extends TypeBase {

  public kind: TypeKind.FunctionType = TypeKind.FunctionType;

  constructor(
    public paramTypes: Type[],
    public returnType: Type,
  ) {
    super();
  }

  public getParameterCount(): number {
    return this.paramTypes.length;
  }

  public getTypeAtParameterIndex(index: number) {
    if (index < 0 || index >= this.paramTypes.length) {
      throw new Error(`Could not get the parameter type at index ${index} because the index  was out of bounds.`);
    }
    return this.paramTypes[index];
  }

}

export class VariantType extends TypeBase {

  public kind: TypeKind.VariantType = TypeKind.VariantType;

  constructor(public elementTypes: Type[]) {
    super();
  }

  public getOwnElementTypes(): IterableIterator<Type> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export class UnionType extends TypeBase {

  public kind: TypeKind.UnionType = TypeKind.UnionType;

}

export type RecordFieldType
 = PlainRecordFieldType

class PlainRecordFieldType extends TypeBase {

  public kind: TypeKind.PlainRecordFieldType = TypeKind.PlainRecordFieldType;

  constructor(public type: Type) {
    super();
  }

}

export class RecordType {

  public kind: TypeKind.RecordType = TypeKind.RecordType;

  private fieldTypes = new FastStringMap<string, RecordFieldType>();

  constructor(
    iterable?: Iterable<[string, RecordFieldType]>,
  ) {
    if (iterable !== undefined) {
      for (const [name, type] of iterable) {
        this.fieldTypes.set(name, type);
      }
    }
  }

  public addField(name: string, type: RecordFieldType): void {
    this.fieldTypes.set(name, type);
  }

  public hasField(name: string) {
    return name in this.fieldTypes;
  }

  public getFieldType(name: string) {
    return this.fieldTypes.get(name);
  }

  public clear(): void {
    this.fieldTypes.clear();
  }

}

export class TupleType extends TypeBase {

  kind: TypeKind.TupleType = TypeKind.TupleType;

  constructor(public elementTypes: Type[]) {
    super();
  }

}

//export function narrowType(outer: Type, inner: Type): Type {
//  if (isAnyType(outer) || isNeverType(inner)) {
//    return inner;
//  }
//  // TODO cover the other cases
//  return outer;
//}

//export function intersectTypes(a: Type, b: Type): Type {
//  if (a.kind === TypeKind.NeverType && b.kind === TypeKind.NeverType)
//    return new NeverType();
//  }
//  if (a.kind == TypeKind.AnyType) {
//    return a
//  }
//  if (isAnyType(a)) {
//    return b;
//  }
//  if (a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType) {
//    if (a.paramTypes.length !== b.paramTypes.length) {
//      return new NeverType();
//    }
//    const returnType = intersectTypes(a.returnType, b.returnType);
//    const paramTypes = a.paramTypes
//      .map((_, i) => intersectTypes(a.paramTypes[i], b.paramTypes[i]));
//    return new FunctionType(paramTypes, returnType)
//  }
//  return new NeverType();
//}

interface AssignmentError {
  node: Syntax;
}

export class TypeChecker {

  private opaqueTypes = new FastStringMap<number, OpaqueType>();

  private stringType = new OpaqueType();
  private intType = new OpaqueType();
  private floatType = new OpaqueType();

  constructor(private resolver: SymbolResolver) {

  }

  public getTypeOfValue(value: Value): Type {
    if (typeof(value) === 'string') {
      return this.stringType;
    } else if (typeof(value) === 'bigint') {
      return this.intType;
    } else if (typeof(value) === 'number') {
      return this.floatType;
    } else if (isPlainObject(value)) {
      const recordType = new RecordType()   
      for (const key of Object.keys(value)) {
         recordType.addField(key, new PlainRecordFieldType(this.getTypeOfValue(value[key])));
      }
      return recordType;
    } else {
      throw new Error(`Could not determine type of given value.`);
    }
  }

  public getTypeOfNode(node: Syntax) {

    switch (node.kind) {

      case SyntaxKind.BoltRecordDeclaration:
      {
        const recordSym = this.resolver.getSymbolForNode(node);
        assert(recordSym !== null);
        if (this.opaqueTypes.has(recordSym!.id)) {
          return this.opaqueTypes.get(recordSym!.id);
        }
        const opaqueType = new OpaqueType(recordSym!);
        this.opaqueTypes.set(recordSym!.id, opaqueType);
        break;
      }

      case SyntaxKind.BoltConstantExpression:
        return this.getTypeOfValue(node.value);

    }

  }

  public isVoid(node: Syntax): boolean {
    switch (node.kind) {
      case SyntaxKind.BoltTupleExpression:
        return node.elements.length === 0;
      default:
        throw new Error(`Could not determine whether the given type resolves to the void type.`)
    }
  }

  public *getAssignmentErrors(left: Syntax, right: Syntax): IterableIterator<AssignmentError> {

    // TODO For function bodies, we can do something special.
    //      Sort the return types and find the largest types, eliminating types that fall under other types.
    //      Next, add the resulting types as type hints to `fnReturnType`.

  }
  
  public getCallableFunctions(node: BoltExpression): BoltFunctionDeclaration[] {

    const resolver = this.resolver;

    const results: BoltFunctionDeclaration[] = [];
    visitExpression(node);
    return results;

    function visitExpression(node: BoltExpression) {
      switch (node.kind) {
        case SyntaxKind.BoltMemberExpression:
        {
          visitExpression(node.expression);
          break;
        }
        case SyntaxKind.BoltQuoteExpression:
        {
          // TODO visit all unquote expressions
          //visitExpression(node.tokens);
          break;
        }
        case SyntaxKind.BoltCallExpression:
        {
          // TODO
          break;
        }
        case SyntaxKind.BoltReferenceExpression:
        {
          const scope = resolver.getScopeForNode(node, ScopeType.Variable);
          assert(scope !== null);
          const resolvedSym = resolver.resolveSymbolPath(getSymbolPathFromNode(node), scope!);
          if (resolvedSym !== null) {
            for (const decl of resolvedSym.declarations) {
              visitFunctionBodyElement(decl as BoltFunctionBodyElement);
            }
          }
          break;
        }
        default:
          throw new Error(`Unexpected node type ${kindToString(node.kind)}`);
      }
    }

    function visitFunctionBodyElement(node: BoltFunctionBodyElement) {
      switch (node.kind) {
        case SyntaxKind.BoltFunctionDeclaration:
          results.push(node);
          break;
        case SyntaxKind.BoltVariableDeclaration:
          if (node.value !== null) {
            visitExpression(node.value);
          }
          break;
        default:
          throw new Error(`Unexpected node type ${kindToString(node.kind)}`);
      }
    }

  }

}
