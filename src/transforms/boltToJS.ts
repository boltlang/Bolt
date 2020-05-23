
import { 
  TypeChecker,
  Scope
} from "../checker"

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
  createJSBindPattern,
  JSDeclarationModifiers,
  JSParameter,
  BoltSyntax,
  BoltPattern,
  createJSImportDeclaration,
  JSSyntax,
  JSSourceFile,
  isBoltSourceFile,
  BoltImportDeclaration,
  BoltIdentifier,
  isBoltDeclaration,
  isBoltStatement,
  JSBindPattern,
  BoltSourceElement,
} from "../ast"

import { hasPublicModifier, setOrigNodeRange } from "../util"
import { Program, SourceFile } from "../program"
import { Transformer, TransformManager } from "./index"
import { assert } from "../util"
import { inject } from "../di"

export interface JSCompilerOptions {

}

function toJSIdentifier(node: BoltIdentifier) {
  const result = createJSIdentifier(node.text);
  setOrigNodeRange(result, node, node);
  return result;
}

function pushAll<T>(arr: T[], els: T[]) {
  for (const el of els) {
    arr.push(el)
  }
}

type Ref<T> = { value: T };

class CompileContext {

  private generatedNodes: JSSyntax[] = [];

  constructor(public scope: Scope) {

  }

  public appendNode(node: JSSyntax): void {
    this.generatedNodes.push(node);
  }

  public getGeneratedNodes(): JSSyntax[] {
    return this.generatedNodes;
  }

}

export class BoltToJSTransform implements Transformer {

  constructor(
    private transforms: TransformManager,
    @inject private program: Program,
    @inject private checker: TypeChecker,
  ) {

  }

  public isApplicable(sourceFile: SourceFile): boolean {
    return isBoltSourceFile(sourceFile);
  }

  public transform(sourceFile: BoltSourceFile): JSSourceFile {
    const ctx = new CompileContext(this.checker.getScope(sourceFile))
    for (const element of sourceFile.elements) {
      this.compileSourceElement(element, ctx);
    }
    return createJSSourceFile(ctx.getGeneratedNodes() as JSSourceElement[], sourceFile.span);
  }

  private compileExpression(node: Syntax, ctx: CompileContext): JSExpression {

    switch (node.kind) {

      case SyntaxKind.BoltCallExpression:
        const compiledOperator = this.compileExpression(node.operator, ctx);
        const compiledArgs = node.operands.map(arg => this.compileExpression(arg, ctx))
        return createJSCallExpression(
          compiledOperator,
          compiledArgs,
          node.span,
        );

      case SyntaxKind.BoltReferenceExpression:
        assert(node.name.modulePath === null);
        return createJSReferenceExpression(
          node.name.name.text,
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

  private convertPattern(node: BoltPattern): JSBindPattern {
    if (node.kind !== SyntaxKind.BoltBindPattern) {
      throw new Error(`The provided node should have been eliminated by a previous pass.`);
    }
    const jsIdent = toJSIdentifier((node as BoltBindPattern).name);
    const jsBindPatt = createJSBindPattern(jsIdent);
    setOrigNodeRange(jsBindPatt, node, node);
    return jsBindPatt;
  }

  protected compileSourceElement(node: BoltSourceElement, ctx: CompileContext) {

    switch (node.kind) {

      case SyntaxKind.BoltRecordDeclaration:
      case SyntaxKind.BoltTypeAliasDeclaration:
        break;

      case SyntaxKind.BoltExpressionStatement:
        const jsExpr = this.compileExpression(node.expression, ctx)
        const jsExprStmt = createJSExpressionStatement(jsExpr)
        setOrigNodeRange(jsExprStmt, node, node);
        ctx.appendNode(jsExprStmt);
        break;

      case SyntaxKind.BoltImportDeclaration:
        // TODO
        break;

      case SyntaxKind.BoltVariableDeclaration:
        const jsValue = node.value !== null ? this.compileExpression(node.value, ctx) : null;
        const jsValueBindPatt = this.convertPattern(node.bindings);
        const jsValueDecl = createJSLetDeclaration(
          jsValueBindPatt,
          jsValue,
        );
        ctx.appendNode(jsValueDecl);
        break;

      case SyntaxKind.BoltFunctionDeclaration:
          if (node.body === null) {
            break;
          }
        if (node.target === "JS") {
          const params: JSParameter[] = [];
          let body: JSStatement[] = [];
          for (const param of node.params) {
            assert(param.defaultValue === null);
            const jsPatt = this.convertPattern(param.bindings)
            params.push(jsPatt);
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
          ctx.appendNode(result)
        } else {
          // TODO
          throw new Error(`Compiling native functions is not yet implemented.`);
        }
        break;

      default:
        throw new Error(`Could not compile node ${kindToString(node.kind)}`);

    }

  }

}

export default BoltToJSTransform;
