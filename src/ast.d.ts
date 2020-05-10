
export const enum SyntaxKind {
  FunctionBody = 1
  BoltStringLiteral = 4
  BoltIntegerLiteral = 5
  BoltIdentifier = 7
  BoltOperator = 8
  BoltEOS = 9
  BoltComma = 10
  BoltSemi = 11
  BoltDot = 12
  BoltDotDot = 13
  BoltRArrow = 14
  BoltLArrow = 15
  BoltEqSign = 16
  BoltFnKeyword = 18
  BoltForeignKeyword = 19
  BoltLetKeyword = 20
  BoltImportKeyword = 21
  BoltPubKeyword = 22
  BoltModKeyword = 23
  BoltEnumKeyword = 24
  BoltStructKeyword = 25
  BoltNewTypeKeyword = 26
  BoltParenthesized = 28
  BoltBraced = 29
  BoltBracketed = 30
  BoltSourceElement = 31
  BoltSourceFile = 32
  BoltQualName = 33
  BoltReferenceTypeNode = 35
  BoltBindPattern = 37
  BoltTypePattern = 38
  BoltExpressionPattern = 39
  BoltTuplePatternElement = 40
  BoltTuplePattern = 41
  BoltRecordPatternField = 42
  BoltRecordPattern = 43
  BoltCallExpression = 45
  BoltYieldExpression = 46
  BoltMatchArm = 47
  BoltMatchExpression = 48
  BoltCase = 49
  BoltCaseExpression = 50
  BoltBlockExpression = 51
  BoltConstantExpression = 52
  BoltReturnStatement = 54
  BoltResumeStatement = 55
  BoltExpressionStatement = 56
  BoltModule = 57
  BoltParameter = 58
  BoltFunctionDeclaration = 60
  BoltForeignFunctionDeclaration = 61
  BoltVariableDeclaration = 62
  BoltPlainImportSymbol = 64
  BoltImportDeclaration = 65
  BoltRecordDeclarationField = 66
  BoltRecordDeclaration = 67
  JSOperator = 70
  JSIdentifier = 71
  JSPattern = 72
  JSBindPattern = 73
  JSConstantExpression = 75
  JSMemberExpression = 77
  JSCallExpression = 78
  JSBinaryExpression = 79
  JSUnaryExpression = 80
  JSNewExpression = 81
  JSSequenceExpression = 82
  JSConditionalExpression = 83
  JSReferenceExpression = 84
  JSConditionalStatement = 86
  JSSourceFile = 88
  JSSourceElement = 89
}

export function createFunctionBody(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltStringLiteral(value: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltIntegerLiteral(value: number, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltIdentifier(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltOperator(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltEOS(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltComma(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltSemi(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltDot(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltDotDot(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltRArrow(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltLArrow(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltEqSign(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltFnKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltForeignKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltLetKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltImportKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltPubKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltModKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltEnumKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltStructKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltNewTypeKeyword(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltParenthesized(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltBraced(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltBracketed(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltSourceElement(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltSourceFile(elements: BoltSourceElement[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltQualName(modulePath: BoltIdentifier[], name: BoltSymbol, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltReferenceTypeNode(name: BoltQualName, arguments: BoltTypeNode[] | null, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltBindPattern(name: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltTypePattern(typeNode: BoltTypeNode, nestedPattern: BoltPattern, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltExpressionPattern(expression: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltTuplePatternElement(index: number, pattern: BoltPattern, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltTuplePattern(elements: BoltTuplePatternElement[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltRecordPatternField(name: BoltIdentifier, pattern: BoltPattern, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltRecordPattern(fields: BoltRecordPatternField[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltCallExpression(operator: BoltExpression, operands: BoltExpression[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltYieldExpression(value: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltMatchArm(pattern: BoltPattern, body: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltMatchExpression(value: BoltExpression, arms: BoltMatchArm[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltCase(test: BoltExpression, result: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltCaseExpression(cases: BoltCase[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltBlockExpression(statements: BoltStatement[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltConstantExpression(value: BoltValue, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltReturnStatement(value: BoltExpression | null, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltResumeStatement(value: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltExpressionStatement(expression: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltModule(modifiers: BoltDeclarationModifiers, name: BoltQualName, elements: BoltSourceElement, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltParameter(index: number, bindings: BoltPattern, typeNode: BoltTypeNode | null, defaultValue: BoltExpression | null, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltFunctionDeclaration(modifiers: BoltDeclarationModifiers, name: BoltSymbol, params: BoltParameter[], type: BoltTypeNode | null, body: BoltExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltForeignFunctionDeclaration(modifiers: BoltDeclarationModifiers, name: BoltSymbol, params: BoltParameter[], type: BoltTypeNode | null, body: FunctionBody, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltVariableDeclaration(modifiers: BoltDeclarationModifiers, name: BoltSymbol, type: BoltTypeNode | null, value: BoltExpression | null, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltPlainImportSymbol(name: BoltQualName, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltImportDeclaration(file: string, symbols: BoltImportSymbol[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltRecordDeclarationField(name: BoltIdentifier, type: BoltTypeNode, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createBoltRecordDeclaration(name: BoltQualName, fields: BoltRecordDeclarationField[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSOperator(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSIdentifier(text: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSPattern(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSBindPattern(name: JSIdentifier, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSConstantExpression(value: BoltValue, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSMemberExpression(value: JSExpression, property: JSExpression, modifiers: JSMemberExpressionModifiers, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSCallExpression(operator: JSExpression, operands: JSExpression[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSBinaryExpression(left: JSExpression, operator: JSOperator, right: JSExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSUnaryExpression(operator: JSOperator, operand: JSExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSNewExpression(target: JSExpression, arguments: JSExpression[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSSequenceExpression(expressions: JSExpression[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSConditionalExpression(test: JSExpression, consequent: JSExpression, alternate: JSExpression, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSReferenceExpression(name: string, span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSConditionalStatement(test: JSExpression, consequent: JSStatement[], alternate: JSStatement[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSSourceFile(elements: JSSourceElement[], span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
export function createJSSourceElement(span: TextSpan | null = null, origNodes: SyntaxRange | null = null);
