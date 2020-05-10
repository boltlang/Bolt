
import {
  Syntax,
  kindToString,
  SyntaxKind,
  BoltImportDeclaration,
  BoltPattern,
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

  fieldTypes: FastStringMap<Type> = Object.create(null);

  constructor(
    iterable: IterableIterator<[string, Type]>,
  ) {
    for (const [name, typ] of iterable) {
      this.fieldTypes[name] = typ;
    }
  }

  hasField(name: string) {
    return name in this.fieldTypes;
  }

  getTypeOfField(name: string) {
    if (name in this.fieldTypes) {
      return this.fieldTypes[name]
    }
    throw new Error(`Field '${name}' does not exist on this record type.`)
  }

}

export class Scope {

  constructor(public origin: Syntax) {

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

  protected symbols: FastStringMap<Type> = Object.create(null)
  protected types = new Map<Syntax, Type>();
  protected scopes = new Map<Syntax, Scope>();

  constructor() {
  }

  protected inferTypeFromUsage(bindings: BoltPattern, body: Body) {
    return anyType;
  }

  protected getTypeOfBody(body: Body) {
    return anyType;
  }

  protected createType(node: Syntax): Type {

    console.error(`creating type for ${kindToString(node.kind)}`);

    switch (node.kind) {

      case SyntaxKind.BoltReferenceExpression:
        return anyType;

      case SyntaxKind.BoltConstantExpression:
        return node.value.type;

      case SyntaxKind.BoltNewTypeDeclaration:
        console.log(getFullName(node.name))
        this.symbols[getFullName(node.name)] = new PrimType();
        return noneType;

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

      case SyntaxKind.BoltReferenceTypeNode:
        const name = getFullTextOfQualName(node.name);
        const reffed = this.getTypeNamed(name);
        if (reffed === null) {
          throw new Error(`Could not find a type named '${name}'`);
        }
        return reffed;

      case SyntaxKind.BoltRecordDeclaration:

        const typ = new RecordType(map(node.fields, field => ([field.name.text, this.getTypeOfNode(field.type)])));

        this.symbols[getFullName(node)] = typ;

        return typ;

      case SyntaxKind.BoltParameter:
        if (node.type !== null) {
          return this.getTypeOfNode(node.type)
        }
        return anyType;

      default:
        throw new Error(`Could not derive type of ${kindToString(node.kind)}`)

    }

  }

  getTypeNamed(name: string) {
    return name in this.symbols
      ? this.symbols[name]
      : null
  }

  getTypeOfNode(node: Syntax): Type {
    if (this.types.has(node)) {
      return this.types.get(node)!
    }
    const newType = this.createType(node)
    this.types.set(node, newType)
    return newType;
  }

  check(node: Syntax) {

    this.getTypeOfNode(node);

    switch (node.kind) {

      case SyntaxKind.BoltSentence:
      case SyntaxKind.BoltRecordDeclaration:
      case SyntaxKind.BoltNewTypeDeclaration:
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

  getImportedSymbols(node: BoltImportDeclaration) {
    return [{ name: 'fac' }]
  }

  getScope(node: Syntax): Scope {
    while (node.kind !== SyntaxKind.BoltFunctionDeclaration && node.kind !== SyntaxKind.BoltSourceFile) {
      node = node.parentNode!;
    }
    if (this.scopes.has(node)) {
      return this.scopes.get(node)!
    }
    const scope = new Scope(node)
    this.scopes.set(node, scope)
    return scope
  }

  protected intersectTypes(a: Type, b: Type): Type {
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

  // getMapperForNode(target: string, node: Syntax): Mapper {
  //   return this.getScope(node).getMapper(target)
  // }

}

