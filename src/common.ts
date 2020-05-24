import {
  BoltFunctionBodyElement,
  BoltReturnStatement,
  SyntaxKind,
  BoltExpression,
  BoltSourceFile,
  JSSourceFile
} from "./ast";

export type SourceFile
  = BoltSourceFile
  | JSSourceFile

export type BoltFunctionBody = BoltFunctionBodyElement[];

export function getReturnStatementsInFunctionBody(body: BoltFunctionBody): BoltReturnStatement[] {

  const results: BoltReturnStatement[] = [];

  for (const element of body) {
    visit(element);
  }

  return results;

  function visit(node: BoltFunctionBodyElement) {
    switch (node.kind) {
      case SyntaxKind.BoltReturnStatement:
        results.push(node);
        break;
      case SyntaxKind.BoltExpressionStatement:
        visitExpression(node.expression);
        break;
    }
  }

  function visitExpression(node: BoltExpression) {
    switch (node.kind) {
      case SyntaxKind.BoltBlockExpression:
        for (const element of node.statements) {
          visit(element);
        }
        break;
      case SyntaxKind.BoltMatchExpression:
        for (const arm of node.arms) {
          visitExpression(arm.body);
        }
        break;
      case SyntaxKind.BoltCallExpression:
        visitExpression(node.operator);
        for (const operand of node.operands) {
          visitExpression(operand);
        }
        break;
    }
  }

}

