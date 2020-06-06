
import { Type } from "./types"
import { TextSpan } from "./text"
import { Value } from "./evaluator"
import { Package } from "./package"
import { Diagnostic } from "./diagnostics";
import { serializeTag, serialize, JsonObject } from "./util";

let nextNodeId = 1;

type SyntaxKind = number;

export type ResolveSyntaxKind<K extends SyntaxKind> = Extract<Syntax, { kind: K }>;

export abstract class Syntax {

  public id: number;

  public type?: Type;

  public errors: Diagnostic[] = [];

  public abstract kind: SyntaxKind;

  public abstract parentNode: Syntax | null = null;

  public abstract getChildNodes(): IterableIterator<Syntax>;

  constructor(public span: TextSpan | null = null) {
    this.id = nextNodeId++;
  }

  [serializeTag]() {
    const result: JsonObject = {};
    for (const key of Object.keys(this)) {
      if (key === 'parentNode' || key === 'errors' || key === 'type' || key === 'id') {
        continue;
      }
      result[key] = serialize((this as any)[key]);
    }
    return result;
  }

  *preorder() {
    const stack: Syntax[] = [ this as unknown as Syntax ] ;
    while (stack.length > 0) {
      const node = stack.pop()!;
      yield node
      for (const childNode of node.getChildNodes()) {
        stack.push(childNode);
      }
    }
  }

  mayContainKind(kind: SyntaxKind) {
    // TODO
    return true;
  }

  getParentOfKind(kind: SyntaxKind) {
    let currNode = this.parentNode;
    while (currNode !== null) {
      if (currNode.kind === kind) {
        return currNode;
      }
      currNode = currNode.parentNode;
    }
    return null;
  }

  *findAllChildrenOfKind<K extends SyntaxKind>(kind: K): IterableIterator<ResolveSyntaxKind<K>> {
    for (const node of this.preorder()) {
      if (!node.mayContainKind(kind)) {
        break;
      }
      if (node.kind === kind) {
        yield node as ResolveSyntaxKind<K>;
      }
    }
  }

}

export function setParents(node: Syntax, parentNode: Syntax | null = null) {
  // NOTE We cast to any here because TypeScript does not like this complex assignment
  node.parentNode = parentNode as any;
  for (const child of node.getChildNodes()) {
    setParents(child, node)
  }
} 

export interface EndOfFile extends BoltToken, JSToken {}

export interface Token {}
export interface SourceFile {}
export interface FunctionBodyElement {}
export interface ReturnStatement {}

// Bolt language AST definitions

export interface BoltSyntax extends Syntax {}

export interface BoltToken extends Token, BoltSyntax {}

export interface BoltStringLiteral extends BoltToken {
  value: string,
}

export interface BoltIntegerLiteral extends BoltToken {
  value: bigint,
}

export interface BoltSymbol extends BoltToken {}

export interface BoltIdentifier extends BoltSymbol {
  text: string,
}

export interface BoltOperatorLike extends BoltSymbol {}

export interface BoltOperator extends BoltSymbol {
  text: string,
}

export interface BoltAssignment extends BoltToken {
  operator: string | null,
}

export interface BoltComma      extends BoltToken {}
export interface BoltSemi       extends BoltToken {}
export interface BoltColon      extends BoltToken {}
export interface BoltColonColon extends BoltToken {}
export interface BoltDot        extends BoltToken {}
export interface BoltDotDot     extends BoltToken {}
export interface BoltRArrow     extends BoltToken {}
export interface BoltRArrowAlt  extends BoltToken {}
export interface BoltLArrow     extends BoltToken {}
export interface BoltEqSign     extends BoltToken {}

export interface BoltGtSign    extends BoltToken, BoltOperatorLike {}
export interface BoltExMark    extends BoltToken, BoltOperatorLike {}
export interface BoltLtSign    extends BoltToken, BoltOperatorLike {}
export interface BoltVBar      extends BoltToken, BoltOperatorLike {}

export interface BoltKeyword {}

export interface BoltWhereKeyword   extends BoltToken, BoltKeyword {}
export interface BoltQuoteKeyword   extends BoltToken, BoltKeyword {}
export interface BoltFnKeyword      extends BoltToken, BoltKeyword {}
export interface BoltForeignKeyword extends BoltToken, BoltKeyword {}
export interface BoltForKeyword     extends BoltToken, BoltKeyword {}
export interface BoltLetKeyword     extends BoltToken, BoltKeyword {}
export interface BoltReturnKeyword  extends BoltToken, BoltKeyword {}
export interface BoltLoopKeyword    extends BoltToken, BoltKeyword {}
export interface BoltYieldKeyword   extends BoltToken, BoltKeyword {}
export interface BoltMatchKeyword   extends BoltToken, BoltKeyword {}
export interface BoltImportKeyword  extends BoltToken, BoltKeyword {}
export interface BoltExportKeyword  extends BoltToken, BoltKeyword {}
export interface BoltPubKeyword     extends BoltToken, BoltKeyword {}
export interface BoltModKeyword     extends BoltToken, BoltKeyword {}
export interface BoltMutKeyword     extends BoltToken, BoltKeyword {}
export interface BoltEnumKeyword    extends BoltToken, BoltKeyword {}
export interface BoltStructKeyword  extends BoltToken, BoltKeyword {}
export interface BoltTypeKeyword    extends BoltToken, BoltKeyword {}
export interface BoltTraitKeyword   extends BoltToken, BoltKeyword {}
export interface BoltImplKeyword    extends BoltToken, BoltKeyword {}

export interface BoltPunctuated extends BoltToken {
  text: string,
}

export interface BoltParenthesized extends BoltPunctuated {}
export interface BoltBraced extends BoltPunctuated {}
export interface BoltBracketed extends BoltPunctuated {}

export interface BoltSourceFile extends BoltSyntax, SourceFile {
  elements: BoltSourceElement[],
  pkg: Package | null,
}

export interface BoltQualName extends BoltSyntax {
  isAbsolute: boolean,
  modulePath: BoltIdentifier[],
  name: BoltSymbol,
}

export interface BoltTypeExpression extends BoltSyntax {}

export interface BoltTypeOfExpression extends BoltTypeExpression {
  expression: BoltExpression,
}

export interface BoltReferenceTypeExpression extends BoltTypeExpression {
  name: BoltQualName,
  typeArgs: BoltTypeExpression[] | null,
}

export interface BoltFunctionTypeExpression extends BoltTypeExpression {
  params: BoltParameter[],
  returnType: BoltTypeExpression | null,
}

export interface BoltLiftedTypeExpression extends BoltTypeExpression {
  expression: BoltExpression, 
}

export interface BoltTypeParameter extends BoltSyntax {
  index: number,
  name: BoltIdentifier,
  typeExpr: BoltTypeExpression | null,
  defaultType: BoltTypeExpression | null,
}

export interface BoltPattern extends BoltSyntax {}

export interface BoltBindPattern extends BoltPattern {
  name: BoltIdentifier,
}

export interface BoltTypePattern extends BoltPattern {
  typeExpr: BoltTypeExpression,
  nestedPattern: BoltPattern,
}

export interface BoltExpressionPattern extends BoltPattern {
  expression: BoltExpression,
}

export interface BoltTuplePatternElement extends BoltSyntax {
  index: number,
  pattern: BoltPattern,
}

export interface BoltTuplePattern extends BoltPattern {
  elements: BoltTuplePatternElement[],
}

export interface BoltRecordFieldPattern extends BoltSyntax {
  isRest: boolean,
  name: BoltIdentifier | null,
  pattern: BoltPattern | null,
}

export interface BoltRecordPattern extends BoltPattern {
  name: BoltTypeExpression,
  fields: BoltRecordFieldPattern[],
}

export interface BoltExpression extends BoltSyntax {}

export interface BoltQuoteExpression extends BoltExpression {
  tokens: (Token | BoltExpression)[],
}

export interface BoltTupleExpression extends BoltExpression {
  elements: BoltExpression[],
}

export interface BoltReferenceExpression extends BoltExpression {
  name: BoltQualName,
}

export interface BoltMemberExpression extends BoltExpression {
  expression: BoltExpression,
  path: BoltIdentifier[],
}

export interface BoltFunctionExpression extends BoltExpression {
  params: BoltParameter[],
  returnType: BoltTypeExpression | null,
  body: BoltFunctionBodyElement[],
}

export interface BoltCallExpression extends BoltExpression {
  operator: BoltExpression,
  operands: BoltExpression[],
}

export interface BoltYieldExpression extends BoltExpression {
  value: BoltExpression,
}

export interface BoltMatchArm extends BoltSyntax {
  pattern: BoltPattern,
  body: BoltExpression,
}

export interface BoltMatchExpression extends BoltExpression {
  value: BoltExpression,
  arms: BoltMatchArm[],
}

export interface BoltCase extends BoltSyntax {
  test: BoltExpression,
  result: BoltExpression,
}

export interface BoltCaseExpression extends BoltExpression {
  cases: BoltCase[],
}

export interface BoltBlockExpression extends BoltExpression {
  elements: BoltFunctionBodyElement[],
}

export interface BoltConstantExpression extends BoltExpression {
  value: Value,
}

export interface BoltStatement extends BoltSyntax, BoltFunctionBodyElement, BoltSourceElement {}

export interface BoltReturnStatement extends ReturnStatement, BoltStatement {
  value: BoltExpression | null,
}

export interface BoltConditionalCase extends BoltSyntax {
  test: BoltExpression | null,
  body: BoltFunctionBodyElement[],
}

export interface BoltConditionalStatement extends BoltStatement {
  cases: BoltConditionalCase[],
}

export interface BoltResumeStatement extends BoltStatement {
  value: BoltExpression,
}

export interface BoltExpressionStatement extends BoltStatement {
  expression: BoltExpression,
}

export interface BoltLoopStatement extends BoltStatement {
  elements: BoltFunctionBodyElement[],
}

export interface BoltParameter extends BoltSyntax {
  index: number,
  bindings: BoltPattern,
  typeExpr: BoltTypeExpression | null,
  defaultValue: BoltExpression | null,
}

export interface BoltDeclaration extends BoltSyntax, BoltSourceElement {}

export interface BoltTypeDeclaration extends BoltSyntax, BoltSourceElement {}

export enum BoltModifiers {
  IsMutable   = 0x1,
  IsPublic    = 0x2,
}

export interface BoltModule extends BoltSyntax, BoltSourceElement {
  modifiers: BoltModifiers,
  name: BoltIdentifier[],
  elements: BoltSourceElement[],
}

export interface BoltDeclarationLike {}

export interface BoltFunctionBodyElement extends FunctionBodyElement {}

export interface BoltFunctionDeclaration extends BoltFunctionBodyElement, BoltDeclaration, BoltDeclarationLike, BoltTraitOrImplElement {
  modifiers: BoltModifiers,
  target: string,
  name: BoltSymbol,
  params: BoltParameter[],
  returnType: BoltTypeExpression | null,
  typeParams: BoltTypeParameter[] | null,
  body: BoltFunctionBodyElement[],
}

export interface BoltVariableDeclaration extends BoltFunctionBodyElement, BoltDeclaration, BoltDeclarationLike {
  modifiers: BoltModifiers,
  bindings: BoltPattern,
  typeExpr: BoltTypeExpression | null,
  value: BoltExpression | null,
}

export interface BoltImportSymbol extends BoltSyntax {}

export interface BoltPlainImportSymbol extends BoltImportSymbol {
  remote: BoltQualName,
  local: BoltSymbol,
}

export interface BoltImportDirective extends BoltSyntax, BoltSourceElement {
  modifiers: BoltModifiers,
  file: BoltStringLiteral,
  symbols: BoltImportSymbol[] | null,
}

export interface BoltExportSymbol extends BoltSyntax {}

export interface BoltPlainExportSymbol extends BoltExportSymbol {
  local: BoltQualName,
  remote: BoltSymbol,
}

export interface BoltExportDirective extends BoltSourceElement {
  file: string,
  symbols: BoltExportSymbol[] | null,
}

export interface BoltTraitOrImplElement {}

export interface BoltTraitDeclaration extends BoltDeclarationLike, BoltTypeDeclaration {
  modifiers: BoltModifiers,
  typeParams: BoltTypeParameter[] | null,
  name: BoltIdentifier,
  typeBoundExpr: BoltTypeExpression | null,
  elements: BoltTraitOrImplElement[] | null,
}

export interface BoltImplDeclaration extends BoltTypeDeclaration, BoltDeclarationLike {
  modifiers: BoltModifiers,
  typeParams: BoltTypeParameter[] | null,
  name: BoltIdentifier,
  traitTypeExpr: BoltTypeExpression | null,
  elements: BoltTraitOrImplElement[],
}

export interface BoltTypeAliasDeclaration extends BoltDeclarationLike, BoltTypeDeclaration, BoltTraitOrImplElement {
  modifiers: BoltModifiers,
  name: BoltIdentifier,
  typeParams: BoltTypeParameter[] | null,
  typeExpr: BoltTypeExpression,
}

export interface BoltRecordMember extends BoltSyntax {}

export interface BoltRecordField extends BoltRecordMember {
  name: BoltIdentifier,
  typeExpr: BoltTypeExpression,
}

export interface BoltRecordDeclaration extends BoltDeclaration, BoltTypeDeclaration, BoltDeclarationLike {
  modifiers: BoltModifiers,
  name: BoltIdentifier,
  typeParms: BoltTypeParameter[] | null,
  members: BoltRecordMember[] | null,
}

export interface BoltSourceElement {}

export interface BoltMacroCall extends BoltRecordMember, BoltSourceElement, BoltTraitOrImplElement, BoltFunctionBodyElement {
  name: BoltIdentifier,
  text: string,
}

// JavaScript AST definitions

export interface JSSyntax extends Syntax {}

export interface JSToken extends JSSyntax, Token {}

export interface JSIdentifier extends JSToken {
  text: string,
}

export interface JSString extends JSToken {
  value: string,
}

export interface JSInteger extends JSToken {
  value: bigint,
}

export interface JSFromKeyword extends JSToken {}
export interface JSReturnKeyword extends JSToken {}
export interface JSTryKeyword extends JSToken {}
export interface JSFinallyKeyword extends JSToken {}
export interface JSCatchKeyword extends JSToken {}
export interface JSImportKeyword extends JSToken {}
export interface JSAsKeyword extends JSToken {}
export interface JSConstKeyword extends JSToken {}
export interface JSLetKeyword extends JSToken {}
export interface JSExportKeyword extends JSToken {}
export interface JSFunctionKeyword extends JSToken {}
export interface JSWhileKeyword extends JSToken {}
export interface JSForKeyword extends JSToken {}

export interface JSOperatorLike {}

export interface JSOperator extends JSToken {
  text: string,
}

export interface JSCloseBrace extends JSToken {}
export interface JSCloseBracket extends JSToken {}
export interface JSCloseParen extends JSToken {}
export interface JSOpenBrace extends JSToken {}
export interface JSOpenBracket extends JSToken {}
export interface JSOpenParen extends JSToken {}
export interface JSSemi extends JSToken {}
export interface JSComma extends JSToken {}
export interface JSDot extends JSToken {}
export interface JSDotDotDot extends JSToken {}
export interface JSMulOp extends JSToken, JSOperatorLike {}
export interface JSAddOp extends JSToken, JSOperatorLike {}
export interface JSDivOp extends JSToken, JSOperatorLike {}
export interface JSSubOp extends JSToken, JSOperatorLike {}
export interface JSLtOp extends JSToken, JSOperatorLike {}
export interface JSGtOp extends JSToken, JSOperatorLike {}
export interface JSBOrOp extends JSToken, JSOperatorLike {}
export interface JSBXorOp extends JSToken, JSOperatorLike {}
export interface JSBAndOp extends JSToken, JSOperatorLike {}
export interface JSBNotOp extends JSToken, JSOperatorLike {}
export interface JSNotOp extends JSToken, JSOperatorLike {}

export interface JSPattern extends JSSyntax {}

export interface JSBindPattern extends JSPattern {
  name: JSIdentifier,
}

export interface JSExpression extends JSSyntax {}

export interface JSConstantExpression extends JSExpression {
  value: Value,
}

export interface JSMemberExpression extends JSExpression {
  value: JSExpression,
  property: JSIdentifier,
}

export interface JSCallExpression extends JSExpression {
  operator: JSExpression,
  operands: JSExpression[],
}

export interface JSBinaryExpression extends JSExpression {
  left: JSExpression,
  operator: JSOperator,
  right: JSExpression,
}

export interface JSUnaryExpression extends JSExpression {
  operator: JSOperator,
  operand: JSExpression
}

export interface JSNewExpression extends JSExpression {
  target: JSExpression,
  args: JSExpression[],
}

export interface JSSequenceExpression extends JSExpression {
  expressions: JSExpression[],
}

export interface JSConditionalExpression extends JSExpression {
  test: JSExpression,
  consequent: JSExpression,
  alternate: JSExpression,
}

export interface JSLiteralExpression extends JSExpression {
  value: Value,
}

export interface JSReferenceExpression extends JSExpression {
  name: string,
}

export interface JSSourceElement {}

export interface JSFunctionBodyElement extends FunctionBodyElement {}

export interface JSStatement extends JSSyntax, JSSourceElement, JSFunctionBodyElement {}

export interface JSCatchBlock extends JSSyntax {
  bindings: JSPattern | null,
  elements: JSSourceElement[],
}

export interface JSTryCatchStatement extends JSStatement {
  tryBlock: JSSourceElement[],
  catchBlock: JSCatchBlock | null,
  finalBlock: JSSourceElement[] | null,
}

export interface JSExpressionStatement extends JSStatement {
  expression: JSExpression,
}

export interface JSConditionalCase extends JSSyntax {
  test: JSExpression | null,
  body: JSFunctionBodyElement[],
}

export interface JSConditionalStatement extends JSStatement {
  cases: JSConditionalCase[],
}

export interface JSReturnStatement extends ReturnStatement, JSStatement {
  value: JSExpression | null,
}

export interface JSParameter extends JSSyntax {
  index: number,
  bindings: JSPattern,
  defaultValue: JSExpression | null,
}

export interface JSDeclaration extends JSSyntax, JSSourceElement {}

export enum JSDeclarationModifiers {
  IsExported = 0x1,
}

export interface JSImportBinding extends JSSyntax {}

export interface JSImportStarBinding extends JSImportBinding {
  local: JSIdentifier,
}

export interface JSImportAsBinding extends JSImportBinding {
  remote: JSIdentifier,
  local: JSIdentifier | null,
}

// By exception, we alloww 'import ..'-statements to appear in foreign function bodies
export interface JSImportDeclaration extends JSDeclaration, JSFunctionBodyElement {
  bindings: JSImportBinding[],
  filename: JSString,
}

export interface JSFunctionDeclaration extends JSDeclaration, JSFunctionBodyElement {
  modifiers: JSDeclarationModifiers,
  name: JSIdentifier,
  params: JSParameter[],
  body: JSStatement[],
}

export interface JSArrowFunctionDeclaration extends JSDeclaration, JSFunctionBodyElement {
  name: JSIdentifier,
  params: JSParameter[],
  body: JSExpression,
}

export interface JSLetDeclaration extends JSDeclaration, JSFunctionBodyElement {
  bindings: JSPattern,
  value: JSExpression | null,
}

export interface JSSourceFile extends JSSyntax, SourceFile {
  elements: JSSourceElement[],
} 
