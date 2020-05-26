
import { FastStringMap, assert } from "./util";
import { SyntaxKind, Syntax, isBoltTypeExpression, BoltExpression, BoltFunctionDeclaration, BoltFunctionBodyElement, kindToString } from "./ast";
import { getSymbolPathFromNode, ScopeType, SymbolResolver } from "./resolver";

enum TypeKind {
  OpaqueType,
  AnyType,
  NeverType,
  FunctionType,
  RecordType,
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
  abstract kind: TypeKind;
}

export class OpaqueType extends TypeBase {
  kind: TypeKind.OpaqueType = TypeKind.OpaqueType;
}

export class AnyType extends TypeBase {
  kind: TypeKind.AnyType = TypeKind.AnyType;
}

export class NeverType extends TypeBase {
  kind: TypeKind.NeverType = TypeKind.NeverType;
}

export class FunctionType extends TypeBase {

  kind: TypeKind.FunctionType = TypeKind.FunctionType;

  constructor(
    public paramTypes: Type[],
    public returnType: Type,
  ) {
    super();
  }

  public getParameterCount(): number {
    return this.paramTypes.length;
  }

  public getParamTypeAtIndex(index: number) {
    if (index < 0 || index >= this.paramTypes.length) {
      throw new Error(`Could not get the parameter type at index ${index} because the index  was out of bounds.`);
    }
    return this.paramTypes[index];
  }

}

export class VariantType extends TypeBase {

  kind: TypeKind.VariantType = TypeKind.VariantType;

  constructor(public elementTypes: Type[]) {
    super();
  }

  public getOwnElementTypes(): IterableIterator<Type> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export function isVariantType(value: any): value is VariantType {
  return value instanceof VariantType;
}

export class RecordType {

  kind: TypeKind.RecordType = TypeKind.RecordType;

  private fieldTypes = new FastStringMap<string, Type>();

  constructor(
    iterable: IterableIterator<[string, Type]>,
  ) {
    for (const [name, type] of iterable) {
      this.fieldTypes.set(name, type);
    }
  }

  public hasField(name: string) {
    return name in this.fieldTypes;
  }

  public getTypeOfField(name: string) {
    return this.fieldTypes.get(name);
  }

}

export function isRecordType(value: any): value is RecordType {
  return value.kind === TypeKind.RecordType;
}

export class TupleType extends TypeBase {

  kind: TypeKind.TupleType = TypeKind.TupleType;

  constructor(public elementTypes: Type[]) {
    super();
  }

}

export function isTupleType(value: any): value is TupleType {
  return value.kind === TypeKind.TupleType;
}

export function isVoidType(value: any) {
  return isTupleType(value) && value.elementTypes.length === 0;
}

export function narrowType(outer: Type, inner: Type): Type {
  if (isAnyType(outer) || isNeverType(inner)) {
    return inner;
  }
  // TODO cover the other cases
  return outer;
}

export function intersectTypes(a: Type, b: Type): Type {
  if (isNeverType(a) || isNeverType(b)) {
    return new NeverType();
  }
  if (isAnyType(b)) {
    return a
  }
  if (isAnyType(a)) {
    return b;
  }
  if (isFunctionType(a) && isFunctionType(b)) {
    if (a.paramTypes.length !== b.paramTypes.length) {
      return new NeverType();
    }
    const returnType = intersectTypes(a.returnType, b.returnType);
    const paramTypes = a.paramTypes
      .map((_, i) => intersectTypes(a.paramTypes[i], b.paramTypes[i]));
    return new FunctionType(paramTypes, returnType)
  }
  return new NeverType();
}

export type TypeInfo = never;

interface AssignmentError {
  node: Syntax;
}

export class TypeChecker {

  constructor(private resolver: SymbolResolver) {

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
