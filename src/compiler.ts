
import acorn from "acorn"

import { 
  TypeChecker,
  Scope
} from "./checker"

import {
  Syntax,
  SyntaxKind,
  kindToString,
  BoltSourceFile,
  BoltStatement,
  BoltExpression,
  BoltDeclaration,
  BoltBindPattern,
  JSExpression,
  JSStatement,
  JSSourceElement,
  createJSExpressionStatement,
  createJSSourceFile,
  createJSCallExpression,
  createJSReferenceExpression,
  createJSConstantExpression,
  createJSLetDeclaration,
  createJSIdentifier,
  createJSFunctionDeclaration,
  isBoltExpression,
  createJSBindPattern,
  JSDeclarationModifiers,
  JSParameter,
} from "./ast"

import { getFullTextOfQualName, hasPublicModifier } from "./util"

import { Program } from "./program"

export interface CompilerOptions {
  target: string;
}

function pushAll<T>(arr: T[], els: T[]) {
  for (const el of els) {
    arr.push(el)
  }
}

export class Compiler {

  readonly target: string;

  constructor(public program: Program, public checker: TypeChecker, options: CompilerOptions) {
    this.target = options.target;
  }

  compile(files: BoltSourceFile[]) {
    return files.map(s => {
      const body: JSSourceElement[] = [];
      for (const element of s.elements) {
        this.compileDecl(element, body);
      }
      return createJSSourceFile(body, s.span);
    });
  }

  protected compileExpr(node: Syntax, preamble: Syntax[]): JSExpression {

    switch (node.kind) {

      case SyntaxKind.BoltCallExpression:
        const compiledOperator = this.compileExpr(node.operator, preamble);
        const compiledArgs = node.operands.map(a => this.compileExpr(a, preamble))
        return createJSCallExpression(
          compiledOperator,
          compiledArgs,
          node.span,
        );

      case SyntaxKind.BoltReferenceExpression:
        return createJSReferenceExpression(
          getFullTextOfQualName(node.name),
          node.span,
        );

      case SyntaxKind.BoltConstantExpression:
        return createJSConstantExpression(
          node.value,
          node.span,
        );

      default:
        throw new Error(`Could not compile expression node ${kindToString(node.kind)}`)
    }

  }

  protected compileDecl(node: Syntax, preamble: Syntax[]) {

    console.log(`compiling ${kindToString(node.kind)}`)

    //if (isBoltExpression(node)) {
    //  const compiled = this.compileExpr(node, preamble);
    //  preamble.push(createJSExpressionStatement(compiled));
    //  return;
    //}

    switch (node.kind) {

      case SyntaxKind.BoltModule:
        for (const element of node.elements) {
          this.compileDecl(element, preamble);
        }
        break;

      case SyntaxKind.BoltExpressionStatement:
        preamble.push(this.compileExpr(node.expression, preamble));
        break;

      case SyntaxKind.BoltImportDeclaration:
        // TODO
        break;

      case SyntaxKind.BoltVariableDeclaration:
        const compiledValue = node.value !== null ? this.compileExpr(node.value, preamble) : null;
        preamble.push(
          createJSLetDeclaration(
            createJSBindPattern((node.bindings as BoltBindPattern).name, node.bindings.span),
            compiledValue,
            node.span,
          ),
        );
        break;

      case SyntaxKind.BoltForeignFunctionDeclaration:
        if (node.target === this.target && node.body !== null) {
          const params: JSParameter[] = [];
          let body: JSStatement[] = [];
          for (const param of node.params) {
            params.push(this.compilePattern(param.bindings, body));
          }
          let result = createJSFunctionDeclaration(
            0,
            createJSIdentifier(node.name.text, node.name.span),
            params,
            body,
            node.span,
          );
          if (hasPublicModifier(node)) {
            result.modifiers |= JSDeclarationModifiers.IsExported;;
          }
          preamble.push(result)
        }
        break;

      default:
        throw new Error(`Could not compile node ${kindToString(node.kind)}`);

    }

  }

}

