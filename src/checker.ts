
import {
  Syntax,
  SyntaxKind
} from "./ast"

class Type {
  
}

interface FastStringMap<T> {
  [key: string]: T
}

export class Scope {

  constructor(public origin: Syntax) {

  }

}

export class TypeChecker {

  protected stringType = new Type();
  protected intType = new Type();

  protected scopes = new Map<Syntax, Scope>();

  createType(node: Syntax) {
    switch (node.kind) {
      case SyntaxKind.ConstExpr:
        if (typeof node.value === 'bigint') {
          return this.intType;
        } else if (typeof node.value === 'string') {
          return this.stringType;
        }
    }
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

  getMapperForNode(target: string, node: Syntax): Mapper {
    return this.getScope(node).getMapper(target)
  }

}

