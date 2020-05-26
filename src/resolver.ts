import { BoltSyntax, SyntaxKind, Syntax, BoltSourceFile, SourceFile, kindToString } from "./ast";
import { emitNode } from "./emitter";
import { Package, isExported } from "./common";
import { FastStringMap, assert, every } from "./util";
import { Program } from "./program";

const GLOBAL_SCOPE_ID = 'global';

export class SymbolPath {

  constructor(
    private parents: string[],
    public isAbsolute: boolean,
    public name: string
  ) {

  }

  public hasParents(): boolean {
    return this.parents.length > 0;
  }

  public getParents() {
    return this.parents;
  }

}

export function getSymbolPathFromNode(node: BoltSyntax): SymbolPath {
  switch (node.kind) {
    case SyntaxKind.BoltReferenceExpression:
      return new SymbolPath(
        node.modulePath === null ? [] : node.modulePath.elements.map(id => id.text),
        node.modulePath !== null && node.modulePath.isAbsolute,
        emitNode(node.name),
      );
    case SyntaxKind.BoltIdentifier:
      return new SymbolPath([], false, emitNode(node));
    case SyntaxKind.BoltQualName:
      const name = emitNode(node.name);
      if (node.modulePath === null) {
        return new SymbolPath([], false, name);
      }
      return new SymbolPath(node.modulePath.map(id => id.text), false, name);
    case SyntaxKind.BoltModulePath:
      return new SymbolPath(
        node.elements.slice(0, -1).map(el => el.text),
        node.isAbsolute,
        node.elements[node.elements.length-1].text
      );
    default:
      throw new Error(`Could not extract a symbol path from the given node.`);
  }
}

export enum ScopeType {
  Type     = 0x1,
  Variable = 0x2,
  Module   = 0x4,
  Any      = Type | Variable,
}

function* getAllSymbolKinds() {
  for (let i = 1; i <= ScopeType.Any; i *= 2) {
    yield i;
  }
}

interface ScopeSource {
  readonly id: string;
}

class PackageScopeSource implements ScopeSource {

  constructor(public pkg: Package) {

  }

  public get id() {
    return `pkg:${this.pkg.id}`
  }

}

class GlobalScopeSource {

  public get id() {
    return GLOBAL_SCOPE_ID;
  }

}

class NodeScopeSource implements ScopeSource {

  constructor(public node: Syntax) {

  }

  public get id() {
    return `node:${this.node.id}`
  }

}

interface ResolutionStrategy {
  getSymbolName(node: Syntax): string;
  getScopeType(node: Syntax): ScopeType;
  getNextScopeSources(source: ScopeSource, kind: ScopeType): IterableIterator<ScopeSource>;
}

export class BoltSymbolResolutionStrategy implements ResolutionStrategy {

  public hasSymbol(node: Syntax): boolean {
    switch (node.kind) {
      case SyntaxKind.BoltModule:
      case SyntaxKind.BoltBindPattern:
      case SyntaxKind.BoltFunctionDeclaration:
      case SyntaxKind.BoltTypeAliasDeclaration:
      case SyntaxKind.BoltRecordDeclaration:
      case SyntaxKind.BoltBindPattern:
      case SyntaxKind.BoltTraitDeclaration:
      case SyntaxKind.BoltImplDeclaration:
        return true;
      default:
        return false;
    }
  }

  public getSymbolName(node: Syntax): string {
    switch (node.kind) {
      case SyntaxKind.BoltModule:
        return node.name[node.name.length-1].text;
      case SyntaxKind.BoltBindPattern:
        return node.name.text;
      case SyntaxKind.BoltFunctionDeclaration:
        return emitNode(node.name);
      case SyntaxKind.BoltTypeAliasDeclaration:
        return node.name.text;
      case SyntaxKind.BoltRecordDeclaration:
        return node.name.text;
      case SyntaxKind.BoltBindPattern:
        return node.name.text;
      case SyntaxKind.BoltTraitDeclaration:
        return node.name.text;
      case SyntaxKind.BoltImplDeclaration:
        return node.name.text;
      default:
        throw new Error(`Could not derive symbol name of node ${kindToString(node.kind)}`)
    }
  }

  public getScopeType(node: Syntax): ScopeType {
    switch (node.kind) {
      case SyntaxKind.BoltVariableDeclaration:
      case SyntaxKind.BoltFunctionDeclaration:
        return ScopeType.Variable;
      case SyntaxKind.BoltImplDeclaration:
      case SyntaxKind.BoltTraitDeclaration:
      case SyntaxKind.BoltTypeAliasDeclaration:
      case SyntaxKind.BoltRecordDeclaration:
        return ScopeType.Type;
      case SyntaxKind.BoltModule:
        return ScopeType.Module;
      default:
        throw new Error(`Could not derive scope type of node ${kindToString(node.kind)}.`)
    }
  }

  public introducesNewScope(node: Syntax, kind: ScopeType): boolean {
    switch (kind) {
      case ScopeType.Variable:
        return node.kind === SyntaxKind.BoltSourceFile
            || node.kind === SyntaxKind.BoltModule
            || node.kind === SyntaxKind.BoltFunctionDeclaration
            || node.kind === SyntaxKind.BoltBlockExpression;
      case ScopeType.Type:
        return node.kind === SyntaxKind.BoltModule
            || node.kind === SyntaxKind.BoltSourceFile
            || node.kind === SyntaxKind.BoltFunctionDeclaration
            || node.kind === SyntaxKind.BoltRecordDeclaration
            || node.kind === SyntaxKind.BoltTraitDeclaration
            || node.kind === SyntaxKind.BoltImplDeclaration;
        case ScopeType.Module:
          return node.kind === SyntaxKind.BoltModule
              || node.kind === SyntaxKind.BoltRecordDeclaration
              || node.kind === SyntaxKind.BoltSourceFile;
      default:
        throw new Error(`Invalid scope type detected.`)
    }
  }

  public *getNextScopeSources(source: ScopeSource, kind: ScopeType): IterableIterator<ScopeSource> {

    // If we are in the global scope, there is no scope above it.
    if (source instanceof GlobalScopeSource) {
      return;
    }
    
    // If we are at a scope that was created by an AST node, we 
    // search the nearest parent that introduces a new scope of
    // the requested kind. If no such scope was found, then we
    // return the local package scope.
    if (source instanceof NodeScopeSource) {
      let currNode = source.node;
      while (true) {
        if (currNode.kind === SyntaxKind.BoltSourceFile) {
          yield new PackageScopeSource(currNode.package);
          return;
        }
        const nextNode = currNode.parentNode;
        assert(nextNode !== null);
        if (this.introducesNewScope(nextNode, kind)) {
          yield new NodeScopeSource(nextNode);
          return;
        }
        currNode = nextNode;
      }
    }

    // If we already are at the local package scope level, we go one up 
    // to the global scope shared by all packages.
    if (source instanceof PackageScopeSource) {
      yield new GlobalScopeSource();
      return;
    }

    throw new Error(`Unknown scope source provided.`)
    
  }
  
}

let nextSymbolId = 1;

class Scope {

  private static scopeCache = new FastStringMap<string, Scope>();

  private nextScope: Scope | null | undefined;
  private symbols = new FastStringMap<string, SymbolInfo>();

  constructor(
    private resolver: SymbolResolver,
    public kind: ScopeType,
    public source: ScopeSource,
  ) {

  }

  private get globallyUniqueKey() {
    return `${this.kind}:${this.source.id}`;
  }

  public getScope(kind: ScopeType) {
    if (Scope.scopeCache.has(this.globallyUniqueKey)) {
      return Scope.scopeCache.get(this.globallyUniqueKey);
    }
    const newScope = new Scope(this.resolver, kind, this.source);
    Scope.scopeCache.set(newScope.globallyUniqueKey, newScope);
    return newScope;
  }

  public *getNextScopes(): IterableIterator<Scope> {
    let results = [];
    for (const nextSource of this.resolver.strategy.getNextScopeSources(this.source, this.kind)) {
      const key = `${this.kind}:${nextSource.id}`;
      if (Scope.scopeCache.has(key)) {
        yield Scope.scopeCache.get(key);
      } else {
        const newScope = new Scope(this.resolver, this.kind, nextSource);
        Scope.scopeCache.set(key, newScope);
        yield newScope;
      }
    }
  }

  public getLocalSymbol(name: string) {
    if (!this.symbols.has(name)) {
      return null;
    }
    return this.symbols.get(name);
  }
  
  public getExportedSymbol(name: string) {
    if (!this.symbols.has(name)) {
      return null;
    }
    const sym = this.symbols.get(name);
    if (!sym.isExported) {
      return null;
    }
    return sym;
  }

  public getSymbol(name: string): SymbolInfo | null {
    const stack: Scope[] = [ this ];
    while (stack.length > 0) {
      const currScope = stack.pop()!;
      const sym = currScope.getLocalSymbol(name);
      if (sym !== null) {
        return sym;
      }
      for (const nextScope of currScope.getNextScopes()) {
        stack.push(nextScope);
      }
    }
    return null;
  }

  public addNodeAsSymbol(name: string, node: Syntax) {
    if (this.symbols.has(name)) {
      const sym = this.symbols.get(name);
      if (!sym.declarations.has(node)) {
        sym.declarations.add(node);
      }
    } else {
      const sym = {
        id: nextSymbolId++,
        name,
        scope: this,
        declarations: new Set([ node ]),
        isExported: isExported(node)
      } as SymbolInfo;
      this.symbols.set(name, sym);
    }
  }

}

export interface SymbolInfo {
  id: number;
  name: string;
  scope: Scope;
  isExported: boolean;
  declarations: Set<Syntax>,
}

export class SymbolResolver {

  constructor(
    private program: Program,
    public strategy: BoltSymbolResolutionStrategy
  ) {

  }

  private symbols = new FastStringMap<string, SymbolInfo>();

  public registerSourceFile(node: SourceFile): void {

    for (const childNode of node.preorder()) {
      if (this.strategy.hasSymbol(node)) {
        const name = this.strategy.getSymbolName(node);
        const scope = this.getScopeForNode(node, this.strategy.getScopeType(node));
        assert(scope !== null);
        scope!.addNodeAsSymbol(name, node);
      }
    }

    for (const importDir of node.findAllChildrenOfKind(SyntaxKind.BoltImportDirective)) {
      const sourceFile = this.program.resolveToSourceFile(importDir.file.value, importDir) as BoltSourceFile;
      if (sourceFile !== null) {
        if (importDir.symbols !== null) {
          for (const importSymbol of importDir.symbols) {
            switch (importSymbol.kind) {
              case SyntaxKind.BoltPlainImportSymbol:
                for (const scopeType of getAllSymbolKinds()) {
                  const scope = this.getScopeForNode(importDir, scopeType);
                  assert(scope !== null);
                  const exported = this.resolveSymbolPath(getSymbolPathFromNode(importSymbol), scope!);
                  if (exported !== null) {
                    for (const decl of exported.declarations) {
                      scope!.addNodeAsSymbol(this.strategy.getSymbolName(decl), decl);
                    }
                  }
                }
            }
          }
        } else {
          for (const exportedNode of this.getAllExportedNodes(sourceFile)) {
            const scope = this.getScopeForNode(importDir, this.strategy.getScopeType(exportedNode));
            assert(scope !== null);
            scope!.addNodeAsSymbol(this.strategy.getSymbolName(exportedNode), exportedNode);
          }
        }
      }
    }

  }

  private *getAllExportedNodes(node: SourceFile): IterableIterator<Syntax> {
    for (const element of node.elements) {
      if (this.strategy.hasSymbol(element)) {
        if (isExported(element)) {
           yield element;
        }
      }
    }
  }

  public getScopeSurroundingNode(node: Syntax, kind: ScopeType): Scope | null {
    assert(node.parentNode !== null);
    return this.getScopeForNode(node.parentNode!, kind);
  }

  public getScopeForNode(node: Syntax, kind: ScopeType): Scope | null {
    let source: ScopeSource = new NodeScopeSource(node);
    if (!this.strategy.introducesNewScope(source, kind)) {
      const sources = [...this.strategy.getNextScopeSources(source, kind)];
      if (sources.length === 0) {
        return null;
      }
      assert(sources.length === 1);
      source = sources[0];
    }
    return new Scope(this, kind, source);
  }

  public resolveModulePath(path: string[], scope: Scope): Scope | null {

    const stack: Scope[] = [ scope ];

    // We will keep looping until we are at the topmost module of
    // the package corresponding to `node`.
    while (stack.length > 0) {

      let shouldSearchNextScopes = false;
      let scope = stack.pop()!;
      let currScope = scope;

      // Go through each of the parent names in normal order, resolving to the module
      // that declared the name, and mark when we failed to look up the inner module.
      for (const name of path) {
        const sym = currScope.getLocalSymbol(name);
        if (sym === null) {
          shouldSearchNextScopes = true;
          break;
        }
        assert(every(sym.declarations.values(), decl => decl.kind === SyntaxKind.BoltModule));
        currScope = sym.scope;
      }

      // If the previous loop did not fail, we are done.
      if (!shouldSearchNextScopes) {
        return currScope;
      }

      // We continue the outer loop by going up one scope.
      for (const nextScope of scope.getNextScopes()) {
        stack.push(nextScope);
      }

    }

    return null;
  }

  public getSymbolForNode(node: Syntax) {
    assert(this.strategy.hasSymbol(node));
    const scope = this.getScopeForNode(node, this.strategy.getScopeType(node));
    if (scope === null) {
      return null;
    }
    return scope.getSymbol(this.strategy.getSymbolName(node));
  }

  public resolveSymbolPath(path: SymbolPath, scope: Scope): SymbolInfo | null {

    if (path.hasParents()) {

      if (path.isAbsolute) {

        // TODO
 
      } else {

        // Perform the acutal module resolution.
        const resolvedScope = this.resolveModulePath(path.getParents(), scope);

        // Failing to find any module means that we cannot continue, because
        // it does not make sense to get the symbol of a non-existent module.
        if (resolvedScope === null) {
          return null;
        }
        scope = resolvedScope;

      }

    }

    // Once we've handled any module path that might have been present,
    // we resolve the actual symbol using a helper method.

    const sym = scope.getExportedSymbol(path.name);

    if (sym === null) {
      return null;
    }

    return sym;
  }

}