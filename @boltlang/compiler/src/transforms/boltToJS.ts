
import { TypeChecker } from "../checker"

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
  BoltIdentifier,
  isBoltDeclaration,
  isBoltStatement,
  JSBindPattern,
  BoltSourceElement,
  createJSParameter,
} from "../ast"

import { setOrigNodeRange } from "../common"
import { Program } from "../program"
import { Transformer, TransformManager } from "./index"
import { assert } from "../util"
import { inject } from "../ioc"
import { isExported } from "../common"

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

  constructor() {

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
    const ctx = new CompileContext()
    for (const element of sourceFile.elements) {
      this.compileSourceElement(element, ctx);
    }
    return createJSSourceFile(ctx.getGeneratedNodes() as JSSourceElement[], sourceFile.span);
  }

  private compileExpression(node: Syntax, ctx: CompileContext): JSExpression {

    switch (node.kind) {

      case SyntaxKind.BoltCallExpression:
      {
        const compiledOperator = this.compileExpression(node.operator, ctx);
        const compiledArgs = node.operands.map(arg => this.compileExpression(arg, ctx))
        return createJSCallExpression(
          compiledOperator,
          compiledArgs,
          node.span,
        );
      }

      case SyntaxKind.BoltReferenceExpression:
      {
        assert(node.name.modulePath === null);
        const result = createJSReferenceExpression(node.name.name.text);
        setOrigNodeRange(result, node, node);
        return result;
      }

      case SyntaxKind.BoltConstantExpression:
      {
        const result = createJSConstantExpression(node.value);
        setOrigNodeRange(result, node, node);
        return result;
      }

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

  private compileSourceElement(node: BoltSourceElement, ctx: CompileContext) {

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
      {
        const jsValue = node.value !== null ? this.compileExpression(node.value, ctx) : null;
        const jsValueBindPatt = this.convertPattern(node.bindings);
        const jsValueDecl = createJSLetDeclaration(
          jsValueBindPatt,
          jsValue,
        );
        ctx.appendNode(jsValueDecl);
        break;
      }

      case SyntaxKind.BoltFunctionDeclaration:
      {
        if (node.body === null) {
          break;
        }
        const params: JSParameter[] = [];
        let body: JSStatement[] = [];
        let modifiers = 0;
        if (isExported(node)) {
          modifiers |= JSDeclarationModifiers.IsExported;;
        }
        let i = 0;
        for (const param of node.params) {
          assert(param.defaultValue === null);
          const jsPatt = this.convertPattern(param.bindings)
          const jsParam = createJSParameter(i, jsPatt, null);
          params.push(jsParam);
          i++;
        }
        const name = createJSIdentifier(node.name.text)
        setOrigNodeRange(name, node.name, node.name);
        const bodyCtx = new CompileContext();
        if (node.target === "JS") {
          for (const element of node.body) {
            this.compileJSStatement(element, bodyCtx);
          }
        } else {
          for (const element of node.body) {
            this.compileSourceElement(element, bodyCtx);
          }
        }
        const result = createJSFunctionDeclaration(
          modifiers,
          name,
          params,
          bodyCtx.getGeneratedNodes() as JSStatement[],
          node.span,
        );
        setOrigNodeRange(result, node, node);
        ctx.appendNode(result)
        break;
      }

      default:
        throw new Error(`Could not compile node ${kindToString(node.kind)}`);

    }

  }

}

export default BoltToJSTransform;
