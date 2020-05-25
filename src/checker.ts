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
  BoltReferenceTypeExpression
} from "./ast";
import {FastStringMap, memoize, assert, verbose} from "./util";
import {
  DiagnosticPrinter,
  E_TYPES_NOT_ASSIGNABLE,
  E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TYPE_DECLARATION_NOT_FOUND,
  E_DECLARATION_NOT_FOUND,
  E_INVALID_ARGUMENTS
} from "./diagnostics";
import { createAnyType, isOpaqueType, createOpaqueType, Type, createVoidType, createVariantType, isVoidType } from "./types";
import { getReturnStatementsInFunctionBody, isAutoImported, toDeclarationPath, createDeclarationPath, hasRelativeModulePath, hasAbsoluteModulePath, DeclarationPath, getModulePath, getSymbolNameOfDeclarationPath } from "./common";
import {emit} from "./emitter";

const PACKAGE_SCOPE_ID = 0;

interface SymbolInfo<N extends Syntax> {
  declarations: N[];
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

function getScopeId(scope: Scope | TypeScope) {
  if (isSyntax(scope)) {
    return scope.id;
  }
  return PACKAGE_SCOPE_ID;
}

function createSymbol(node: BoltDeclaration): SymbolInfo {
  return { declarations: [ node ] };
}

class SymbolResolver<N extends Syntax> {

  private symbols = new FastStringMap<string, SymbolInfo<N>>();

  constructor(private introducesNewScope: (kind: SyntaxKind) => boolean) {

  }

  public addSymbol(name: string, node: N): void {
    const scope = this.getScopeSurroundingNode(node)
    verbose(`Adding symbol ${name} in scope #${getScopeId(scope)}`);
    const sym = { declarations: [ node ] };
    this.symbols.set(`${name}@${getScopeId(scope)}`, sym);
  }

  public getParentScope(scope: Scope): Scope | null {
    if (!isSyntax(scope)) {
      // Scope is the global package scope
      return null;
    }
    if (scope.kind === SyntaxKind.BoltSourceFile) {
      return scope.package;
    }
    return this.getScopeForNode(scope.parentNode!)
  }

  public getScopeSurroundingNode(node: Syntax): Scope {
    assert(node.parentNode !== null);
    return this.getScopeForNode(node.parentNode!);
  }

  public getScopeForNode(node: Syntax): Scope {
    let currNode = node;
    while (!this.introducesNewScope(currNode.kind)) {
      if (currNode.kind === SyntaxKind.BoltSourceFile) {
        return currNode.package;
      }
      currNode = currNode.parentNode!;
    }
    return currNode;
  }

  private lookupSymbolInScope(name: string, scope: Scope): SymbolInfo<N> | null {
    const key = `${name}@${getScopeId(scope)}`;
    if (!this.symbols.has(key)) {
      return null;
    }
    return this.symbols.get(key);
  }

  public findSymbolInScopeOf(name: string, scope: Scope): SymbolInfo<N> | null {
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

  public resolve(path: DeclarationPath, node: BoltSyntax): BoltSyntax | null {
    let scope = this.getScopeSurroundingNode(node);
    if (hasAbsoluteModulePath(path)) {
      // TODO
    } else if (hasRelativeModulePath(path)) {
      while (true) {
        let shouldSearchParentScopes = false;
        let currScope = scope;
        for (const name of getModulePath(path)) {
          const sym = this.lookupSymbolInScope(name, currScope);
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
    const sym = this.findSymbolInScopeOf(getSymbolNameOfDeclarationPath(path), scope);
    if (sym === null) {
      return null;
    }
    return sym.declarations[0]!;
  }

}

export class TypeChecker {

  constructor(private diagnostics: DiagnosticPrinter) {

  }

  private varResolver = new SymbolResolver<BoltDeclaration>(introducesNewScope);
  private typeResolver = new SymbolResolver<BoltTypeDeclaration>(introducesNewTypeScope);

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

        case SyntaxKind.BoltModule:
        {
          for (const element of node.elements) {
            visitSourceElement(element);
          }
          break;
        }

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

  private resolveType(name: string, node: BoltSyntax): Type | null {
    const sym = this.typeResolver.findSymbolInScopeOf(name, this.typeResolver.getScopeSurroundingNode(node))
    if (sym === null) {
      return null;
    }
    return this.getTypeOfNode(sym.declarations[0]);
  }

  @memoize(node => node.id)
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
      case SyntaxKind.BoltReturnStatement:
      {
        if (node.value === null) {
          return createVoidType();
        }
        return this.getTypeOfNode(node.value)
      }
      case SyntaxKind.BoltConstantExpression:
      {
        let type;
        if (typeof node.value === 'string') {
          type = this.resolveType('String', node)!;
        } else if (typeof node.value === 'boolean') {
          type = this.resolveType('bool', node)!;
        } else if (typeof node.value === 'bigint') {
          type = this.resolveType('i32', node)!;
        } else {
          throw new Error(`Could not derive type of constant expression.`);
        }
        assert(type !== null);
        return type;
      }
      case SyntaxKind.BoltMatchExpression:
      {
        return createVariantType(...node.arms.map(arm => this.getTypeOfNode(arm.body)));
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
        this.varResolver.addSymbol(emit(node.name), node);
        break;
      }

      case SyntaxKind.BoltRecordDeclaration:
      {
        this.typeResolver.addSymbol(node.name.text, node);
      }

    }

  }

  private resolveReferenceExpression(node: BoltReferenceExpression): BoltDeclaration | null {
    return this.varResolver.resolve(createDeclarationPath(node.name), node);
  }

  private resolveTypeReferenceExpression(node: BoltReferenceTypeExpression): BoltTypeDeclaration | null {
    return this.typeResolver.resolve(createDeclarationPath(node.name), node);
  }

}

