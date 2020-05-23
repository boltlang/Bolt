/**
 *
 * ```
 * mod foo {
 *   type MyType1 = i32;
 *   mod bar {
 *     pub type MyType2 = MyType1;
 *   }
 * }
 * ```
 *
 * ```
 * mod foo {
 *   let x = 1;
 *   mod bar {
 *     fn do_something(y) {
 *       return x + y;
 *     }
 *   }
 * }
 * ```
 *
 * Note that the `pub`-keyword is not present on `MyType1`.
 */

import {Syntax, SyntaxKind, BoltReferenceExpression, BoltDeclaration, BoltSourceFile, BoltSyntax, BoltReferenceTypeExpression, BoltTypeDeclaration, BoltExpression, BoltFunctionDeclaration, BoltFunctionBodyElement, kindToString, createBoltReferenceTypeExpression, createBoltIdentifier} from "./ast";
import {FastStringMap, memoize, assert} from "./util";
import {
  DiagnosticPrinter,
  E_TYPES_NOT_ASSIGNABLE,
  E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TYPE_DECLARATION_NOT_FOUND,
  E_DECLARATION_NOT_FOUND,
  E_INVALID_ARGUMENTS
} from "./diagnostics";
import {createAnyType, isOpaqueType, createOpaqueType, Type} from "./types";

interface SymbolInfo {
  declarations: BoltDeclaration[];
}

interface TypeSymbolInfo {
  declarations: BoltTypeDeclaration[];
}

function introducesNewScope(kind: SyntaxKind): boolean {
  return kind === SyntaxKind.BoltSourceFile
      || kind === SyntaxKind.BoltModule
      || kind === SyntaxKind.BoltFunctionDeclaration
      || kind === SyntaxKind.BoltBlockExpression;
}

function introducesNewTypeScope(kind: SyntaxKind): boolean {
  return kind === SyntaxKind.BoltModule
      || kind === SyntaxKind.BoltSourceFile;
}

type Scope = unknown;
type TypeScope = unknown;

function createSymbol(node: BoltDeclaration): SymbolInfo {
  return { declarations: [ node ] };
}

function createTypeSymbol(node: BoltTypeDeclaration): TypeSymbolInfo {
  return { declarations: [ node ] };
}

export class TypeChecker {

  constructor(private diagnostics: DiagnosticPrinter) {

  }

  private symbols = new FastStringMap<string, SymbolInfo>();
  private typeSymbols = new FastStringMap<string, TypeSymbolInfo>();

  public checkSourceFile(node: BoltSourceFile): void {

    const refExps = node.findAllChildrenOfKind(SyntaxKind.BoltReferenceExpression);
    for (const refExp of refExps) {
      if (this.resolveReferenceExpression(refExp) === null) {
        this.diagnostics.add({
          message: E_DECLARATION_NOT_FOUND,
          args: { name: refExp.name.name.text },
          severity: 'error',
          node: refExp,
        })
      }
    }

    const typeRefExps = node.findAllChildrenOfKind(SyntaxKind.BoltReferenceTypeExpression);
    for (const typeRefExp of typeRefExps) {
      if (this.resolveTypeReferenceExpression(typeRefExp) === null) {
        this.diagnostics.add({
          message: E_TYPE_DECLARATION_NOT_FOUND,
          args: { name: typeRefExp.name.name.text },
          severity: 'error',
          node: typeRefExp,
        })
      }
    }

    const callExps = node.findAllChildrenOfKind(SyntaxKind.BoltCallExpression);

    for (const callExp of callExps) {

      const fnDecls = this.getAllFunctionsInExpression(callExp.operator);

      for (const fnDecl of fnDecls) {

        if (fnDecl.params.length > callExp.operands.length) {
          this.diagnostics.add({
            message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
            args: { expected: fnDecl.params.length, actual: callExp.operands.length },
            severity: 'error',
            node: callExp,
          })
        }

        if (fnDecl.params.length < callExp.operands.length) {
          this.diagnostics.add({
            message: E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
            args: { expected: fnDecl.params.length, actual: callExp.operands.length },
            severity: 'error',
            node: callExp,
          })
        }

        const paramCount = fnDecl.params.length;
        for (let i = 0; i < paramCount; i++) {
          const arg = callExp.operands[i];
          const param = fnDecl.params[i];
          let argType = this.getTypeOfNode(arg);
          let paramType = this.getTypeOfNode(param);
          if (!this.isTypeAssignableTo(argType, paramType)) {
            this.diagnostics.add({
              message: E_INVALID_ARGUMENTS,
              severity: 'error',
              args: { name: fnDecl.name.text },
              node: arg,
            });
          }

        }

      }

    }

  }

  private resolveType(name: string, node: BoltSyntax): Type | null {
    const sym = this.findSymbolInTypeScopeOf(name, this.getTypeScopeSurroundingNode(node))
    if (sym === null) {
      return null;
    }
    return this.getTypeOfNode(sym.declarations[0]);
  }

  @memoize
  private getTypeOfNode(node: BoltSyntax): Type {
    switch (node.kind) {
      case SyntaxKind.BoltReferenceTypeExpression:
      {
        const referenced = this.resolveTypeReferenceExpression(node);
        if (referenced === null) {
          return createAnyType();
        }
        return this.getTypeOfNode(referenced);
      }
      case SyntaxKind.BoltRecordDeclaration:
      {
        if (node.members === null) {
          return createOpaqueType();
        }
        // TODO
        throw new Error(`Not yet implemented.`);
      }
      case SyntaxKind.BoltParameter:
      {
        let type: Type = createAnyType();
        if (node.type !== null) {
          type = this.getTypeOfNode(node.type);
        }
        return type;
      }
      case SyntaxKind.BoltConstantExpression:
      {
        let type;
        if (typeof node.value === 'string') {
          type = this.resolveType('String', node)!;
        } else if (typeof node.value === 'boolean') {
          type = this.resolveType('bool', node)!;
        } else if (typeof node.value === 'number') {
          type = this.resolveType('int32', node)!;
        } else {
          throw new Error(`Could not derive type of constant expression.`);
        }
        assert(type !== null);
        return type;
      }
      default:
          throw new Error(`Could not derive type of node ${kindToString(node.kind)}.`);
    }
  }

  private isTypeAssignableTo(left: Type, right: Type): boolean {
    if (isOpaqueType(left) &&  isOpaqueType(right)) {
      return left === right;
    }
    return false;
  }

  private getAllFunctionsInExpression(node: BoltExpression): BoltFunctionDeclaration[] {

    const self = this;

    const results: BoltFunctionDeclaration[] = [];
    visitExpression(node);
    return results;

    function visitExpression(node: BoltExpression) {
      switch (node.kind) {
        case SyntaxKind.BoltReferenceExpression:
          const resolved = self.resolveReferenceExpression(node);
          if (resolved !== null) {
            visitFunctionBodyElement(resolved);
          }
          break;
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

  public registerSourceFile(node: BoltSourceFile): void {
    this.addAllSymbolsInNode(node);
  }

  private addAllSymbolsInNode(node: BoltSyntax): void {

    switch (node.kind) {

      case SyntaxKind.BoltSourceFile:
      case SyntaxKind.BoltModule:
      {
        for (const element of node.elements) {
          this.addAllSymbolsInNode(element);
        }
        break;
      }

      case SyntaxKind.BoltFunctionDeclaration:
      {
        const scope = this.getScopeSurroundingNode(node);
        const sym = createSymbol(node);
        this.addSymbol(node.name.text, scope, sym);
        break;
      }

      case SyntaxKind.BoltRecordDeclaration:
      {
        const typeScope = this.getTypeScopeSurroundingNode(node);
        const typeSym = createTypeSymbol(node);
        this.addTypeSymbol(node.name.text, typeScope, typeSym);
      }

    }

  }

  private addSymbol(name: string, scope: Scope, sym: SymbolInfo): void {
    this.symbols.set(`${name}@${(scope as any).id}`, sym);
  }

  private addTypeSymbol(name: string, scope: TypeScope, sym: TypeSymbolInfo): void {
    this.typeSymbols.set(`${name}@${(scope as any).id}`, sym);
  }

  public getParentScope(scope: Scope): Scope | null {
    let node = scope as Syntax;
    if (node.kind === SyntaxKind.BoltSourceFile) {
      return null;
    }
    node = node.parentNode!;
    while (!introducesNewScope(node.kind)) {
      node = node.parentNode!;
    }
    return node;
  }

  public getParentTypeScope(scope: TypeScope): TypeScope | null {
    let node = scope as Syntax;
    if (node.kind === SyntaxKind.BoltSourceFile) {
      return null;
    }
    node = node.parentNode!;
    while (!introducesNewTypeScope(node.kind)) {
      node = node.parentNode!;
    }
    return node;
  }

  private getScopeSurroundingNode(node: Syntax): Scope {
    if (node.kind === SyntaxKind.BoltSourceFile) {
      return node;
    }
    return this.getScopeForNode(node.parentNode);
  }

  private getTypeScopeSurroundingNode(node: Syntax): TypeScope {
    if (node.kind === SyntaxKind.BoltSourceFile) {
      return node;
    }
    return this.getScopeForNode(node.parentNode);
  }

  private getScopeForNode(node: Syntax): Scope {
    if (node.kind === SyntaxKind.BoltSourceFile) {
      return node;
    }
    let currNode = node;
    while (!introducesNewScope(currNode.kind)) {
      currNode = currNode.parentNode!;
    }
    return currNode;
  }

  private getTypeScopeForNode(node: Syntax): TypeScope {
    if (node.kind === SyntaxKind.BoltSourceFile) {
      return node;
    }
    let currNode = node;
    while (!introducesNewTypeScope(currNode.kind)) {
      currNode = currNode.parentNode!;
    }
    return currNode;
  }

  private lookupSymbolInScope(name: string, scope: Scope): SymbolInfo | null {
    const key = `${name}@${(scope as any).id}`;
    if (!this.symbols.has(key)) {
      return null;
    }
    return this.symbols.get(key);
  }

  private lookupSymbolInTypeScope(name: string, scope: TypeScope): TypeSymbolInfo | null {
    const key = `${name}@${(scope as any).id}`;
    if (!this.typeSymbols.has(key)) {
      return null;
    }
    return this.typeSymbols.get(key);
  }

  public findSymbolInScopeOf(name: string, scope: Scope): SymbolInfo | null {
    while (true) {
      const sym = this.lookupSymbolInScope(name, scope);
      if (sym !== null) {
        return sym;
      }
      const parentScope = this.getParentScope(scope);
      if (parentScope === null) {
        break;
      }
      scope = parentScope;
    }
    return null;
  }

  public findSymbolInTypeScopeOf(name: string, scope: TypeScope): TypeSymbolInfo | null {
    while (true) {
      const sym = this.lookupSymbolInTypeScope(name, scope);
      if (sym !== null) {
        return sym;
      }
      const parentTypeScope = this.getParentTypeScope(scope);
      if (parentTypeScope === null) {
        break;
      }
      scope = parentTypeScope;
    }
    return null;
  }

  public resolveReferenceExpression(node: BoltReferenceExpression): BoltDeclaration | null {
    let scope = this.getScopeSurroundingNode(node);
    if (node.name.modulePath !== null) {
      while (true) {
        let shouldSearchParentScopes = false;
        let currScope = scope;
        for (const name of node.name.modulePath) {
          const sym = this.lookupSymbolInScope(name.text, currScope);
          if (sym === null) {
            shouldSearchParentScopes = true;
            break;
          }
          if (sym.declarations[0].kind !== SyntaxKind.BoltModule) {
            shouldSearchParentScopes = true;
            break;
          }
          currScope = this.getScopeForNode(sym.declarations[0]);
        }
        if (!shouldSearchParentScopes) {
          scope = currScope;
          break;
        }
        const parentScope = this.getParentScope(scope);
        if (parentScope === null) {
          return null;
        }
        scope = parentScope;
      }
    }
    const sym = this.findSymbolInScopeOf(node.name.name.text, scope);
    if (sym === null) {
      return null;
    }
    return sym.declarations[0]!;
  }

  public resolveTypeReferenceExpression(node: BoltReferenceTypeExpression): BoltTypeDeclaration | null {
    const typeScope = this.getTypeScopeSurroundingNode(node);
    const typeSym = this.findSymbolInTypeScopeOf(node.name.name.text, typeScope);
    if (typeSym === null) {
      return null;
    }
    return typeSym.declarations[0]!;
  }

}

