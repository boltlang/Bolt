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

import {Syntax, SyntaxKind, BoltReferenceExpression, BoltDeclaration, BoltSourceFile, BoltSyntax, BoltReferenceTypeExpression, BoltTypeDeclaration} from "./ast";
import {FastStringMap} from "./util";
import {DiagnosticPrinter, E_TYPES_NOT_ASSIGNABLE, E_TYPE_DECLARATION_NOT_FOUND, E_DECLARATION_NOT_FOUND} from "./diagnostics";

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

  }

  public registerSourceFile(node: BoltSourceFile): void {
    this.addAllSymbolsInNode(node);
  }

  private addAllSymbolsInNode(node: BoltSyntax): void {

    switch (node.kind) {

      case SyntaxKind.BoltSourceFile:
      case SyntaxKind.BoltModule:
        for (const element of node.elements) {
          this.addAllSymbolsInNode(element);
        }
        break;

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

