
import { TypeRef } from "./types"
import { Diagnostic } from "./diagnostics"
import { Package } from "./common"
import { TextSpan } from "./text"

export function setParents(node: Syntax): void;

export type SyntaxRange = [Syntax, Syntax];

export function isSyntax(value: any): value is Syntax;

interface SyntaxBase {
  id: number;
  kind: SyntaxKind;
  type?: TypeRef;
  errors: Diagnostic[]
  parentNode: Syntax | null;
  span: TextSpan | null;
  visit(visitors: NodeVisitor[]): void;
  preorder(): IterableIterator<Syntax>;
  getParentOfKind<K1 extends SyntaxKind>(kind: K1): ResolveSyntaxKind<K1> | null;
  getChildNodes(): IterableIterator<Syntax>,
  findAllChildrenOfKind<K1 extends SyntaxKind>(kind: K1): IterableIterator<ResolveSyntaxKind<K1>>;
}

export type ResolveSyntaxKind<K extends SyntaxKind> = Extract<Syntax, { kind: K }>;


export class NodeVisitor {
  public visit(node: Syntax): void;
  protected visitEndOfFile?(node: EndOfFile): void;
  protected visitBoltStringLiteral?(node: BoltStringLiteral): void;
  protected visitBoltIntegerLiteral?(node: BoltIntegerLiteral): void;
  protected visitBoltIdentifier?(node: BoltIdentifier): void;
  protected visitBoltOperator?(node: BoltOperator): void;
  protected visitBoltAssignment?(node: BoltAssignment): void;
  protected visitBoltComma?(node: BoltComma): void;
  protected visitBoltSemi?(node: BoltSemi): void;
  protected visitBoltColon?(node: BoltColon): void;
  protected visitBoltColonColon?(node: BoltColonColon): void;
  protected visitBoltDot?(node: BoltDot): void;
  protected visitBoltDotDot?(node: BoltDotDot): void;
  protected visitBoltRArrow?(node: BoltRArrow): void;
  protected visitBoltRArrowAlt?(node: BoltRArrowAlt): void;
  protected visitBoltLArrow?(node: BoltLArrow): void;
  protected visitBoltEqSign?(node: BoltEqSign): void;
  protected visitBoltGtSign?(node: BoltGtSign): void;
  protected visitBoltExMark?(node: BoltExMark): void;
  protected visitBoltLtSign?(node: BoltLtSign): void;
  protected visitBoltVBar?(node: BoltVBar): void;
  protected visitBoltWhereKeyword?(node: BoltWhereKeyword): void;
  protected visitBoltQuoteKeyword?(node: BoltQuoteKeyword): void;
  protected visitBoltFnKeyword?(node: BoltFnKeyword): void;
  protected visitBoltForeignKeyword?(node: BoltForeignKeyword): void;
  protected visitBoltForKeyword?(node: BoltForKeyword): void;
  protected visitBoltLetKeyword?(node: BoltLetKeyword): void;
  protected visitBoltReturnKeyword?(node: BoltReturnKeyword): void;
  protected visitBoltLoopKeyword?(node: BoltLoopKeyword): void;
  protected visitBoltYieldKeyword?(node: BoltYieldKeyword): void;
  protected visitBoltMatchKeyword?(node: BoltMatchKeyword): void;
  protected visitBoltImportKeyword?(node: BoltImportKeyword): void;
  protected visitBoltExportKeyword?(node: BoltExportKeyword): void;
  protected visitBoltPubKeyword?(node: BoltPubKeyword): void;
  protected visitBoltModKeyword?(node: BoltModKeyword): void;
  protected visitBoltMutKeyword?(node: BoltMutKeyword): void;
  protected visitBoltEnumKeyword?(node: BoltEnumKeyword): void;
  protected visitBoltStructKeyword?(node: BoltStructKeyword): void;
  protected visitBoltTypeKeyword?(node: BoltTypeKeyword): void;
  protected visitBoltTraitKeyword?(node: BoltTraitKeyword): void;
  protected visitBoltImplKeyword?(node: BoltImplKeyword): void;
  protected visitBoltParenthesized?(node: BoltParenthesized): void;
  protected visitBoltBraced?(node: BoltBraced): void;
  protected visitBoltBracketed?(node: BoltBracketed): void;
  protected visitBoltSourceFile?(node: BoltSourceFile): void;
  protected visitBoltQualName?(node: BoltQualName): void;
  protected visitBoltTypeOfExpression?(node: BoltTypeOfExpression): void;
  protected visitBoltReferenceTypeExpression?(node: BoltReferenceTypeExpression): void;
  protected visitBoltFunctionTypeExpression?(node: BoltFunctionTypeExpression): void;
  protected visitBoltLiftedTypeExpression?(node: BoltLiftedTypeExpression): void;
  protected visitBoltTypeParameter?(node: BoltTypeParameter): void;
  protected visitBoltBindPattern?(node: BoltBindPattern): void;
  protected visitBoltTypePattern?(node: BoltTypePattern): void;
  protected visitBoltExpressionPattern?(node: BoltExpressionPattern): void;
  protected visitBoltTuplePatternElement?(node: BoltTuplePatternElement): void;
  protected visitBoltTuplePattern?(node: BoltTuplePattern): void;
  protected visitBoltRecordFieldPattern?(node: BoltRecordFieldPattern): void;
  protected visitBoltRecordPattern?(node: BoltRecordPattern): void;
  protected visitBoltQuoteExpression?(node: BoltQuoteExpression): void;
  protected visitBoltTupleExpression?(node: BoltTupleExpression): void;
  protected visitBoltReferenceExpression?(node: BoltReferenceExpression): void;
  protected visitBoltMemberExpression?(node: BoltMemberExpression): void;
  protected visitBoltFunctionExpression?(node: BoltFunctionExpression): void;
  protected visitBoltCallExpression?(node: BoltCallExpression): void;
  protected visitBoltYieldExpression?(node: BoltYieldExpression): void;
  protected visitBoltMatchArm?(node: BoltMatchArm): void;
  protected visitBoltMatchExpression?(node: BoltMatchExpression): void;
  protected visitBoltCase?(node: BoltCase): void;
  protected visitBoltCaseExpression?(node: BoltCaseExpression): void;
  protected visitBoltBlockExpression?(node: BoltBlockExpression): void;
  protected visitBoltConstantExpression?(node: BoltConstantExpression): void;
  protected visitBoltReturnStatement?(node: BoltReturnStatement): void;
  protected visitBoltConditionalCase?(node: BoltConditionalCase): void;
  protected visitBoltConditionalStatement?(node: BoltConditionalStatement): void;
  protected visitBoltResumeStatement?(node: BoltResumeStatement): void;
  protected visitBoltExpressionStatement?(node: BoltExpressionStatement): void;
  protected visitBoltParameter?(node: BoltParameter): void;
  protected visitBoltModule?(node: BoltModule): void;
  protected visitBoltFunctionDeclaration?(node: BoltFunctionDeclaration): void;
  protected visitBoltVariableDeclaration?(node: BoltVariableDeclaration): void;
  protected visitBoltPlainImportSymbol?(node: BoltPlainImportSymbol): void;
  protected visitBoltImportDirective?(node: BoltImportDirective): void;
  protected visitBoltExportSymbol?(node: BoltExportSymbol): void;
  protected visitBoltPlainExportSymbol?(node: BoltPlainExportSymbol): void;
  protected visitBoltExportDirective?(node: BoltExportDirective): void;
  protected visitBoltTraitDeclaration?(node: BoltTraitDeclaration): void;
  protected visitBoltImplDeclaration?(node: BoltImplDeclaration): void;
  protected visitBoltTypeAliasDeclaration?(node: BoltTypeAliasDeclaration): void;
  protected visitBoltRecordField?(node: BoltRecordField): void;
  protected visitBoltRecordDeclaration?(node: BoltRecordDeclaration): void;
  protected visitBoltMacroCall?(node: BoltMacroCall): void;
  protected visitJSIdentifier?(node: JSIdentifier): void;
  protected visitJSString?(node: JSString): void;
  protected visitJSInteger?(node: JSInteger): void;
  protected visitJSFromKeyword?(node: JSFromKeyword): void;
  protected visitJSReturnKeyword?(node: JSReturnKeyword): void;
  protected visitJSTryKeyword?(node: JSTryKeyword): void;
  protected visitJSFinallyKeyword?(node: JSFinallyKeyword): void;
  protected visitJSCatchKeyword?(node: JSCatchKeyword): void;
  protected visitJSImportKeyword?(node: JSImportKeyword): void;
  protected visitJSAsKeyword?(node: JSAsKeyword): void;
  protected visitJSConstKeyword?(node: JSConstKeyword): void;
  protected visitJSLetKeyword?(node: JSLetKeyword): void;
  protected visitJSExportKeyword?(node: JSExportKeyword): void;
  protected visitJSFunctionKeyword?(node: JSFunctionKeyword): void;
  protected visitJSWhileKeyword?(node: JSWhileKeyword): void;
  protected visitJSForKeyword?(node: JSForKeyword): void;
  protected visitJSCloseBrace?(node: JSCloseBrace): void;
  protected visitJSCloseBracket?(node: JSCloseBracket): void;
  protected visitJSCloseParen?(node: JSCloseParen): void;
  protected visitJSOpenBrace?(node: JSOpenBrace): void;
  protected visitJSOpenBracket?(node: JSOpenBracket): void;
  protected visitJSOpenParen?(node: JSOpenParen): void;
  protected visitJSSemi?(node: JSSemi): void;
  protected visitJSComma?(node: JSComma): void;
  protected visitJSDot?(node: JSDot): void;
  protected visitJSDotDotDot?(node: JSDotDotDot): void;
  protected visitJSMulOp?(node: JSMulOp): void;
  protected visitJSAddOp?(node: JSAddOp): void;
  protected visitJSDivOp?(node: JSDivOp): void;
  protected visitJSSubOp?(node: JSSubOp): void;
  protected visitJSLtOp?(node: JSLtOp): void;
  protected visitJSGtOp?(node: JSGtOp): void;
  protected visitJSBOrOp?(node: JSBOrOp): void;
  protected visitJSBXorOp?(node: JSBXorOp): void;
  protected visitJSBAndOp?(node: JSBAndOp): void;
  protected visitJSBNotOp?(node: JSBNotOp): void;
  protected visitJSNotOp?(node: JSNotOp): void;
  protected visitJSBindPattern?(node: JSBindPattern): void;
  protected visitJSConstantExpression?(node: JSConstantExpression): void;
  protected visitJSMemberExpression?(node: JSMemberExpression): void;
  protected visitJSCallExpression?(node: JSCallExpression): void;
  protected visitJSBinaryExpression?(node: JSBinaryExpression): void;
  protected visitJSUnaryExpression?(node: JSUnaryExpression): void;
  protected visitJSNewExpression?(node: JSNewExpression): void;
  protected visitJSSequenceExpression?(node: JSSequenceExpression): void;
  protected visitJSConditionalExpression?(node: JSConditionalExpression): void;
  protected visitJSLiteralExpression?(node: JSLiteralExpression): void;
  protected visitJSReferenceExpression?(node: JSReferenceExpression): void;
  protected visitJSCatchBlock?(node: JSCatchBlock): void;
  protected visitJSTryCatchStatement?(node: JSTryCatchStatement): void;
  protected visitJSExpressionStatement?(node: JSExpressionStatement): void;
  protected visitJSConditionalCase?(node: JSConditionalCase): void;
  protected visitJSConditionalStatement?(node: JSConditionalStatement): void;
  protected visitJSReturnStatement?(node: JSReturnStatement): void;
  protected visitJSParameter?(node: JSParameter): void;
  protected visitJSImportStarBinding?(node: JSImportStarBinding): void;
  protected visitJSImportAsBinding?(node: JSImportAsBinding): void;
  protected visitJSImportDeclaration?(node: JSImportDeclaration): void;
  protected visitJSFunctionDeclaration?(node: JSFunctionDeclaration): void;
  protected visitJSArrowFunctionDeclaration?(node: JSArrowFunctionDeclaration): void;
  protected visitJSLetDeclaration?(node: JSLetDeclaration): void;
  protected visitJSSourceFile?(node: JSSourceFile): void;
}


export const enum SyntaxKind {
  EndOfFile = 0,
  BoltStringLiteral = 7,
  BoltIntegerLiteral = 8,
  BoltIdentifier = 10,
  BoltOperator = 12,
  BoltAssignment = 13,
  BoltComma = 14,
  BoltSemi = 15,
  BoltColon = 16,
  BoltColonColon = 17,
  BoltDot = 18,
  BoltDotDot = 19,
  BoltRArrow = 20,
  BoltRArrowAlt = 21,
  BoltLArrow = 22,
  BoltEqSign = 23,
  BoltGtSign = 24,
  BoltExMark = 25,
  BoltLtSign = 26,
  BoltVBar = 27,
  BoltWhereKeyword = 29,
  BoltQuoteKeyword = 30,
  BoltFnKeyword = 31,
  BoltForeignKeyword = 32,
  BoltForKeyword = 33,
  BoltLetKeyword = 34,
  BoltReturnKeyword = 35,
  BoltLoopKeyword = 36,
  BoltYieldKeyword = 37,
  BoltMatchKeyword = 38,
  BoltImportKeyword = 39,
  BoltExportKeyword = 40,
  BoltPubKeyword = 41,
  BoltModKeyword = 42,
  BoltMutKeyword = 43,
  BoltEnumKeyword = 44,
  BoltStructKeyword = 45,
  BoltTypeKeyword = 46,
  BoltTraitKeyword = 47,
  BoltImplKeyword = 48,
  BoltParenthesized = 50,
  BoltBraced = 51,
  BoltBracketed = 52,
  BoltSourceFile = 53,
  BoltQualName = 54,
  BoltTypeOfExpression = 56,
  BoltReferenceTypeExpression = 57,
  BoltFunctionTypeExpression = 58,
  BoltLiftedTypeExpression = 59,
  BoltTypeParameter = 60,
  BoltBindPattern = 62,
  BoltTypePattern = 63,
  BoltExpressionPattern = 64,
  BoltTuplePatternElement = 65,
  BoltTuplePattern = 66,
  BoltRecordFieldPattern = 67,
  BoltRecordPattern = 68,
  BoltQuoteExpression = 70,
  BoltTupleExpression = 71,
  BoltReferenceExpression = 72,
  BoltMemberExpression = 73,
  BoltFunctionExpression = 74,
  BoltCallExpression = 75,
  BoltYieldExpression = 76,
  BoltMatchArm = 77,
  BoltMatchExpression = 78,
  BoltCase = 79,
  BoltCaseExpression = 80,
  BoltBlockExpression = 81,
  BoltConstantExpression = 82,
  BoltReturnStatement = 84,
  BoltConditionalCase = 85,
  BoltConditionalStatement = 86,
  BoltResumeStatement = 87,
  BoltExpressionStatement = 88,
  BoltParameter = 89,
  BoltModule = 93,
  BoltFunctionDeclaration = 96,
  BoltVariableDeclaration = 97,
  BoltPlainImportSymbol = 99,
  BoltImportDirective = 100,
  BoltExportSymbol = 101,
  BoltPlainExportSymbol = 102,
  BoltExportDirective = 103,
  BoltTraitDeclaration = 104,
  BoltImplDeclaration = 105,
  BoltTypeAliasDeclaration = 106,
  BoltRecordField = 108,
  BoltRecordDeclaration = 109,
  BoltMacroCall = 111,
  JSIdentifier = 115,
  JSString = 116,
  JSInteger = 117,
  JSFromKeyword = 118,
  JSReturnKeyword = 119,
  JSTryKeyword = 120,
  JSFinallyKeyword = 121,
  JSCatchKeyword = 122,
  JSImportKeyword = 123,
  JSAsKeyword = 124,
  JSConstKeyword = 125,
  JSLetKeyword = 126,
  JSExportKeyword = 127,
  JSFunctionKeyword = 128,
  JSWhileKeyword = 129,
  JSForKeyword = 130,
  JSCloseBrace = 132,
  JSCloseBracket = 133,
  JSCloseParen = 134,
  JSOpenBrace = 135,
  JSOpenBracket = 136,
  JSOpenParen = 137,
  JSSemi = 138,
  JSComma = 139,
  JSDot = 140,
  JSDotDotDot = 141,
  JSMulOp = 142,
  JSAddOp = 143,
  JSDivOp = 144,
  JSSubOp = 145,
  JSLtOp = 146,
  JSGtOp = 147,
  JSBOrOp = 148,
  JSBXorOp = 149,
  JSBAndOp = 150,
  JSBNotOp = 151,
  JSNotOp = 152,
  JSBindPattern = 154,
  JSConstantExpression = 156,
  JSMemberExpression = 157,
  JSCallExpression = 158,
  JSBinaryExpression = 159,
  JSUnaryExpression = 160,
  JSNewExpression = 161,
  JSSequenceExpression = 162,
  JSConditionalExpression = 163,
  JSLiteralExpression = 164,
  JSReferenceExpression = 165,
  JSCatchBlock = 169,
  JSTryCatchStatement = 170,
  JSExpressionStatement = 171,
  JSConditionalCase = 172,
  JSConditionalStatement = 173,
  JSReturnStatement = 174,
  JSParameter = 175,
  JSImportStarBinding = 179,
  JSImportAsBinding = 180,
  JSImportDeclaration = 181,
  JSFunctionDeclaration = 182,
  JSArrowFunctionDeclaration = 183,
  JSLetDeclaration = 184,
  JSSourceFile = 185,
}

export interface EndOfFile extends SyntaxBase {
  kind: SyntaxKind.EndOfFile;
  parentNode: EndOfFileParent;
  getChildNodes(): IterableIterator<EndOfFileChild>
}

export type EndOfFileParent
= BoltQuoteExpression
| never

export type EndOfFileAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type EndOfFileChild
= never

export type Token
  = EndOfFile
  | JSIdentifier
  | JSString
  | JSInteger
  | JSFromKeyword
  | JSReturnKeyword
  | JSTryKeyword
  | JSFinallyKeyword
  | JSCatchKeyword
  | JSImportKeyword
  | JSAsKeyword
  | JSConstKeyword
  | JSLetKeyword
  | JSExportKeyword
  | JSFunctionKeyword
  | JSWhileKeyword
  | JSForKeyword
  | JSCloseBrace
  | JSCloseBracket
  | JSCloseParen
  | JSOpenBrace
  | JSOpenBracket
  | JSOpenParen
  | JSSemi
  | JSComma
  | JSDot
  | JSDotDotDot
  | JSMulOp
  | JSAddOp
  | JSDivOp
  | JSSubOp
  | JSLtOp
  | JSGtOp
  | JSBOrOp
  | JSBXorOp
  | JSBAndOp
  | JSBNotOp
  | JSNotOp
  | BoltStringLiteral
  | BoltIntegerLiteral
  | BoltAssignment
  | BoltComma
  | BoltSemi
  | BoltColon
  | BoltColonColon
  | BoltDot
  | BoltDotDot
  | BoltRArrow
  | BoltRArrowAlt
  | BoltLArrow
  | BoltEqSign
  | BoltGtSign
  | BoltExMark
  | BoltLtSign
  | BoltVBar
  | BoltWhereKeyword
  | BoltQuoteKeyword
  | BoltFnKeyword
  | BoltForeignKeyword
  | BoltForKeyword
  | BoltLetKeyword
  | BoltReturnKeyword
  | BoltLoopKeyword
  | BoltYieldKeyword
  | BoltMatchKeyword
  | BoltImportKeyword
  | BoltExportKeyword
  | BoltPubKeyword
  | BoltModKeyword
  | BoltMutKeyword
  | BoltEnumKeyword
  | BoltStructKeyword
  | BoltTypeKeyword
  | BoltTraitKeyword
  | BoltImplKeyword
  | BoltParenthesized
  | BoltBraced
  | BoltBracketed
  | BoltIdentifier
  | BoltOperator


export type SourceFile
  = BoltSourceFile
  | JSSourceFile


export type FunctionBodyElement
  = JSFunctionDeclaration
  | JSArrowFunctionDeclaration
  | JSLetDeclaration
  | JSExpressionStatement
  | JSConditionalStatement
  | JSReturnStatement
  | BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltReturnStatement
  | BoltConditionalStatement
  | BoltResumeStatement
  | BoltExpressionStatement
  | BoltMacroCall


export type ReturnStatement
  = BoltReturnStatement
  | JSReturnStatement


export type BoltSyntax
  = BoltSourceFile
  | BoltQualName
  | BoltTypeParameter
  | BoltTuplePatternElement
  | BoltRecordFieldPattern
  | BoltMatchArm
  | BoltCase
  | BoltConditionalCase
  | BoltParameter
  | BoltModule
  | BoltExportSymbol
  | BoltRecordField
  | BoltMacroCall
  | BoltPlainImportSymbol
  | BoltTraitDeclaration
  | BoltImplDeclaration
  | BoltTypeAliasDeclaration
  | BoltRecordDeclaration
  | BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltReturnStatement
  | BoltConditionalStatement
  | BoltResumeStatement
  | BoltExpressionStatement
  | BoltQuoteExpression
  | BoltTupleExpression
  | BoltReferenceExpression
  | BoltMemberExpression
  | BoltFunctionExpression
  | BoltCallExpression
  | BoltYieldExpression
  | BoltMatchExpression
  | BoltCaseExpression
  | BoltBlockExpression
  | BoltConstantExpression
  | BoltBindPattern
  | BoltTypePattern
  | BoltExpressionPattern
  | BoltTuplePattern
  | BoltRecordPattern
  | BoltTypeOfExpression
  | BoltReferenceTypeExpression
  | BoltFunctionTypeExpression
  | BoltLiftedTypeExpression
  | EndOfFile
  | BoltStringLiteral
  | BoltIntegerLiteral
  | BoltAssignment
  | BoltComma
  | BoltSemi
  | BoltColon
  | BoltColonColon
  | BoltDot
  | BoltDotDot
  | BoltRArrow
  | BoltRArrowAlt
  | BoltLArrow
  | BoltEqSign
  | BoltGtSign
  | BoltExMark
  | BoltLtSign
  | BoltVBar
  | BoltWhereKeyword
  | BoltQuoteKeyword
  | BoltFnKeyword
  | BoltForeignKeyword
  | BoltForKeyword
  | BoltLetKeyword
  | BoltReturnKeyword
  | BoltLoopKeyword
  | BoltYieldKeyword
  | BoltMatchKeyword
  | BoltImportKeyword
  | BoltExportKeyword
  | BoltPubKeyword
  | BoltModKeyword
  | BoltMutKeyword
  | BoltEnumKeyword
  | BoltStructKeyword
  | BoltTypeKeyword
  | BoltTraitKeyword
  | BoltImplKeyword
  | BoltParenthesized
  | BoltBraced
  | BoltBracketed
  | BoltIdentifier
  | BoltOperator


export type BoltToken
  = EndOfFile
  | BoltStringLiteral
  | BoltIntegerLiteral
  | BoltAssignment
  | BoltComma
  | BoltSemi
  | BoltColon
  | BoltColonColon
  | BoltDot
  | BoltDotDot
  | BoltRArrow
  | BoltRArrowAlt
  | BoltLArrow
  | BoltEqSign
  | BoltGtSign
  | BoltExMark
  | BoltLtSign
  | BoltVBar
  | BoltWhereKeyword
  | BoltQuoteKeyword
  | BoltFnKeyword
  | BoltForeignKeyword
  | BoltForKeyword
  | BoltLetKeyword
  | BoltReturnKeyword
  | BoltLoopKeyword
  | BoltYieldKeyword
  | BoltMatchKeyword
  | BoltImportKeyword
  | BoltExportKeyword
  | BoltPubKeyword
  | BoltModKeyword
  | BoltMutKeyword
  | BoltEnumKeyword
  | BoltStructKeyword
  | BoltTypeKeyword
  | BoltTraitKeyword
  | BoltImplKeyword
  | BoltParenthesized
  | BoltBraced
  | BoltBracketed
  | BoltIdentifier
  | BoltOperator


export interface BoltStringLiteral extends SyntaxBase {
  kind: SyntaxKind.BoltStringLiteral;
  value: string;
  parentNode: BoltStringLiteralParent;
  getChildNodes(): IterableIterator<BoltStringLiteralChild>
}

export type BoltStringLiteralParent
= BoltQuoteExpression
| never

export type BoltStringLiteralAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltStringLiteralChild
= never

export interface BoltIntegerLiteral extends SyntaxBase {
  kind: SyntaxKind.BoltIntegerLiteral;
  value: bigint;
  parentNode: BoltIntegerLiteralParent;
  getChildNodes(): IterableIterator<BoltIntegerLiteralChild>
}

export type BoltIntegerLiteralParent
= BoltQuoteExpression
| never

export type BoltIntegerLiteralAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltIntegerLiteralChild
= never

export type BoltSymbol
  = BoltIdentifier
  | BoltOperator
  | BoltGtSign
  | BoltExMark
  | BoltLtSign
  | BoltVBar


export interface BoltIdentifier extends SyntaxBase {
  kind: SyntaxKind.BoltIdentifier;
  text: string;
  parentNode: BoltIdentifierParent;
  getChildNodes(): IterableIterator<BoltIdentifierChild>
}

export type BoltIdentifierParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| never

export type BoltIdentifierAnyParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| BoltImportDirective
| BoltSourceFile
| BoltModule
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltIdentifierChild
= never

export type BoltOperatorLike
  = BoltGtSign
  | BoltExMark
  | BoltLtSign
  | BoltVBar


export interface BoltOperator extends SyntaxBase {
  kind: SyntaxKind.BoltOperator;
  text: string;
  parentNode: BoltOperatorParent;
  getChildNodes(): IterableIterator<BoltOperatorChild>
}

export type BoltOperatorParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| never

export type BoltOperatorAnyParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| BoltImportDirective
| BoltSourceFile
| BoltModule
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltOperatorChild
= never

export interface BoltAssignment extends SyntaxBase {
  kind: SyntaxKind.BoltAssignment;
  operator: string | null;
  parentNode: BoltAssignmentParent;
  getChildNodes(): IterableIterator<BoltAssignmentChild>
}

export type BoltAssignmentParent
= BoltQuoteExpression
| never

export type BoltAssignmentAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltAssignmentChild
= never

export interface BoltComma extends SyntaxBase {
  kind: SyntaxKind.BoltComma;
  parentNode: BoltCommaParent;
  getChildNodes(): IterableIterator<BoltCommaChild>
}

export type BoltCommaParent
= BoltQuoteExpression
| never

export type BoltCommaAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltCommaChild
= never

export interface BoltSemi extends SyntaxBase {
  kind: SyntaxKind.BoltSemi;
  parentNode: BoltSemiParent;
  getChildNodes(): IterableIterator<BoltSemiChild>
}

export type BoltSemiParent
= BoltQuoteExpression
| never

export type BoltSemiAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltSemiChild
= never

export interface BoltColon extends SyntaxBase {
  kind: SyntaxKind.BoltColon;
  parentNode: BoltColonParent;
  getChildNodes(): IterableIterator<BoltColonChild>
}

export type BoltColonParent
= BoltQuoteExpression
| never

export type BoltColonAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltColonChild
= never

export interface BoltColonColon extends SyntaxBase {
  kind: SyntaxKind.BoltColonColon;
  parentNode: BoltColonColonParent;
  getChildNodes(): IterableIterator<BoltColonColonChild>
}

export type BoltColonColonParent
= BoltQuoteExpression
| never

export type BoltColonColonAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltColonColonChild
= never

export interface BoltDot extends SyntaxBase {
  kind: SyntaxKind.BoltDot;
  parentNode: BoltDotParent;
  getChildNodes(): IterableIterator<BoltDotChild>
}

export type BoltDotParent
= BoltQuoteExpression
| never

export type BoltDotAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltDotChild
= never

export interface BoltDotDot extends SyntaxBase {
  kind: SyntaxKind.BoltDotDot;
  parentNode: BoltDotDotParent;
  getChildNodes(): IterableIterator<BoltDotDotChild>
}

export type BoltDotDotParent
= BoltQuoteExpression
| never

export type BoltDotDotAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltDotDotChild
= never

export interface BoltRArrow extends SyntaxBase {
  kind: SyntaxKind.BoltRArrow;
  parentNode: BoltRArrowParent;
  getChildNodes(): IterableIterator<BoltRArrowChild>
}

export type BoltRArrowParent
= BoltQuoteExpression
| never

export type BoltRArrowAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltRArrowChild
= never

export interface BoltRArrowAlt extends SyntaxBase {
  kind: SyntaxKind.BoltRArrowAlt;
  parentNode: BoltRArrowAltParent;
  getChildNodes(): IterableIterator<BoltRArrowAltChild>
}

export type BoltRArrowAltParent
= BoltQuoteExpression
| never

export type BoltRArrowAltAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltRArrowAltChild
= never

export interface BoltLArrow extends SyntaxBase {
  kind: SyntaxKind.BoltLArrow;
  parentNode: BoltLArrowParent;
  getChildNodes(): IterableIterator<BoltLArrowChild>
}

export type BoltLArrowParent
= BoltQuoteExpression
| never

export type BoltLArrowAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltLArrowChild
= never

export interface BoltEqSign extends SyntaxBase {
  kind: SyntaxKind.BoltEqSign;
  parentNode: BoltEqSignParent;
  getChildNodes(): IterableIterator<BoltEqSignChild>
}

export type BoltEqSignParent
= BoltQuoteExpression
| never

export type BoltEqSignAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltEqSignChild
= never

export interface BoltGtSign extends SyntaxBase {
  kind: SyntaxKind.BoltGtSign;
  parentNode: BoltGtSignParent;
  getChildNodes(): IterableIterator<BoltGtSignChild>
}

export type BoltGtSignParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| never

export type BoltGtSignAnyParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| BoltImportDirective
| BoltSourceFile
| BoltModule
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltGtSignChild
= never

export interface BoltExMark extends SyntaxBase {
  kind: SyntaxKind.BoltExMark;
  parentNode: BoltExMarkParent;
  getChildNodes(): IterableIterator<BoltExMarkChild>
}

export type BoltExMarkParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| never

export type BoltExMarkAnyParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| BoltImportDirective
| BoltSourceFile
| BoltModule
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltExMarkChild
= never

export interface BoltLtSign extends SyntaxBase {
  kind: SyntaxKind.BoltLtSign;
  parentNode: BoltLtSignParent;
  getChildNodes(): IterableIterator<BoltLtSignChild>
}

export type BoltLtSignParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| never

export type BoltLtSignAnyParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| BoltImportDirective
| BoltSourceFile
| BoltModule
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltLtSignChild
= never

export interface BoltVBar extends SyntaxBase {
  kind: SyntaxKind.BoltVBar;
  parentNode: BoltVBarParent;
  getChildNodes(): IterableIterator<BoltVBarChild>
}

export type BoltVBarParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| never

export type BoltVBarAnyParent
= BoltQualName
| BoltQuoteExpression
| BoltFunctionDeclaration
| BoltPlainImportSymbol
| BoltPlainExportSymbol
| BoltImportDirective
| BoltSourceFile
| BoltModule
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltVBarChild
= never

export type BoltKeyword
  = BoltWhereKeyword
  | BoltQuoteKeyword
  | BoltFnKeyword
  | BoltForeignKeyword
  | BoltForKeyword
  | BoltLetKeyword
  | BoltReturnKeyword
  | BoltLoopKeyword
  | BoltYieldKeyword
  | BoltMatchKeyword
  | BoltImportKeyword
  | BoltExportKeyword
  | BoltPubKeyword
  | BoltModKeyword
  | BoltMutKeyword
  | BoltEnumKeyword
  | BoltStructKeyword
  | BoltTypeKeyword
  | BoltTraitKeyword
  | BoltImplKeyword


export interface BoltWhereKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltWhereKeyword;
  parentNode: BoltWhereKeywordParent;
  getChildNodes(): IterableIterator<BoltWhereKeywordChild>
}

export type BoltWhereKeywordParent
= BoltQuoteExpression
| never

export type BoltWhereKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltWhereKeywordChild
= never

export interface BoltQuoteKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltQuoteKeyword;
  parentNode: BoltQuoteKeywordParent;
  getChildNodes(): IterableIterator<BoltQuoteKeywordChild>
}

export type BoltQuoteKeywordParent
= BoltQuoteExpression
| never

export type BoltQuoteKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltQuoteKeywordChild
= never

export interface BoltFnKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltFnKeyword;
  parentNode: BoltFnKeywordParent;
  getChildNodes(): IterableIterator<BoltFnKeywordChild>
}

export type BoltFnKeywordParent
= BoltQuoteExpression
| never

export type BoltFnKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltFnKeywordChild
= never

export interface BoltForeignKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltForeignKeyword;
  parentNode: BoltForeignKeywordParent;
  getChildNodes(): IterableIterator<BoltForeignKeywordChild>
}

export type BoltForeignKeywordParent
= BoltQuoteExpression
| never

export type BoltForeignKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltForeignKeywordChild
= never

export interface BoltForKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltForKeyword;
  parentNode: BoltForKeywordParent;
  getChildNodes(): IterableIterator<BoltForKeywordChild>
}

export type BoltForKeywordParent
= BoltQuoteExpression
| never

export type BoltForKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltForKeywordChild
= never

export interface BoltLetKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltLetKeyword;
  parentNode: BoltLetKeywordParent;
  getChildNodes(): IterableIterator<BoltLetKeywordChild>
}

export type BoltLetKeywordParent
= BoltQuoteExpression
| never

export type BoltLetKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltLetKeywordChild
= never

export interface BoltReturnKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltReturnKeyword;
  parentNode: BoltReturnKeywordParent;
  getChildNodes(): IterableIterator<BoltReturnKeywordChild>
}

export type BoltReturnKeywordParent
= BoltQuoteExpression
| never

export type BoltReturnKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltReturnKeywordChild
= never

export interface BoltLoopKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltLoopKeyword;
  parentNode: BoltLoopKeywordParent;
  getChildNodes(): IterableIterator<BoltLoopKeywordChild>
}

export type BoltLoopKeywordParent
= BoltQuoteExpression
| never

export type BoltLoopKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltLoopKeywordChild
= never

export interface BoltYieldKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltYieldKeyword;
  parentNode: BoltYieldKeywordParent;
  getChildNodes(): IterableIterator<BoltYieldKeywordChild>
}

export type BoltYieldKeywordParent
= BoltQuoteExpression
| never

export type BoltYieldKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltYieldKeywordChild
= never

export interface BoltMatchKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltMatchKeyword;
  parentNode: BoltMatchKeywordParent;
  getChildNodes(): IterableIterator<BoltMatchKeywordChild>
}

export type BoltMatchKeywordParent
= BoltQuoteExpression
| never

export type BoltMatchKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltMatchKeywordChild
= never

export interface BoltImportKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltImportKeyword;
  parentNode: BoltImportKeywordParent;
  getChildNodes(): IterableIterator<BoltImportKeywordChild>
}

export type BoltImportKeywordParent
= BoltQuoteExpression
| never

export type BoltImportKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltImportKeywordChild
= never

export interface BoltExportKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltExportKeyword;
  parentNode: BoltExportKeywordParent;
  getChildNodes(): IterableIterator<BoltExportKeywordChild>
}

export type BoltExportKeywordParent
= BoltQuoteExpression
| never

export type BoltExportKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltExportKeywordChild
= never

export interface BoltPubKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltPubKeyword;
  parentNode: BoltPubKeywordParent;
  getChildNodes(): IterableIterator<BoltPubKeywordChild>
}

export type BoltPubKeywordParent
= BoltQuoteExpression
| never

export type BoltPubKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltPubKeywordChild
= never

export interface BoltModKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltModKeyword;
  parentNode: BoltModKeywordParent;
  getChildNodes(): IterableIterator<BoltModKeywordChild>
}

export type BoltModKeywordParent
= BoltQuoteExpression
| never

export type BoltModKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltModKeywordChild
= never

export interface BoltMutKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltMutKeyword;
  parentNode: BoltMutKeywordParent;
  getChildNodes(): IterableIterator<BoltMutKeywordChild>
}

export type BoltMutKeywordParent
= BoltQuoteExpression
| never

export type BoltMutKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltMutKeywordChild
= never

export interface BoltEnumKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltEnumKeyword;
  parentNode: BoltEnumKeywordParent;
  getChildNodes(): IterableIterator<BoltEnumKeywordChild>
}

export type BoltEnumKeywordParent
= BoltQuoteExpression
| never

export type BoltEnumKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltEnumKeywordChild
= never

export interface BoltStructKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltStructKeyword;
  parentNode: BoltStructKeywordParent;
  getChildNodes(): IterableIterator<BoltStructKeywordChild>
}

export type BoltStructKeywordParent
= BoltQuoteExpression
| never

export type BoltStructKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltStructKeywordChild
= never

export interface BoltTypeKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltTypeKeyword;
  parentNode: BoltTypeKeywordParent;
  getChildNodes(): IterableIterator<BoltTypeKeywordChild>
}

export type BoltTypeKeywordParent
= BoltQuoteExpression
| never

export type BoltTypeKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltTypeKeywordChild
= never

export interface BoltTraitKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltTraitKeyword;
  parentNode: BoltTraitKeywordParent;
  getChildNodes(): IterableIterator<BoltTraitKeywordChild>
}

export type BoltTraitKeywordParent
= BoltQuoteExpression
| never

export type BoltTraitKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltTraitKeywordChild
= never

export interface BoltImplKeyword extends SyntaxBase {
  kind: SyntaxKind.BoltImplKeyword;
  parentNode: BoltImplKeywordParent;
  getChildNodes(): IterableIterator<BoltImplKeywordChild>
}

export type BoltImplKeywordParent
= BoltQuoteExpression
| never

export type BoltImplKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltImplKeywordChild
= never

export type BoltPunctuated
  = BoltParenthesized
  | BoltBraced
  | BoltBracketed


export interface BoltParenthesized extends SyntaxBase {
  kind: SyntaxKind.BoltParenthesized;
  text: string;
  parentNode: BoltParenthesizedParent;
  getChildNodes(): IterableIterator<BoltParenthesizedChild>
}

export type BoltParenthesizedParent
= BoltQuoteExpression
| never

export type BoltParenthesizedAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltParenthesizedChild
= never

export interface BoltBraced extends SyntaxBase {
  kind: SyntaxKind.BoltBraced;
  text: string;
  parentNode: BoltBracedParent;
  getChildNodes(): IterableIterator<BoltBracedChild>
}

export type BoltBracedParent
= BoltQuoteExpression
| never

export type BoltBracedAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltBracedChild
= never

export interface BoltBracketed extends SyntaxBase {
  kind: SyntaxKind.BoltBracketed;
  text: string;
  parentNode: BoltBracketedParent;
  getChildNodes(): IterableIterator<BoltBracketedChild>
}

export type BoltBracketedParent
= BoltQuoteExpression
| never

export type BoltBracketedAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltBracketedChild
= never

export interface BoltSourceFile extends SyntaxBase {
  kind: SyntaxKind.BoltSourceFile;
  elements: BoltSourceElement[];
  package: Package;
  parentNode: BoltSourceFileParent;
  getChildNodes(): IterableIterator<BoltSourceFileChild>
}

export type BoltSourceFileParent
= never

export type BoltSourceFileAnyParent
= never

export type BoltSourceFileChild
= never

export interface BoltQualName extends SyntaxBase {
  kind: SyntaxKind.BoltQualName;
  isAbsolute: boolean;
  modulePath: BoltIdentifier[];
  name: BoltSymbol;
  parentNode: BoltQualNameParent;
  getChildNodes(): IterableIterator<BoltQualNameChild>
}

export type BoltQualNameParent
= never

export type BoltQualNameAnyParent
= never

export type BoltQualNameChild
= never

export type BoltTypeExpression
  = BoltTypeOfExpression
  | BoltReferenceTypeExpression
  | BoltFunctionTypeExpression
  | BoltLiftedTypeExpression


export interface BoltTypeOfExpression extends SyntaxBase {
  kind: SyntaxKind.BoltTypeOfExpression;
  expression: BoltExpression;
  parentNode: BoltTypeOfExpressionParent;
  getChildNodes(): IterableIterator<BoltTypeOfExpressionChild>
}

export type BoltTypeOfExpressionParent
= BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| never

export type BoltTypeOfExpressionAnyParent
= BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltBlockExpression
| BoltConditionalCase
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltTuplePatternElement
| BoltRecordFieldPattern
| never

export type BoltTypeOfExpressionChild
= never

export interface BoltReferenceTypeExpression extends SyntaxBase {
  kind: SyntaxKind.BoltReferenceTypeExpression;
  name: BoltQualName;
  arguments: BoltTypeExpression[] | null;
  parentNode: BoltReferenceTypeExpressionParent;
  getChildNodes(): IterableIterator<BoltReferenceTypeExpressionChild>
}

export type BoltReferenceTypeExpressionParent
= BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| never

export type BoltReferenceTypeExpressionAnyParent
= BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltBlockExpression
| BoltConditionalCase
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltTuplePatternElement
| BoltRecordFieldPattern
| never

export type BoltReferenceTypeExpressionChild
= never

export interface BoltFunctionTypeExpression extends SyntaxBase {
  kind: SyntaxKind.BoltFunctionTypeExpression;
  params: BoltParameter[];
  returnType: BoltTypeExpression | null;
  parentNode: BoltFunctionTypeExpressionParent;
  getChildNodes(): IterableIterator<BoltFunctionTypeExpressionChild>
}

export type BoltFunctionTypeExpressionParent
= BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| never

export type BoltFunctionTypeExpressionAnyParent
= BoltReferenceTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltBlockExpression
| BoltConditionalCase
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltTuplePatternElement
| BoltRecordFieldPattern
| never

export type BoltFunctionTypeExpressionChild
= never

export interface BoltLiftedTypeExpression extends SyntaxBase {
  kind: SyntaxKind.BoltLiftedTypeExpression;
  expression: BoltExpression;
  parentNode: BoltLiftedTypeExpressionParent;
  getChildNodes(): IterableIterator<BoltLiftedTypeExpressionChild>
}

export type BoltLiftedTypeExpressionParent
= BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| never

export type BoltLiftedTypeExpressionAnyParent
= BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypePattern
| BoltRecordPattern
| BoltFunctionExpression
| BoltParameter
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltImplDeclaration
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltBlockExpression
| BoltConditionalCase
| BoltTypeOfExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltTuplePatternElement
| BoltRecordFieldPattern
| never

export type BoltLiftedTypeExpressionChild
= never

export interface BoltTypeParameter extends SyntaxBase {
  kind: SyntaxKind.BoltTypeParameter;
  index: number;
  name: BoltIdentifier;
  typeExpr: BoltTypeExpression | null;
  defaultType: BoltTypeExpression | null;
  parentNode: BoltTypeParameterParent;
  getChildNodes(): IterableIterator<BoltTypeParameterChild>
}

export type BoltTypeParameterParent
= never

export type BoltTypeParameterAnyParent
= never

export type BoltTypeParameterChild
= never

export type BoltPattern
  = BoltBindPattern
  | BoltTypePattern
  | BoltExpressionPattern
  | BoltTuplePattern
  | BoltRecordPattern


export interface BoltBindPattern extends SyntaxBase {
  kind: SyntaxKind.BoltBindPattern;
  name: BoltIdentifier;
  parentNode: BoltBindPatternParent;
  getChildNodes(): IterableIterator<BoltBindPatternChild>
}

export type BoltBindPatternParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltBindPatternAnyParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltBindPatternChild
= never

export interface BoltTypePattern extends SyntaxBase {
  kind: SyntaxKind.BoltTypePattern;
  typeExpr: BoltTypeExpression;
  nestedPattern: BoltPattern;
  parentNode: BoltTypePatternParent;
  getChildNodes(): IterableIterator<BoltTypePatternChild>
}

export type BoltTypePatternParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltTypePatternAnyParent
= BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltTypePatternChild
= never

export interface BoltExpressionPattern extends SyntaxBase {
  kind: SyntaxKind.BoltExpressionPattern;
  expression: BoltExpression;
  parentNode: BoltExpressionPatternParent;
  getChildNodes(): IterableIterator<BoltExpressionPatternChild>
}

export type BoltExpressionPatternParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltExpressionPatternAnyParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltExpressionPatternChild
= never

export interface BoltTuplePatternElement extends SyntaxBase {
  kind: SyntaxKind.BoltTuplePatternElement;
  index: number;
  pattern: BoltPattern;
  parentNode: BoltTuplePatternElementParent;
  getChildNodes(): IterableIterator<BoltTuplePatternElementChild>
}

export type BoltTuplePatternElementParent
= never

export type BoltTuplePatternElementAnyParent
= never

export type BoltTuplePatternElementChild
= never

export interface BoltTuplePattern extends SyntaxBase {
  kind: SyntaxKind.BoltTuplePattern;
  elements: BoltTuplePatternElement[];
  parentNode: BoltTuplePatternParent;
  getChildNodes(): IterableIterator<BoltTuplePatternChild>
}

export type BoltTuplePatternParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltTuplePatternAnyParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltTuplePatternChild
= never

export interface BoltRecordFieldPattern extends SyntaxBase {
  kind: SyntaxKind.BoltRecordFieldPattern;
  isRest: boolean;
  name: BoltIdentifier | null;
  pattern: BoltPattern | null;
  parentNode: BoltRecordFieldPatternParent;
  getChildNodes(): IterableIterator<BoltRecordFieldPatternChild>
}

export type BoltRecordFieldPatternParent
= never

export type BoltRecordFieldPatternAnyParent
= never

export type BoltRecordFieldPatternChild
= never

export interface BoltRecordPattern extends SyntaxBase {
  kind: SyntaxKind.BoltRecordPattern;
  name: BoltTypeExpression;
  fields: BoltRecordFieldPattern[];
  parentNode: BoltRecordPatternParent;
  getChildNodes(): IterableIterator<BoltRecordPatternChild>
}

export type BoltRecordPatternParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltRecordPatternAnyParent
= BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltMatchArm
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltRecordPatternChild
= never

export type BoltExpression
  = BoltQuoteExpression
  | BoltTupleExpression
  | BoltReferenceExpression
  | BoltMemberExpression
  | BoltFunctionExpression
  | BoltCallExpression
  | BoltYieldExpression
  | BoltMatchExpression
  | BoltCaseExpression
  | BoltBlockExpression
  | BoltConstantExpression
  | BoltMacroCall


export interface BoltQuoteExpression extends SyntaxBase {
  kind: SyntaxKind.BoltQuoteExpression;
  tokens: (Token | BoltExpression)[];
  parentNode: BoltQuoteExpressionParent;
  getChildNodes(): IterableIterator<BoltQuoteExpressionChild>
}

export type BoltQuoteExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltQuoteExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltQuoteExpressionChild
= never

export interface BoltTupleExpression extends SyntaxBase {
  kind: SyntaxKind.BoltTupleExpression;
  elements: BoltExpression[];
  parentNode: BoltTupleExpressionParent;
  getChildNodes(): IterableIterator<BoltTupleExpressionChild>
}

export type BoltTupleExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltTupleExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltTupleExpressionChild
= never

export interface BoltReferenceExpression extends SyntaxBase {
  kind: SyntaxKind.BoltReferenceExpression;
  name: BoltQualName;
  parentNode: BoltReferenceExpressionParent;
  getChildNodes(): IterableIterator<BoltReferenceExpressionChild>
}

export type BoltReferenceExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltReferenceExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltReferenceExpressionChild
= never

export interface BoltMemberExpression extends SyntaxBase {
  kind: SyntaxKind.BoltMemberExpression;
  expression: BoltExpression;
  path: BoltIdentifier[];
  parentNode: BoltMemberExpressionParent;
  getChildNodes(): IterableIterator<BoltMemberExpressionChild>
}

export type BoltMemberExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltMemberExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltMemberExpressionChild
= never

export interface BoltFunctionExpression extends SyntaxBase {
  kind: SyntaxKind.BoltFunctionExpression;
  params: BoltParameter[];
  returnType: BoltTypeExpression | null;
  body: BoltFunctionBodyElement[];
  parentNode: BoltFunctionExpressionParent;
  getChildNodes(): IterableIterator<BoltFunctionExpressionChild>
}

export type BoltFunctionExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltFunctionExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltFunctionExpressionChild
= never

export interface BoltCallExpression extends SyntaxBase {
  kind: SyntaxKind.BoltCallExpression;
  operator: BoltExpression;
  operands: BoltExpression[];
  parentNode: BoltCallExpressionParent;
  getChildNodes(): IterableIterator<BoltCallExpressionChild>
}

export type BoltCallExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltCallExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltCallExpressionChild
= never

export interface BoltYieldExpression extends SyntaxBase {
  kind: SyntaxKind.BoltYieldExpression;
  value: BoltExpression;
  parentNode: BoltYieldExpressionParent;
  getChildNodes(): IterableIterator<BoltYieldExpressionChild>
}

export type BoltYieldExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltYieldExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltYieldExpressionChild
= never

export interface BoltMatchArm extends SyntaxBase {
  kind: SyntaxKind.BoltMatchArm;
  pattern: BoltPattern;
  body: BoltExpression;
  parentNode: BoltMatchArmParent;
  getChildNodes(): IterableIterator<BoltMatchArmChild>
}

export type BoltMatchArmParent
= never

export type BoltMatchArmAnyParent
= never

export type BoltMatchArmChild
= never

export interface BoltMatchExpression extends SyntaxBase {
  kind: SyntaxKind.BoltMatchExpression;
  value: BoltExpression;
  arms: BoltMatchArm[];
  parentNode: BoltMatchExpressionParent;
  getChildNodes(): IterableIterator<BoltMatchExpressionChild>
}

export type BoltMatchExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltMatchExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltMatchExpressionChild
= never

export interface BoltCase extends SyntaxBase {
  kind: SyntaxKind.BoltCase;
  test: BoltExpression;
  result: BoltExpression;
  parentNode: BoltCaseParent;
  getChildNodes(): IterableIterator<BoltCaseChild>
}

export type BoltCaseParent
= never

export type BoltCaseAnyParent
= never

export type BoltCaseChild
= never

export interface BoltCaseExpression extends SyntaxBase {
  kind: SyntaxKind.BoltCaseExpression;
  cases: BoltCase[];
  parentNode: BoltCaseExpressionParent;
  getChildNodes(): IterableIterator<BoltCaseExpressionChild>
}

export type BoltCaseExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltCaseExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltCaseExpressionChild
= never

export interface BoltBlockExpression extends SyntaxBase {
  kind: SyntaxKind.BoltBlockExpression;
  elements: BoltFunctionBodyElement[];
  parentNode: BoltBlockExpressionParent;
  getChildNodes(): IterableIterator<BoltBlockExpressionChild>
}

export type BoltBlockExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltBlockExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltBlockExpressionChild
= never

export interface BoltConstantExpression extends SyntaxBase {
  kind: SyntaxKind.BoltConstantExpression;
  value: BoltValue;
  parentNode: BoltConstantExpressionParent;
  getChildNodes(): IterableIterator<BoltConstantExpressionChild>
}

export type BoltConstantExpressionParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| never

export type BoltConstantExpressionAnyParent
= BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltConstantExpressionChild
= never

export type BoltStatement
  = BoltReturnStatement
  | BoltConditionalStatement
  | BoltResumeStatement
  | BoltExpressionStatement
  | BoltMacroCall


export interface BoltReturnStatement extends SyntaxBase {
  kind: SyntaxKind.BoltReturnStatement;
  value: BoltExpression | null;
  parentNode: BoltReturnStatementParent;
  getChildNodes(): IterableIterator<BoltReturnStatementChild>
}

export type BoltReturnStatementParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| never

export type BoltReturnStatementAnyParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltReturnStatementChild
= never

export interface BoltConditionalCase extends SyntaxBase {
  kind: SyntaxKind.BoltConditionalCase;
  test: BoltExpression | null;
  body: BoltFunctionBodyElement[];
  parentNode: BoltConditionalCaseParent;
  getChildNodes(): IterableIterator<BoltConditionalCaseChild>
}

export type BoltConditionalCaseParent
= never

export type BoltConditionalCaseAnyParent
= never

export type BoltConditionalCaseChild
= never

export interface BoltConditionalStatement extends SyntaxBase {
  kind: SyntaxKind.BoltConditionalStatement;
  cases: BoltConditionalCase[];
  parentNode: BoltConditionalStatementParent;
  getChildNodes(): IterableIterator<BoltConditionalStatementChild>
}

export type BoltConditionalStatementParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| never

export type BoltConditionalStatementAnyParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltConditionalStatementChild
= never

export interface BoltResumeStatement extends SyntaxBase {
  kind: SyntaxKind.BoltResumeStatement;
  value: BoltExpression;
  parentNode: BoltResumeStatementParent;
  getChildNodes(): IterableIterator<BoltResumeStatementChild>
}

export type BoltResumeStatementParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| never

export type BoltResumeStatementAnyParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltResumeStatementChild
= never

export interface BoltExpressionStatement extends SyntaxBase {
  kind: SyntaxKind.BoltExpressionStatement;
  expression: BoltExpression;
  parentNode: BoltExpressionStatementParent;
  getChildNodes(): IterableIterator<BoltExpressionStatementChild>
}

export type BoltExpressionStatementParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| never

export type BoltExpressionStatementAnyParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltExpressionStatementChild
= never

export interface BoltParameter extends SyntaxBase {
  kind: SyntaxKind.BoltParameter;
  index: number;
  bindings: BoltPattern;
  typeExpr: BoltTypeExpression | null;
  defaultValue: BoltExpression | null;
  parentNode: BoltParameterParent;
  getChildNodes(): IterableIterator<BoltParameterChild>
}

export type BoltParameterParent
= never

export type BoltParameterAnyParent
= never

export type BoltParameterChild
= never

export type BoltDeclaration
  = BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltRecordDeclaration
  | BoltMacroCall


export type BoltTypeDeclaration
  = BoltTraitDeclaration
  | BoltImplDeclaration
  | BoltTypeAliasDeclaration
  | BoltRecordDeclaration


export const enum BoltModifiers {
  IsMutable = 1,IsPublic = 2,}

export interface BoltModule extends SyntaxBase {
  kind: SyntaxKind.BoltModule;
  modifiers: BoltModifiers;
  name: BoltIdentifier[];
  elements: BoltSourceElement[];
  parentNode: BoltModuleParent;
  getChildNodes(): IterableIterator<BoltModuleChild>
}

export type BoltModuleParent
= BoltSourceFile
| BoltModule
| never

export type BoltModuleAnyParent
= BoltSourceFile
| never

export type BoltModuleChild
= never

export type BoltDeclarationLike
  = BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltTraitDeclaration
  | BoltImplDeclaration
  | BoltTypeAliasDeclaration
  | BoltRecordDeclaration


export type BoltFunctionBodyElement
  = BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltReturnStatement
  | BoltConditionalStatement
  | BoltResumeStatement
  | BoltExpressionStatement
  | BoltMacroCall


export interface BoltFunctionDeclaration extends SyntaxBase {
  kind: SyntaxKind.BoltFunctionDeclaration;
  modifiers: BoltModifiers;
  target: string;
  name: BoltSymbol;
  params: BoltParameter[];
  returnType: BoltTypeExpression | null;
  typeParams: BoltTypeParameter[] | null;
  body: BoltFunctionBodyElement[];
  parentNode: BoltFunctionDeclarationParent;
  getChildNodes(): IterableIterator<BoltFunctionDeclarationChild>
}

export type BoltFunctionDeclarationParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| never

export type BoltFunctionDeclarationAnyParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltFunctionDeclarationChild
= never

export interface BoltVariableDeclaration extends SyntaxBase {
  kind: SyntaxKind.BoltVariableDeclaration;
  modifiers: BoltModifiers;
  bindings: BoltPattern;
  typeExpr: BoltTypeExpression | null;
  value: BoltExpression | null;
  parentNode: BoltVariableDeclarationParent;
  getChildNodes(): IterableIterator<BoltVariableDeclarationChild>
}

export type BoltVariableDeclarationParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| never

export type BoltVariableDeclarationAnyParent
= BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltConditionalCase
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type BoltVariableDeclarationChild
= never

export type BoltImportSymbol
  = BoltPlainImportSymbol


export interface BoltPlainImportSymbol extends SyntaxBase {
  kind: SyntaxKind.BoltPlainImportSymbol;
  remote: BoltQualName;
  local: BoltSymbol;
  parentNode: BoltPlainImportSymbolParent;
  getChildNodes(): IterableIterator<BoltPlainImportSymbolChild>
}

export type BoltPlainImportSymbolParent
= BoltImportDirective
| never

export type BoltPlainImportSymbolAnyParent
= BoltImportDirective
| BoltSourceFile
| BoltModule
| never

export type BoltPlainImportSymbolChild
= never

export interface BoltImportDirective extends SyntaxBase {
  kind: SyntaxKind.BoltImportDirective;
  modifiers: BoltModifiers;
  file: BoltStringLiteral;
  symbols: BoltImportSymbol[] | null;
  parentNode: BoltImportDirectiveParent;
  getChildNodes(): IterableIterator<BoltImportDirectiveChild>
}

export type BoltImportDirectiveParent
= BoltSourceFile
| BoltModule
| never

export type BoltImportDirectiveAnyParent
= BoltSourceFile
| BoltModule
| never

export type BoltImportDirectiveChild
= never

export interface BoltExportSymbol extends SyntaxBase {
  kind: SyntaxKind.BoltExportSymbol;
  parentNode: BoltExportSymbolParent;
  getChildNodes(): IterableIterator<BoltExportSymbolChild>
}

export type BoltExportSymbolParent
= never

export type BoltExportSymbolAnyParent
= never

export type BoltExportSymbolChild
= never

export interface BoltPlainExportSymbol extends SyntaxBase {
  kind: SyntaxKind.BoltPlainExportSymbol;
  local: BoltQualName;
  remote: BoltSymbol;
  parentNode: BoltPlainExportSymbolParent;
  getChildNodes(): IterableIterator<BoltPlainExportSymbolChild>
}

export type BoltPlainExportSymbolParent
= never

export type BoltPlainExportSymbolAnyParent
= never

export type BoltPlainExportSymbolChild
= never

export interface BoltExportDirective extends SyntaxBase {
  kind: SyntaxKind.BoltExportDirective;
  file: string;
  symbols: BoltExportSymbol[] | null;
  parentNode: BoltExportDirectiveParent;
  getChildNodes(): IterableIterator<BoltExportDirectiveChild>
}

export type BoltExportDirectiveParent
= BoltSourceFile
| BoltModule
| never

export type BoltExportDirectiveAnyParent
= BoltSourceFile
| BoltModule
| never

export type BoltExportDirectiveChild
= never

export interface BoltTraitDeclaration extends SyntaxBase {
  kind: SyntaxKind.BoltTraitDeclaration;
  modifiers: BoltModifiers;
  name: BoltIdentifier;
  typeParams: BoltTypeParameter[] | null;
  elements: BoltDeclaration[];
  parentNode: BoltTraitDeclarationParent;
  getChildNodes(): IterableIterator<BoltTraitDeclarationChild>
}

export type BoltTraitDeclarationParent
= BoltSourceFile
| BoltModule
| never

export type BoltTraitDeclarationAnyParent
= BoltSourceFile
| BoltModule
| never

export type BoltTraitDeclarationChild
= never

export interface BoltImplDeclaration extends SyntaxBase {
  kind: SyntaxKind.BoltImplDeclaration;
  modifiers: BoltModifiers;
  name: BoltIdentifier;
  trait: BoltTypeExpression;
  typeParams: BoltTypeParameter[] | null;
  elements: BoltDeclaration[];
  parentNode: BoltImplDeclarationParent;
  getChildNodes(): IterableIterator<BoltImplDeclarationChild>
}

export type BoltImplDeclarationParent
= BoltSourceFile
| BoltModule
| never

export type BoltImplDeclarationAnyParent
= BoltSourceFile
| BoltModule
| never

export type BoltImplDeclarationChild
= never

export interface BoltTypeAliasDeclaration extends SyntaxBase {
  kind: SyntaxKind.BoltTypeAliasDeclaration;
  modifiers: BoltModifiers;
  name: BoltIdentifier;
  typeParams: BoltTypeParameter[] | null;
  typeExpr: BoltTypeExpression;
  parentNode: BoltTypeAliasDeclarationParent;
  getChildNodes(): IterableIterator<BoltTypeAliasDeclarationChild>
}

export type BoltTypeAliasDeclarationParent
= BoltSourceFile
| BoltModule
| never

export type BoltTypeAliasDeclarationAnyParent
= BoltSourceFile
| BoltModule
| never

export type BoltTypeAliasDeclarationChild
= never

export type BoltRecordMember
  = BoltRecordField
  | BoltMacroCall


export interface BoltRecordField extends SyntaxBase {
  kind: SyntaxKind.BoltRecordField;
  name: BoltIdentifier;
  typeExpr: BoltTypeExpression;
  parentNode: BoltRecordFieldParent;
  getChildNodes(): IterableIterator<BoltRecordFieldChild>
}

export type BoltRecordFieldParent
= BoltRecordDeclaration
| never

export type BoltRecordFieldAnyParent
= BoltRecordDeclaration
| BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltImplDeclaration
| never

export type BoltRecordFieldChild
= never

export interface BoltRecordDeclaration extends SyntaxBase {
  kind: SyntaxKind.BoltRecordDeclaration;
  modifiers: BoltModifiers;
  name: BoltIdentifier;
  typeParms: BoltTypeParameter[] | null;
  members: BoltRecordMember[] | null;
  parentNode: BoltRecordDeclarationParent;
  getChildNodes(): IterableIterator<BoltRecordDeclarationChild>
}

export type BoltRecordDeclarationParent
= BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltImplDeclaration
| never

export type BoltRecordDeclarationAnyParent
= BoltSourceFile
| BoltModule
| BoltTraitDeclaration
| BoltImplDeclaration
| never

export type BoltRecordDeclarationChild
= never

export type BoltSourceElement
  = BoltModule
  | BoltImportDirective
  | BoltExportDirective
  | BoltTraitDeclaration
  | BoltImplDeclaration
  | BoltTypeAliasDeclaration
  | BoltRecordDeclaration
  | BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltMacroCall
  | BoltReturnStatement
  | BoltConditionalStatement
  | BoltResumeStatement
  | BoltExpressionStatement


export interface BoltMacroCall extends SyntaxBase {
  kind: SyntaxKind.BoltMacroCall;
  name: BoltIdentifier;
  text: string;
  parentNode: BoltMacroCallParent;
  getChildNodes(): IterableIterator<BoltMacroCallChild>
}

export type BoltMacroCallParent
= BoltSourceFile
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltFunctionExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltBlockExpression
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltModule
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltRecordDeclaration
| never

export type BoltMacroCallAnyParent
= BoltSourceFile
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltFunctionExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltBlockExpression
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltModule
| BoltFunctionDeclaration
| BoltVariableDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltRecordDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| never

export type BoltMacroCallChild
= never

export type JSSyntax
  = JSCatchBlock
  | JSTryCatchStatement
  | JSConditionalCase
  | JSParameter
  | JSSourceFile
  | JSImportStarBinding
  | JSImportAsBinding
  | JSImportDeclaration
  | JSFunctionDeclaration
  | JSArrowFunctionDeclaration
  | JSLetDeclaration
  | JSExpressionStatement
  | JSConditionalStatement
  | JSReturnStatement
  | JSConstantExpression
  | JSMemberExpression
  | JSCallExpression
  | JSBinaryExpression
  | JSUnaryExpression
  | JSNewExpression
  | JSSequenceExpression
  | JSConditionalExpression
  | JSLiteralExpression
  | JSReferenceExpression
  | JSBindPattern
  | EndOfFile
  | JSIdentifier
  | JSString
  | JSInteger
  | JSFromKeyword
  | JSReturnKeyword
  | JSTryKeyword
  | JSFinallyKeyword
  | JSCatchKeyword
  | JSImportKeyword
  | JSAsKeyword
  | JSConstKeyword
  | JSLetKeyword
  | JSExportKeyword
  | JSFunctionKeyword
  | JSWhileKeyword
  | JSForKeyword
  | JSCloseBrace
  | JSCloseBracket
  | JSCloseParen
  | JSOpenBrace
  | JSOpenBracket
  | JSOpenParen
  | JSSemi
  | JSComma
  | JSDot
  | JSDotDotDot
  | JSMulOp
  | JSAddOp
  | JSDivOp
  | JSSubOp
  | JSLtOp
  | JSGtOp
  | JSBOrOp
  | JSBXorOp
  | JSBAndOp
  | JSBNotOp
  | JSNotOp


export type JSToken
  = EndOfFile
  | JSIdentifier
  | JSString
  | JSInteger
  | JSFromKeyword
  | JSReturnKeyword
  | JSTryKeyword
  | JSFinallyKeyword
  | JSCatchKeyword
  | JSImportKeyword
  | JSAsKeyword
  | JSConstKeyword
  | JSLetKeyword
  | JSExportKeyword
  | JSFunctionKeyword
  | JSWhileKeyword
  | JSForKeyword
  | JSCloseBrace
  | JSCloseBracket
  | JSCloseParen
  | JSOpenBrace
  | JSOpenBracket
  | JSOpenParen
  | JSSemi
  | JSComma
  | JSDot
  | JSDotDotDot
  | JSMulOp
  | JSAddOp
  | JSDivOp
  | JSSubOp
  | JSLtOp
  | JSGtOp
  | JSBOrOp
  | JSBXorOp
  | JSBAndOp
  | JSBNotOp
  | JSNotOp


export type JSOperator
  = JSMulOp
  | JSAddOp
  | JSDivOp
  | JSSubOp
  | JSLtOp
  | JSGtOp
  | JSBOrOp
  | JSBXorOp
  | JSBAndOp
  | JSBNotOp
  | JSNotOp


export interface JSIdentifier extends SyntaxBase {
  kind: SyntaxKind.JSIdentifier;
  text: string;
  parentNode: JSIdentifierParent;
  getChildNodes(): IterableIterator<JSIdentifierChild>
}

export type JSIdentifierParent
= BoltQuoteExpression
| never

export type JSIdentifierAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSIdentifierChild
= never

export interface JSString extends SyntaxBase {
  kind: SyntaxKind.JSString;
  value: string;
  parentNode: JSStringParent;
  getChildNodes(): IterableIterator<JSStringChild>
}

export type JSStringParent
= BoltQuoteExpression
| never

export type JSStringAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSStringChild
= never

export interface JSInteger extends SyntaxBase {
  kind: SyntaxKind.JSInteger;
  value: bigint;
  parentNode: JSIntegerParent;
  getChildNodes(): IterableIterator<JSIntegerChild>
}

export type JSIntegerParent
= BoltQuoteExpression
| never

export type JSIntegerAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSIntegerChild
= never

export interface JSFromKeyword extends SyntaxBase {
  kind: SyntaxKind.JSFromKeyword;
  parentNode: JSFromKeywordParent;
  getChildNodes(): IterableIterator<JSFromKeywordChild>
}

export type JSFromKeywordParent
= BoltQuoteExpression
| never

export type JSFromKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSFromKeywordChild
= never

export interface JSReturnKeyword extends SyntaxBase {
  kind: SyntaxKind.JSReturnKeyword;
  parentNode: JSReturnKeywordParent;
  getChildNodes(): IterableIterator<JSReturnKeywordChild>
}

export type JSReturnKeywordParent
= BoltQuoteExpression
| never

export type JSReturnKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSReturnKeywordChild
= never

export interface JSTryKeyword extends SyntaxBase {
  kind: SyntaxKind.JSTryKeyword;
  parentNode: JSTryKeywordParent;
  getChildNodes(): IterableIterator<JSTryKeywordChild>
}

export type JSTryKeywordParent
= BoltQuoteExpression
| never

export type JSTryKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSTryKeywordChild
= never

export interface JSFinallyKeyword extends SyntaxBase {
  kind: SyntaxKind.JSFinallyKeyword;
  parentNode: JSFinallyKeywordParent;
  getChildNodes(): IterableIterator<JSFinallyKeywordChild>
}

export type JSFinallyKeywordParent
= BoltQuoteExpression
| never

export type JSFinallyKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSFinallyKeywordChild
= never

export interface JSCatchKeyword extends SyntaxBase {
  kind: SyntaxKind.JSCatchKeyword;
  parentNode: JSCatchKeywordParent;
  getChildNodes(): IterableIterator<JSCatchKeywordChild>
}

export type JSCatchKeywordParent
= BoltQuoteExpression
| never

export type JSCatchKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSCatchKeywordChild
= never

export interface JSImportKeyword extends SyntaxBase {
  kind: SyntaxKind.JSImportKeyword;
  parentNode: JSImportKeywordParent;
  getChildNodes(): IterableIterator<JSImportKeywordChild>
}

export type JSImportKeywordParent
= BoltQuoteExpression
| never

export type JSImportKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSImportKeywordChild
= never

export interface JSAsKeyword extends SyntaxBase {
  kind: SyntaxKind.JSAsKeyword;
  parentNode: JSAsKeywordParent;
  getChildNodes(): IterableIterator<JSAsKeywordChild>
}

export type JSAsKeywordParent
= BoltQuoteExpression
| never

export type JSAsKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSAsKeywordChild
= never

export interface JSConstKeyword extends SyntaxBase {
  kind: SyntaxKind.JSConstKeyword;
  parentNode: JSConstKeywordParent;
  getChildNodes(): IterableIterator<JSConstKeywordChild>
}

export type JSConstKeywordParent
= BoltQuoteExpression
| never

export type JSConstKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSConstKeywordChild
= never

export interface JSLetKeyword extends SyntaxBase {
  kind: SyntaxKind.JSLetKeyword;
  parentNode: JSLetKeywordParent;
  getChildNodes(): IterableIterator<JSLetKeywordChild>
}

export type JSLetKeywordParent
= BoltQuoteExpression
| never

export type JSLetKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSLetKeywordChild
= never

export interface JSExportKeyword extends SyntaxBase {
  kind: SyntaxKind.JSExportKeyword;
  parentNode: JSExportKeywordParent;
  getChildNodes(): IterableIterator<JSExportKeywordChild>
}

export type JSExportKeywordParent
= BoltQuoteExpression
| never

export type JSExportKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSExportKeywordChild
= never

export interface JSFunctionKeyword extends SyntaxBase {
  kind: SyntaxKind.JSFunctionKeyword;
  parentNode: JSFunctionKeywordParent;
  getChildNodes(): IterableIterator<JSFunctionKeywordChild>
}

export type JSFunctionKeywordParent
= BoltQuoteExpression
| never

export type JSFunctionKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSFunctionKeywordChild
= never

export interface JSWhileKeyword extends SyntaxBase {
  kind: SyntaxKind.JSWhileKeyword;
  parentNode: JSWhileKeywordParent;
  getChildNodes(): IterableIterator<JSWhileKeywordChild>
}

export type JSWhileKeywordParent
= BoltQuoteExpression
| never

export type JSWhileKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSWhileKeywordChild
= never

export interface JSForKeyword extends SyntaxBase {
  kind: SyntaxKind.JSForKeyword;
  parentNode: JSForKeywordParent;
  getChildNodes(): IterableIterator<JSForKeywordChild>
}

export type JSForKeywordParent
= BoltQuoteExpression
| never

export type JSForKeywordAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSForKeywordChild
= never

export type JSOperator
  = JSMulOp
  | JSAddOp
  | JSDivOp
  | JSSubOp
  | JSLtOp
  | JSGtOp
  | JSBOrOp
  | JSBXorOp
  | JSBAndOp
  | JSBNotOp
  | JSNotOp


export interface JSCloseBrace extends SyntaxBase {
  kind: SyntaxKind.JSCloseBrace;
  parentNode: JSCloseBraceParent;
  getChildNodes(): IterableIterator<JSCloseBraceChild>
}

export type JSCloseBraceParent
= BoltQuoteExpression
| never

export type JSCloseBraceAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSCloseBraceChild
= never

export interface JSCloseBracket extends SyntaxBase {
  kind: SyntaxKind.JSCloseBracket;
  parentNode: JSCloseBracketParent;
  getChildNodes(): IterableIterator<JSCloseBracketChild>
}

export type JSCloseBracketParent
= BoltQuoteExpression
| never

export type JSCloseBracketAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSCloseBracketChild
= never

export interface JSCloseParen extends SyntaxBase {
  kind: SyntaxKind.JSCloseParen;
  parentNode: JSCloseParenParent;
  getChildNodes(): IterableIterator<JSCloseParenChild>
}

export type JSCloseParenParent
= BoltQuoteExpression
| never

export type JSCloseParenAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSCloseParenChild
= never

export interface JSOpenBrace extends SyntaxBase {
  kind: SyntaxKind.JSOpenBrace;
  parentNode: JSOpenBraceParent;
  getChildNodes(): IterableIterator<JSOpenBraceChild>
}

export type JSOpenBraceParent
= BoltQuoteExpression
| never

export type JSOpenBraceAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSOpenBraceChild
= never

export interface JSOpenBracket extends SyntaxBase {
  kind: SyntaxKind.JSOpenBracket;
  parentNode: JSOpenBracketParent;
  getChildNodes(): IterableIterator<JSOpenBracketChild>
}

export type JSOpenBracketParent
= BoltQuoteExpression
| never

export type JSOpenBracketAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSOpenBracketChild
= never

export interface JSOpenParen extends SyntaxBase {
  kind: SyntaxKind.JSOpenParen;
  parentNode: JSOpenParenParent;
  getChildNodes(): IterableIterator<JSOpenParenChild>
}

export type JSOpenParenParent
= BoltQuoteExpression
| never

export type JSOpenParenAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSOpenParenChild
= never

export interface JSSemi extends SyntaxBase {
  kind: SyntaxKind.JSSemi;
  parentNode: JSSemiParent;
  getChildNodes(): IterableIterator<JSSemiChild>
}

export type JSSemiParent
= BoltQuoteExpression
| never

export type JSSemiAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSSemiChild
= never

export interface JSComma extends SyntaxBase {
  kind: SyntaxKind.JSComma;
  parentNode: JSCommaParent;
  getChildNodes(): IterableIterator<JSCommaChild>
}

export type JSCommaParent
= BoltQuoteExpression
| never

export type JSCommaAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSCommaChild
= never

export interface JSDot extends SyntaxBase {
  kind: SyntaxKind.JSDot;
  parentNode: JSDotParent;
  getChildNodes(): IterableIterator<JSDotChild>
}

export type JSDotParent
= BoltQuoteExpression
| never

export type JSDotAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSDotChild
= never

export interface JSDotDotDot extends SyntaxBase {
  kind: SyntaxKind.JSDotDotDot;
  parentNode: JSDotDotDotParent;
  getChildNodes(): IterableIterator<JSDotDotDotChild>
}

export type JSDotDotDotParent
= BoltQuoteExpression
| never

export type JSDotDotDotAnyParent
= BoltQuoteExpression
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSDotDotDotChild
= never

export interface JSMulOp extends SyntaxBase {
  kind: SyntaxKind.JSMulOp;
  parentNode: JSMulOpParent;
  getChildNodes(): IterableIterator<JSMulOpChild>
}

export type JSMulOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSMulOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSMulOpChild
= never

export interface JSAddOp extends SyntaxBase {
  kind: SyntaxKind.JSAddOp;
  parentNode: JSAddOpParent;
  getChildNodes(): IterableIterator<JSAddOpChild>
}

export type JSAddOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSAddOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSAddOpChild
= never

export interface JSDivOp extends SyntaxBase {
  kind: SyntaxKind.JSDivOp;
  parentNode: JSDivOpParent;
  getChildNodes(): IterableIterator<JSDivOpChild>
}

export type JSDivOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSDivOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSDivOpChild
= never

export interface JSSubOp extends SyntaxBase {
  kind: SyntaxKind.JSSubOp;
  parentNode: JSSubOpParent;
  getChildNodes(): IterableIterator<JSSubOpChild>
}

export type JSSubOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSSubOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSSubOpChild
= never

export interface JSLtOp extends SyntaxBase {
  kind: SyntaxKind.JSLtOp;
  parentNode: JSLtOpParent;
  getChildNodes(): IterableIterator<JSLtOpChild>
}

export type JSLtOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSLtOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSLtOpChild
= never

export interface JSGtOp extends SyntaxBase {
  kind: SyntaxKind.JSGtOp;
  parentNode: JSGtOpParent;
  getChildNodes(): IterableIterator<JSGtOpChild>
}

export type JSGtOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSGtOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSGtOpChild
= never

export interface JSBOrOp extends SyntaxBase {
  kind: SyntaxKind.JSBOrOp;
  parentNode: JSBOrOpParent;
  getChildNodes(): IterableIterator<JSBOrOpChild>
}

export type JSBOrOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSBOrOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSBOrOpChild
= never

export interface JSBXorOp extends SyntaxBase {
  kind: SyntaxKind.JSBXorOp;
  parentNode: JSBXorOpParent;
  getChildNodes(): IterableIterator<JSBXorOpChild>
}

export type JSBXorOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSBXorOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSBXorOpChild
= never

export interface JSBAndOp extends SyntaxBase {
  kind: SyntaxKind.JSBAndOp;
  parentNode: JSBAndOpParent;
  getChildNodes(): IterableIterator<JSBAndOpChild>
}

export type JSBAndOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSBAndOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSBAndOpChild
= never

export interface JSBNotOp extends SyntaxBase {
  kind: SyntaxKind.JSBNotOp;
  parentNode: JSBNotOpParent;
  getChildNodes(): IterableIterator<JSBNotOpChild>
}

export type JSBNotOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSBNotOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSBNotOpChild
= never

export interface JSNotOp extends SyntaxBase {
  kind: SyntaxKind.JSNotOp;
  parentNode: JSNotOpParent;
  getChildNodes(): IterableIterator<JSNotOpChild>
}

export type JSNotOpParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| never

export type JSNotOpAnyParent
= BoltQuoteExpression
| JSBinaryExpression
| JSUnaryExpression
| JSMemberExpression
| JSCallExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| BoltTypeOfExpression
| BoltLiftedTypeExpression
| BoltExpressionPattern
| BoltTupleExpression
| BoltMemberExpression
| BoltCallExpression
| BoltYieldExpression
| BoltMatchArm
| BoltMatchExpression
| BoltCase
| BoltReturnStatement
| BoltConditionalCase
| BoltResumeStatement
| BoltExpressionStatement
| BoltParameter
| BoltVariableDeclaration
| BoltSourceFile
| BoltFunctionExpression
| BoltBlockExpression
| BoltModule
| BoltFunctionDeclaration
| BoltTraitDeclaration
| BoltImplDeclaration
| BoltTypePattern
| BoltTuplePatternElement
| BoltRecordFieldPattern
| BoltReferenceTypeExpression
| BoltFunctionTypeExpression
| BoltTypeParameter
| BoltRecordPattern
| BoltTypeAliasDeclaration
| BoltRecordField
| BoltRecordDeclaration
| never

export type JSNotOpChild
= never

export type JSPattern
  = JSBindPattern


export interface JSBindPattern extends SyntaxBase {
  kind: SyntaxKind.JSBindPattern;
  name: JSIdentifier;
  parentNode: JSBindPatternParent;
  getChildNodes(): IterableIterator<JSBindPatternChild>
}

export type JSBindPatternParent
= JSCatchBlock
| JSParameter
| JSLetDeclaration
| never

export type JSBindPatternAnyParent
= JSCatchBlock
| JSParameter
| JSLetDeclaration
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSBindPatternChild
= never

export type JSExpression
  = JSConstantExpression
  | JSMemberExpression
  | JSCallExpression
  | JSBinaryExpression
  | JSUnaryExpression
  | JSNewExpression
  | JSSequenceExpression
  | JSConditionalExpression
  | JSLiteralExpression
  | JSReferenceExpression


export interface JSConstantExpression extends SyntaxBase {
  kind: SyntaxKind.JSConstantExpression;
  value: BoltValue;
  parentNode: JSConstantExpressionParent;
  getChildNodes(): IterableIterator<JSConstantExpressionChild>
}

export type JSConstantExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSConstantExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSConstantExpressionChild
= never

export interface JSMemberExpression extends SyntaxBase {
  kind: SyntaxKind.JSMemberExpression;
  value: JSExpression;
  property: JSIdentifier;
  parentNode: JSMemberExpressionParent;
  getChildNodes(): IterableIterator<JSMemberExpressionChild>
}

export type JSMemberExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSMemberExpressionAnyParent
= JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSMemberExpressionChild
= never

export interface JSCallExpression extends SyntaxBase {
  kind: SyntaxKind.JSCallExpression;
  operator: JSExpression;
  operands: JSExpression[];
  parentNode: JSCallExpressionParent;
  getChildNodes(): IterableIterator<JSCallExpressionChild>
}

export type JSCallExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSCallExpressionAnyParent
= JSMemberExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSCallExpressionChild
= never

export interface JSBinaryExpression extends SyntaxBase {
  kind: SyntaxKind.JSBinaryExpression;
  left: JSExpression;
  operator: JSOperator;
  right: JSExpression;
  parentNode: JSBinaryExpressionParent;
  getChildNodes(): IterableIterator<JSBinaryExpressionChild>
}

export type JSBinaryExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSBinaryExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSBinaryExpressionChild
= never

export interface JSUnaryExpression extends SyntaxBase {
  kind: SyntaxKind.JSUnaryExpression;
  operator: JSOperator;
  operand: JSExpression;
  parentNode: JSUnaryExpressionParent;
  getChildNodes(): IterableIterator<JSUnaryExpressionChild>
}

export type JSUnaryExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSUnaryExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSUnaryExpressionChild
= never

export interface JSNewExpression extends SyntaxBase {
  kind: SyntaxKind.JSNewExpression;
  target: JSExpression;
  arguments: JSExpression[];
  parentNode: JSNewExpressionParent;
  getChildNodes(): IterableIterator<JSNewExpressionChild>
}

export type JSNewExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSNewExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSNewExpressionChild
= never

export interface JSSequenceExpression extends SyntaxBase {
  kind: SyntaxKind.JSSequenceExpression;
  expressions: JSExpression[];
  parentNode: JSSequenceExpressionParent;
  getChildNodes(): IterableIterator<JSSequenceExpressionChild>
}

export type JSSequenceExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSSequenceExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSSequenceExpressionChild
= never

export interface JSConditionalExpression extends SyntaxBase {
  kind: SyntaxKind.JSConditionalExpression;
  test: JSExpression;
  consequent: JSExpression;
  alternate: JSExpression;
  parentNode: JSConditionalExpressionParent;
  getChildNodes(): IterableIterator<JSConditionalExpressionChild>
}

export type JSConditionalExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSConditionalExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSConditionalExpressionChild
= never

export interface JSLiteralExpression extends SyntaxBase {
  kind: SyntaxKind.JSLiteralExpression;
  value: JSValue;
  parentNode: JSLiteralExpressionParent;
  getChildNodes(): IterableIterator<JSLiteralExpressionChild>
}

export type JSLiteralExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSLiteralExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSLiteralExpressionChild
= never

export interface JSReferenceExpression extends SyntaxBase {
  kind: SyntaxKind.JSReferenceExpression;
  name: string;
  parentNode: JSReferenceExpressionParent;
  getChildNodes(): IterableIterator<JSReferenceExpressionChild>
}

export type JSReferenceExpressionParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| never

export type JSReferenceExpressionAnyParent
= JSMemberExpression
| JSCallExpression
| JSBinaryExpression
| JSUnaryExpression
| JSNewExpression
| JSSequenceExpression
| JSConditionalExpression
| JSExpressionStatement
| JSConditionalCase
| JSReturnStatement
| JSParameter
| JSArrowFunctionDeclaration
| JSLetDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| JSFunctionDeclaration
| never

export type JSReferenceExpressionChild
= never

export type JSSourceElement
  = JSImportDeclaration
  | JSFunctionDeclaration
  | JSArrowFunctionDeclaration
  | JSLetDeclaration
  | JSExpressionStatement
  | JSConditionalStatement
  | JSReturnStatement


export type JSFunctionBodyElement
  = JSFunctionDeclaration
  | JSArrowFunctionDeclaration
  | JSLetDeclaration
  | JSExpressionStatement
  | JSConditionalStatement
  | JSReturnStatement


export type JSStatement
  = JSExpressionStatement
  | JSConditionalStatement
  | JSReturnStatement


export interface JSCatchBlock extends SyntaxBase {
  kind: SyntaxKind.JSCatchBlock;
  bindings: JSPattern | null;
  elements: JSSourceElement[];
  parentNode: JSCatchBlockParent;
  getChildNodes(): IterableIterator<JSCatchBlockChild>
}

export type JSCatchBlockParent
= never

export type JSCatchBlockAnyParent
= never

export type JSCatchBlockChild
= never

export interface JSTryCatchStatement extends SyntaxBase {
  kind: SyntaxKind.JSTryCatchStatement;
  tryBlock: JSSourceElement[];
  catchBlock: JSCatchBlock | null;
  finalBlock: JSSourceElement[] | null;
  parentNode: JSTryCatchStatementParent;
  getChildNodes(): IterableIterator<JSTryCatchStatementChild>
}

export type JSTryCatchStatementParent
= never

export type JSTryCatchStatementAnyParent
= never

export type JSTryCatchStatementChild
= never

export interface JSExpressionStatement extends SyntaxBase {
  kind: SyntaxKind.JSExpressionStatement;
  expression: JSExpression;
  parentNode: JSExpressionStatementParent;
  getChildNodes(): IterableIterator<JSExpressionStatementChild>
}

export type JSExpressionStatementParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSFunctionDeclaration
| JSSourceFile
| never

export type JSExpressionStatementAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSFunctionDeclaration
| JSSourceFile
| never

export type JSExpressionStatementChild
= never

export interface JSConditionalCase extends SyntaxBase {
  kind: SyntaxKind.JSConditionalCase;
  test: JSExpression | null;
  body: JSFunctionBodyElement[];
  parentNode: JSConditionalCaseParent;
  getChildNodes(): IterableIterator<JSConditionalCaseChild>
}

export type JSConditionalCaseParent
= never

export type JSConditionalCaseAnyParent
= never

export type JSConditionalCaseChild
= never

export interface JSConditionalStatement extends SyntaxBase {
  kind: SyntaxKind.JSConditionalStatement;
  cases: JSConditionalCase[];
  parentNode: JSConditionalStatementParent;
  getChildNodes(): IterableIterator<JSConditionalStatementChild>
}

export type JSConditionalStatementParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSFunctionDeclaration
| JSSourceFile
| never

export type JSConditionalStatementAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSFunctionDeclaration
| JSSourceFile
| never

export type JSConditionalStatementChild
= never

export interface JSReturnStatement extends SyntaxBase {
  kind: SyntaxKind.JSReturnStatement;
  value: JSExpression | null;
  parentNode: JSReturnStatementParent;
  getChildNodes(): IterableIterator<JSReturnStatementChild>
}

export type JSReturnStatementParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSFunctionDeclaration
| JSSourceFile
| never

export type JSReturnStatementAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSFunctionDeclaration
| JSSourceFile
| never

export type JSReturnStatementChild
= never

export interface JSParameter extends SyntaxBase {
  kind: SyntaxKind.JSParameter;
  index: number;
  bindings: JSPattern;
  defaultValue: JSExpression | null;
  parentNode: JSParameterParent;
  getChildNodes(): IterableIterator<JSParameterChild>
}

export type JSParameterParent
= never

export type JSParameterAnyParent
= never

export type JSParameterChild
= never

export type JSDeclaration
  = JSImportDeclaration
  | JSFunctionDeclaration
  | JSArrowFunctionDeclaration
  | JSLetDeclaration


export const enum JSDeclarationModifiers {
  IsExported = 1,}

export type JSImportBinding
  = JSImportStarBinding
  | JSImportAsBinding


export interface JSImportStarBinding extends SyntaxBase {
  kind: SyntaxKind.JSImportStarBinding;
  local: JSIdentifier;
  parentNode: JSImportStarBindingParent;
  getChildNodes(): IterableIterator<JSImportStarBindingChild>
}

export type JSImportStarBindingParent
= JSImportDeclaration
| never

export type JSImportStarBindingAnyParent
= JSImportDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| never

export type JSImportStarBindingChild
= never

export interface JSImportAsBinding extends SyntaxBase {
  kind: SyntaxKind.JSImportAsBinding;
  remote: JSIdentifier;
  local: JSIdentifier | null;
  parentNode: JSImportAsBindingParent;
  getChildNodes(): IterableIterator<JSImportAsBindingChild>
}

export type JSImportAsBindingParent
= JSImportDeclaration
| never

export type JSImportAsBindingAnyParent
= JSImportDeclaration
| JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| never

export type JSImportAsBindingChild
= never

export interface JSImportDeclaration extends SyntaxBase {
  kind: SyntaxKind.JSImportDeclaration;
  bindings: JSImportBinding[];
  filename: JSString;
  parentNode: JSImportDeclarationParent;
  getChildNodes(): IterableIterator<JSImportDeclarationChild>
}

export type JSImportDeclarationParent
= JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| never

export type JSImportDeclarationAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSSourceFile
| never

export type JSImportDeclarationChild
= never

export interface JSFunctionDeclaration extends SyntaxBase {
  kind: SyntaxKind.JSFunctionDeclaration;
  modifiers: JSDeclarationModifiers;
  name: JSIdentifier;
  params: JSParameter[];
  body: JSStatement[];
  parentNode: JSFunctionDeclarationParent;
  getChildNodes(): IterableIterator<JSFunctionDeclarationChild>
}

export type JSFunctionDeclarationParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSFunctionDeclarationAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSFunctionDeclarationChild
= never

export interface JSArrowFunctionDeclaration extends SyntaxBase {
  kind: SyntaxKind.JSArrowFunctionDeclaration;
  name: JSIdentifier;
  params: JSParameter[];
  body: JSExpression;
  parentNode: JSArrowFunctionDeclarationParent;
  getChildNodes(): IterableIterator<JSArrowFunctionDeclarationChild>
}

export type JSArrowFunctionDeclarationParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSArrowFunctionDeclarationAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSArrowFunctionDeclarationChild
= never

export interface JSLetDeclaration extends SyntaxBase {
  kind: SyntaxKind.JSLetDeclaration;
  bindings: JSPattern;
  value: JSExpression | null;
  parentNode: JSLetDeclarationParent;
  getChildNodes(): IterableIterator<JSLetDeclarationChild>
}

export type JSLetDeclarationParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSLetDeclarationAnyParent
= JSCatchBlock
| JSTryCatchStatement
| JSConditionalCase
| JSSourceFile
| never

export type JSLetDeclarationChild
= never

export interface JSSourceFile extends SyntaxBase {
  kind: SyntaxKind.JSSourceFile;
  elements: JSSourceElement[];
  parentNode: JSSourceFileParent;
  getChildNodes(): IterableIterator<JSSourceFileChild>
}

export type JSSourceFileParent
= never

export type JSSourceFileAnyParent
= never

export type JSSourceFileChild
= never

export type Syntax
  = EndOfFile
  | BoltStringLiteral
  | BoltIntegerLiteral
  | BoltIdentifier
  | BoltOperator
  | BoltAssignment
  | BoltComma
  | BoltSemi
  | BoltColon
  | BoltColonColon
  | BoltDot
  | BoltDotDot
  | BoltRArrow
  | BoltRArrowAlt
  | BoltLArrow
  | BoltEqSign
  | BoltGtSign
  | BoltExMark
  | BoltLtSign
  | BoltVBar
  | BoltWhereKeyword
  | BoltQuoteKeyword
  | BoltFnKeyword
  | BoltForeignKeyword
  | BoltForKeyword
  | BoltLetKeyword
  | BoltReturnKeyword
  | BoltLoopKeyword
  | BoltYieldKeyword
  | BoltMatchKeyword
  | BoltImportKeyword
  | BoltExportKeyword
  | BoltPubKeyword
  | BoltModKeyword
  | BoltMutKeyword
  | BoltEnumKeyword
  | BoltStructKeyword
  | BoltTypeKeyword
  | BoltTraitKeyword
  | BoltImplKeyword
  | BoltParenthesized
  | BoltBraced
  | BoltBracketed
  | BoltSourceFile
  | BoltQualName
  | BoltTypeOfExpression
  | BoltReferenceTypeExpression
  | BoltFunctionTypeExpression
  | BoltLiftedTypeExpression
  | BoltTypeParameter
  | BoltBindPattern
  | BoltTypePattern
  | BoltExpressionPattern
  | BoltTuplePatternElement
  | BoltTuplePattern
  | BoltRecordFieldPattern
  | BoltRecordPattern
  | BoltQuoteExpression
  | BoltTupleExpression
  | BoltReferenceExpression
  | BoltMemberExpression
  | BoltFunctionExpression
  | BoltCallExpression
  | BoltYieldExpression
  | BoltMatchArm
  | BoltMatchExpression
  | BoltCase
  | BoltCaseExpression
  | BoltBlockExpression
  | BoltConstantExpression
  | BoltReturnStatement
  | BoltConditionalCase
  | BoltConditionalStatement
  | BoltResumeStatement
  | BoltExpressionStatement
  | BoltParameter
  | BoltModule
  | BoltFunctionDeclaration
  | BoltVariableDeclaration
  | BoltPlainImportSymbol
  | BoltImportDirective
  | BoltExportSymbol
  | BoltPlainExportSymbol
  | BoltExportDirective
  | BoltTraitDeclaration
  | BoltImplDeclaration
  | BoltTypeAliasDeclaration
  | BoltRecordField
  | BoltRecordDeclaration
  | BoltMacroCall
  | JSIdentifier
  | JSString
  | JSInteger
  | JSFromKeyword
  | JSReturnKeyword
  | JSTryKeyword
  | JSFinallyKeyword
  | JSCatchKeyword
  | JSImportKeyword
  | JSAsKeyword
  | JSConstKeyword
  | JSLetKeyword
  | JSExportKeyword
  | JSFunctionKeyword
  | JSWhileKeyword
  | JSForKeyword
  | JSCloseBrace
  | JSCloseBracket
  | JSCloseParen
  | JSOpenBrace
  | JSOpenBracket
  | JSOpenParen
  | JSSemi
  | JSComma
  | JSDot
  | JSDotDotDot
  | JSMulOp
  | JSAddOp
  | JSDivOp
  | JSSubOp
  | JSLtOp
  | JSGtOp
  | JSBOrOp
  | JSBXorOp
  | JSBAndOp
  | JSBNotOp
  | JSNotOp
  | JSBindPattern
  | JSConstantExpression
  | JSMemberExpression
  | JSCallExpression
  | JSBinaryExpression
  | JSUnaryExpression
  | JSNewExpression
  | JSSequenceExpression
  | JSConditionalExpression
  | JSLiteralExpression
  | JSReferenceExpression
  | JSCatchBlock
  | JSTryCatchStatement
  | JSExpressionStatement
  | JSConditionalCase
  | JSConditionalStatement
  | JSReturnStatement
  | JSParameter
  | JSImportStarBinding
  | JSImportAsBinding
  | JSImportDeclaration
  | JSFunctionDeclaration
  | JSArrowFunctionDeclaration
  | JSLetDeclaration
  | JSSourceFile


export function kindToString(kind: SyntaxKind): string;

export function createEndOfFile(span?: TextSpan | null): EndOfFile;
export function createBoltStringLiteral(value: string, span?: TextSpan | null): BoltStringLiteral;
export function createBoltIntegerLiteral(value: bigint, span?: TextSpan | null): BoltIntegerLiteral;
export function createBoltIdentifier(text: string, span?: TextSpan | null): BoltIdentifier;
export function createBoltOperator(text: string, span?: TextSpan | null): BoltOperator;
export function createBoltAssignment(operator: string | null, span?: TextSpan | null): BoltAssignment;
export function createBoltComma(span?: TextSpan | null): BoltComma;
export function createBoltSemi(span?: TextSpan | null): BoltSemi;
export function createBoltColon(span?: TextSpan | null): BoltColon;
export function createBoltColonColon(span?: TextSpan | null): BoltColonColon;
export function createBoltDot(span?: TextSpan | null): BoltDot;
export function createBoltDotDot(span?: TextSpan | null): BoltDotDot;
export function createBoltRArrow(span?: TextSpan | null): BoltRArrow;
export function createBoltRArrowAlt(span?: TextSpan | null): BoltRArrowAlt;
export function createBoltLArrow(span?: TextSpan | null): BoltLArrow;
export function createBoltEqSign(span?: TextSpan | null): BoltEqSign;
export function createBoltGtSign(span?: TextSpan | null): BoltGtSign;
export function createBoltExMark(span?: TextSpan | null): BoltExMark;
export function createBoltLtSign(span?: TextSpan | null): BoltLtSign;
export function createBoltVBar(span?: TextSpan | null): BoltVBar;
export function createBoltWhereKeyword(span?: TextSpan | null): BoltWhereKeyword;
export function createBoltQuoteKeyword(span?: TextSpan | null): BoltQuoteKeyword;
export function createBoltFnKeyword(span?: TextSpan | null): BoltFnKeyword;
export function createBoltForeignKeyword(span?: TextSpan | null): BoltForeignKeyword;
export function createBoltForKeyword(span?: TextSpan | null): BoltForKeyword;
export function createBoltLetKeyword(span?: TextSpan | null): BoltLetKeyword;
export function createBoltReturnKeyword(span?: TextSpan | null): BoltReturnKeyword;
export function createBoltLoopKeyword(span?: TextSpan | null): BoltLoopKeyword;
export function createBoltYieldKeyword(span?: TextSpan | null): BoltYieldKeyword;
export function createBoltMatchKeyword(span?: TextSpan | null): BoltMatchKeyword;
export function createBoltImportKeyword(span?: TextSpan | null): BoltImportKeyword;
export function createBoltExportKeyword(span?: TextSpan | null): BoltExportKeyword;
export function createBoltPubKeyword(span?: TextSpan | null): BoltPubKeyword;
export function createBoltModKeyword(span?: TextSpan | null): BoltModKeyword;
export function createBoltMutKeyword(span?: TextSpan | null): BoltMutKeyword;
export function createBoltEnumKeyword(span?: TextSpan | null): BoltEnumKeyword;
export function createBoltStructKeyword(span?: TextSpan | null): BoltStructKeyword;
export function createBoltTypeKeyword(span?: TextSpan | null): BoltTypeKeyword;
export function createBoltTraitKeyword(span?: TextSpan | null): BoltTraitKeyword;
export function createBoltImplKeyword(span?: TextSpan | null): BoltImplKeyword;
export function createBoltParenthesized(text: string, span?: TextSpan | null): BoltParenthesized;
export function createBoltBraced(text: string, span?: TextSpan | null): BoltBraced;
export function createBoltBracketed(text: string, span?: TextSpan | null): BoltBracketed;
export function createBoltSourceFile(elements: BoltSourceElement[], package: Package, span?: TextSpan | null): BoltSourceFile;
export function createBoltQualName(isAbsolute: boolean, modulePath: BoltIdentifier[], name: BoltSymbol, span?: TextSpan | null): BoltQualName;
export function createBoltTypeOfExpression(expression: BoltExpression, span?: TextSpan | null): BoltTypeOfExpression;
export function createBoltReferenceTypeExpression(name: BoltQualName, arguments: BoltTypeExpression[] | null, span?: TextSpan | null): BoltReferenceTypeExpression;
export function createBoltFunctionTypeExpression(params: BoltParameter[], returnType: BoltTypeExpression | null, span?: TextSpan | null): BoltFunctionTypeExpression;
export function createBoltLiftedTypeExpression(expression: BoltExpression, span?: TextSpan | null): BoltLiftedTypeExpression;
export function createBoltTypeParameter(index: number, name: BoltIdentifier, typeExpr: BoltTypeExpression | null, defaultType: BoltTypeExpression | null, span?: TextSpan | null): BoltTypeParameter;
export function createBoltBindPattern(name: BoltIdentifier, span?: TextSpan | null): BoltBindPattern;
export function createBoltTypePattern(typeExpr: BoltTypeExpression, nestedPattern: BoltPattern, span?: TextSpan | null): BoltTypePattern;
export function createBoltExpressionPattern(expression: BoltExpression, span?: TextSpan | null): BoltExpressionPattern;
export function createBoltTuplePatternElement(index: number, pattern: BoltPattern, span?: TextSpan | null): BoltTuplePatternElement;
export function createBoltTuplePattern(elements: BoltTuplePatternElement[], span?: TextSpan | null): BoltTuplePattern;
export function createBoltRecordFieldPattern(isRest: boolean, name: BoltIdentifier | null, pattern: BoltPattern | null, span?: TextSpan | null): BoltRecordFieldPattern;
export function createBoltRecordPattern(name: BoltTypeExpression, fields: BoltRecordFieldPattern[], span?: TextSpan | null): BoltRecordPattern;
export function createBoltQuoteExpression(tokens: (Token | BoltExpression)[], span?: TextSpan | null): BoltQuoteExpression;
export function createBoltTupleExpression(elements: BoltExpression[], span?: TextSpan | null): BoltTupleExpression;
export function createBoltReferenceExpression(name: BoltQualName, span?: TextSpan | null): BoltReferenceExpression;
export function createBoltMemberExpression(expression: BoltExpression, path: BoltIdentifier[], span?: TextSpan | null): BoltMemberExpression;
export function createBoltFunctionExpression(params: BoltParameter[], returnType: BoltTypeExpression | null, body: BoltFunctionBodyElement[], span?: TextSpan | null): BoltFunctionExpression;
export function createBoltCallExpression(operator: BoltExpression, operands: BoltExpression[], span?: TextSpan | null): BoltCallExpression;
export function createBoltYieldExpression(value: BoltExpression, span?: TextSpan | null): BoltYieldExpression;
export function createBoltMatchArm(pattern: BoltPattern, body: BoltExpression, span?: TextSpan | null): BoltMatchArm;
export function createBoltMatchExpression(value: BoltExpression, arms: BoltMatchArm[], span?: TextSpan | null): BoltMatchExpression;
export function createBoltCase(test: BoltExpression, result: BoltExpression, span?: TextSpan | null): BoltCase;
export function createBoltCaseExpression(cases: BoltCase[], span?: TextSpan | null): BoltCaseExpression;
export function createBoltBlockExpression(elements: BoltFunctionBodyElement[], span?: TextSpan | null): BoltBlockExpression;
export function createBoltConstantExpression(value: BoltValue, span?: TextSpan | null): BoltConstantExpression;
export function createBoltReturnStatement(value: BoltExpression | null, span?: TextSpan | null): BoltReturnStatement;
export function createBoltConditionalCase(test: BoltExpression | null, body: BoltFunctionBodyElement[], span?: TextSpan | null): BoltConditionalCase;
export function createBoltConditionalStatement(cases: BoltConditionalCase[], span?: TextSpan | null): BoltConditionalStatement;
export function createBoltResumeStatement(value: BoltExpression, span?: TextSpan | null): BoltResumeStatement;
export function createBoltExpressionStatement(expression: BoltExpression, span?: TextSpan | null): BoltExpressionStatement;
export function createBoltParameter(index: number, bindings: BoltPattern, typeExpr: BoltTypeExpression | null, defaultValue: BoltExpression | null, span?: TextSpan | null): BoltParameter;
export function createBoltModule(modifiers: BoltModifiers, name: BoltIdentifier[], elements: BoltSourceElement[], span?: TextSpan | null): BoltModule;
export function createBoltFunctionDeclaration(modifiers: BoltModifiers, target: string, name: BoltSymbol, params: BoltParameter[], returnType: BoltTypeExpression | null, typeParams: BoltTypeParameter[] | null, body: BoltFunctionBodyElement[], span?: TextSpan | null): BoltFunctionDeclaration;
export function createBoltVariableDeclaration(modifiers: BoltModifiers, bindings: BoltPattern, typeExpr: BoltTypeExpression | null, value: BoltExpression | null, span?: TextSpan | null): BoltVariableDeclaration;
export function createBoltPlainImportSymbol(remote: BoltQualName, local: BoltSymbol, span?: TextSpan | null): BoltPlainImportSymbol;
export function createBoltImportDirective(modifiers: BoltModifiers, file: BoltStringLiteral, symbols: BoltImportSymbol[] | null, span?: TextSpan | null): BoltImportDirective;
export function createBoltExportSymbol(span?: TextSpan | null): BoltExportSymbol;
export function createBoltPlainExportSymbol(local: BoltQualName, remote: BoltSymbol, span?: TextSpan | null): BoltPlainExportSymbol;
export function createBoltExportDirective(file: string, symbols: BoltExportSymbol[] | null, span?: TextSpan | null): BoltExportDirective;
export function createBoltTraitDeclaration(modifiers: BoltModifiers, name: BoltIdentifier, typeParams: BoltTypeParameter[] | null, elements: BoltDeclaration[], span?: TextSpan | null): BoltTraitDeclaration;
export function createBoltImplDeclaration(modifiers: BoltModifiers, name: BoltIdentifier, trait: BoltTypeExpression, typeParams: BoltTypeParameter[] | null, elements: BoltDeclaration[], span?: TextSpan | null): BoltImplDeclaration;
export function createBoltTypeAliasDeclaration(modifiers: BoltModifiers, name: BoltIdentifier, typeParams: BoltTypeParameter[] | null, typeExpr: BoltTypeExpression, span?: TextSpan | null): BoltTypeAliasDeclaration;
export function createBoltRecordField(name: BoltIdentifier, typeExpr: BoltTypeExpression, span?: TextSpan | null): BoltRecordField;
export function createBoltRecordDeclaration(modifiers: BoltModifiers, name: BoltIdentifier, typeParms: BoltTypeParameter[] | null, members: BoltRecordMember[] | null, span?: TextSpan | null): BoltRecordDeclaration;
export function createBoltMacroCall(name: BoltIdentifier, text: string, span?: TextSpan | null): BoltMacroCall;
export function createJSIdentifier(text: string, span?: TextSpan | null): JSIdentifier;
export function createJSString(value: string, span?: TextSpan | null): JSString;
export function createJSInteger(value: bigint, span?: TextSpan | null): JSInteger;
export function createJSFromKeyword(span?: TextSpan | null): JSFromKeyword;
export function createJSReturnKeyword(span?: TextSpan | null): JSReturnKeyword;
export function createJSTryKeyword(span?: TextSpan | null): JSTryKeyword;
export function createJSFinallyKeyword(span?: TextSpan | null): JSFinallyKeyword;
export function createJSCatchKeyword(span?: TextSpan | null): JSCatchKeyword;
export function createJSImportKeyword(span?: TextSpan | null): JSImportKeyword;
export function createJSAsKeyword(span?: TextSpan | null): JSAsKeyword;
export function createJSConstKeyword(span?: TextSpan | null): JSConstKeyword;
export function createJSLetKeyword(span?: TextSpan | null): JSLetKeyword;
export function createJSExportKeyword(span?: TextSpan | null): JSExportKeyword;
export function createJSFunctionKeyword(span?: TextSpan | null): JSFunctionKeyword;
export function createJSWhileKeyword(span?: TextSpan | null): JSWhileKeyword;
export function createJSForKeyword(span?: TextSpan | null): JSForKeyword;
export function createJSCloseBrace(span?: TextSpan | null): JSCloseBrace;
export function createJSCloseBracket(span?: TextSpan | null): JSCloseBracket;
export function createJSCloseParen(span?: TextSpan | null): JSCloseParen;
export function createJSOpenBrace(span?: TextSpan | null): JSOpenBrace;
export function createJSOpenBracket(span?: TextSpan | null): JSOpenBracket;
export function createJSOpenParen(span?: TextSpan | null): JSOpenParen;
export function createJSSemi(span?: TextSpan | null): JSSemi;
export function createJSComma(span?: TextSpan | null): JSComma;
export function createJSDot(span?: TextSpan | null): JSDot;
export function createJSDotDotDot(span?: TextSpan | null): JSDotDotDot;
export function createJSMulOp(span?: TextSpan | null): JSMulOp;
export function createJSAddOp(span?: TextSpan | null): JSAddOp;
export function createJSDivOp(span?: TextSpan | null): JSDivOp;
export function createJSSubOp(span?: TextSpan | null): JSSubOp;
export function createJSLtOp(span?: TextSpan | null): JSLtOp;
export function createJSGtOp(span?: TextSpan | null): JSGtOp;
export function createJSBOrOp(span?: TextSpan | null): JSBOrOp;
export function createJSBXorOp(span?: TextSpan | null): JSBXorOp;
export function createJSBAndOp(span?: TextSpan | null): JSBAndOp;
export function createJSBNotOp(span?: TextSpan | null): JSBNotOp;
export function createJSNotOp(span?: TextSpan | null): JSNotOp;
export function createJSBindPattern(name: JSIdentifier, span?: TextSpan | null): JSBindPattern;
export function createJSConstantExpression(value: BoltValue, span?: TextSpan | null): JSConstantExpression;
export function createJSMemberExpression(value: JSExpression, property: JSIdentifier, span?: TextSpan | null): JSMemberExpression;
export function createJSCallExpression(operator: JSExpression, operands: JSExpression[], span?: TextSpan | null): JSCallExpression;
export function createJSBinaryExpression(left: JSExpression, operator: JSOperator, right: JSExpression, span?: TextSpan | null): JSBinaryExpression;
export function createJSUnaryExpression(operator: JSOperator, operand: JSExpression, span?: TextSpan | null): JSUnaryExpression;
export function createJSNewExpression(target: JSExpression, arguments: JSExpression[], span?: TextSpan | null): JSNewExpression;
export function createJSSequenceExpression(expressions: JSExpression[], span?: TextSpan | null): JSSequenceExpression;
export function createJSConditionalExpression(test: JSExpression, consequent: JSExpression, alternate: JSExpression, span?: TextSpan | null): JSConditionalExpression;
export function createJSLiteralExpression(value: JSValue, span?: TextSpan | null): JSLiteralExpression;
export function createJSReferenceExpression(name: string, span?: TextSpan | null): JSReferenceExpression;
export function createJSCatchBlock(bindings: JSPattern | null, elements: JSSourceElement[], span?: TextSpan | null): JSCatchBlock;
export function createJSTryCatchStatement(tryBlock: JSSourceElement[], catchBlock: JSCatchBlock | null, finalBlock: JSSourceElement[] | null, span?: TextSpan | null): JSTryCatchStatement;
export function createJSExpressionStatement(expression: JSExpression, span?: TextSpan | null): JSExpressionStatement;
export function createJSConditionalCase(test: JSExpression | null, body: JSFunctionBodyElement[], span?: TextSpan | null): JSConditionalCase;
export function createJSConditionalStatement(cases: JSConditionalCase[], span?: TextSpan | null): JSConditionalStatement;
export function createJSReturnStatement(value: JSExpression | null, span?: TextSpan | null): JSReturnStatement;
export function createJSParameter(index: number, bindings: JSPattern, defaultValue: JSExpression | null, span?: TextSpan | null): JSParameter;
export function createJSImportStarBinding(local: JSIdentifier, span?: TextSpan | null): JSImportStarBinding;
export function createJSImportAsBinding(remote: JSIdentifier, local: JSIdentifier | null, span?: TextSpan | null): JSImportAsBinding;
export function createJSImportDeclaration(bindings: JSImportBinding[], filename: JSString, span?: TextSpan | null): JSImportDeclaration;
export function createJSFunctionDeclaration(modifiers: JSDeclarationModifiers, name: JSIdentifier, params: JSParameter[], body: JSStatement[], span?: TextSpan | null): JSFunctionDeclaration;
export function createJSArrowFunctionDeclaration(name: JSIdentifier, params: JSParameter[], body: JSExpression, span?: TextSpan | null): JSArrowFunctionDeclaration;
export function createJSLetDeclaration(bindings: JSPattern, value: JSExpression | null, span?: TextSpan | null): JSLetDeclaration;
export function createJSSourceFile(elements: JSSourceElement[], span?: TextSpan | null): JSSourceFile;

export function isEndOfFile(value: any): value is EndOfFile;
export function isToken(value: any): value is Token;
export function isSourceFile(value: any): value is SourceFile;
export function isFunctionBodyElement(value: any): value is FunctionBodyElement;
export function isReturnStatement(value: any): value is ReturnStatement;
export function isBoltSyntax(value: any): value is BoltSyntax;
export function isBoltToken(value: any): value is BoltToken;
export function isBoltStringLiteral(value: any): value is BoltStringLiteral;
export function isBoltIntegerLiteral(value: any): value is BoltIntegerLiteral;
export function isBoltSymbol(value: any): value is BoltSymbol;
export function isBoltIdentifier(value: any): value is BoltIdentifier;
export function isBoltOperatorLike(value: any): value is BoltOperatorLike;
export function isBoltOperator(value: any): value is BoltOperator;
export function isBoltAssignment(value: any): value is BoltAssignment;
export function isBoltComma(value: any): value is BoltComma;
export function isBoltSemi(value: any): value is BoltSemi;
export function isBoltColon(value: any): value is BoltColon;
export function isBoltColonColon(value: any): value is BoltColonColon;
export function isBoltDot(value: any): value is BoltDot;
export function isBoltDotDot(value: any): value is BoltDotDot;
export function isBoltRArrow(value: any): value is BoltRArrow;
export function isBoltRArrowAlt(value: any): value is BoltRArrowAlt;
export function isBoltLArrow(value: any): value is BoltLArrow;
export function isBoltEqSign(value: any): value is BoltEqSign;
export function isBoltGtSign(value: any): value is BoltGtSign;
export function isBoltExMark(value: any): value is BoltExMark;
export function isBoltLtSign(value: any): value is BoltLtSign;
export function isBoltVBar(value: any): value is BoltVBar;
export function isBoltKeyword(value: any): value is BoltKeyword;
export function isBoltWhereKeyword(value: any): value is BoltWhereKeyword;
export function isBoltQuoteKeyword(value: any): value is BoltQuoteKeyword;
export function isBoltFnKeyword(value: any): value is BoltFnKeyword;
export function isBoltForeignKeyword(value: any): value is BoltForeignKeyword;
export function isBoltForKeyword(value: any): value is BoltForKeyword;
export function isBoltLetKeyword(value: any): value is BoltLetKeyword;
export function isBoltReturnKeyword(value: any): value is BoltReturnKeyword;
export function isBoltLoopKeyword(value: any): value is BoltLoopKeyword;
export function isBoltYieldKeyword(value: any): value is BoltYieldKeyword;
export function isBoltMatchKeyword(value: any): value is BoltMatchKeyword;
export function isBoltImportKeyword(value: any): value is BoltImportKeyword;
export function isBoltExportKeyword(value: any): value is BoltExportKeyword;
export function isBoltPubKeyword(value: any): value is BoltPubKeyword;
export function isBoltModKeyword(value: any): value is BoltModKeyword;
export function isBoltMutKeyword(value: any): value is BoltMutKeyword;
export function isBoltEnumKeyword(value: any): value is BoltEnumKeyword;
export function isBoltStructKeyword(value: any): value is BoltStructKeyword;
export function isBoltTypeKeyword(value: any): value is BoltTypeKeyword;
export function isBoltTraitKeyword(value: any): value is BoltTraitKeyword;
export function isBoltImplKeyword(value: any): value is BoltImplKeyword;
export function isBoltPunctuated(value: any): value is BoltPunctuated;
export function isBoltParenthesized(value: any): value is BoltParenthesized;
export function isBoltBraced(value: any): value is BoltBraced;
export function isBoltBracketed(value: any): value is BoltBracketed;
export function isBoltSourceFile(value: any): value is BoltSourceFile;
export function isBoltQualName(value: any): value is BoltQualName;
export function isBoltTypeExpression(value: any): value is BoltTypeExpression;
export function isBoltTypeOfExpression(value: any): value is BoltTypeOfExpression;
export function isBoltReferenceTypeExpression(value: any): value is BoltReferenceTypeExpression;
export function isBoltFunctionTypeExpression(value: any): value is BoltFunctionTypeExpression;
export function isBoltLiftedTypeExpression(value: any): value is BoltLiftedTypeExpression;
export function isBoltTypeParameter(value: any): value is BoltTypeParameter;
export function isBoltPattern(value: any): value is BoltPattern;
export function isBoltBindPattern(value: any): value is BoltBindPattern;
export function isBoltTypePattern(value: any): value is BoltTypePattern;
export function isBoltExpressionPattern(value: any): value is BoltExpressionPattern;
export function isBoltTuplePatternElement(value: any): value is BoltTuplePatternElement;
export function isBoltTuplePattern(value: any): value is BoltTuplePattern;
export function isBoltRecordFieldPattern(value: any): value is BoltRecordFieldPattern;
export function isBoltRecordPattern(value: any): value is BoltRecordPattern;
export function isBoltExpression(value: any): value is BoltExpression;
export function isBoltQuoteExpression(value: any): value is BoltQuoteExpression;
export function isBoltTupleExpression(value: any): value is BoltTupleExpression;
export function isBoltReferenceExpression(value: any): value is BoltReferenceExpression;
export function isBoltMemberExpression(value: any): value is BoltMemberExpression;
export function isBoltFunctionExpression(value: any): value is BoltFunctionExpression;
export function isBoltCallExpression(value: any): value is BoltCallExpression;
export function isBoltYieldExpression(value: any): value is BoltYieldExpression;
export function isBoltMatchArm(value: any): value is BoltMatchArm;
export function isBoltMatchExpression(value: any): value is BoltMatchExpression;
export function isBoltCase(value: any): value is BoltCase;
export function isBoltCaseExpression(value: any): value is BoltCaseExpression;
export function isBoltBlockExpression(value: any): value is BoltBlockExpression;
export function isBoltConstantExpression(value: any): value is BoltConstantExpression;
export function isBoltStatement(value: any): value is BoltStatement;
export function isBoltReturnStatement(value: any): value is BoltReturnStatement;
export function isBoltConditionalCase(value: any): value is BoltConditionalCase;
export function isBoltConditionalStatement(value: any): value is BoltConditionalStatement;
export function isBoltResumeStatement(value: any): value is BoltResumeStatement;
export function isBoltExpressionStatement(value: any): value is BoltExpressionStatement;
export function isBoltParameter(value: any): value is BoltParameter;
export function isBoltDeclaration(value: any): value is BoltDeclaration;
export function isBoltTypeDeclaration(value: any): value is BoltTypeDeclaration;
export function isBoltModule(value: any): value is BoltModule;
export function isBoltDeclarationLike(value: any): value is BoltDeclarationLike;
export function isBoltFunctionBodyElement(value: any): value is BoltFunctionBodyElement;
export function isBoltFunctionDeclaration(value: any): value is BoltFunctionDeclaration;
export function isBoltVariableDeclaration(value: any): value is BoltVariableDeclaration;
export function isBoltImportSymbol(value: any): value is BoltImportSymbol;
export function isBoltPlainImportSymbol(value: any): value is BoltPlainImportSymbol;
export function isBoltImportDirective(value: any): value is BoltImportDirective;
export function isBoltExportSymbol(value: any): value is BoltExportSymbol;
export function isBoltPlainExportSymbol(value: any): value is BoltPlainExportSymbol;
export function isBoltExportDirective(value: any): value is BoltExportDirective;
export function isBoltTraitDeclaration(value: any): value is BoltTraitDeclaration;
export function isBoltImplDeclaration(value: any): value is BoltImplDeclaration;
export function isBoltTypeAliasDeclaration(value: any): value is BoltTypeAliasDeclaration;
export function isBoltRecordMember(value: any): value is BoltRecordMember;
export function isBoltRecordField(value: any): value is BoltRecordField;
export function isBoltRecordDeclaration(value: any): value is BoltRecordDeclaration;
export function isBoltSourceElement(value: any): value is BoltSourceElement;
export function isBoltMacroCall(value: any): value is BoltMacroCall;
export function isJSSyntax(value: any): value is JSSyntax;
export function isJSToken(value: any): value is JSToken;
export function isJSOperator(value: any): value is JSOperator;
export function isJSIdentifier(value: any): value is JSIdentifier;
export function isJSString(value: any): value is JSString;
export function isJSInteger(value: any): value is JSInteger;
export function isJSFromKeyword(value: any): value is JSFromKeyword;
export function isJSReturnKeyword(value: any): value is JSReturnKeyword;
export function isJSTryKeyword(value: any): value is JSTryKeyword;
export function isJSFinallyKeyword(value: any): value is JSFinallyKeyword;
export function isJSCatchKeyword(value: any): value is JSCatchKeyword;
export function isJSImportKeyword(value: any): value is JSImportKeyword;
export function isJSAsKeyword(value: any): value is JSAsKeyword;
export function isJSConstKeyword(value: any): value is JSConstKeyword;
export function isJSLetKeyword(value: any): value is JSLetKeyword;
export function isJSExportKeyword(value: any): value is JSExportKeyword;
export function isJSFunctionKeyword(value: any): value is JSFunctionKeyword;
export function isJSWhileKeyword(value: any): value is JSWhileKeyword;
export function isJSForKeyword(value: any): value is JSForKeyword;
export function isJSOperator(value: any): value is JSOperator;
export function isJSCloseBrace(value: any): value is JSCloseBrace;
export function isJSCloseBracket(value: any): value is JSCloseBracket;
export function isJSCloseParen(value: any): value is JSCloseParen;
export function isJSOpenBrace(value: any): value is JSOpenBrace;
export function isJSOpenBracket(value: any): value is JSOpenBracket;
export function isJSOpenParen(value: any): value is JSOpenParen;
export function isJSSemi(value: any): value is JSSemi;
export function isJSComma(value: any): value is JSComma;
export function isJSDot(value: any): value is JSDot;
export function isJSDotDotDot(value: any): value is JSDotDotDot;
export function isJSMulOp(value: any): value is JSMulOp;
export function isJSAddOp(value: any): value is JSAddOp;
export function isJSDivOp(value: any): value is JSDivOp;
export function isJSSubOp(value: any): value is JSSubOp;
export function isJSLtOp(value: any): value is JSLtOp;
export function isJSGtOp(value: any): value is JSGtOp;
export function isJSBOrOp(value: any): value is JSBOrOp;
export function isJSBXorOp(value: any): value is JSBXorOp;
export function isJSBAndOp(value: any): value is JSBAndOp;
export function isJSBNotOp(value: any): value is JSBNotOp;
export function isJSNotOp(value: any): value is JSNotOp;
export function isJSPattern(value: any): value is JSPattern;
export function isJSBindPattern(value: any): value is JSBindPattern;
export function isJSExpression(value: any): value is JSExpression;
export function isJSConstantExpression(value: any): value is JSConstantExpression;
export function isJSMemberExpression(value: any): value is JSMemberExpression;
export function isJSCallExpression(value: any): value is JSCallExpression;
export function isJSBinaryExpression(value: any): value is JSBinaryExpression;
export function isJSUnaryExpression(value: any): value is JSUnaryExpression;
export function isJSNewExpression(value: any): value is JSNewExpression;
export function isJSSequenceExpression(value: any): value is JSSequenceExpression;
export function isJSConditionalExpression(value: any): value is JSConditionalExpression;
export function isJSLiteralExpression(value: any): value is JSLiteralExpression;
export function isJSReferenceExpression(value: any): value is JSReferenceExpression;
export function isJSSourceElement(value: any): value is JSSourceElement;
export function isJSFunctionBodyElement(value: any): value is JSFunctionBodyElement;
export function isJSStatement(value: any): value is JSStatement;
export function isJSCatchBlock(value: any): value is JSCatchBlock;
export function isJSTryCatchStatement(value: any): value is JSTryCatchStatement;
export function isJSExpressionStatement(value: any): value is JSExpressionStatement;
export function isJSConditionalCase(value: any): value is JSConditionalCase;
export function isJSConditionalStatement(value: any): value is JSConditionalStatement;
export function isJSReturnStatement(value: any): value is JSReturnStatement;
export function isJSParameter(value: any): value is JSParameter;
export function isJSDeclaration(value: any): value is JSDeclaration;
export function isJSImportBinding(value: any): value is JSImportBinding;
export function isJSImportStarBinding(value: any): value is JSImportStarBinding;
export function isJSImportAsBinding(value: any): value is JSImportAsBinding;
export function isJSImportDeclaration(value: any): value is JSImportDeclaration;
export function isJSFunctionDeclaration(value: any): value is JSFunctionDeclaration;
export function isJSArrowFunctionDeclaration(value: any): value is JSArrowFunctionDeclaration;
export function isJSLetDeclaration(value: any): value is JSLetDeclaration;
export function isJSSourceFile(value: any): value is JSSourceFile;
