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
  Syntax,
  SyntaxKind,
  BoltReferenceExpression,
  BoltDeclaration,
  BoltSourceFile,
  BoltSyntax,
  BoltReferenceTypeExpression,
  BoltTypeDeclaration,
  BoltExpression,
  BoltFunctionDeclaration,
  BoltFunctionBodyElement,
  kindToString,
  BoltStatement,
  BoltTypeExpression,
  BoltSourceElement,
  isBoltStatement,
  isBoltDeclaration
} from "./ast";
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
import { createAnyType, isOpaqueType, createOpaqueType, Type, createVoidType, createVariantType, isVoidType } from "./types";
import { getReturnStatementsInFunctionBody } from "./common";
import {emit} from "./emitter";

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

    const self = this;
    for (const element of node.elements) {
      visitSourceElement(element);
    }

    function visitExpression(node: BoltExpression) {

      switch (node.kind) {

        case SyntaxKind.BoltReferenceExpression:
        {
          if (self.resolveReferenceExpression(node) === null) {
            self.diagnostics.add({
              message: E_DECLARATION_NOT_FOUND,
              args: { name: node.name.name.text },
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
              args: { name: node.name.name.text },
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
      } else {
        throw new Error(`Unknown node of kind ${kindToString(node)}`);
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
        this.addSymbol(emit(node.name), scope, sym);
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
    console.error(`Adding symbol ${name}`);
    this.symbols.set(`${name}@${(scope as any).id}`, sym);
  }

  private addTypeSymbol(name: string, scope: TypeScope, sym: TypeSymbolInfo): void {
    console.error(`Adding type symbol ${name}`);
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
    const sym = this.findSymbolInScopeOf(emit(node.name.name), scope);
    if (sym === null) {
      return null;
    }
    return sym.declarations[0]!;
  }

  public resolveTypeReferenceExpression(node: BoltReferenceTypeExpression): BoltTypeDeclaration | null {
    const typeScope = this.getTypeScopeSurroundingNode(node);
    const typeSym = this.findSymbolInTypeScopeOf(emit(node.name.name), typeScope);
    if (typeSym === null) {
      return null;
    }
    return typeSym.declarations[0]!;
  }

}

