
import {
  Syntax,
  kindToString,
  SyntaxKind,
  BoltImportDeclaration,
  BoltPattern,
  isBoltTypeAliasDeclaration,
  BoltFunctionBodyElement,
} from "./ast"

import { FastStringMap, getFullTextOfQualName } from "./util"

export class Type {
  
}

export class PrimType extends Type {

}

export class FunctionType extends Type {

  constructor(
    public paramTypes: Type[],
    public returnType: Type,
  ) {
    super();
  }

}

export class VariantType extends Type {

  constructor(public elementTypes: Type[]) {
    super();
  }

}

export const stringType = new PrimType()
export const intType = new PrimType()
export const boolType = new PrimType()
export const voidType = new PrimType()
export const anyType = new PrimType()
export const noneType = new PrimType();

export class RecordType {

  private fieldTypes = new FastStringMap<string, Type>();

  constructor(
    iterable: IterableIterator<[string, Type]>,
  ) {
    for (const [name, type] of iterable) {
      this.fieldTypes.set(name, type);
    }
  }

  hasField(name: string) {
    return name in this.fieldTypes;
  }

  getTypeOfField(name: string) {
    return this.fieldTypes.get(name);
  }

}

interface SymbolInfo {
  type: Type | null;
  definitions: Syntax[];
}

export class Scope {

  private symbolsByLocalName = new FastStringMap<string, SymbolInfo>();

  constructor(
    public originatingNode: Syntax,
    public parentScope?: Scope | null
  ) {

  }

  public getSymbolNamed(name: string): SymbolInfo | null {
    let currScope: Scope | null = this;
    while (true) {
      if (currScope.symbolsByLocalName.has(name)) {
        return currScope.symbolsByLocalName.get(name);
      }
      currScope = currScope.parentScope;
      if (currScope === null) {
        break;
      }
    }
    return null;
  }

  public getTypeNamed(name: string): Type | null {
    const sym = this.getSymbolNamed(name);
    if (sym === null || !introducesNewType(sym.definitions[0].kind)) {
      return null;
    }
    return sym.type!;
  }

}

function* map<T, R>(iterable: Iterable<T>, proc: (value: T) => R): IterableIterator<R> {
  const iterator = iterable[Symbol.iterator]();
  while (true) {
    let { done, value }= iterator.next();
    if (done) {
      break
    }
    yield proc(value)
  }
}

function introducesNewType(kind: SyntaxKind): boolean {
  return kind === SyntaxKind.BoltRecordDeclaration
      || kind === SyntaxKind.BoltTypeAliasDeclaration;
}

function introducesNewScope(kind: SyntaxKind): boolean {
  return kind === SyntaxKind.BoltFunctionDeclaration 
      || kind === SyntaxKind.BoltSourceFile;
}

function getFullName(node: Syntax) {
  let out = []
  let curr: Syntax | null = node;
  while (true) {
    switch (curr.kind) {
      case SyntaxKind.BoltIdentifier:
        out.unshift(curr.text)
        break;
      case SyntaxKind.BoltModule:
        out.unshift(getFullTextOfQualName(curr.name));
        break;
      case SyntaxKind.BoltRecordDeclaration:
        out.unshift(getFullTextOfQualName(curr.name))
        break;
    }
    curr = curr.parentNode;
    if (curr === null) {
      break;
    }
  }
  return out.join('.');
}

export class TypeChecker {

  private symbols = new FastStringMap<string, Type>();
  private types = new Map<Syntax, Type>();
  private scopes = new Map<Syntax, Scope>();

  private inferTypeFromUsage(bindings: BoltPattern, body: BoltFunctionBodyElement[]) {
    return anyType;
  }

  private getTypeOfBody(body: BoltFunctionBodyElement[]) {
    return anyType;
  }

  private createType(node: Syntax): Type {

    console.error(`creating type for ${kindToString(node.kind)}`);

    switch (node.kind) {

      case SyntaxKind.BoltReferenceExpression:
        return anyType;

      case SyntaxKind.BoltConstantExpression:
        return node.value.type;

      case SyntaxKind.BoltExpressionStatement:
        return voidType;

      case SyntaxKind.BoltCallExpression:
        // TODO
        return anyType;

      case SyntaxKind.BoltFunctionDeclaration:
        let returnType = anyType;
        if (node.returnType !== null) {
          returnType = this.getTypeOfNode(node.returnType)
        }
        if (node.body !== null) {
          returnType = this.intersectTypes(returnType, this.getTypeOfBody(node.body))
        }
        let paramTypes = node.params.map(param => {
          let paramType = this.getTypeOfNode(param);
          if (node.body !== null) {
            paramType = this.intersectTypes(
              paramType,
              this.inferTypeFromUsage(param.bindings, node.body)
            )
          }
          return paramType
        })
        return new FunctionType(paramTypes, returnType);

      case SyntaxKind.BoltReferenceTypeExpression:
        const name = getFullTextOfQualName(node.name);
        const scope = this.getScope(node);
        let reffed = scope.getTypeNamed(name);
        if (reffed === null) {
          reffed = anyType;
        }
        return reffed;

      case SyntaxKind.BoltRecordDeclaration:

        const fullName = getFullName(node);
        let type;

        if (node.members === null) {
          type = new PrimType();
          this.symbols.set(fullName, type);
        } else {
          type = new RecordType(map(node.members, member => ([field.name.text, this.getTypeOfNode(field.type)])));
          this.symbols.set(fullName, type);
        }

        return type;

      case SyntaxKind.BoltParameter:
        if (node.type !== null) {
          return this.getTypeOfNode(node.type)
        }
        return anyType;

      default:
        throw new Error(`Could not derive type of ${kindToString(node.kind)}`)

    }

  }

  public getSymbolNamed(name: string) {
    if (!this.symbols.has(name)) {
      return null;
    }
    return this.symbols.get(name);
  }

  public getTypeOfNode(node: Syntax): Type {
    if (this.types.has(node)) {
      return this.types.get(node)!
    }
    const newType = this.createType(node)
    this.types.set(node, newType)
    return newType;
  }

  public check(node: Syntax) {

    this.getTypeOfNode(node);

    switch (node.kind) {

      case SyntaxKind.BoltRecordDeclaration:
      case SyntaxKind.BoltConstantExpression:
        break;

      case SyntaxKind.BoltExpressionStatement:
        this.check(node.expression);
        break;

      case SyntaxKind.BoltCallExpression:
        this.check(node.operator);
        for (const operand of node.operands) {
          this.check(operand);
        }
        // TODO check whether the overload matches the referenced operator
        break;

      case SyntaxKind.BoltFunctionDeclaration:
        if (node.body !== null) {
          if (Array.isArray(node.body)) {
            for (const element of node.body) {
              this.check(element)
            }
          }
        }
        break;

      case SyntaxKind.BoltReferenceExpression:
        // TODO implement this
        break;

      case SyntaxKind.BoltModule:
      case SyntaxKind.BoltSourceFile:
        for (const element of node.elements) {
          this.check(element)
        }
        break;

      default:
        throw new Error(`Could not type-check node ${kindToString(node.kind)}`)

    }

  }

  public getScope(node: Syntax): Scope {
    while (!introducesNewScope(node.kind)) {
      node = node.parentNode!;
    }
    if (this.scopes.has(node)) {
      return this.scopes.get(node)!
    }
    const scope = new Scope(node)
    this.scopes.set(node, scope)
    return scope
  }

  private intersectTypes(a: Type, b: Type): Type {
    if (a === noneType || b == noneType) {
      return noneType;
    }
    if (b === anyType) {
      return a
    }
    if (a === anyType) {
      return b;
    }
    if (a instanceof FunctionType && b instanceof FunctionType) {
      if (a.paramTypes.length !== b.paramTypes.length) {
        return noneType;
      }
      const returnType = this.intersectTypes(a.returnType, b.returnType);
      const paramTypes = a.paramTypes.map((_, i) => this.intersectTypes(a.paramTypes[i], b.paramTypes[i]));
      return new FunctionType(paramTypes, returnType)
    }
    return noneType;
  }

}

