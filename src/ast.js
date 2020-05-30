
export class NodeVisitor {
  visit(node) {
    for (const child of node.preorder()) {
      const key = `visit${kindToString(child.kind)}`;
      if (this[key] !== undefined) {
        this[key](child);
      }
    }
  }
}

let nextNodeId = 1;

class SyntaxBase {

  constructor(span) {
    this.id = nextNodeId++;
    this.errors = [];
    this.span = span;
  }

  *getChildNodes() {
    for (const key of Object.keys(this)) {
      if (key === 'span' || key === 'parentNode' || key === 'type') {
        continue
      }
      const value = this[key];
      if (Array.isArray(value)) {
        for (const element of value) {
          if (isSyntax(element)) {
            yield element;
          }
        }
      } else {
        if (isSyntax(value)) {
          yield value;
        }
      }
    }
  }

  visit(visitors) {
    const stack = [this];
    while (stack.length > 0) {
      const node = stack.pop();
      const kindName = kindToString(node.kind);
      const kindNamesToVisit = [kindName, ...NODE_TYPES[kindName].parents];
      for (const visitor of visitors) {
        for (const kindName of kindNamesToVisit) {
          const key = `visit${kindName}`
          if (visitor[key] !== undefined) {
            visitor[key](node);
          }
        }
      }
      for (const childNode of node.getChildNodes()) {
        stack.push(childNode);
      }
    }
  }

  *preorder() {
    const stack = [this];
    while (stack.length > 0) {
      const node = stack.pop();
      yield node
      for (const childNode of node.getChildNodes()) {
        stack.push(childNode);
      }
    }
  }

  mayContainKind(kind) {
    // TODO
    return true;
  }

  getParentOfKind(kind) {
    let currNode = this.parentNode;
    while (currNode !== null) {
      if (currNode.kind === kind) {
        return currNode;
      }
      currNode = currNode.parentNode;
    }
    return null;
  }

  *findAllChildrenOfKind(kind) {
    for (const node of this.preorder()) {
      if (!node.mayContainKind(kind)) {
        break;
      }
      if (node.kind === kind) {
        yield node
      }
    }
  }

}

export function isSyntax(value) {
  return typeof value === 'object'
      && value !== null
      && value.__NODE_TYPE !== undefined;
}

export function setParents(node, parentNode = null) {
  node.parentNode = parentNode;
  for (const child of node.getChildNodes()) {
    setParents(child, node)
  }
}
class EndOfFile extends SyntaxBase {

  static kind = 0;

  static parents = [
    "BoltToken",
    "JSToken",
    "JSSyntax",
    "Token",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltStringLiteral extends SyntaxBase {

  static kind = 7;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class BoltIntegerLiteral extends SyntaxBase {

  static kind = 8;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class BoltIdentifier extends SyntaxBase {

  static kind = 10;

  static parents = [
    "BoltSymbol",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class BoltOperator extends SyntaxBase {

  static kind = 12;

  static parents = [
    "BoltSymbol",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class BoltAssignment extends SyntaxBase {

  static kind = 13;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    operator,
    span = null,
  ) {
    super(span);
    this.operator = operator;
    this.span = span
  }

}
class BoltComma extends SyntaxBase {

  static kind = 14;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltSemi extends SyntaxBase {

  static kind = 15;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltColon extends SyntaxBase {

  static kind = 16;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltColonColon extends SyntaxBase {

  static kind = 17;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltDot extends SyntaxBase {

  static kind = 18;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltDotDot extends SyntaxBase {

  static kind = 19;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltRArrow extends SyntaxBase {

  static kind = 20;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltRArrowAlt extends SyntaxBase {

  static kind = 21;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltLArrow extends SyntaxBase {

  static kind = 22;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltEqSign extends SyntaxBase {

  static kind = 23;

  static parents = [
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltGtSign extends SyntaxBase {

  static kind = 24;

  static parents = [
    "BoltToken",
    "BoltOperatorLike",
    "BoltSymbol",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltExMark extends SyntaxBase {

  static kind = 25;

  static parents = [
    "BoltToken",
    "BoltOperatorLike",
    "BoltSymbol",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltLtSign extends SyntaxBase {

  static kind = 26;

  static parents = [
    "BoltToken",
    "BoltOperatorLike",
    "BoltSymbol",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltVBar extends SyntaxBase {

  static kind = 27;

  static parents = [
    "BoltToken",
    "BoltOperatorLike",
    "BoltSymbol",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltWhereKeyword extends SyntaxBase {

  static kind = 29;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltQuoteKeyword extends SyntaxBase {

  static kind = 30;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltFnKeyword extends SyntaxBase {

  static kind = 31;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltForeignKeyword extends SyntaxBase {

  static kind = 32;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltForKeyword extends SyntaxBase {

  static kind = 33;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltLetKeyword extends SyntaxBase {

  static kind = 34;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltReturnKeyword extends SyntaxBase {

  static kind = 35;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltLoopKeyword extends SyntaxBase {

  static kind = 36;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltYieldKeyword extends SyntaxBase {

  static kind = 37;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltMatchKeyword extends SyntaxBase {

  static kind = 38;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltImportKeyword extends SyntaxBase {

  static kind = 39;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltExportKeyword extends SyntaxBase {

  static kind = 40;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltPubKeyword extends SyntaxBase {

  static kind = 41;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltModKeyword extends SyntaxBase {

  static kind = 42;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltMutKeyword extends SyntaxBase {

  static kind = 43;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltEnumKeyword extends SyntaxBase {

  static kind = 44;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltStructKeyword extends SyntaxBase {

  static kind = 45;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltTypeKeyword extends SyntaxBase {

  static kind = 46;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltTraitKeyword extends SyntaxBase {

  static kind = 47;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltImplKeyword extends SyntaxBase {

  static kind = 48;

  static parents = [
    "BoltToken",
    "BoltKeyword",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltParenthesized extends SyntaxBase {

  static kind = 50;

  static parents = [
    "BoltPunctuated",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class BoltBraced extends SyntaxBase {

  static kind = 51;

  static parents = [
    "BoltPunctuated",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class BoltBracketed extends SyntaxBase {

  static kind = 52;

  static parents = [
    "BoltPunctuated",
    "BoltToken",
    "Token",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class BoltSourceFile extends SyntaxBase {

  static kind = 53;

  static parents = [
    "BoltSyntax",
    "SourceFile",
    "Syntax"
  ];

  constructor(
    elements,
    pkg,
    span = null,
  ) {
    super(span);
    this.elements = elements;
    this.pkg = pkg;
    this.span = span
  }

}
class BoltQualName extends SyntaxBase {

  static kind = 54;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    isAbsolute,
    modulePath,
    name,
    span = null,
  ) {
    super(span);
    this.isAbsolute = isAbsolute;
    this.modulePath = modulePath;
    this.name = name;
    this.span = span
  }

}
class BoltTypeOfExpression extends SyntaxBase {

  static kind = 56;

  static parents = [
    "BoltTypeExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    expression,
    span = null,
  ) {
    super(span);
    this.expression = expression;
    this.span = span
  }

}
class BoltReferenceTypeExpression extends SyntaxBase {

  static kind = 57;

  static parents = [
    "BoltTypeExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    name,
    typeArgs,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.typeArgs = typeArgs;
    this.span = span
  }

}
class BoltFunctionTypeExpression extends SyntaxBase {

  static kind = 58;

  static parents = [
    "BoltTypeExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    params,
    returnType,
    span = null,
  ) {
    super(span);
    this.params = params;
    this.returnType = returnType;
    this.span = span
  }

}
class BoltLiftedTypeExpression extends SyntaxBase {

  static kind = 59;

  static parents = [
    "BoltTypeExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    expression,
    span = null,
  ) {
    super(span);
    this.expression = expression;
    this.span = span
  }

}
class BoltTypeParameter extends SyntaxBase {

  static kind = 60;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    index,
    name,
    typeExpr,
    defaultType,
    span = null,
  ) {
    super(span);
    this.index = index;
    this.name = name;
    this.typeExpr = typeExpr;
    this.defaultType = defaultType;
    this.span = span
  }

}
class BoltBindPattern extends SyntaxBase {

  static kind = 62;

  static parents = [
    "BoltPattern",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    name,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.span = span
  }

}
class BoltTypePattern extends SyntaxBase {

  static kind = 63;

  static parents = [
    "BoltPattern",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    typeExpr,
    nestedPattern,
    span = null,
  ) {
    super(span);
    this.typeExpr = typeExpr;
    this.nestedPattern = nestedPattern;
    this.span = span
  }

}
class BoltExpressionPattern extends SyntaxBase {

  static kind = 64;

  static parents = [
    "BoltPattern",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    expression,
    span = null,
  ) {
    super(span);
    this.expression = expression;
    this.span = span
  }

}
class BoltTuplePatternElement extends SyntaxBase {

  static kind = 65;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    index,
    pattern,
    span = null,
  ) {
    super(span);
    this.index = index;
    this.pattern = pattern;
    this.span = span
  }

}
class BoltTuplePattern extends SyntaxBase {

  static kind = 66;

  static parents = [
    "BoltPattern",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    elements,
    span = null,
  ) {
    super(span);
    this.elements = elements;
    this.span = span
  }

}
class BoltRecordFieldPattern extends SyntaxBase {

  static kind = 67;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    isRest,
    name,
    pattern,
    span = null,
  ) {
    super(span);
    this.isRest = isRest;
    this.name = name;
    this.pattern = pattern;
    this.span = span
  }

}
class BoltRecordPattern extends SyntaxBase {

  static kind = 68;

  static parents = [
    "BoltPattern",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    name,
    fields,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.fields = fields;
    this.span = span
  }

}
class BoltQuoteExpression extends SyntaxBase {

  static kind = 70;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    tokens,
    span = null,
  ) {
    super(span);
    this.tokens = tokens;
    this.span = span
  }

}
class BoltTupleExpression extends SyntaxBase {

  static kind = 71;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    elements,
    span = null,
  ) {
    super(span);
    this.elements = elements;
    this.span = span
  }

}
class BoltReferenceExpression extends SyntaxBase {

  static kind = 72;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    name,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.span = span
  }

}
class BoltMemberExpression extends SyntaxBase {

  static kind = 73;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    expression,
    path,
    span = null,
  ) {
    super(span);
    this.expression = expression;
    this.path = path;
    this.span = span
  }

}
class BoltFunctionExpression extends SyntaxBase {

  static kind = 74;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    params,
    returnType,
    body,
    span = null,
  ) {
    super(span);
    this.params = params;
    this.returnType = returnType;
    this.body = body;
    this.span = span
  }

}
class BoltCallExpression extends SyntaxBase {

  static kind = 75;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    operator,
    operands,
    span = null,
  ) {
    super(span);
    this.operator = operator;
    this.operands = operands;
    this.span = span
  }

}
class BoltYieldExpression extends SyntaxBase {

  static kind = 76;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class BoltMatchArm extends SyntaxBase {

  static kind = 77;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    pattern,
    body,
    span = null,
  ) {
    super(span);
    this.pattern = pattern;
    this.body = body;
    this.span = span
  }

}
class BoltMatchExpression extends SyntaxBase {

  static kind = 78;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    value,
    arms,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.arms = arms;
    this.span = span
  }

}
class BoltCase extends SyntaxBase {

  static kind = 79;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    test,
    result,
    span = null,
  ) {
    super(span);
    this.test = test;
    this.result = result;
    this.span = span
  }

}
class BoltCaseExpression extends SyntaxBase {

  static kind = 80;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    cases,
    span = null,
  ) {
    super(span);
    this.cases = cases;
    this.span = span
  }

}
class BoltBlockExpression extends SyntaxBase {

  static kind = 81;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    elements,
    span = null,
  ) {
    super(span);
    this.elements = elements;
    this.span = span
  }

}
class BoltConstantExpression extends SyntaxBase {

  static kind = 82;

  static parents = [
    "BoltExpression",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class BoltReturnStatement extends SyntaxBase {

  static kind = 84;

  static parents = [
    "ReturnStatement",
    "BoltStatement",
    "BoltSyntax",
    "BoltFunctionBodyElement",
    "BoltSourceElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class BoltConditionalCase extends SyntaxBase {

  static kind = 85;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    test,
    body,
    span = null,
  ) {
    super(span);
    this.test = test;
    this.body = body;
    this.span = span
  }

}
class BoltConditionalStatement extends SyntaxBase {

  static kind = 86;

  static parents = [
    "BoltStatement",
    "BoltSyntax",
    "BoltFunctionBodyElement",
    "BoltSourceElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    cases,
    span = null,
  ) {
    super(span);
    this.cases = cases;
    this.span = span
  }

}
class BoltResumeStatement extends SyntaxBase {

  static kind = 87;

  static parents = [
    "BoltStatement",
    "BoltSyntax",
    "BoltFunctionBodyElement",
    "BoltSourceElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class BoltExpressionStatement extends SyntaxBase {

  static kind = 88;

  static parents = [
    "BoltStatement",
    "BoltSyntax",
    "BoltFunctionBodyElement",
    "BoltSourceElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    expression,
    span = null,
  ) {
    super(span);
    this.expression = expression;
    this.span = span
  }

}
class BoltParameter extends SyntaxBase {

  static kind = 89;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    index,
    bindings,
    typeExpr,
    defaultValue,
    span = null,
  ) {
    super(span);
    this.index = index;
    this.bindings = bindings;
    this.typeExpr = typeExpr;
    this.defaultValue = defaultValue;
    this.span = span
  }

}
class BoltModule extends SyntaxBase {

  static kind = 93;

  static parents = [
    "BoltSyntax",
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    name,
    elements,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.name = name;
    this.elements = elements;
    this.span = span
  }

}
class BoltFunctionDeclaration extends SyntaxBase {

  static kind = 96;

  static parents = [
    "BoltFunctionBodyElement",
    "BoltDeclaration",
    "BoltDeclarationLike",
    "BoltTraitOrImplElement",
    "BoltSyntax",
    "BoltSourceElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    target,
    name,
    params,
    returnType,
    typeParams,
    body,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.target = target;
    this.name = name;
    this.params = params;
    this.returnType = returnType;
    this.typeParams = typeParams;
    this.body = body;
    this.span = span
  }

}
class BoltVariableDeclaration extends SyntaxBase {

  static kind = 97;

  static parents = [
    "BoltFunctionBodyElement",
    "BoltDeclaration",
    "BoltDeclarationLike",
    "BoltSyntax",
    "BoltSourceElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    bindings,
    typeExpr,
    value,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.bindings = bindings;
    this.typeExpr = typeExpr;
    this.value = value;
    this.span = span
  }

}
class BoltPlainImportSymbol extends SyntaxBase {

  static kind = 99;

  static parents = [
    "BoltImportSymbol",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    remote,
    local,
    span = null,
  ) {
    super(span);
    this.remote = remote;
    this.local = local;
    this.span = span
  }

}
class BoltImportDirective extends SyntaxBase {

  static kind = 100;

  static parents = [
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    file,
    symbols,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.file = file;
    this.symbols = symbols;
    this.span = span
  }

}
class BoltExportSymbol extends SyntaxBase {

  static kind = 101;

  static parents = [
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class BoltPlainExportSymbol extends SyntaxBase {

  static kind = 102;

  static parents = [
    "Syntax"
  ];

  constructor(
    local,
    remote,
    span = null,
  ) {
    super(span);
    this.local = local;
    this.remote = remote;
    this.span = span
  }

}
class BoltExportDirective extends SyntaxBase {

  static kind = 103;

  static parents = [
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    file,
    symbols,
    span = null,
  ) {
    super(span);
    this.file = file;
    this.symbols = symbols;
    this.span = span
  }

}
class BoltTraitDeclaration extends SyntaxBase {

  static kind = 105;

  static parents = [
    "BoltDeclarationLike",
    "BoltTypeDeclaration",
    "BoltSyntax",
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    typeParams,
    name,
    typeBoundExpr,
    elements,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.typeParams = typeParams;
    this.name = name;
    this.typeBoundExpr = typeBoundExpr;
    this.elements = elements;
    this.span = span
  }

}
class BoltImplDeclaration extends SyntaxBase {

  static kind = 106;

  static parents = [
    "BoltTypeDeclaration",
    "BoltDeclarationLike",
    "BoltSyntax",
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    typeParams,
    name,
    traitTypeExpr,
    elements,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.typeParams = typeParams;
    this.name = name;
    this.traitTypeExpr = traitTypeExpr;
    this.elements = elements;
    this.span = span
  }

}
class BoltTypeAliasDeclaration extends SyntaxBase {

  static kind = 107;

  static parents = [
    "BoltDeclarationLike",
    "BoltTypeDeclaration",
    "BoltTraitOrImplElement",
    "BoltSyntax",
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    name,
    typeParams,
    typeExpr,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.name = name;
    this.typeParams = typeParams;
    this.typeExpr = typeExpr;
    this.span = span
  }

}
class BoltRecordField extends SyntaxBase {

  static kind = 109;

  static parents = [
    "BoltRecordMember",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    name,
    typeExpr,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.typeExpr = typeExpr;
    this.span = span
  }

}
class BoltRecordDeclaration extends SyntaxBase {

  static kind = 110;

  static parents = [
    "BoltDeclaration",
    "BoltTypeDeclaration",
    "BoltDeclarationLike",
    "BoltSyntax",
    "BoltSourceElement",
    "BoltSyntax",
    "BoltSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    name,
    typeParms,
    members,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.name = name;
    this.typeParms = typeParms;
    this.members = members;
    this.span = span
  }

}
class BoltMacroCall extends SyntaxBase {

  static kind = 112;

  static parents = [
    "BoltRecordMember",
    "BoltSourceElement",
    "BoltTraitOrImplElement",
    "BoltFunctionBodyElement",
    "FunctionBodyElement",
    "BoltSyntax",
    "Syntax"
  ];

  constructor(
    name,
    text,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.text = text;
    this.span = span
  }

}
class JSIdentifier extends SyntaxBase {

  static kind = 115;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class JSString extends SyntaxBase {

  static kind = 116;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class JSInteger extends SyntaxBase {

  static kind = 117;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class JSFromKeyword extends SyntaxBase {

  static kind = 118;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSReturnKeyword extends SyntaxBase {

  static kind = 119;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSTryKeyword extends SyntaxBase {

  static kind = 120;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSFinallyKeyword extends SyntaxBase {

  static kind = 121;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSCatchKeyword extends SyntaxBase {

  static kind = 122;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSImportKeyword extends SyntaxBase {

  static kind = 123;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSAsKeyword extends SyntaxBase {

  static kind = 124;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSConstKeyword extends SyntaxBase {

  static kind = 125;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSLetKeyword extends SyntaxBase {

  static kind = 126;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSExportKeyword extends SyntaxBase {

  static kind = 127;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSFunctionKeyword extends SyntaxBase {

  static kind = 128;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSWhileKeyword extends SyntaxBase {

  static kind = 129;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSForKeyword extends SyntaxBase {

  static kind = 130;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSOperator extends SyntaxBase {

  static kind = 132;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    text,
    span = null,
  ) {
    super(span);
    this.text = text;
    this.span = span
  }

}
class JSCloseBrace extends SyntaxBase {

  static kind = 133;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSCloseBracket extends SyntaxBase {

  static kind = 134;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSCloseParen extends SyntaxBase {

  static kind = 135;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSOpenBrace extends SyntaxBase {

  static kind = 136;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSOpenBracket extends SyntaxBase {

  static kind = 137;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSOpenParen extends SyntaxBase {

  static kind = 138;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSSemi extends SyntaxBase {

  static kind = 139;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSComma extends SyntaxBase {

  static kind = 140;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSDot extends SyntaxBase {

  static kind = 141;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSDotDotDot extends SyntaxBase {

  static kind = 142;

  static parents = [
    "JSToken",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSMulOp extends SyntaxBase {

  static kind = 143;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSAddOp extends SyntaxBase {

  static kind = 144;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSDivOp extends SyntaxBase {

  static kind = 145;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSSubOp extends SyntaxBase {

  static kind = 146;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSLtOp extends SyntaxBase {

  static kind = 147;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSGtOp extends SyntaxBase {

  static kind = 148;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSBOrOp extends SyntaxBase {

  static kind = 149;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSBXorOp extends SyntaxBase {

  static kind = 150;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSBAndOp extends SyntaxBase {

  static kind = 151;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSBNotOp extends SyntaxBase {

  static kind = 152;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSNotOp extends SyntaxBase {

  static kind = 153;

  static parents = [
    "JSToken",
    "JSOperatorLike",
    "JSSyntax",
    "Token",
    "Syntax"
  ];

  constructor(
    span = null,
  ) {
    super(span);
    this.span = span
  }

}
class JSBindPattern extends SyntaxBase {

  static kind = 155;

  static parents = [
    "JSPattern",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    name,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.span = span
  }

}
class JSConstantExpression extends SyntaxBase {

  static kind = 157;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class JSMemberExpression extends SyntaxBase {

  static kind = 158;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    value,
    property,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.property = property;
    this.span = span
  }

}
class JSCallExpression extends SyntaxBase {

  static kind = 159;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    operator,
    operands,
    span = null,
  ) {
    super(span);
    this.operator = operator;
    this.operands = operands;
    this.span = span
  }

}
class JSBinaryExpression extends SyntaxBase {

  static kind = 160;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    left,
    operator,
    right,
    span = null,
  ) {
    super(span);
    this.left = left;
    this.operator = operator;
    this.right = right;
    this.span = span
  }

}
class JSUnaryExpression extends SyntaxBase {

  static kind = 161;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    operator,
    operand,
    span = null,
  ) {
    super(span);
    this.operator = operator;
    this.operand = operand;
    this.span = span
  }

}
class JSNewExpression extends SyntaxBase {

  static kind = 162;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    target,
    args,
    span = null,
  ) {
    super(span);
    this.target = target;
    this.args = args;
    this.span = span
  }

}
class JSSequenceExpression extends SyntaxBase {

  static kind = 163;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    expressions,
    span = null,
  ) {
    super(span);
    this.expressions = expressions;
    this.span = span
  }

}
class JSConditionalExpression extends SyntaxBase {

  static kind = 164;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    test,
    consequent,
    alternate,
    span = null,
  ) {
    super(span);
    this.test = test;
    this.consequent = consequent;
    this.alternate = alternate;
    this.span = span
  }

}
class JSLiteralExpression extends SyntaxBase {

  static kind = 165;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class JSReferenceExpression extends SyntaxBase {

  static kind = 166;

  static parents = [
    "JSExpression",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    name,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.span = span
  }

}
class JSCatchBlock extends SyntaxBase {

  static kind = 170;

  static parents = [
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    bindings,
    elements,
    span = null,
  ) {
    super(span);
    this.bindings = bindings;
    this.elements = elements;
    this.span = span
  }

}
class JSTryCatchStatement extends SyntaxBase {

  static kind = 171;

  static parents = [
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    tryBlock,
    catchBlock,
    finalBlock,
    span = null,
  ) {
    super(span);
    this.tryBlock = tryBlock;
    this.catchBlock = catchBlock;
    this.finalBlock = finalBlock;
    this.span = span
  }

}
class JSExpressionStatement extends SyntaxBase {

  static kind = 172;

  static parents = [
    "JSStatement",
    "JSSyntax",
    "JSSourceElement",
    "JSFunctionBodyElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    expression,
    span = null,
  ) {
    super(span);
    this.expression = expression;
    this.span = span
  }

}
class JSConditionalCase extends SyntaxBase {

  static kind = 173;

  static parents = [
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    test,
    body,
    span = null,
  ) {
    super(span);
    this.test = test;
    this.body = body;
    this.span = span
  }

}
class JSConditionalStatement extends SyntaxBase {

  static kind = 174;

  static parents = [
    "JSStatement",
    "JSSyntax",
    "JSSourceElement",
    "JSFunctionBodyElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    cases,
    span = null,
  ) {
    super(span);
    this.cases = cases;
    this.span = span
  }

}
class JSReturnStatement extends SyntaxBase {

  static kind = 175;

  static parents = [
    "ReturnStatement",
    "JSStatement",
    "JSSyntax",
    "JSSourceElement",
    "JSFunctionBodyElement",
    "FunctionBodyElement",
    "Syntax"
  ];

  constructor(
    value,
    span = null,
  ) {
    super(span);
    this.value = value;
    this.span = span
  }

}
class JSParameter extends SyntaxBase {

  static kind = 176;

  static parents = [
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    index,
    bindings,
    defaultValue,
    span = null,
  ) {
    super(span);
    this.index = index;
    this.bindings = bindings;
    this.defaultValue = defaultValue;
    this.span = span
  }

}
class JSImportStarBinding extends SyntaxBase {

  static kind = 180;

  static parents = [
    "JSImportBinding",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    local,
    span = null,
  ) {
    super(span);
    this.local = local;
    this.span = span
  }

}
class JSImportAsBinding extends SyntaxBase {

  static kind = 181;

  static parents = [
    "JSImportBinding",
    "JSSyntax",
    "Syntax"
  ];

  constructor(
    remote,
    local,
    span = null,
  ) {
    super(span);
    this.remote = remote;
    this.local = local;
    this.span = span
  }

}
class JSImportDeclaration extends SyntaxBase {

  static kind = 182;

  static parents = [
    "JSDeclaration",
    "JSSyntax",
    "JSSourceElement",
    "Syntax"
  ];

  constructor(
    bindings,
    filename,
    span = null,
  ) {
    super(span);
    this.bindings = bindings;
    this.filename = filename;
    this.span = span
  }

}
class JSFunctionDeclaration extends SyntaxBase {

  static kind = 183;

  static parents = [
    "JSDeclaration",
    "JSFunctionBodyElement",
    "FunctionBodyElement",
    "JSSyntax",
    "JSSourceElement",
    "Syntax"
  ];

  constructor(
    modifiers,
    name,
    params,
    body,
    span = null,
  ) {
    super(span);
    this.modifiers = modifiers;
    this.name = name;
    this.params = params;
    this.body = body;
    this.span = span
  }

}
class JSArrowFunctionDeclaration extends SyntaxBase {

  static kind = 184;

  static parents = [
    "JSDeclaration",
    "JSFunctionBodyElement",
    "FunctionBodyElement",
    "JSSyntax",
    "JSSourceElement",
    "Syntax"
  ];

  constructor(
    name,
    params,
    body,
    span = null,
  ) {
    super(span);
    this.name = name;
    this.params = params;
    this.body = body;
    this.span = span
  }

}
class JSLetDeclaration extends SyntaxBase {

  static kind = 185;

  static parents = [
    "JSDeclaration",
    "JSFunctionBodyElement",
    "FunctionBodyElement",
    "JSSyntax",
    "JSSourceElement",
    "Syntax"
  ];

  constructor(
    bindings,
    value,
    span = null,
  ) {
    super(span);
    this.bindings = bindings;
    this.value = value;
    this.span = span
  }

}
class JSSourceFile extends SyntaxBase {

  static kind = 186;

  static parents = [
    "JSSyntax",
    "SourceFile",
    "Syntax"
  ];

  constructor(
    elements,
    span = null,
  ) {
    super(span);
    this.elements = elements;
    this.span = span
  }

}
const NODE_CLASSES = {
  EndOfFile,
  BoltStringLiteral,
  BoltIntegerLiteral,
  BoltIdentifier,
  BoltOperator,
  BoltAssignment,
  BoltComma,
  BoltSemi,
  BoltColon,
  BoltColonColon,
  BoltDot,
  BoltDotDot,
  BoltRArrow,
  BoltRArrowAlt,
  BoltLArrow,
  BoltEqSign,
  BoltGtSign,
  BoltExMark,
  BoltLtSign,
  BoltVBar,
  BoltWhereKeyword,
  BoltQuoteKeyword,
  BoltFnKeyword,
  BoltForeignKeyword,
  BoltForKeyword,
  BoltLetKeyword,
  BoltReturnKeyword,
  BoltLoopKeyword,
  BoltYieldKeyword,
  BoltMatchKeyword,
  BoltImportKeyword,
  BoltExportKeyword,
  BoltPubKeyword,
  BoltModKeyword,
  BoltMutKeyword,
  BoltEnumKeyword,
  BoltStructKeyword,
  BoltTypeKeyword,
  BoltTraitKeyword,
  BoltImplKeyword,
  BoltParenthesized,
  BoltBraced,
  BoltBracketed,
  BoltSourceFile,
  BoltQualName,
  BoltTypeOfExpression,
  BoltReferenceTypeExpression,
  BoltFunctionTypeExpression,
  BoltLiftedTypeExpression,
  BoltTypeParameter,
  BoltBindPattern,
  BoltTypePattern,
  BoltExpressionPattern,
  BoltTuplePatternElement,
  BoltTuplePattern,
  BoltRecordFieldPattern,
  BoltRecordPattern,
  BoltQuoteExpression,
  BoltTupleExpression,
  BoltReferenceExpression,
  BoltMemberExpression,
  BoltFunctionExpression,
  BoltCallExpression,
  BoltYieldExpression,
  BoltMatchArm,
  BoltMatchExpression,
  BoltCase,
  BoltCaseExpression,
  BoltBlockExpression,
  BoltConstantExpression,
  BoltReturnStatement,
  BoltConditionalCase,
  BoltConditionalStatement,
  BoltResumeStatement,
  BoltExpressionStatement,
  BoltParameter,
  BoltModule,
  BoltFunctionDeclaration,
  BoltVariableDeclaration,
  BoltPlainImportSymbol,
  BoltImportDirective,
  BoltExportSymbol,
  BoltPlainExportSymbol,
  BoltExportDirective,
  BoltTraitDeclaration,
  BoltImplDeclaration,
  BoltTypeAliasDeclaration,
  BoltRecordField,
  BoltRecordDeclaration,
  BoltMacroCall,
  JSIdentifier,
  JSString,
  JSInteger,
  JSFromKeyword,
  JSReturnKeyword,
  JSTryKeyword,
  JSFinallyKeyword,
  JSCatchKeyword,
  JSImportKeyword,
  JSAsKeyword,
  JSConstKeyword,
  JSLetKeyword,
  JSExportKeyword,
  JSFunctionKeyword,
  JSWhileKeyword,
  JSForKeyword,
  JSOperator,
  JSCloseBrace,
  JSCloseBracket,
  JSCloseParen,
  JSOpenBrace,
  JSOpenBracket,
  JSOpenParen,
  JSSemi,
  JSComma,
  JSDot,
  JSDotDotDot,
  JSMulOp,
  JSAddOp,
  JSDivOp,
  JSSubOp,
  JSLtOp,
  JSGtOp,
  JSBOrOp,
  JSBXorOp,
  JSBAndOp,
  JSBNotOp,
  JSNotOp,
  JSBindPattern,
  JSConstantExpression,
  JSMemberExpression,
  JSCallExpression,
  JSBinaryExpression,
  JSUnaryExpression,
  JSNewExpression,
  JSSequenceExpression,
  JSConditionalExpression,
  JSLiteralExpression,
  JSReferenceExpression,
  JSCatchBlock,
  JSTryCatchStatement,
  JSExpressionStatement,
  JSConditionalCase,
  JSConditionalStatement,
  JSReturnStatement,
  JSParameter,
  JSImportStarBinding,
  JSImportAsBinding,
  JSImportDeclaration,
  JSFunctionDeclaration,
  JSArrowFunctionDeclaration,
  JSLetDeclaration,
  JSSourceFile,
}

export function kindToString (kind) {
  switch (kind) {
    case 0: return 'EndOfFile';
    case 7: return 'BoltStringLiteral';
    case 8: return 'BoltIntegerLiteral';
    case 10: return 'BoltIdentifier';
    case 12: return 'BoltOperator';
    case 13: return 'BoltAssignment';
    case 14: return 'BoltComma';
    case 15: return 'BoltSemi';
    case 16: return 'BoltColon';
    case 17: return 'BoltColonColon';
    case 18: return 'BoltDot';
    case 19: return 'BoltDotDot';
    case 20: return 'BoltRArrow';
    case 21: return 'BoltRArrowAlt';
    case 22: return 'BoltLArrow';
    case 23: return 'BoltEqSign';
    case 24: return 'BoltGtSign';
    case 25: return 'BoltExMark';
    case 26: return 'BoltLtSign';
    case 27: return 'BoltVBar';
    case 29: return 'BoltWhereKeyword';
    case 30: return 'BoltQuoteKeyword';
    case 31: return 'BoltFnKeyword';
    case 32: return 'BoltForeignKeyword';
    case 33: return 'BoltForKeyword';
    case 34: return 'BoltLetKeyword';
    case 35: return 'BoltReturnKeyword';
    case 36: return 'BoltLoopKeyword';
    case 37: return 'BoltYieldKeyword';
    case 38: return 'BoltMatchKeyword';
    case 39: return 'BoltImportKeyword';
    case 40: return 'BoltExportKeyword';
    case 41: return 'BoltPubKeyword';
    case 42: return 'BoltModKeyword';
    case 43: return 'BoltMutKeyword';
    case 44: return 'BoltEnumKeyword';
    case 45: return 'BoltStructKeyword';
    case 46: return 'BoltTypeKeyword';
    case 47: return 'BoltTraitKeyword';
    case 48: return 'BoltImplKeyword';
    case 50: return 'BoltParenthesized';
    case 51: return 'BoltBraced';
    case 52: return 'BoltBracketed';
    case 53: return 'BoltSourceFile';
    case 54: return 'BoltQualName';
    case 56: return 'BoltTypeOfExpression';
    case 57: return 'BoltReferenceTypeExpression';
    case 58: return 'BoltFunctionTypeExpression';
    case 59: return 'BoltLiftedTypeExpression';
    case 60: return 'BoltTypeParameter';
    case 62: return 'BoltBindPattern';
    case 63: return 'BoltTypePattern';
    case 64: return 'BoltExpressionPattern';
    case 65: return 'BoltTuplePatternElement';
    case 66: return 'BoltTuplePattern';
    case 67: return 'BoltRecordFieldPattern';
    case 68: return 'BoltRecordPattern';
    case 70: return 'BoltQuoteExpression';
    case 71: return 'BoltTupleExpression';
    case 72: return 'BoltReferenceExpression';
    case 73: return 'BoltMemberExpression';
    case 74: return 'BoltFunctionExpression';
    case 75: return 'BoltCallExpression';
    case 76: return 'BoltYieldExpression';
    case 77: return 'BoltMatchArm';
    case 78: return 'BoltMatchExpression';
    case 79: return 'BoltCase';
    case 80: return 'BoltCaseExpression';
    case 81: return 'BoltBlockExpression';
    case 82: return 'BoltConstantExpression';
    case 84: return 'BoltReturnStatement';
    case 85: return 'BoltConditionalCase';
    case 86: return 'BoltConditionalStatement';
    case 87: return 'BoltResumeStatement';
    case 88: return 'BoltExpressionStatement';
    case 89: return 'BoltParameter';
    case 93: return 'BoltModule';
    case 96: return 'BoltFunctionDeclaration';
    case 97: return 'BoltVariableDeclaration';
    case 99: return 'BoltPlainImportSymbol';
    case 100: return 'BoltImportDirective';
    case 101: return 'BoltExportSymbol';
    case 102: return 'BoltPlainExportSymbol';
    case 103: return 'BoltExportDirective';
    case 105: return 'BoltTraitDeclaration';
    case 106: return 'BoltImplDeclaration';
    case 107: return 'BoltTypeAliasDeclaration';
    case 109: return 'BoltRecordField';
    case 110: return 'BoltRecordDeclaration';
    case 112: return 'BoltMacroCall';
    case 115: return 'JSIdentifier';
    case 116: return 'JSString';
    case 117: return 'JSInteger';
    case 118: return 'JSFromKeyword';
    case 119: return 'JSReturnKeyword';
    case 120: return 'JSTryKeyword';
    case 121: return 'JSFinallyKeyword';
    case 122: return 'JSCatchKeyword';
    case 123: return 'JSImportKeyword';
    case 124: return 'JSAsKeyword';
    case 125: return 'JSConstKeyword';
    case 126: return 'JSLetKeyword';
    case 127: return 'JSExportKeyword';
    case 128: return 'JSFunctionKeyword';
    case 129: return 'JSWhileKeyword';
    case 130: return 'JSForKeyword';
    case 132: return 'JSOperator';
    case 133: return 'JSCloseBrace';
    case 134: return 'JSCloseBracket';
    case 135: return 'JSCloseParen';
    case 136: return 'JSOpenBrace';
    case 137: return 'JSOpenBracket';
    case 138: return 'JSOpenParen';
    case 139: return 'JSSemi';
    case 140: return 'JSComma';
    case 141: return 'JSDot';
    case 142: return 'JSDotDotDot';
    case 143: return 'JSMulOp';
    case 144: return 'JSAddOp';
    case 145: return 'JSDivOp';
    case 146: return 'JSSubOp';
    case 147: return 'JSLtOp';
    case 148: return 'JSGtOp';
    case 149: return 'JSBOrOp';
    case 150: return 'JSBXorOp';
    case 151: return 'JSBAndOp';
    case 152: return 'JSBNotOp';
    case 153: return 'JSNotOp';
    case 155: return 'JSBindPattern';
    case 157: return 'JSConstantExpression';
    case 158: return 'JSMemberExpression';
    case 159: return 'JSCallExpression';
    case 160: return 'JSBinaryExpression';
    case 161: return 'JSUnaryExpression';
    case 162: return 'JSNewExpression';
    case 163: return 'JSSequenceExpression';
    case 164: return 'JSConditionalExpression';
    case 165: return 'JSLiteralExpression';
    case 166: return 'JSReferenceExpression';
    case 170: return 'JSCatchBlock';
    case 171: return 'JSTryCatchStatement';
    case 172: return 'JSExpressionStatement';
    case 173: return 'JSConditionalCase';
    case 174: return 'JSConditionalStatement';
    case 175: return 'JSReturnStatement';
    case 176: return 'JSParameter';
    case 180: return 'JSImportStarBinding';
    case 181: return 'JSImportAsBinding';
    case 182: return 'JSImportDeclaration';
    case 183: return 'JSFunctionDeclaration';
    case 184: return 'JSArrowFunctionDeclaration';
    case 185: return 'JSLetDeclaration';
    case 186: return 'JSSourceFile';
  }
}

export function isEndOfFile(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 0;
}
export function isToken(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 0 || value.kind === 115 || value.kind === 116 || value.kind === 117 || value.kind === 118 || value.kind === 119 || value.kind === 120 || value.kind === 121 || value.kind === 122 || value.kind === 123 || value.kind === 124 || value.kind === 125 || value.kind === 126 || value.kind === 127 || value.kind === 128 || value.kind === 129 || value.kind === 130 || value.kind === 132 || value.kind === 133 || value.kind === 134 || value.kind === 135 || value.kind === 136 || value.kind === 137 || value.kind === 138 || value.kind === 139 || value.kind === 140 || value.kind === 141 || value.kind === 142 || value.kind === 143 || value.kind === 144 || value.kind === 145 || value.kind === 146 || value.kind === 147 || value.kind === 148 || value.kind === 149 || value.kind === 150 || value.kind === 151 || value.kind === 152 || value.kind === 153 || value.kind === 0 || value.kind === 7 || value.kind === 8 || value.kind === 13 || value.kind === 14 || value.kind === 15 || value.kind === 16 || value.kind === 17 || value.kind === 18 || value.kind === 19 || value.kind === 20 || value.kind === 21 || value.kind === 22 || value.kind === 23 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27 || value.kind === 29 || value.kind === 30 || value.kind === 31 || value.kind === 32 || value.kind === 33 || value.kind === 34 || value.kind === 35 || value.kind === 36 || value.kind === 37 || value.kind === 38 || value.kind === 39 || value.kind === 40 || value.kind === 41 || value.kind === 42 || value.kind === 43 || value.kind === 44 || value.kind === 45 || value.kind === 46 || value.kind === 47 || value.kind === 48 || value.kind === 50 || value.kind === 51 || value.kind === 52 || value.kind === 10 || value.kind === 12 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27
}
export function isSourceFile(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 53 || value.kind === 186
}
export function isFunctionBodyElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 183 || value.kind === 184 || value.kind === 185 || value.kind === 172 || value.kind === 174 || value.kind === 175 || value.kind === 96 || value.kind === 97 || value.kind === 112 || value.kind === 84 || value.kind === 86 || value.kind === 87 || value.kind === 88
}
export function isReturnStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 84 || value.kind === 175
}
export function isBoltSyntax(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 53 || value.kind === 54 || value.kind === 60 || value.kind === 65 || value.kind === 67 || value.kind === 77 || value.kind === 79 || value.kind === 85 || value.kind === 89 || value.kind === 93 || value.kind === 101 || value.kind === 109 || value.kind === 112 || value.kind === 99 || value.kind === 105 || value.kind === 106 || value.kind === 107 || value.kind === 110 || value.kind === 96 || value.kind === 97 || value.kind === 110 || value.kind === 84 || value.kind === 86 || value.kind === 87 || value.kind === 88 || value.kind === 70 || value.kind === 71 || value.kind === 72 || value.kind === 73 || value.kind === 74 || value.kind === 75 || value.kind === 76 || value.kind === 78 || value.kind === 80 || value.kind === 81 || value.kind === 82 || value.kind === 62 || value.kind === 63 || value.kind === 64 || value.kind === 66 || value.kind === 68 || value.kind === 56 || value.kind === 57 || value.kind === 58 || value.kind === 59 || value.kind === 0 || value.kind === 7 || value.kind === 8 || value.kind === 13 || value.kind === 14 || value.kind === 15 || value.kind === 16 || value.kind === 17 || value.kind === 18 || value.kind === 19 || value.kind === 20 || value.kind === 21 || value.kind === 22 || value.kind === 23 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27 || value.kind === 29 || value.kind === 30 || value.kind === 31 || value.kind === 32 || value.kind === 33 || value.kind === 34 || value.kind === 35 || value.kind === 36 || value.kind === 37 || value.kind === 38 || value.kind === 39 || value.kind === 40 || value.kind === 41 || value.kind === 42 || value.kind === 43 || value.kind === 44 || value.kind === 45 || value.kind === 46 || value.kind === 47 || value.kind === 48 || value.kind === 50 || value.kind === 51 || value.kind === 52 || value.kind === 10 || value.kind === 12 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27
}
export function isBoltToken(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 0 || value.kind === 7 || value.kind === 8 || value.kind === 13 || value.kind === 14 || value.kind === 15 || value.kind === 16 || value.kind === 17 || value.kind === 18 || value.kind === 19 || value.kind === 20 || value.kind === 21 || value.kind === 22 || value.kind === 23 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27 || value.kind === 29 || value.kind === 30 || value.kind === 31 || value.kind === 32 || value.kind === 33 || value.kind === 34 || value.kind === 35 || value.kind === 36 || value.kind === 37 || value.kind === 38 || value.kind === 39 || value.kind === 40 || value.kind === 41 || value.kind === 42 || value.kind === 43 || value.kind === 44 || value.kind === 45 || value.kind === 46 || value.kind === 47 || value.kind === 48 || value.kind === 50 || value.kind === 51 || value.kind === 52 || value.kind === 10 || value.kind === 12 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27
}
export function isBoltStringLiteral(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 7;
}
export function isBoltIntegerLiteral(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 8;
}
export function isBoltSymbol(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 10 || value.kind === 12 || value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27
}
export function isBoltIdentifier(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 10;
}
export function isBoltOperatorLike(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 24 || value.kind === 25 || value.kind === 26 || value.kind === 27
}
export function isBoltOperator(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 12;
}
export function isBoltAssignment(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 13;
}
export function isBoltComma(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 14;
}
export function isBoltSemi(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 15;
}
export function isBoltColon(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 16;
}
export function isBoltColonColon(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 17;
}
export function isBoltDot(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 18;
}
export function isBoltDotDot(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 19;
}
export function isBoltRArrow(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 20;
}
export function isBoltRArrowAlt(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 21;
}
export function isBoltLArrow(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 22;
}
export function isBoltEqSign(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 23;
}
export function isBoltGtSign(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 24;
}
export function isBoltExMark(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 25;
}
export function isBoltLtSign(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 26;
}
export function isBoltVBar(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 27;
}
export function isBoltKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 29 || value.kind === 30 || value.kind === 31 || value.kind === 32 || value.kind === 33 || value.kind === 34 || value.kind === 35 || value.kind === 36 || value.kind === 37 || value.kind === 38 || value.kind === 39 || value.kind === 40 || value.kind === 41 || value.kind === 42 || value.kind === 43 || value.kind === 44 || value.kind === 45 || value.kind === 46 || value.kind === 47 || value.kind === 48
}
export function isBoltWhereKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 29;
}
export function isBoltQuoteKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 30;
}
export function isBoltFnKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 31;
}
export function isBoltForeignKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 32;
}
export function isBoltForKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 33;
}
export function isBoltLetKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 34;
}
export function isBoltReturnKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 35;
}
export function isBoltLoopKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 36;
}
export function isBoltYieldKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 37;
}
export function isBoltMatchKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 38;
}
export function isBoltImportKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 39;
}
export function isBoltExportKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 40;
}
export function isBoltPubKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 41;
}
export function isBoltModKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 42;
}
export function isBoltMutKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 43;
}
export function isBoltEnumKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 44;
}
export function isBoltStructKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 45;
}
export function isBoltTypeKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 46;
}
export function isBoltTraitKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 47;
}
export function isBoltImplKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 48;
}
export function isBoltPunctuated(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 50 || value.kind === 51 || value.kind === 52
}
export function isBoltParenthesized(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 50;
}
export function isBoltBraced(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 51;
}
export function isBoltBracketed(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 52;
}
export function isBoltSourceFile(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 53;
}
export function isBoltQualName(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 54;
}
export function isBoltTypeExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 56 || value.kind === 57 || value.kind === 58 || value.kind === 59
}
export function isBoltTypeOfExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 56;
}
export function isBoltReferenceTypeExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 57;
}
export function isBoltFunctionTypeExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 58;
}
export function isBoltLiftedTypeExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 59;
}
export function isBoltTypeParameter(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 60;
}
export function isBoltPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 62 || value.kind === 63 || value.kind === 64 || value.kind === 66 || value.kind === 68
}
export function isBoltBindPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 62;
}
export function isBoltTypePattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 63;
}
export function isBoltExpressionPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 64;
}
export function isBoltTuplePatternElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 65;
}
export function isBoltTuplePattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 66;
}
export function isBoltRecordFieldPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 67;
}
export function isBoltRecordPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 68;
}
export function isBoltExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 70 || value.kind === 71 || value.kind === 72 || value.kind === 73 || value.kind === 74 || value.kind === 75 || value.kind === 76 || value.kind === 78 || value.kind === 80 || value.kind === 81 || value.kind === 82
}
export function isBoltQuoteExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 70;
}
export function isBoltTupleExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 71;
}
export function isBoltReferenceExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 72;
}
export function isBoltMemberExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 73;
}
export function isBoltFunctionExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 74;
}
export function isBoltCallExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 75;
}
export function isBoltYieldExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 76;
}
export function isBoltMatchArm(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 77;
}
export function isBoltMatchExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 78;
}
export function isBoltCase(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 79;
}
export function isBoltCaseExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 80;
}
export function isBoltBlockExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 81;
}
export function isBoltConstantExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 82;
}
export function isBoltStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 84 || value.kind === 86 || value.kind === 87 || value.kind === 88
}
export function isBoltReturnStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 84;
}
export function isBoltConditionalCase(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 85;
}
export function isBoltConditionalStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 86;
}
export function isBoltResumeStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 87;
}
export function isBoltExpressionStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 88;
}
export function isBoltParameter(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 89;
}
export function isBoltDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 96 || value.kind === 97 || value.kind === 110
}
export function isBoltTypeDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 105 || value.kind === 106 || value.kind === 107 || value.kind === 110
}
export function isBoltModule(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 93;
}
export function isBoltDeclarationLike(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 96 || value.kind === 97 || value.kind === 105 || value.kind === 106 || value.kind === 107 || value.kind === 110
}
export function isBoltFunctionBodyElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 96 || value.kind === 97 || value.kind === 112 || value.kind === 84 || value.kind === 86 || value.kind === 87 || value.kind === 88
}
export function isBoltFunctionDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 96;
}
export function isBoltVariableDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 97;
}
export function isBoltImportSymbol(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 99
}
export function isBoltPlainImportSymbol(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 99;
}
export function isBoltImportDirective(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 100;
}
export function isBoltExportSymbol(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 101;
}
export function isBoltPlainExportSymbol(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 102;
}
export function isBoltExportDirective(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 103;
}
export function isBoltTraitOrImplElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 96 || value.kind === 107 || value.kind === 112
}
export function isBoltTraitDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 105;
}
export function isBoltImplDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 106;
}
export function isBoltTypeAliasDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 107;
}
export function isBoltRecordMember(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 109 || value.kind === 112
}
export function isBoltRecordField(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 109;
}
export function isBoltRecordDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 110;
}
export function isBoltSourceElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 93 || value.kind === 100 || value.kind === 103 || value.kind === 112 || value.kind === 105 || value.kind === 106 || value.kind === 107 || value.kind === 110 || value.kind === 96 || value.kind === 97 || value.kind === 110 || value.kind === 84 || value.kind === 86 || value.kind === 87 || value.kind === 88
}
export function isBoltMacroCall(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 112;
}
export function isJSSyntax(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 170 || value.kind === 171 || value.kind === 173 || value.kind === 176 || value.kind === 186 || value.kind === 180 || value.kind === 181 || value.kind === 182 || value.kind === 183 || value.kind === 184 || value.kind === 185 || value.kind === 172 || value.kind === 174 || value.kind === 175 || value.kind === 157 || value.kind === 158 || value.kind === 159 || value.kind === 160 || value.kind === 161 || value.kind === 162 || value.kind === 163 || value.kind === 164 || value.kind === 165 || value.kind === 166 || value.kind === 155 || value.kind === 0 || value.kind === 115 || value.kind === 116 || value.kind === 117 || value.kind === 118 || value.kind === 119 || value.kind === 120 || value.kind === 121 || value.kind === 122 || value.kind === 123 || value.kind === 124 || value.kind === 125 || value.kind === 126 || value.kind === 127 || value.kind === 128 || value.kind === 129 || value.kind === 130 || value.kind === 132 || value.kind === 133 || value.kind === 134 || value.kind === 135 || value.kind === 136 || value.kind === 137 || value.kind === 138 || value.kind === 139 || value.kind === 140 || value.kind === 141 || value.kind === 142 || value.kind === 143 || value.kind === 144 || value.kind === 145 || value.kind === 146 || value.kind === 147 || value.kind === 148 || value.kind === 149 || value.kind === 150 || value.kind === 151 || value.kind === 152 || value.kind === 153
}
export function isJSToken(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 0 || value.kind === 115 || value.kind === 116 || value.kind === 117 || value.kind === 118 || value.kind === 119 || value.kind === 120 || value.kind === 121 || value.kind === 122 || value.kind === 123 || value.kind === 124 || value.kind === 125 || value.kind === 126 || value.kind === 127 || value.kind === 128 || value.kind === 129 || value.kind === 130 || value.kind === 132 || value.kind === 133 || value.kind === 134 || value.kind === 135 || value.kind === 136 || value.kind === 137 || value.kind === 138 || value.kind === 139 || value.kind === 140 || value.kind === 141 || value.kind === 142 || value.kind === 143 || value.kind === 144 || value.kind === 145 || value.kind === 146 || value.kind === 147 || value.kind === 148 || value.kind === 149 || value.kind === 150 || value.kind === 151 || value.kind === 152 || value.kind === 153
}
export function isJSIdentifier(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 115;
}
export function isJSString(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 116;
}
export function isJSInteger(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 117;
}
export function isJSFromKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 118;
}
export function isJSReturnKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 119;
}
export function isJSTryKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 120;
}
export function isJSFinallyKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 121;
}
export function isJSCatchKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 122;
}
export function isJSImportKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 123;
}
export function isJSAsKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 124;
}
export function isJSConstKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 125;
}
export function isJSLetKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 126;
}
export function isJSExportKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 127;
}
export function isJSFunctionKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 128;
}
export function isJSWhileKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 129;
}
export function isJSForKeyword(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 130;
}
export function isJSOperatorLike(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 143 || value.kind === 144 || value.kind === 145 || value.kind === 146 || value.kind === 147 || value.kind === 148 || value.kind === 149 || value.kind === 150 || value.kind === 151 || value.kind === 152 || value.kind === 153
}
export function isJSOperator(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 132;
}
export function isJSCloseBrace(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 133;
}
export function isJSCloseBracket(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 134;
}
export function isJSCloseParen(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 135;
}
export function isJSOpenBrace(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 136;
}
export function isJSOpenBracket(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 137;
}
export function isJSOpenParen(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 138;
}
export function isJSSemi(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 139;
}
export function isJSComma(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 140;
}
export function isJSDot(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 141;
}
export function isJSDotDotDot(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 142;
}
export function isJSMulOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 143;
}
export function isJSAddOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 144;
}
export function isJSDivOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 145;
}
export function isJSSubOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 146;
}
export function isJSLtOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 147;
}
export function isJSGtOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 148;
}
export function isJSBOrOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 149;
}
export function isJSBXorOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 150;
}
export function isJSBAndOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 151;
}
export function isJSBNotOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 152;
}
export function isJSNotOp(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 153;
}
export function isJSPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 155
}
export function isJSBindPattern(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 155;
}
export function isJSExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 157 || value.kind === 158 || value.kind === 159 || value.kind === 160 || value.kind === 161 || value.kind === 162 || value.kind === 163 || value.kind === 164 || value.kind === 165 || value.kind === 166
}
export function isJSConstantExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 157;
}
export function isJSMemberExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 158;
}
export function isJSCallExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 159;
}
export function isJSBinaryExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 160;
}
export function isJSUnaryExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 161;
}
export function isJSNewExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 162;
}
export function isJSSequenceExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 163;
}
export function isJSConditionalExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 164;
}
export function isJSLiteralExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 165;
}
export function isJSReferenceExpression(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 166;
}
export function isJSSourceElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 182 || value.kind === 183 || value.kind === 184 || value.kind === 185 || value.kind === 172 || value.kind === 174 || value.kind === 175
}
export function isJSFunctionBodyElement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 183 || value.kind === 184 || value.kind === 185 || value.kind === 172 || value.kind === 174 || value.kind === 175
}
export function isJSStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 172 || value.kind === 174 || value.kind === 175
}
export function isJSCatchBlock(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 170;
}
export function isJSTryCatchStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 171;
}
export function isJSExpressionStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 172;
}
export function isJSConditionalCase(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 173;
}
export function isJSConditionalStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 174;
}
export function isJSReturnStatement(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 175;
}
export function isJSParameter(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 176;
}
export function isJSDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 182 || value.kind === 183 || value.kind === 184 || value.kind === 185
}
export function isJSImportBinding(value) {
  if (!isSyntax(value)) {
    return false;
  }
  return value.kind === 180 || value.kind === 181
}
export function isJSImportStarBinding(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 180;
}
export function isJSImportAsBinding(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 181;
}
export function isJSImportDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 182;
}
export function isJSFunctionDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 183;
}
export function isJSArrowFunctionDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 184;
}
export function isJSLetDeclaration(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 185;
}
export function isJSSourceFile(value) {
  if (!isSyntax(value)) {
    return false;
  }
    return value.kind === 186;
}
