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

import {
  isSyntax,
  Syntax,
  SyntaxKind,
  BoltReferenceExpression,
  BoltDeclaration,
  BoltSourceFile,
  BoltSyntax,
  BoltTypeDeclaration,
  BoltExpression,
  BoltFunctionDeclaration,
  BoltFunctionBodyElement,
  kindToString,
  BoltStatement,
  BoltTypeExpression,
  BoltSourceElement,
  isBoltStatement,
  isBoltDeclaration,
  isSourceFile,
  BoltReferenceTypeExpression,
  isBoltTypeDeclaration,
  SourceFile,
  BoltModifiers
} from "./ast";
import {FastStringMap, countDigits, assert, verbose} from "./util";
import {
  DiagnosticPrinter,
  E_TYPES_NOT_ASSIGNABLE,
  E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TYPE_DECLARATION_NOT_FOUND,
  E_DECLARATION_NOT_FOUND,
  E_INVALID_ARGUMENTS,
  E_FILE_NOT_FOUND,
} from "./diagnostics";
import { createAnyType, isOpaqueType, createOpaqueType, Type, createVoidType, createVariantType, isVoidType } from "./types";
import { getReturnStatementsInFunctionBody, Package } from "./common";
import {emit} from "./emitter";
import {Program} from "./program";
import {type} from "os";

// TODO
const GLOBAL_SCOPE_ID = 0;

class SymbolPath {

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

function nodeToSymbolPath(node: BoltSyntax): SymbolPath {
  switch (node.kind) {
    case SyntaxKind.BoltIdentifier:
      return new SymbolPath([], false, emit(node));
    case SyntaxKind.BoltQualName:
      const name = emit(node.name);
      if (node.modulePath === null) {
        return new SymbolPath([], false, name);
      }
      return new SymbolPath(node.modulePath.map(id => id.text), false, name);
    default:
      throw new Error(`Could not extract a symbol path from the given node.`);
  }
}

enum SymbolKind {
  Type     = 0x1,
  Variable = 0x2,
  Module   = 0x4,
}

function* getAllSymbolKindsInMask(symbolKindMask: SymbolKind) {
  const n = countDigits(symbolKindMask, 2);
  for (let i = 1; i <= n; i++) {
    if ((symbolKindMask & i) > 0) {
      yield i;
    }
  }
}

export interface ScopeInfo {
  id: number;
  declaration: BoltSyntax | Package;
  parentScope?: ScopeInfo;
  kind: SymbolKind,
}

interface SymbolInfo {
  kind: SymbolKind;
  declarations: BoltSyntax[];
}

export class TypeChecker {

  constructor(
    private diagnostics: DiagnosticPrinter,
    private program: Program
  ) {

  }

  private symbols = new FastStringMap<string, SymbolInfo>();

  public checkSourceFile(node: BoltSourceFile): void {

    const self = this;
    for (const element of node.elements) {
      visitSourceElement(element);
    }

    function visitExpression(node: BoltExpression) {

      switch (node.kind) {

        case SyntaxKind.BoltConstantExpression:
          break;

        case SyntaxKind.BoltReferenceExpression:
        {
          if (self.resolveReferenceExpression(node) === null) {
            self.diagnostics.add({
              message: E_DECLARATION_NOT_FOUND,
              args: { name: emit(node.name.name) },
              severity: 'error',
              node: node,
            })
          }
          break;
        }

        case SyntaxKind.BoltCallExpression:
        {

          const fnDecls = self.getAllFunctionsInExpression(node.operator);

          for (const fnDecl of fnDecls) {

            if (fnDecl.params.length > node.operands.length) {

              self.diagnostics.add({
                message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
                args: { expected: fnDecl.params.length, actual: node.operands.length },
                severity: 'error',
                node: node,
              });

            } else if (fnDecl.params.length < node.operands.length) {

              self.diagnostics.add({
                message: E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
                args: { expected: fnDecl.params.length, actual: node.operands.length },
                severity: 'error',
                node: node,
              });

            } else {

              const paramCount = fnDecl.params.length;
              for (let i = 0; i < paramCount; i++) {
                const arg = node.operands[i];
                const param = fnDecl.params[i];
                let argType = self.getTypeOfNode(arg);
                let paramType = self.getTypeOfNode(param);
                if (!self.isTypeAssignableTo(argType, paramType)) {
                  self.diagnostics.add({
                    message: E_INVALID_ARGUMENTS,
                    severity: 'error',
                    args: { name: fnDecl.name.text },
                    node: arg,
                  });
                }
              }

            }

          }

          break;
        }

        default:
          throw new Error(`Unknown node of type ${kindToString(node.kind)}.`);

      }

    }

    function visitTypeExpressionn(node: BoltTypeExpression) {

      switch (node.kind) {

        case SyntaxKind.BoltReferenceTypeExpression:
        {
          if (self.resolveTypeReferenceExpression(node) === null) {
            self.diagnostics.add({
              message: E_TYPE_DECLARATION_NOT_FOUND,
              args: { name: emit(node.name.name) },
              severity: 'error',
              node: node,
            })
          }
          break;
        }

        default:
          throw new Error(`Unknown node of type ${kindToString(node.kind)}.`);
      }
    }

    function visitDeclaration(node: BoltDeclaration) {

      switch (node.kind) {

        case SyntaxKind.BoltRecordDeclaration:
        {
          if (node.members !== null) {
            for (const member of node.members) {
              if (member.kind === SyntaxKind.BoltRecordField) {
                visitTypeExpressionn(member.type);
              }
            }
          }
          break;
        }

        case SyntaxKind.BoltTypeAliasDeclaration:
        {
          // TODO
          break;
        }

        case SyntaxKind.BoltTraitDeclaration:
        {
          // TODO
          break;
        }

        case SyntaxKind.BoltImplDeclaration:
        {
          // TODO
          break;
        }

        case SyntaxKind.BoltFunctionDeclaration:
        {
          let fnReturnType: Type = createAnyType();

          if (node.returnType !== null) {
            fnReturnType = self.getTypeOfNode(node.returnType);
          }

          if (node.body !== null) {
            const returnStmts = getReturnStatementsInFunctionBody(node.body)
            const validReturnTypes: Type[] = [];
            for (const returnStmt of returnStmts) {
              if (returnStmt.value === null) {
                if (!isVoidType(fnReturnType)) {
                  self.diagnostics.add({
                    message: E_MUST_RETURN_A_VALUE,
                    node: returnStmt,
                    severity: 'error',
                  });
                }
              } else {
                checkExpressionMatchesType(returnStmt.value, fnReturnType);
              }
              //const returnType = self.getTypeOfNode(returnStmt);
              //if (!self.isTypeAssignableTo(fnReturnType, returnType)) {
                //self.diagnostics.add({
                  //severity: 'error',
                  //node: returnStmt.value !== null ? returnStmt.value : returnStmt,
                  //args: { left: fnReturnType, right: returnType },
                  //message: E_TYPES_NOT_ASSIGNABLE,
                //});
              //} else {
                //validReturnTypes.push(returnType);
              //}
            }
          }

          // TODO Sort the return types and find the largest types, eliminating types that fall under other types.
          //      Next, add the resulting types as type hints to `fnReturnType`.

          break;
        }

        default:
          throw new Error(`Unknown node of type ${kindToString(node.kind)}.`);

      }

    }

    function checkExpressionMatchesType(node: BoltExpression, expectedType: Type) {
      switch (node.kind) {
        case SyntaxKind.BoltMatchExpression:
        {
          for (const matchArm of node.arms) {
            checkExpressionMatchesType(matchArm.body, expectedType);
          }
          break;
        }
        default:
        {
          const actualType = self.getTypeOfNode(node);
          if (!self.isTypeAssignableTo(expectedType, actualType)) {
            self.diagnostics.add({
              severity: 'error',
              message: E_TYPES_NOT_ASSIGNABLE,
              args: { left: expectedType, right: actualType },
              node,
            });
          }
          break;
        }
      }
    }

    function visitStatement(node: BoltStatement) {
      switch (node.kind) {
        case SyntaxKind.BoltExpressionStatement:
          // TODO check for values that should be unwrapped
          visitExpression(node.expression);
          break;
        case SyntaxKind.BoltReturnStatement:
          if (node.value !== null) {
            visitExpression(node.value);
          }
          break;

        default:
          throw new Error(`Unknown node of type ${kindToString(node.kind)}.`);
      }
    }

    function visitSourceElement(node: BoltSourceElement) {
      if (isBoltStatement(node)) {
        visitStatement(node);
      } else if (isBoltDeclaration(node)) {
        visitDeclaration(node);
      } else if (node.kind === SyntaxKind.BoltModule) {
        for (const element of node.elements) {
          visitSourceElement(element);
        }
      } else if (node.kind !== SyntaxKind.BoltImportDirective) {
        throw new Error(`Unknown node of kind ${kindToString(node.kind)}`);
      }
    }

  }

  private resolveTypeName(name: string, node: BoltSyntax): Type | null {
    const sym = this.findSymbolInScopeOf(name, this.getScopeSurroundingNode(node));
    if (sym === null) {
      return null;
    }
    return this.getTypeOfNode(sym.declarations[0]);
  }

  private createType(node: BoltSyntax): Type {
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
      case SyntaxKind.BoltReturnStatement:
      {
        if (node.value === null) {
          return createVoidType();
        }
        return this.getTypeOfNode(node.value)
      }
      case SyntaxKind.BoltConstantExpression:
      {
        return node.value.getType();
        //if (typeof node.value === 'string') {
        //  type = this.resolveTypeName('String', node)!;
        //} else if (typeof node.value === 'boolean') {
        //  type = this.resolveTypeName('bool', node)!;
        //} else if (typeof node.value === 'bigint') {
        //  type = this.resolveTypeName('i32', node)!;
        //} else {
        //  throw new Error(`Could not derive type of constant expression.`);
        //}
        //assert(type !== null);
        //return type;
      }
      case SyntaxKind.BoltMatchExpression:
      {
        return createVariantType(...node.arms.map(arm => this.getTypeOfNode(arm.body)));
      }
      default:
          throw new Error(`Could not derive type of node ${kindToString(node.kind)}.`);
    }
  }

  private getTypeOfNode(node: BoltSyntax): Type {
    if (node._type !== undefined) {
      return node._type;
    }
    const type = this.createType(node);
    node._type = type;
    return type;
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
        {
          const resolved = self.resolveReferenceExpression(node);
          if (resolved !== null) {
            visitFunctionBodyElement(resolved);
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

  public registerSourceFile(node: BoltSourceFile): void {

    const self = this;

    addAllSymbolsToScope(
      node,
      this.getScopeForNode(node, SymbolKind.Variable),
      this.getScopeForNode(node, SymbolKind.Type),
      this.getScopeForNode(node, SymbolKind.Module)
    );

    function addAllSymbolsToScope(node: BoltSyntax, variableScope: ScopeInfo, typeScope: ScopeInfo, moduleScope: ScopeInfo, allowDuplicates = false): void {

      switch (node.kind) {

        case SyntaxKind.BoltImportDirective:
        {
          if (node.symbols !== null) {
            for (const importSymbol of node.symbols) {
              // TODO
            }
          } else {
            const sourceFile = self.program.resolveToSourceFile(node.file.value, node) as BoltSourceFile;
            if (sourceFile === null) {
              self.diagnostics.add({
                severity: 'error',
                message: E_FILE_NOT_FOUND,
                args: { filename: node.file.value },
                node: node.file,
              });
            } else {
              for (const exportedNode of self.getAllExportedNodes(sourceFile)) {
                addAllSymbolsToScope(exportedNode, variableScope, typeScope, moduleScope, true);
              }
            }
          }
          break;
        }

        case SyntaxKind.BoltSourceFile:
        case SyntaxKind.BoltModule:
        {
          for (const element of node.elements) {
            addAllSymbolsToScope(element, variableScope, typeScope, moduleScope);
          }
          break;
        }

        case SyntaxKind.BoltFunctionDeclaration:
        {
          const symbolName = emit(node.name);
          const sym = self.lookupSymbolInScope(symbolName, variableScope, SymbolKind.Variable)
          if (sym !== null) {
            if (!allowDuplicates) {
              throw new Error(`Symbol '${name}' is already defined.`);
            }
            if (sym.declarations.indexOf(node) === -1) {
              throw new Error(`Different symbols imported under the same name.`);
            }
          } else {
            self.addSymbolToScope(symbolName, node, variableScope, SymbolKind.Variable);
          }
          break;
        }

        case SyntaxKind.BoltRecordDeclaration:
        {
          const symbolName = emit(node.name);
          const sym = self.lookupSymbolInScope(symbolName, typeScope, SymbolKind.Type)
          if (sym !== null) {
            if (!allowDuplicates) {
              throw new Error(`Symbol '${name}' is already defined.`);
            }
            if (sym.declarations.indexOf(node) === -1) {
              throw new Error(`Different symbols imported under the same name.`);
            }
          } else {
            self.addSymbolToScope(node.name.text, node, typeScope, SymbolKind.Type);
          }
          break;
        }

      }

    }

  }

  private getAllExportedNodes(node: BoltSyntax): BoltSyntax[] {

    const nodes: BoltSyntax[] = [];
    visit(node);
    return nodes;

    function visit(node: BoltSyntax) {
      if (isBoltDeclaration(node) || isBoltTypeDeclaration(node)) {
        if ((node.modifiers & BoltModifiers.IsPublic) > 0) {
          nodes.push(node);
        }
      }
      switch (node.kind) {
        case SyntaxKind.BoltFunctionDeclaration:
        case SyntaxKind.BoltRecordDeclaration:
        case SyntaxKind.BoltTypeAliasDeclaration:
          nodes.push(node);
          break;
        case SyntaxKind.BoltModule:
        case SyntaxKind.BoltSourceFile:
          for (const element of node.elements) {
            visit(element);
          }
          break;
      }
    }

  }

  private resolveReferenceExpression(node: BoltReferenceExpression): BoltDeclaration | null {
    const symbolPath = nodeToSymbolPath(node.name)
    return this.resolveSymbolPath(symbolPath, node, SymbolKind.Variable);
  }

  private resolveTypeReferenceExpression(node: BoltReferenceTypeExpression): BoltTypeDeclaration | null {
    const symbolPath = nodeToSymbolPath(node.name);
    const scope = this.getScopeForNode(node, SymbolKind.Type);
    return this.resolveSymbolPath(symbolPath, scope, SymbolKind.Type);
  }

  public addSymbol(name: string, node: BoltSyntax, kind: SymbolKind): void {
    const scope = this.getScopeSurroundingNode(node, kind);
    this.addSymbolToScope(name, node, scope, kind)
  }

  public addSymbolToScope(name: string, node: BoltSyntax, scope: ScopeInfo, symbolKindMask: SymbolKind): void {
    verbose(`Adding symbol ${name} in scope #${scope.id}`);
    const sym = { kind: symbolKindMask, declarations: [ node ] } as SymbolInfo;
    for (const symbolKind of getAllSymbolKindsInMask(symbolKindMask)) {
      this.symbols.set(`${symbolKind}:${name}:${scope.id}`, sym);
    }
  }

  public getParentScope(scope: ScopeInfo, kind: SymbolKind): ScopeInfo | null {

    // We might have already calculcated this scope's parent scope before;;
    if (scope.parentScope !== undefined) {
      return scope.parentScope;
    }

    if (isSyntax(scope.declaration)) {

      // Edge case where there are no parent nodes left to traverse
      if (scope.declaration.kind === SyntaxKind.BoltSourceFile) {
        const pkg = (scope.declaration as BoltSourceFile).package;
        return {
          id: pkg.id,
          declaration: pkg,
        } as ScopeInfo;
      }

      return this.getScopeForNode(scope.declaration.parentNode!, kind)
    }

    // If the declaration was not an AST node, it can only be a package
    return null;
  }

  public getScopeSurroundingNode(node: Syntax, kind: SymbolKind): ScopeInfo {
    assert(node.parentNode !== null);
    return this.getScopeForNode(node.parentNode!, kind);
  }

  public getScopeForNode(node: BoltSyntax, kind: SymbolKind): ScopeInfo {

    let currNode = node;

    while (true) {

      // We might have created a scope for this node before,
      // or saved the relevant scope for efficiency.
      if (node._scope !== undefined) {
        return node._scope;
      }

      // When we've reached a node that introduces a new scope according
      // to the rules of the SymbolKind, we may continue.
      if (this.introducesNewScope(currNode.kind, kind)) {
        break;
      }

      assert(currNode.parentNode !== null);
      currNode = currNode.parentNode!;
    }

    return {
      id: currNode.id,
      declaration: currNode,
    } as ScopeInfo;
  }

  private lookupSymbolInScope(name: string, scope: ScopeInfo, symbolKindMask: SymbolKind): SymbolInfo | null {
    for (const symbolKind of getAllSymbolKindsInMask(symbolKindMask)) {
      const key = `${symbolKind}:${name}:${scope.id}`;
      if (this.symbols.has(key)) {
        return this.symbols.get(key);
      }
    }
    return null;
  }

  private findSymbolInScopeOf(name: string, scope: ScopeInfo, kind: SymbolKind): SymbolInfo | null {
    while (true) {

      // Attempt to look up the symbol in the scope that was either passed to this
      // method or one of its parents. If we found one, we're done.
      const sym = this.lookupSymbolInScope(name, scope, kind);
      if (sym !== null) {
        return sym;
      }

      const parentScope = this.getParentScope(scope, kind);

      // Failing to find a parent scope means that none of the enclosing
      // scopes had the given variable. If this is the case, jump to the
      // error handling logic.
      if (parentScope === null) {
        break;
      }

      scope = parentScope;
    }

    return null;
  }

  public resolveSymbolPath(path: SymbolPath, scope: ScopeInfo, kind: SymbolKind): BoltSyntax | null {

    if (path.hasParents()) {

      if (path.isAbsolute) {

        // TODO

      } else {

        // We will keep looping until we are at the topmost module of
        // the package corresponding to `node`.
        while (true) {

          let shouldSearchParentScopes = false;
          let currScope = scope;

          // Go through each of the parent names in normal order, resolving to the module
          // that declared the name, and mark when we failed to look up the inner module.
          for (const name of path.getParents()) {
            const sym = this.lookupSymbolInScope(name, currScope, SymbolKind.Module);
            if (sym === null) {
              shouldSearchParentScopes = true;
              break;
            }
            if (sym.declarations[0].kind !== SyntaxKind.BoltModule) {
              shouldSearchParentScopes = true;
              break;
            }
            currScope = this.getScopeForNode(sym.declarations[0], SymbolKind.Module);
          }

          // If the previous loop did not fail, we are done.
          if (!shouldSearchParentScopes) {
            scope = currScope;
            break;
          }

          // We continue the outer loop by getting the parent module, which should be
          // equivalent to getting the parent module scope.
          const parentScope = this.getParentScope(scope, SymbolKind.Module);
          if (parentScope === null) {
            return null;
          }
          scope = parentScope;
        }

      }

    }

    // Once we've handled any module path that might have been present,
    // we resolve the actual symbol using a helper method.

    const sym = this.findSymbolInScopeOf(path.name, scope, kind);

    if (sym === null) {
      return null;
    }

    return sym.declarations[0]!;
  }

  private introducesNewScope(nodeKind: SyntaxKind, symbolKind: SymbolKind) {
    switch (symbolKind) {
      case SymbolKind.Variable:
        return nodeKind === SyntaxKind.BoltSourceFile
            || nodeKind === SyntaxKind.BoltModule
            || nodeKind === SyntaxKind.BoltFunctionDeclaration
            || nodeKind === SyntaxKind.BoltBlockExpression;
      case SymbolKind.Type:
        return nodeKind === SyntaxKind.BoltModule
            || nodeKind === SyntaxKind.BoltSourceFile;
      case SymbolKind.Module:
        return nodeKind === SyntaxKind.BoltModule
            || nodeKind === SyntaxKind.BoltSourceFile;
    }
  }

}

