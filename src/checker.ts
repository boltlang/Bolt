
import {
  Syntax,
  SyntaxKind,
  ImportDecl,
  isNode,
} from "./ast"

import { FastStringMap } from "./util"

export class Type {
  
}

export class PrimType extends Type {

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

  protected createType(node: Syntax): Type {

    console.log(node)

    switch (node.kind) {

      case SyntaxKind.ConstExpr:
        return node.value.type;

      case SyntaxKind.RecordDecl:

        const typ = new RecordType(map(node.fields, ([name, typ]) => ([name.text, typ])));

        this.symbols[getFullName(node)] = typ;

        return typ;

        // if (typeof node.value === 'bigint') {
        //   return intType;
        // } else if (typeof node.value === 'string') {
        //   return stringType;
        // } else if (typeof node.value === 'boolean') {
        //   return boolType;
        // } else if (isNode(node.value)) {
        //   return this.getTypeNamed(`Bolt.AST.${SyntaxKind[node.value.kind]}`)!
        // } else {
        //   throw new Error(`Unrecognised kind of value associated with ConstExpr`)
        // }

      default:
        throw new Error(`Could not derive type of ${SyntaxKind[node.kind]}`)

    }

  }

  getTypeNamed(name: string) {
    return name in this.typeNames
      ? this.typeNames[name]
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

    switch (node.kind) {

      case SyntaxKind.Sentence:
        break;

      case SyntaxKind.RecordDecl:
        this.getTypeOfNode(node);
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

  // getMapperForNode(target: string, node: Syntax): Mapper {
  //   return this.getScope(node).getMapper(target)
  // }

}

