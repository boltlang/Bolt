
import { CBuiltinType, CBuiltinTypeKind, CCallExpr, CConstExpr, CDecl, CDir, CExpr, CExprStmt, CFuncDecl, CIncDir, CProgram, CRefExpr, CStmt } from "./cast";
import { Expression, SourceFile, Syntax, SyntaxKind } from "./cst";
import { assert } from "./util";

interface Context {
  body: CStmt[];
}

export function generateCode(sourceFile: SourceFile): CProgram {

  const intType = new CBuiltinType(CBuiltinTypeKind.Int);

  const decls: (CDecl | CDir)[] = [];

  decls.push(new CIncDir("runtime.h"));

  const mainBody: CStmt[] = [];

  decls.push(
    new CFuncDecl(
      intType,
      'main',
      [],
      mainBody
    )
  );

  visit(sourceFile, { body: mainBody });

  return new CProgram(decls);

  function visit(node: Syntax, context: Context): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          visit(element, context);
        }
        break;
      }

      case SyntaxKind.ExpressionStatement:
      {
        context.body.push(
          new CExprStmt(
            visitExpression(node.expression, context)
          )
        );
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        // TODO
        break;
      }

    }

  }

  function visitExpression(node: Expression, context: Context): CExpr {
    switch (node.kind) {
      case SyntaxKind.ReferenceExpression:
        assert(node.modulePath.length === 0);
        return new CRefExpr(node.name.text);
      case SyntaxKind.CallExpression:
        const operator = visitExpression(node.func, context);
        const args = node.args.map(arg => visitExpression(arg, context));
        return new CCallExpr(operator, args);
      case SyntaxKind.ConstantExpression:
        return new CConstExpr(node.token.getValue());
      default:
        throw new Error(`Unexpected ${node}`);
    }
  }

}

