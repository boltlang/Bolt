
import { CBuiltinType, CBuiltinTypeKind, CCallExpr, CConstExpr, CDecl, CDir, CExpr, CExprStmt, CFuncDecl, CIncDir, CNode, CProgram, CRefExpr, CStmt } from "../c";
import { Expression, Syntax, SyntaxKind } from "../cst";
import type { Pass } from "../program";
import { assert } from "../util";

interface Context {
  body: CStmt[];
}

class BoltToC implements Pass<Syntax, CNode> {

  public apply(input: Syntax): CNode {

    assert(input.kind === SyntaxKind.SourceFile);

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

    visit(input, { body: mainBody });

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

}

export default BoltToC;
