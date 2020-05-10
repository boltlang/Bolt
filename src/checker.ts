
import {
  Syntax,
  SyntaxKind,
  ImportDecl,
  Patt,
} from "./ast"

import { FastStringMap } from "./util"

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
      case SyntaxKind.Identifier:
        out.unshift(curr.text)
        break;
      case SyntaxKind.Module:
        out.unshift(curr.name.fullText);
        break;
      case SyntaxKind.RecordDecl:
        out.unshift(curr.name.fullText)
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

  protected inferTypeFromUsage(bindings: Patt, body: Body) {
    return anyType;
  }

  protected getTypeOfBody(body: Body) {
    return anyType;
  }

  protected createType(node: Syntax): Type {

    switch (node.kind) {

      case SyntaxKind.RefExpr:
        return anyType;

      case SyntaxKind.ConstExpr:
        return node.value.type;

      case SyntaxKind.NewTypeDecl:
        console.log(getFullName(node.name))
        this.symbols[getFullName(node.name)] = new PrimType();
        return noneType;

      case SyntaxKind.FuncDecl:
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

      case SyntaxKind.TypeRef:
        const reffed = this.getTypeNamed(node.name.fullText);
        if (reffed === null) {
          throw new Error(`Could not find a type named '${node.name.fullText}'`);
        }
        return reffed;

      case SyntaxKind.RecordDecl:

        const typ = new RecordType(map(node.fields, ([name, typ]) => ([name.text, typ])));

        this.symbols[getFullName(node)] = typ;

        return typ;

      case SyntaxKind.Param:
        if (node.typeDecl !== null) {
          return this.getTypeOfNode(node.typeDecl)
        }
        return anyType;

      default:
        throw new Error(`Could not derive type of ${SyntaxKind[node.kind]}`)

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

      case SyntaxKind.Sentence:
      case SyntaxKind.RecordDecl:
      case SyntaxKind.NewTypeDecl:
        break;

      case SyntaxKind.FuncDecl:
        if (node.body !== null) {
          if (Array.isArray(node.body)) {
            for (const element of node.body) {
              this.check(element)
            }
          }
        }
        break;

      case SyntaxKind.RefExpr:
        // TODO implement this
        break;

      case SyntaxKind.Module:
      case SyntaxKind.SourceFile:
        for (const element of node.elements) {
          this.check(element)
        }
        break;

      default:
        throw new Error(`Could not type-check node ${SyntaxKind[node.kind]}`)

    }

  }

  getImportedSymbols(node: ImportDecl) {
    return [{ name: 'fac' }]
  }

  getScope(node: Syntax): Scope {
    while (node.kind !== SyntaxKind.FuncDecl && node.kind !== SyntaxKind.SourceFile) {
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

