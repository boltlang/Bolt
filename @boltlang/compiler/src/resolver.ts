import { BoltSyntax, SyntaxKind, Syntax, BoltSourceFile, SourceFile, kindToString } from "./ast";
import { emitNode } from "./emitter";
import { isExported } from "./common";
import { Package } from "./package"
import { FastStringMap, assert, every } from "./util";
import { Program } from "./program";

const GLOBAL_SCOPE_ID = 'global';

export class SymbolPath {

  constructor(
    public modulePath: string[],
    public isAbsolute: boolean,
    public name: string
  ) {

  }

  public encode(): string {
    let out = '';
    if (this.isAbsolute) {
      out += '::'
    }
    for (const element of this.modulePath) {
      out += element + '::'
    }
    out += this.name;
    return out;
  }

  public hasModulePath(): boolean {
    return this.modulePath.length > 0;
  }

  public getModulePath() {
    return this.modulePath;
  }

}

export function convertNodeToSymbolPath(node: Syntax): SymbolPath {
  switch (node.kind) {
    case SyntaxKind.BoltRecordDeclaration:
      return new SymbolPath(
        [],
        false,
        node.name.text,
      );
    case SyntaxKind.BoltFunctionDeclaration:
      return new SymbolPath(
        [],
        false,
        emitNode(node.name),
      );
    case SyntaxKind.BoltReferenceExpression:
      return new SymbolPath(
        node.name.modulePath.map(id => id.text),
        node.name.isAbsolute,
        emitNode(node.name.name),
      );
    case SyntaxKind.BoltReferenceTypeExpression:
      return new SymbolPath(
        node.name.modulePath.map(id => id.text),
        node.name.isAbsolute,
        emitNode(node.name.name),
      );
    case SyntaxKind.BoltIdentifier:
      return new SymbolPath([], false, emitNode(node));
    case SyntaxKind.BoltQualName:
      const name = emitNode(node.name);
      if (node.modulePath === null) {
        return new SymbolPath([], false, name);
      }
      return new SymbolPath(node.modulePath.map(id => id.text), false, name);
    default:
      throw new Error(`Could not extract a symbol path from node ${kindToString(node.kind)}.`);
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
  getScopeTypes(node: Syntax): ScopeType[];
  getNextScopeSource(source: ScopeSource, kind: ScopeType): ScopeSource | null;
}

export class BoltSymbolResolutionStrategy implements ResolutionStrategy {

  public hasSymbol(node: Syntax): boolean {
    switch (node.kind) {
      case SyntaxKind.BoltModule:
      case SyntaxKind.BoltBindPattern:
      case SyntaxKind.BoltFunctionDeclaration:
      case SyntaxKind.BoltTypeAliasDeclaration:
      case SyntaxKind.BoltRecordDeclaration:
      case SyntaxKind.BoltTraitDeclaration:
      case SyntaxKind.BoltTypeParameter:
        return true;
      default:
        return false;
    }
  }

  public getSymbolName(node: Syntax): string {
    switch (node.kind) {
      case SyntaxKind.BoltTypeParameter:
        return node.name.text;
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
      default:
        throw new Error(`Could not derive symbol name of node ${kindToString(node.kind)}`)
    }
  }

  public getScopeTypes(node: Syntax): ScopeType[] {
    switch (node.kind) {
      case SyntaxKind.BoltVariableDeclaration:
      case SyntaxKind.BoltBindPattern:
        return [ ScopeType.Variable ];
      case SyntaxKind.BoltFunctionDeclaration:
        return [ ScopeType.Type, ScopeType.Variable ];
      case SyntaxKind.BoltTraitDeclaration:
      case SyntaxKind.BoltRecordDeclaration:
        return [ ScopeType.Type, ScopeType.Module ];
      case SyntaxKind.BoltFunctionDeclaration:
      case SyntaxKind.BoltImplDeclaration:
      case SyntaxKind.BoltTypeAliasDeclaration:
      case SyntaxKind.BoltTypeParameter:
        return [ ScopeType.Type ];
      case SyntaxKind.BoltModule:
        return [ ScopeType.Module ];
      default:
        throw new Error(`Could not derive scope type of node ${kindToString(node.kind)}.`)
    }
  }

  public introducesNewScope(source: ScopeSource, kind: ScopeType): boolean {
    if (source instanceof PackageScopeSource || source instanceof GlobalScopeSource) {
      return true;
    }
    if (source instanceof NodeScopeSource) {
      switch (kind) {
        case ScopeType.Variable:
          return source.node.kind === SyntaxKind.BoltSourceFile
              || source.node.kind === SyntaxKind.BoltModule
              || source.node.kind === SyntaxKind.BoltFunctionDeclaration
              || source.node.kind === SyntaxKind.BoltBlockExpression
              || source.node.kind === SyntaxKind.BoltImplDeclaration;
        case ScopeType.Type:
          return source.node.kind === SyntaxKind.BoltModule
              || source.node.kind === SyntaxKind.BoltSourceFile
              || source.node.kind === SyntaxKind.BoltFunctionDeclaration
              || source.node.kind === SyntaxKind.BoltRecordDeclaration
              || source.node.kind === SyntaxKind.BoltTraitDeclaration
              || source.node.kind === SyntaxKind.BoltImplDeclaration;
          case ScopeType.Module:
            return source.node.kind === SyntaxKind.BoltModule
                || source.node.kind === SyntaxKind.BoltRecordDeclaration
                || source.node.kind === SyntaxKind.BoltTraitDeclaration
                || source.node.kind === SyntaxKind.BoltSourceFile;
        default:
          throw new Error(`Invalid scope type detected.`)
      }
    }
    throw new Error(`Invalid source type detected.`)
  }

  public getNextScopeSource(source: ScopeSource, kind: ScopeType): ScopeSource | null {

    // If we are in the global scope, there is no scope above it.
    if (source instanceof GlobalScopeSource) {
      return null;
    }
    
    // If we are at a scope that was created by an AST node, we 
    // search the nearest parent that introduces a new scope of
    // the requested kind. If no such scope was found, then we
    // return the local package scope.
    if (source instanceof NodeScopeSource) {
      let currNode = source.node;
      while (true) {
        if (currNode.kind === SyntaxKind.BoltSourceFile) {
          return new PackageScopeSource(currNode.pkg);
        }
        const nextNode = currNode.parentNode;
        assert(nextNode !== null);
        if (this.introducesNewScope(new NodeScopeSource(nextNode!), kind)) {
          return new NodeScopeSource(nextNode!);
        }
        currNode = nextNode!;
      }
    }

    // If we already are at the local package scope level, we go one up 
    // to the global scope shared by all packages.
    if (source instanceof PackageScopeSource) {
      return new GlobalScopeSource();;
    }

    throw new Error(`Unknown scope source provided.`)
    
  }
  
}

let nextSymbolId = 1;

class Scope {

  private static scopeCache = new FastStringMap<string, Scope>();

  private nextScope: Scope | null | undefined;
  private symbols = new FastStringMap<string, SymbolInfo>();

  public static findOrCreate(resolver: SymbolResolver, kind: ScopeType, source: ScopeSource): Scope {
    const key = `${kind}:${source.id}`;
    if (Scope.scopeCache.has(key)) {
      return Scope.scopeCache.get(key);
    } else {
      const newScope = new Scope(resolver, kind, source);
      Scope.scopeCache.set(key, newScope);
      return newScope;
    }
  }

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
    let source: ScopeSource = this.source;
    while (!this.resolver.strategy.introducesNewScope(source, kind)) {
      const nextSource = this.resolver.strategy.getNextScopeSource(source, kind);
      assert(nextSource !== null);
      source = nextSource!;
    }
    return Scope.findOrCreate(this.resolver, kind, source);
  }

  public getNextScope(): Scope | null {
    let results = [];
    const nextSource = this.resolver.strategy.getNextScopeSource(this.source, this.kind);
    if (nextSource === null) {
      return null;
    }
    return Scope.findOrCreate(this.resolver, this.kind, nextSource);
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
    let currScope: Scope = this;
    while (true) {
      const sym = currScope.getLocalSymbol(name);
      if (sym !== null) {
        return sym;
      }
      const nextScope = currScope.getNextScope();
      if (nextScope === null) {
        break;
      }
      currScope = nextScope;
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
      if (this.strategy.hasSymbol(childNode)) {
        for (const scopeType of this.strategy.getScopeTypes(childNode)) {
          const name = this.strategy.getSymbolName(childNode);
          const scope = this.getScopeSurroundingNode(childNode, scopeType);
          assert(scope !== null);
          scope!.addNodeAsSymbol(name, childNode);
        }
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
                  const exported = this.resolveSymbolPath(convertNodeToSymbolPath(importSymbol), scope!);
                  if (exported !== null) {
                    for (const decl of exported.declarations) {
                      scope!.addNodeAsSymbol(this.strategy.getSymbolName(decl), decl);
                    }
                  }
                }
                break;
              default:
                throw new Error(`Could not import symbols due to an unknown node.`)
            }
          }
        } else {
          for (const exportedNode of this.getAllExportedNodes(sourceFile)) {
            for (const exportedScopeType of this.strategy.getScopeTypes(exportedNode)) {
              const importName = this.strategy.getSymbolName(exportedNode);
              const scope = this.getScopeForNode(importDir, exportedScopeType);
              assert(scope !== null);
              scope!.addNodeAsSymbol(importName, exportedNode);
            }
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
      const nextSource = this.strategy.getNextScopeSource(source, kind);
      if (nextSource === null) {
        return null;
      }
      source = nextSource;
    }
    return Scope.findOrCreate(this, kind, source);
  }

  public resolveModulePath(path: string[], scope: Scope): Scope | null {
  
    let modScope = scope.getScope(ScopeType.Module);

    // We will keep looping until we are at the topmost module of
    // the package corresponding to `node`.
    while (true) {

      let failedToFindScope = false;
      let currScope = modScope;

      // Go through each of the parent names in normal order, resolving to the module
      // that declared the name, and mark when we failed to look up the inner module.
      for (const name of path) {
        const sym = currScope.getLocalSymbol(name);
        if (sym === null) {
          failedToFindScope = true;
          break;
        }
        assert(every(sym.declarations.values(), decl => decl.kind === SyntaxKind.BoltModule));
        currScope = sym.scope;
      }

      // If the previous loop did not fail, we are done.
      if (!failedToFindScope) {
        return currScope.getScope(scope.kind);
      }

      // We continue the outer loop by going up one scope.
      const nextScope = modScope.getNextScope();
      if (nextScope === null) {
        break;
      }
      modScope = nextScope;

    }

    return null;
  }

  public getSymbolForNode(node: Syntax, kind: ScopeType) {
    assert(this.strategy.hasSymbol(node));
    const scope = this.getScopeForNode(node, kind);
    if (scope === null) {
      return null;
    }
    return scope.getSymbol(this.strategy.getSymbolName(node));
  }

  public resolveGlobalSymbol(path: SymbolPath, kind: ScopeType) {
    for (const sourceFile of this.program.getAllGloballyDeclaredSourceFiles()) {
      const scope = this.getScopeForNode(sourceFile, kind);
      assert(scope !== null);
      const modScope = this.resolveModulePath(path.getModulePath(), scope!);
      if (modScope === null) {
        continue;
      }
      const sym = modScope?.getLocalSymbol(path.name)
      if (sym !== null) {
        return sym;
      }
    }
    return null;
  }

  public resolveSymbolPath(path: SymbolPath, scope: Scope): SymbolInfo | null {

    if (path.hasModulePath()) {

      if (path.isAbsolute) {

        // TODO
 
      } else {

        // Perform the acutal module resolution.
        const resolvedScope = this.resolveModulePath(path.getModulePath(), scope);

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

    const sym = scope.getSymbol(path.name);
    if (sym !== null) {
      return sym
    }
    return this.resolveGlobalSymbol(path, scope.kind);
  }

}
