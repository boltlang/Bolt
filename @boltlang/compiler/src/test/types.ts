
import test from "ava";
import { createBoltConstantExpression, createBoltIdentifier, createBoltQualName, createBoltReferenceExpression } from "../ast";

import { TypeChecker } from "../checker"

function createSimpleBoltReferenceExpression(name: string) {
  return createBoltReferenceExpression(
    createBoltQualName(false, [], createBoltIdentifier(name)),
  )
}

test('an intersection with an any-type should remove that any-type', t => {
  const expr = createBoltConstantExpression(BigInt(1));
  const checker = new TypeChecker();
  const exprType = checker.getTypeOfNode(expr);
  t.assert(checker.isIntType(exprType))
});
