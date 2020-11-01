
import {
  SyntaxKind,
  kindToString,
  BoltToken,
  BoltIdentifier,
  BoltConstantExpression,
  BoltReferenceExpression,
  BoltExpression,
  BoltRecordDeclaration,
  BoltStatement,
  BoltDeclaration,
  BoltParameter,
  BoltSourceElement,
  createBoltQualName,
  BoltPattern,
  createBoltBindPattern,
  BoltImportDirective,
  BoltTypeExpression,
  createBoltReferenceTypeExpression,
  createBoltConstantExpression,
  createBoltReferenceExpression,
  createBoltParameter,
  BoltBindPattern,
  createBoltRecordDeclaration,
  createBoltImportDirective,
  BoltModifiers,
  BoltStringLiteral,
  BoltExpressionStatement,
  createBoltExpressionStatement,
  BoltVariableDeclaration,
  BoltSyntax,
  createBoltVariableDeclaration,
  BoltReturnStatement,
  createBoltReturnStatement,
  BoltModule,
  createBoltModule,
  BoltTypeAliasDeclaration,
  createBoltTypeAliasDeclaration,
  BoltFunctionDeclaration,
  createBoltFunctionDeclaration,
  createBoltCallExpression,
  BoltSymbol,
  BoltTypeParameter,
  createBoltTypeParameter,
  BoltTraitDeclaration,
  createBoltTraitDeclaration,
  createBoltImplDeclaration,
  BoltImplDeclaration,
  BoltSourceFile,
  BoltFunctionBodyElement,
  createBoltSourceFile,
  BoltMatchExpression,
  createBoltMatchArm,
  BoltMatchArm,
  createBoltMatchExpression,
  createBoltExpressionPattern,
  BoltFunctionTypeExpression,
  BoltReferenceTypeExpression,
  createBoltFunctionTypeExpression,
  BoltRecordPattern,
  createBoltRecordPattern,
  createBoltRecordFieldPattern,
  isBoltPunctuated,
  Token,
  createBoltQuoteExpression,
  BoltQuoteExpression,
  BoltBlockExpression,
  createBoltBlockExpression,
  isBoltOperatorLike,
  BoltFunctionExpression,
  createBoltFunctionExpression,
  BoltMacroCall,
  createBoltMacroCall,
  createBoltMemberExpression,
  BoltDeclarationLike,
  BoltTraitOrImplElement,
  BoltQualName,
  BoltLoopStatement,
  createBoltLoopStatement,
  createBoltRecordDeclarationField,
  createBoltRecordExpression,
  BoltRecordExpressionElement,
  createBoltRecordFieldValue,
  BoltRecordExpression,
  BoltForKeyword,
  BoltBraced,
  BoltAssignStatement,
  createBoltAssignStatement,
  BoltExportDirective,
  BoltParenthesized,
  createBoltCaseStatementCase,
  createBoltCaseStatement,
  createBoltElseKeyword,
} from "./ast"

import { parseForeignLanguage } from "./foreign"

import { 
  OperatorKind,
  OperatorTable,
  assertToken,
  ParseError,
  setOrigNodeRange,
  createTokenStream,
} from "./common"
import { Stream, uniq, assert, isInsideDirectory, first } from "./util"

import { Scanner } from "./scanner"
import { TextSpan, TextPos } from "./text"
import {JSScanner} from "./foreign/js/scanner";
import { Package } from "./package"
import { BoltCaseStatement, Syntax } from "./ast-spec"
import { resourceUsage } from "process"

export type BoltTokenStream = Stream<BoltToken>;

export function isModifierKeyword(kind: SyntaxKind) {
  return kind === SyntaxKind.BoltPubKeyword
      || kind === SyntaxKind.BoltForeignKeyword;
}

function assertNoTokens(tokens: BoltTokenStream) {
  const t0 = tokens.peek(1);
  if (t0.kind !== SyntaxKind.EndOfFile) {
    throw new ParseError(t0, [SyntaxKind.EndOfFile]);
  }
}

const KIND_OPERATOR = [
  SyntaxKind.BoltOperator,
  SyntaxKind.BoltVBar,
  SyntaxKind.BoltLtSign,
  SyntaxKind.BoltGtSign,
];

const KIND_EXPRESSION_T0 = uniq([
  SyntaxKind.BoltStringLiteral,
  SyntaxKind.BoltIntegerLiteral,
  SyntaxKind.BoltOperator,
  SyntaxKind.BoltMatchKeyword,
  SyntaxKind.BoltQuoteKeyword,
  SyntaxKind.BoltYieldKeyword,
  SyntaxKind.BoltIdentifier,
  SyntaxKind.BoltParenthesized,
])

const KIND_STATEMENT_T0 = uniq([
  SyntaxKind.BoltReturnKeyword,
  SyntaxKind.BoltIfKeyword,
  ...KIND_EXPRESSION_T0,
])

const KIND_DECLARATION_KEYWORD = [
  SyntaxKind.BoltImplKeyword,
  SyntaxKind.BoltTraitKeyword,
  SyntaxKind.BoltFnKeyword,
  SyntaxKind.BoltEnumKeyword,
  SyntaxKind.BoltLetKeyword,
  SyntaxKind.BoltModKeyword,
  SyntaxKind.BoltStructKeyword,
  SyntaxKind.BoltTypeKeyword,
]

const KIND_DECLARATION_T0 = uniq([
  SyntaxKind.BoltPubKeyword,
  SyntaxKind.BoltForeignKeyword,
  ...KIND_DECLARATION_KEYWORD,
])

const KIND_SOURCEELEMENT_T0 = uniq([
  SyntaxKind.BoltModKeyword,
  SyntaxKind.BoltImportKeyword,
  ...KIND_EXPRESSION_T0,
  ...KIND_STATEMENT_T0,
  ...KIND_DECLARATION_T0,
])

function isRightAssoc(kind: OperatorKind): boolean {
  return kind === OperatorKind.InfixR;
}

export class Parser {

  private exprOperatorTable = new OperatorTable([
    [
      [OperatorKind.InfixL, 2, '==']
    ],
    [
      [OperatorKind.InfixL, 2, '&&'],
      [OperatorKind.InfixL, 2, '||']
    ],
    [
      [OperatorKind.InfixL, 2, '<'],
      [OperatorKind.InfixL, 2, '>'],
      [OperatorKind.InfixL, 2, '<='],
      [OperatorKind.InfixL, 2, '>=']
    ],
    [
      [OperatorKind.InfixL, 2, '>>'],
      [OperatorKind.InfixL, 2, '<<']
    ],
    [
      [OperatorKind.InfixL, 2, '+'],
      [OperatorKind.InfixL, 2, '-'],
    ],
    [
      [OperatorKind.InfixL, 2, '/'],
      [OperatorKind.InfixL, 2, '*'],
      [OperatorKind.InfixL, 2, '%'],
    ],
    [
      [OperatorKind.Prefix, 1, '!']
    ],
  ]);

  private typeOperatorTable = new OperatorTable([
    [
      [OperatorKind.InfixL, 2, '|'],
    ]
  ]);

  public parse(kind: SyntaxKind, tokens: BoltTokenStream): BoltSyntax {
    return (this as any)['parse' + kindToString(kind).substring('Bolt'.length)](tokens);
  }

  // private parseModulePath(tokens: BoltTokenStream): BoltModulePath | null {

  //   let firstToken = tokens.peek();;
  //   let lastToken: Token;
  //   let isAbsolute = false;
  //   let elements = [];

  //   const t0 = tokens.peek();
  //   if (t0.kind === SyntaxKind.BoltColonColon) {
  //     isAbsolute = true;
  //     tokens.get();
  //     lastToken = t0;
  //   }

  //   if (tokens.peek(2).kind === SyntaxKind.BoltColonColon) {
  //     while (true) {
  //       const t1 = tokens.get();
  //       assertToken(t1, SyntaxKind.BoltIdentifier);
  //       elements.push(t1 as BoltIdentifier)
  //       const t2 = tokens.get();
  //       if (tokens.peek(2).kind !== SyntaxKind.BoltColonColon) {
  //         lastToken = t2;
  //         break;
  //       }
  //     }
  //   }

  //   if (!isAbsolute && elements.length === 0) {
  //     return null;
  //   }
  //   const result = createBoltModulePath(isAbsolute, elements);
  //   setOrigNodeRange(result, firstToken, lastToken!);
  //   return result;
  // }

  public parseQualName(tokens: BoltTokenStream): BoltQualName {

    const firstToken = tokens.peek();
    let isAbsolute = false;
    let modulePath = [];
    let name;
    if (tokens.peek().kind === SyntaxKind.BoltColonColon) {
      isAbsolute = true;
      tokens.get();
    }

    while (true) {
      const t1 = tokens.get();
      if (tokens.peek().kind === SyntaxKind.BoltColonColon) {
        assertToken(t1, SyntaxKind.BoltIdentifier);
        modulePath.push(t1 as BoltIdentifier);
        tokens.get();
      } else {
        if (t1.kind === SyntaxKind.BoltParenthesized) {
          const innerTokens = createTokenStream(t1);
          const t2 = innerTokens.get();
          if (!isBoltOperatorLike(t2)) {
            throw new ParseError(t2, KIND_OPERATOR);
          }
          assertNoTokens(innerTokens);
          name = t2;
        } else if (t1.kind === SyntaxKind.BoltIdentifier) {
          name = t1;
        } else {
          throw new ParseError(t1, [SyntaxKind.BoltParenthesized, SyntaxKind.BoltIdentifier]);
        }
        break;
      }
    }

    const node = createBoltQualName(isAbsolute, modulePath, name as BoltIdentifier, null);
    setOrigNodeRange(node, firstToken, name);
    return node;
  }

  public parseBindPattern(tokens: BoltTokenStream): BoltBindPattern {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltIdentifier);
    const node = createBoltBindPattern(t0 as BoltIdentifier);
    setOrigNodeRange(node, t0, t0);
    return node;
  }

  public parseRecordPattern(tokens: BoltTokenStream): BoltRecordPattern {

    const name = this.parseTypeExpression(tokens);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltBraced);

    const innerTokens = createTokenStream(t1);
    const members = [];

    while (true) {

      let t0 = innerTokens.get();
      const firstToken = t0;

      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }

      let isRest = false;
      let name = null;
      let pattern = null;
      if (t0.kind === SyntaxKind.BoltDotDot) {
        isRest = true;
        t0 = innerTokens.peek();
      }
      if (t0.kind === SyntaxKind.BoltIdentifier) {
        name = t0;
        t0 = innerTokens.peek();
      } else if (!isRest) {
        throw new ParseError(t0, [SyntaxKind.BoltIdentifier]);
      }
      if (t0.kind === SyntaxKind.BoltEqSign) {
        pattern = this.parsePattern(innerTokens);
      }
      let member = createBoltRecordFieldPattern(isRest, name, pattern);
      setOrigNodeRange(member, firstToken, t0);
      members.push(member);

      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      assertToken(t0, SyntaxKind.BoltComma);

    }

    const result = createBoltRecordPattern(name, members);
    setOrigNodeRange(result, name, t1);
    return result;
  }

  public parsePattern(tokens: BoltTokenStream): BoltPattern {
    const t0 = tokens.peek(1);
    const t1 = tokens.peek(2);
    if (t0.kind === SyntaxKind.BoltIdentifier && t1.kind === SyntaxKind.BoltBraced) {
      return this.parseRecordPattern(tokens);
    } else if (t0.kind === SyntaxKind.BoltIdentifier) {
      return this.parseBindPattern(tokens);
    } else if (t0.kind === SyntaxKind.BoltOperator && t0.text === '^') {
      tokens.get();
      const refExpr = this.parseReferenceExpression(tokens);
      const result = createBoltExpressionPattern(refExpr);
      setOrigNodeRange(result, t0, refExpr);
      return result;
    } else if (KIND_EXPRESSION_T0.indexOf(t0.kind) !== -1) {
      const expr = this.parseExpression(tokens);
      const result = createBoltExpressionPattern(expr);
      setOrigNodeRange(result, expr, expr);
      return result;
    } else {
      const expected = KIND_EXPRESSION_T0.slice();
      expected.push(SyntaxKind.BoltOperator);
      throw new ParseError(t0, expected)
    }
  }

  public parseImportDirective(tokens: BoltTokenStream): BoltImportDirective {

    let modifiers = 0;
    if (tokens.peek().kind === SyntaxKind.BoltPubKeyword) {
      tokens.get();
      modifiers |= BoltModifiers.IsPublic;
    }

    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltImportKeyword);

    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltStringLiteral);

    const symbols = null;
    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltParenthesized) {
      // TODO implement grammar and parsing logic for symbols
    }

    const node = createBoltImportDirective(modifiers, t1 as BoltStringLiteral, symbols);
    setOrigNodeRange(node, t0, t1);
    return node;
  }

  public parseFunctionTypeExpression(tokens: BoltTokenStream): BoltFunctionTypeExpression {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltFnKeyword);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltParenthesized);
    const innerTokens = createTokenStream(t1);
    let i = 0;
    const params: BoltParameter[] = [];
    while (true) {
      const t2 = innerTokens.peek();
      if (t2.kind === SyntaxKind.EndOfFile) {
        break;
      }
      const param = this.parseParameter(innerTokens, i++)
      params.push(param);
      const t3 = innerTokens.peek()
      if (t3.kind === SyntaxKind.EndOfFile) {
        break;
      }
      const t4 = innerTokens.get();
      assertToken(t4, SyntaxKind.BoltComma);
    }
    const t3 = tokens.peek();
    let returnType = null;
    if (t3.kind === SyntaxKind.BoltRArrow) {
      tokens.get();
      returnType = this.parseTypeExpression(tokens);
    }
    const result = createBoltFunctionTypeExpression(params, returnType)
    setOrigNodeRange(result, t0, returnType !== null ? returnType : t1);
    return result;
  }

  public parseReferenceTypeExpression(tokens: BoltTokenStream): BoltReferenceTypeExpression {

    const firstToken = tokens.peek();
    let isAbsolute = false;
    let modulePath = [];
    let name;

    if (tokens.peek().kind === SyntaxKind.BoltColonColon) {
      isAbsolute = true;
      tokens.get();
    }

    while (true) {
      const t1 = tokens.get();
      if (tokens.peek().kind === SyntaxKind.BoltColonColon) {
        assertToken(t1, SyntaxKind.BoltIdentifier);
        modulePath.push(t1 as BoltIdentifier);
        tokens.get();
      } else {
        assertToken(t1, SyntaxKind.BoltIdentifier);
        name = t1 as BoltIdentifier;
        break;
      }
    }

    let lastToken: BoltToken = name;
    let typeArgs: BoltTypeExpression[] | null = null;

    if (tokens.peek().kind === SyntaxKind.BoltLtSign) {
      typeArgs = [];
      tokens.get();
      let first = true;
      while (true) {
        const t2 = tokens.peek();
        if (t2.kind === SyntaxKind.BoltGtSign) { 
          break;
        }
        if (first) {
          first = false;
        } else {
          assertToken(t2, SyntaxKind.BoltComma);
          tokens.get();
        }
        typeArgs!.push(this.parseTypeExpression(tokens));
      }
      const t4 = tokens.get();
      assertToken(t4, SyntaxKind.BoltGtSign);
      lastToken = t4;
    }

    const qualName = createBoltQualName(isAbsolute, modulePath, name);
    setOrigNodeRange(qualName, firstToken, modulePath.length > 0 ? modulePath[modulePath.length-1] : firstToken)
    const node = createBoltReferenceTypeExpression(qualName, typeArgs);
    setOrigNodeRange(node, firstToken, lastToken);
    return node;
  }

  private parsePrimTypeExpression(tokens: BoltTokenStream): BoltTypeExpression {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltFnKeyword) {
      return this.parseFunctionTypeExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltIdentifier) {
      return this.parseReferenceTypeExpression(tokens);
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier]);
    }
  }

  //private parseTypeExpressionOperators(tokens: BoltTokenStream, lhs: BoltTypeExpression, minPrecedence: number): BoltTypeExpression {
  //  while (true) {
  //    const t0 = tokens.peek();
  //    if (!isBoltOperatorLike(t0)) {
  //      break;
  //    }
  //    let desc0 = this.typeOperatorTable.lookup(emitNode(t0));
  //    if (desc0 === null || desc0.arity !== 2 || desc0.precedence < minPrecedence) {
  //      break;
  //    }
  //    tokens.get();
  //    let rhs = this.parsePrimTypeExpression(tokens);
  //    while (true) {
  //      const t1 = tokens.peek()
  //      if (!isBoltOperatorLike(t1.kind)) {
  //        break;
  //      }
  //      const desc1 = this.typeOperatorTable.lookup(emitNode(t1))
  //      if (desc1 === null || desc1.arity !== 2 || desc1.precedence < desc0.precedence || !isRightAssoc(desc1.kind)) {
  //        break;
  //      }
  //      rhs = this.parseTypeExpressionOperators(tokens, rhs, desc1.precedence);
  //    }
  //    const name = emitNode(t0);
  //    switch (name) {
  //      case '|':
  //        return createBoltFunctionTypeExpression();
  //        )
  //    }
  //    lhs = createBoltReferenceTypeExpression(null, t0, [lhs, rhs]);
  //    setOrigNodeRange(lhs, t0, rhs);
  //  }
  //  return lhs
  //}

  public parseTypeExpression(tokens: BoltTokenStream) {
    //const lhs = this.parsePrimTypeExpression(tokens);
    //return this.parseTypeExpressionOperators(tokens, lhs, 0);
    return this.parsePrimTypeExpression(tokens);
  }

  public parseConstantExpression(tokens: BoltTokenStream): BoltConstantExpression {
    const t0 = tokens.get();
    let value: boolean | string | bigint;
    if (t0.kind === SyntaxKind.BoltStringLiteral) {
      value = t0.value;
    } else if (t0.kind === SyntaxKind.BoltIntegerLiteral) {
      value = t0.value;
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltStringLiteral, SyntaxKind.BoltIntegerLiteral]);
    }
    const node = createBoltConstantExpression(value);
    setOrigNodeRange(node, t0, t0);
    return node;
  }

  public parseFunctionExpression(tokens: BoltTokenStream): BoltFunctionExpression {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltVBar);
    let i = 0;
    const params: BoltParameter[] = [];
    while (true) {
      const t1 = tokens.peek();
      if (t1.kind === SyntaxKind.BoltVBar) {
        tokens.get();
        break;
      }
      const param = this.parseParameter(tokens, i++);
      params.push(param);
      const t2 = tokens.peek();
      if (t2.kind === SyntaxKind.BoltVBar) {
        tokens.get();
        break;
      }
      const t3 = tokens.get();
      assertToken(t3, SyntaxKind.BoltComma);
    }
    let returnType = null;
    let t4 = tokens.get();
    if (t4.kind === SyntaxKind.BoltRArrow) {
      returnType = this.parseTypeExpression(tokens);
      t4 = tokens.get();
    }
    assertToken(t4, SyntaxKind.BoltBraced);
    const innerTokens = createTokenStream(t4);
    const body = this.parseFunctionBodyElements(innerTokens);
    const result = createBoltFunctionExpression(params, returnType, body);
    setOrigNodeRange(result, t0, t4);
    return result;
  }

  public parseReferenceExpression(tokens: BoltTokenStream): BoltReferenceExpression {
    const firstToken = tokens.peek();
    const name = this.parseQualName(tokens);
    const node = createBoltReferenceExpression(name);
    setOrigNodeRange(node, firstToken, name);
    return node;
  }

  public parseMatchExpression(tokens: BoltTokenStream): BoltMatchExpression {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltMatchKeyword);
    const expr = this.parseExpression(tokens);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltBraced);
    const innerTokens = createTokenStream(t1);
    const matchArms: BoltMatchArm[] = [];
    while (true) {
      const t2 = innerTokens.peek();
      if (t2.kind === SyntaxKind.EndOfFile) {
        break;
      }
      const pattern = this.parsePattern(innerTokens);
      const t3 = innerTokens.get();
      assertToken(t3, SyntaxKind.BoltRArrowAlt);
      const expression = this.parseExpression(innerTokens);
      const arm = createBoltMatchArm(pattern, expression);
      setOrigNodeRange(arm, pattern, expression);
      matchArms.push(arm);
      const t4 = innerTokens.peek();
      if (t4.kind === SyntaxKind.EndOfFile) {
        break;
      }
      assertToken(t4, SyntaxKind.BoltComma);
      innerTokens.get();
    }
    const result = createBoltMatchExpression(expr, matchArms);
    setOrigNodeRange(result, t0, t1);
    return result;
  }

  public parseQuoteExpression(tokens: BoltTokenStream): BoltQuoteExpression {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltQuoteKeyword);
    let t1 = tokens.get();
    let target = "Bolt";
    if (t1.kind === SyntaxKind.BoltStringLiteral) {
      target = t1.value;
      t1 = tokens.get();
    }
    if (!isBoltPunctuated(t1)) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced, SyntaxKind.BoltParenthesized, SyntaxKind.BoltBracketed]);
    }
    let scanner;
    switch (target) {
      case "Bolt":
        scanner = new Scanner(t1.span!.file, t1.text, t1.span!.start.clone());
        break;
      case "JS":
        scanner = new JSScanner(t1.span!.file, t1.text, t1.span!.start.clone());
        break;
      default:
        throw new Error(`Unrecognised language.`);
    }
    const scanned: Token[] = [];
    while (true) {
      const t2 = scanner.scan();
      if (t2.kind === SyntaxKind.EndOfFile) {
        break;
      }
      scanned.push(t2);
    }
    const result = createBoltQuoteExpression(scanned as Token[]);
    setOrigNodeRange(result, t0, t1);
    return result;
  }

  public parseBlockExpression(tokens: BoltTokenStream): BoltBlockExpression {
    const t0 = tokens.get();
    const innerTokens = createTokenStream(t0);
    const elements = this.parseFunctionBodyElements(innerTokens)
    const result = createBoltBlockExpression(elements);
    setOrigNodeRange(result, t0, t0);
    return result;
  }

  private parseRecordExpression(tokens: BoltTokenStream): BoltRecordExpression {
    const typeRef = this.parseReferenceTypeExpression(tokens);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltBraced);
    const innerTokens = createTokenStream(t1);
    const fields: BoltRecordExpressionElement[] = [];
    while (true) {
      let name;
      let value = null;
      const t1 = innerTokens.get();
      if (t1.kind === SyntaxKind.EndOfFile) {
        break;
      }
      assertToken(t1, SyntaxKind.BoltIdentifier);
      name = t1 as BoltIdentifier
      const t2 = innerTokens.peek();
      if (t2.kind === SyntaxKind.BoltColon) {
        innerTokens.get();
        value = this.parseExpression(innerTokens);
      }
      const t3 = innerTokens.peek();
      if (t3.kind === SyntaxKind.BoltComma) {
        innerTokens.get();
      } else {
        assertToken(t3, SyntaxKind.EndOfFile)
      }
      const element = createBoltRecordFieldValue(name, value); 
      fields.push(element);
    }
    const result = createBoltRecordExpression(
      typeRef,
      fields
    );
    setOrigNodeRange(result, typeRef, t1)
    return result;
  }

  private getTokenAfterTypeRef(tokens: BoltTokenStream, i = 1): number {

    // Peek actual qualified name

    let t0 = tokens.peek(i);
    if (t0.kind === SyntaxKind.BoltColonColon) {
      i++;
      t0 = tokens.peek(i);
    }
    if (t0.kind !== SyntaxKind.BoltIdentifier) {
      return -1;
    }
    i++;
    while (tokens.peek(i).kind === SyntaxKind.BoltColonColon) {
      i++;
      if (tokens.peek(i).kind !== SyntaxKind.BoltIdentifier) {
        return -1;
      }
    }

    // Peek anything that comes between '<' and '>'

    const t2 = tokens.peek(i);
    if (t2.kind === SyntaxKind.BoltLtSign) {
      let count = 1;
      i++;
      while (count > 0) {
        const t3 = tokens.peek(i++);
        if (t3.kind === SyntaxKind.BoltLtSign) {
          count++;
        }
        if (t3.kind === SyntaxKind.BoltGtSign) {
          count--;
        }
      }
    }

    // Return wherever position we landed

    return i;

  }

  public parseExpression(tokens: BoltTokenStream) {
    return this.parseBinaryExpression(tokens, this.parseUnaryExpression(tokens), 0);
  }

  private parseUnaryExpression(tokens: BoltTokenStream) {
    return this.parseExpressionPrimitive(tokens);
  }

  private parseBinaryExpression(tokens: BoltTokenStream, lhs: BoltExpression, minPrecedence: number) {
    let lookahead = tokens.peek(1);
    while (true) {
      if (lookahead.kind !== SyntaxKind.BoltOperator) {
        break;
      }
      const lookaheadDesc = this.exprOperatorTable.lookup(2, lookahead.text);
      if (lookaheadDesc === null || lookaheadDesc.precedence < minPrecedence) {
        break;
      }
      const op = lookahead;
      const opDesc = this.exprOperatorTable.lookup(2, op.text);
      if (opDesc === null) {
        break;
      }
      tokens.get();
      let rhs = this.parseUnaryExpression(tokens)
      lookahead = tokens.peek()
      while (lookaheadDesc.arity === 2 
          && ((lookaheadDesc.precedence > opDesc.precedence)
            || lookaheadDesc.kind === OperatorKind.InfixR && lookaheadDesc.precedence === opDesc.precedence)) {
          rhs = this.parseBinaryExpression(tokens, rhs, lookaheadDesc.precedence)
          lookahead = tokens.peek();
      }
      lookahead = tokens.peek();
      const qualName = createBoltQualName(false, [], op);
      setOrigNodeRange(qualName, op, op);
      const refExpr = createBoltReferenceExpression(qualName);
      setOrigNodeRange(refExpr, op, op);
      const binExpr = createBoltCallExpression(refExpr, [lhs, rhs]);
      setOrigNodeRange(binExpr, lhs, rhs);
      lhs = binExpr;
    }
    return lhs
  }

  private parseExpressionPrimitive(tokens: BoltTokenStream): BoltExpression {

    try {
      const forked = tokens.fork();
      const recordExpr = this.parseRecordExpression(forked);
      tokens.join(forked);
      return recordExpr;
    } catch (e) {
      if (!(e instanceof ParseError)) {
        throw e;
      }
    }

    const t0 = tokens.peek();

    let result;
    if (t0.kind === SyntaxKind.BoltVBar) {
      result = this.parseFunctionExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltParenthesized) {
      tokens.get();
      const innerTokens = createTokenStream(t0);
      result = this.parseExpression(innerTokens);
      setOrigNodeRange(result, t0, t0);
    } else if (t0.kind === SyntaxKind.BoltBraced) {
      result = this.parseBlockExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltQuoteKeyword) {
      result = this.parseQuoteExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltMatchKeyword) {
      result = this.parseMatchExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltIntegerLiteral || t0.kind === SyntaxKind.BoltStringLiteral) {
      result = this.parseConstantExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltIdentifier) {
      result = this.parseReferenceExpression(tokens);
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltStringLiteral, SyntaxKind.BoltIdentifier]);
    }

    while (true) {

      // FIXME The following expression is incorrectly parsed: 0..fac()
    
      let t2 = tokens.peek();
      const firstToken = t2;

      // Parse all path elements of the member expression: a.foo.bar

      const path: BoltIdentifier[] = [];

      while (t2.kind === SyntaxKind.BoltDot) {
        tokens.get();
        const t3 = tokens.get();
        assertToken(t3, SyntaxKind.BoltIdentifier);
        path.push(t3 as BoltIdentifier);
        t2 = tokens.peek();
      }

      if (path.length > 0) {
        const node = createBoltMemberExpression(result, path);
        setOrigNodeRange(node, firstToken, t2);
        result = node;
      }

      if (t2.kind !== SyntaxKind.BoltParenthesized) {
        break;
      }

      tokens.get();

      const args: BoltExpression[] = []
      const innerTokens = createTokenStream(t2);

      while (true) {
        const t3 = innerTokens.peek();
        if (t3.kind === SyntaxKind.EndOfFile) {
          break; 
        }
        args.push(this.parseExpression(innerTokens))
        const t4 = innerTokens.get();
        if (t4.kind === SyntaxKind.EndOfFile) {
          break
        }
        if (t4.kind !== SyntaxKind.BoltComma) {
          throw new ParseError(t4, [SyntaxKind.BoltComma])
        }
      }

      const node = createBoltCallExpression(result, args, null)
      setOrigNodeRange(node, result, t2);
      result = node;

    }

    return result;

  }

  public parseParameter(tokens: BoltTokenStream, index: number): BoltParameter {

    let defaultValue = null;
    let typeDecl = null;

    const pattern = this.parsePattern(tokens)

    let t0 = tokens.peek();
    let endNode: BoltSyntax = pattern;
    if (t0.kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeDecl = this.parseTypeExpression(tokens);
      endNode = typeDecl;
      t0 = tokens.peek();
    }
    if (t0.kind === SyntaxKind.BoltEqSign) {
      tokens.get();
      defaultValue = this.parseExpression(tokens);
      endNode = defaultValue;
    }

    const node = createBoltParameter(index, pattern, typeDecl, defaultValue)
    setOrigNodeRange(node, pattern, endNode);
    return node;

  }

  public parseVariableDeclaration(tokens: BoltTokenStream): BoltVariableDeclaration {

    let modifiers = 0;
    let typeDecl = null;
    let value = null;

    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltLetKeyword);

    const t1 = tokens.peek();
    if (t1.kind === SyntaxKind.BoltMutKeyword) {
      tokens.get();
      modifiers |= BoltModifiers.IsMutable;
    }

    const bindings = this.parsePattern(tokens)

    let t2 = tokens.peek();
    let lastNode: BoltSyntax = bindings;

    if (t2.kind === SyntaxKind.BoltColon) {
      tokens.get();
      lastNode = typeDecl = this.parseTypeExpression(tokens);
      t2 = tokens.peek();
    }

    if (t2.kind === SyntaxKind.BoltEqSign) {
      tokens.get();
      lastNode = value = this.parseExpression(tokens);
    }

    const node = createBoltVariableDeclaration(modifiers, bindings, typeDecl, value)
    setOrigNodeRange(node, t0, lastNode);
    return node;
  }

  public parseReturnStatement(tokens: BoltTokenStream): BoltReturnStatement {

    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltReturnKeyword);

    let expr = null;

    const t1 = tokens.peek();
    if (t1.kind !== SyntaxKind.EndOfFile) { 
      expr = this.parseExpression(tokens)
    }

    const node = createBoltReturnStatement(expr);
    setOrigNodeRange(node, t0, expr !== null ? expr : t0);
    return node;
  }

  protected isUnaryOperator(name: string) {
    // TODO
    return false;
  }

  protected lookaheadHasExpression(tokens: BoltTokenStream, i = 1): boolean {
    const t0 = tokens.peek(i);
    if (t0.kind === SyntaxKind.BoltParenthesized) {
      return this.lookaheadHasExpression(tokens, i+1);
    }
    return t0.kind === SyntaxKind.BoltIdentifier
        || t0.kind === SyntaxKind.BoltStringLiteral
        || t0.kind === SyntaxKind.BoltIntegerLiteral
        || (t0.kind === SyntaxKind.BoltOperator && this.isUnaryOperator(t0.text));
  }

  public parseExpressionStatement(tokens: BoltTokenStream): BoltExpressionStatement {
    const expression = this.parseExpression(tokens)
    const node = createBoltExpressionStatement(expression)
    setOrigNodeRange(node, expression, expression);
    return node;
  }

  public parseAsssignStatement(tokens: BoltTokenStream): BoltAssignStatement {
    const pattern = this.parsePattern(tokens);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltEqSign);
    const expr = this.parseExpression(tokens);
    const result = createBoltAssignStatement(pattern, expr);
    setOrigNodeRange(result, pattern, expr);
    return result;
  }

  public parseLoopStatement(tokens: BoltTokenStream): BoltLoopStatement {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltLoopKeyword);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltBraced);
    const innerTokens = createTokenStream(t1);
    const elements = this.parseFunctionBodyElements(innerTokens);
    const result = createBoltLoopStatement(elements);
    setOrigNodeRange(result, t0, t1);
    return result;
  }

  public parseStatement(tokens: BoltTokenStream): BoltStatement {

    try {
      const forked = tokens.fork();
      const result = this.parseAsssignStatement(forked);
      tokens.join(forked);
      return result;
    } catch (e) {
      if (!(e instanceof ParseError)) {
        throw e;
      }
    }

    // if (this.lookaheadIsMacroCall(tokens)) {
    //   return this.parseMacroCall(tokens);
    // }
    const t0 = tokens.peek();
    if (KIND_EXPRESSION_T0.indexOf(t0.kind) !== -1) {
      return this.parseExpressionStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltReturnKeyword) {
      return this.parseReturnStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltLoopKeyword) {
      return this.parseLoopStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltIfKeyword) {
      return this.parseIfStatement(tokens);
    } else {
      throw new ParseError(t0, KIND_STATEMENT_T0);
    }
  }

  public parseIfStatement(tokens: BoltTokenStream): BoltCaseStatement {

    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltIfKeyword);

    const test = this.parseExpression(tokens);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltBraced);
    const bodyTokens = createTokenStream(t1);
    const body = this.parseFunctionBodyElements(bodyTokens);
    const firstCase = createBoltCaseStatementCase(test, body)
    setOrigNodeRange(firstCase, t0, t1);
    const cases = [ firstCase ];

    let lastToken = t1;

    while (true) {
      const t2 = tokens.peek(1);
      const t3 = tokens.peek(2);
      if (t2.kind !== SyntaxKind.BoltElseKeyword || t3.kind !== SyntaxKind.BoltIfKeyword) {
        break;
      }
      tokens.get();
      tokens.get();
      const test = this.parseExpression(tokens);
      const t4 = tokens.get();
      assertToken(t4, SyntaxKind.BoltBraced);
      const bodyTokens = createTokenStream(t4);
      const body = this.parseFunctionBodyElements(bodyTokens);
      const altCase = createBoltCaseStatementCase(test, body);
      setOrigNodeRange(altCase, t2, t4);
      cases.push(altCase);
      lastToken = t4;
    }

    let alternative = null;
    const t4 = tokens.peek();
    if (t4.kind === SyntaxKind.BoltElseKeyword) {
      tokens.get();
      const t5 = tokens.get();
      assertToken(t5, SyntaxKind.BoltBraced);
      const alternativeTokens = createTokenStream(t5);
      alternative = this.parseFunctionBodyElements(alternativeTokens)
      lastToken = t5;
    }

    const result = createBoltCaseStatement(cases, alternative);
    setOrigNodeRange(result, t0, lastToken);
    return result;
  }

  public parseGenericTypeParameter(tokens: BoltTokenStream): BoltTypeParameter {
    const t0 = tokens.peek();
    tokens.get();
    assertToken(t0, SyntaxKind.BoltIdentifier);
    let typeBound = null;
    const t1 = tokens.peek();
    if (t1.kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeBound = this.parseTypeExpression(tokens);
    }
    const node = createBoltTypeParameter(0, t0 as BoltIdentifier, typeBound, null)
    setOrigNodeRange(node, t0, t0);
    return node;
  }

  private parseGenericTypeParameters(tokens: BoltTokenStream): BoltTypeParameter[] {
    let typeParams: BoltTypeParameter[] = [];
    while (true) {
      let t1 = tokens.peek();
      if (t1.kind !== SyntaxKind.BoltIdentifier) {
        break;
      }
      typeParams.push(this.parseGenericTypeParameter(tokens));
      const t2 = tokens.peek();
      if (t2.kind !== SyntaxKind.BoltComma) {
        break;
      }
      tokens.get();
    }
    return typeParams;
  }

  public parseRecordDeclaration(tokens: BoltTokenStream): BoltRecordDeclaration {

    let modifiers = 0;
    let typeParams = null;

    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltModifiers.IsPublic;
      t0 = tokens.get();
    }

    assertToken(t0, SyntaxKind.BoltStructKeyword);

    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltIdentifier);
    const name = t1 as BoltIdentifier;

    let t2 = tokens.peek();

    if (t2.kind === SyntaxKind.EndOfFile) {
      const node = createBoltRecordDeclaration(modifiers, name, null, []);
      setOrigNodeRange(node, firstToken, t2);
      return node;
    }

    if (t2.kind === SyntaxKind.BoltLtSign) {
      tokens.get();
      typeParams = this.parseGenericTypeParameters(tokens);
      const t3 = tokens.get();
      assertToken(t3, SyntaxKind.BoltGtSign);
      t2 = tokens.peek();
    }

    let members = null;

    if (t2.kind !== SyntaxKind.BoltSemi) {

      if (t2.kind !== SyntaxKind.BoltBraced) {
        throw new ParseError(t2, [SyntaxKind.BoltBraced])
      }

      members = [];

      tokens.get();
      const innerTokens = createTokenStream(t2);

      while (true) {
        const t3 = innerTokens.peek();
        if (t3.kind === SyntaxKind.EndOfFile) {
          break;
        }
        assertToken(t3, SyntaxKind.BoltIdentifier);
        innerTokens.get();
        const name = t3 as BoltIdentifier;
        const t4 = innerTokens.get();
        assertToken(t4, SyntaxKind.BoltColon);
        const type = this.parseTypeExpression(innerTokens);
        const field = createBoltRecordDeclarationField(name as BoltIdentifier, type);
        const t5 = innerTokens.get();
        if (t5.kind === SyntaxKind.EndOfFile) {
          break;
        }
        assertToken(t5, SyntaxKind.BoltComma);
        setOrigNodeRange(field, name, type);
        members.push(field);
      }

    }

    const node = createBoltRecordDeclaration(modifiers, name, typeParams, members);
    setOrigNodeRange(node, firstToken, t2);
    return node;
  }

  public parseStatements(tokens: BoltTokenStream): BoltStatement[] {
    const statements: BoltStatement[] = [];
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (t0.kind === SyntaxKind.BoltSemi) {
        tokens.get();
        continue;
      }
      const statement = this.parseStatement(tokens);
      statements.push(statement);
    }
    return statements;
  }

  public parseModuleDeclaration(tokens: BoltTokenStream): BoltModule {

    let modifiers = 0;
    let pathElements = [];

    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltModifiers.IsPublic;
      t0 = tokens.get();
    }

    if (t0.kind !== SyntaxKind.BoltModKeyword) {
      throw new ParseError(t0, [SyntaxKind.BoltModKeyword])
    }

    while (true) {
      const t1 = tokens.get();
      assertToken(t1, SyntaxKind.BoltIdentifier)
      pathElements.push(t1 as BoltIdentifier)
      const t2 = tokens.peek();
      if (t2.kind !== SyntaxKind.BoltColonColon) {
        break;
      }
      tokens.get();
    }

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced])
    }
    const elements = this.parseSourceElements(createTokenStream(t1));

    const node = createBoltModule(modifiers, pathElements, elements);
    setOrigNodeRange(node, firstToken, t1);
    return node;
  }


  public parseTypeAliasDeclaration(tokens: BoltTokenStream): BoltTypeAliasDeclaration {

    let modifiers = 0;
    let typeParams = null;

    let t0 = tokens.get();
    const firstToken = t0;

    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltModifiers.IsPublic;
      t0 = tokens.get();
    }

    assertToken(t0, SyntaxKind.BoltTypeKeyword);

    const name = tokens.get();
    if (name.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(name, [SyntaxKind.BoltIdentifier])
    }

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltLtSign) {
      tokens.get();
      typeParams = this.parseGenericTypeParameters(tokens);
      const t3 = tokens.get();
      assertToken(t3, SyntaxKind.BoltGtSign);
    }

    const t3 = tokens.get();
    assertToken(t3, SyntaxKind.BoltEqSign);

    const typeExpr = this.parseTypeExpression(tokens);

    const node = createBoltTypeAliasDeclaration(modifiers, name, typeParams, typeExpr)
    setOrigNodeRange(node, firstToken, typeExpr);
    return node;
  }

  private parseFunctionDeclaration(tokens: BoltTokenStream): BoltFunctionDeclaration {

    let target = "Bolt";
    let modifiers = 0;

    let k0 = tokens.peek();
    const firstToken = k0;

    if (k0.kind === SyntaxKind.BoltPubKeyword) {
      tokens.get();
      modifiers |= BoltModifiers.IsPublic;
      k0 = tokens.peek();
    }

    if (k0.kind === SyntaxKind.BoltForeignKeyword) {
      tokens.get();
      const l1 = tokens.get();
      if (l1.kind !== SyntaxKind.BoltStringLiteral) {
        throw new ParseError(l1, [SyntaxKind.BoltStringLiteral])
      }
      target = l1.value;
      k0 = tokens.peek();
    }

    if (k0.kind !== SyntaxKind.BoltFnKeyword) {
      throw new ParseError(k0, [SyntaxKind.BoltFnKeyword])
    }

    tokens.get();

    let name: BoltSymbol;
    let returnType = null;
    let body: any = null; // FIXME type-checking should not be disabled
    let params: BoltParameter[] = [];
    let typeParams = null;

    // Parse parameters

    let i = 0;

    const t0 = tokens.peek(1);
    const t1 = tokens.peek(2);

    const isParamLike = (token: BoltToken) =>
        token.kind === SyntaxKind.BoltIdentifier || token.kind === SyntaxKind.BoltParenthesized;

    const parseParamLike = (tokens: BoltTokenStream) => {
      const t0 = tokens.peek(1);
      if (t0.kind === SyntaxKind.BoltIdentifier) {
        tokens.get();
        const bindings = createBoltBindPattern(t0 as BoltIdentifier);
        setOrigNodeRange(bindings, t0, t0);
        const param = createBoltParameter(i++, bindings, null, null);
        setOrigNodeRange(param, t0, t0);
        return param;
      } else if (t0.kind === SyntaxKind.BoltParenthesized) {
        tokens.get();
        const innerTokens = createTokenStream(t0);
        const param = this.parseParameter(innerTokens, i++)
        assertNoTokens(innerTokens);
        return param
      } else {
        throw new ParseError(t0, [SyntaxKind.BoltIdentifier, SyntaxKind.BoltParenthesized])
      }
    }

    if (t0.kind === SyntaxKind.BoltOperator) {

      name = t0;
      tokens.get();
      params.push(parseParamLike(tokens))

    } else if (isParamLike(t0) && t1.kind == SyntaxKind.BoltOperator) {

      params.push(parseParamLike(tokens));
      name = t1;
      while (true) {
        const t2 = tokens.peek();
        if (t2.kind !== SyntaxKind.BoltOperator) {
          break;
        }
        if (t2.text !== t1.text) {
          throw new Error(`Operators have to match when defining or declaring an n-ary operator.`);
        }
        tokens.get();
        params.push(parseParamLike(tokens))
      }

    } else if (t0.kind === SyntaxKind.BoltIdentifier) {

      name = t0;
      tokens.get();
      const t2 = tokens.get();
      if (t2.kind === SyntaxKind.BoltParenthesized) {
        const innerTokens = createTokenStream(t2);
        while (true) {
          const t3 = innerTokens.peek();
          if (t3.kind === SyntaxKind.EndOfFile) {
            break;
          }
          params.push(this.parseParameter(innerTokens, i++))
          const t4 = innerTokens.get();
          if (t4.kind === SyntaxKind.BoltComma) {
            continue;
          } else if (t4.kind === SyntaxKind.EndOfFile) {
            break;
          } else {
            throw new ParseError(t4, [SyntaxKind.BoltComma, SyntaxKind.EndOfFile])
          }
        }
      }

    } else {

      throw new ParseError(t0, [SyntaxKind.BoltIdentifier, SyntaxKind.BoltOperator, SyntaxKind.BoltParenthesized])

    }

    // Parse return type

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltRArrow) {
      tokens.get();
      returnType = this.parseTypeExpression(tokens);
    }

    // Parse second possible version of generic type parameters

    const t4 = tokens.peek();
    if (t4.kind === SyntaxKind.BoltWhereKeyword) {
      tokens.get();
      typeParams = this.parseGenericTypeParameters(tokens);
    }

    // Parse function body

    const t3 = tokens.peek();
    if (t3.kind === SyntaxKind.BoltBraced) {
      tokens.get();
      switch (target) {
        case "Bolt":
          body = this.parseFunctionBodyElements(createTokenStream(t3));
          break;
        default:
          body = parseForeignLanguage(target, t3.text, t3.span!.file, t3.span!.start);
          break;
      }
    } else if (t3.kind !== SyntaxKind.BoltSemi) {
      const expected = [ SyntaxKind.BoltBraced, SyntaxKind.BoltSemi ];
      if (returnType === null) {
        expected.push(SyntaxKind.BoltRArrow);
      }
      throw new ParseError(t3, expected);
    }

    const result = createBoltFunctionDeclaration(
      modifiers,
      target,
      name,
      params,
      returnType,
      typeParams,
      body
    );
    setOrigNodeRange(result, firstToken, t3);
    return result;

  }

  public parseTraitDeclaration(tokens: BoltTokenStream): BoltTraitDeclaration {

    let modifiers = 0;
    let typeParams = null;
    let name;
    let typeBoundExpr = null;
    let elements = null;

    // Parse the 'pub' keyword, if present
    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltModifiers.IsPublic;
      t0 = tokens.get();
    }

    // By now, we should encounter the 'trait' keyword'
    assertToken(t0, SyntaxKind.BoltTraitKeyword);

    // Type parameters are introduced by '<' and end with '>'
    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltLtSign) {
      tokens.get();
      typeParams = this.parseGenericTypeParameters(tokens);
      const t2 = tokens.get();
      assertToken(t2, SyntaxKind.BoltGtSign);
    }

    // A trait must be named by an identifier
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltIdentifier);
    name = t1 as BoltIdentifier;

    let lastToken = t1;

    if (tokens.peek().kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeBoundExpr = this.parseTypeExpression(tokens);
    }

    // The trait may optionally have 'fn ...' and 'type ..' elements wrapped in braces.
    const t3 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltBraced) {
      const t3 = tokens.get();
      lastToken = t3;
      const innerTokens = createTokenStream(t3);
      elements = this.parseTraitOrImplElements(innerTokens);
      assertNoTokens(innerTokens);
    }

    // Create and return the resulting AST node
    const result = createBoltTraitDeclaration(modifiers, typeParams, name, typeBoundExpr, elements);
    setOrigNodeRange(result, firstToken, lastToken);
    return result;
  }

  public parseImplDeclaration(tokens: BoltTokenStream): BoltImplDeclaration {

    let modifiers = 0;
    let typeParams = null;
    let traitTypeExpr;
    let typeExpr = null;

    // Parse the 'pub' keyword
    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltModifiers.IsPublic;
      t0 = tokens.get();
    }

    // By now, we should encounter the 'impl' keyword
    assertToken(t0, SyntaxKind.BoltImplKeyword);

    // Type parameters are introduced by '<' and end with '>'
    const t1 = tokens.peek();
    if (t1.kind === SyntaxKind.BoltLtSign) {
      typeParams = this.parseGenericTypeParameters(tokens);
      const t2 = tokens.get();
      assertToken(t2, SyntaxKind.BoltGtSign);
    }

    // Check for the 'for' keyword occuring before '{' .. '}'
    let i = 2;
    let foundForKeyword = false;
    while (true) {
      const tn = tokens.peek(i++);
      if (tn.kind === SyntaxKind.BoltBraced || tn.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (tn.kind === SyntaxKind.BoltForKeyword) {
        foundForKeyword = true;
      }
    }

    if (foundForKeyword) {

      // Parse the type expression that references the trait the user wants to implement
      traitTypeExpr = this.parseTypeExpression(tokens);

      // Skip the 'for' keyword itself
      assertToken(tokens.get(), SyntaxKind.BoltForKeyword);

      // Parse the type that this implementation is for
      typeExpr = this.parseTypeExpression(tokens);

    } else {

      // Just parse the trait the user wants to implement and leave the rest as is
      const resultTypeExpr = this.parseTypeExpression(tokens);

      // We cheat a bit by assigning the referenced trait to both fields
      // NOTE Assigning the same node by reference to different fields should be done with great care.
      typeExpr =  resultTypeExpr;
      traitTypeExpr = resultTypeExpr;

    }

    // Parse all 'fn ...' and 'type ...' elements
    const t5 = tokens.get();
    assertToken(t5, SyntaxKind.BoltBraced);
    const elements = this.parseTraitOrImplElements(createTokenStream(t5));

    // Create and return the result
    const result = createBoltImplDeclaration(modifiers, typeParams, typeExpr, traitTypeExpr, elements);
    setOrigNodeRange(result, firstToken, t5);
    return result;
  }

  public parseDeclarationLike(tokens: BoltTokenStream): BoltDeclarationLike {
    let t0 = tokens.peek(1);
    let i = 1;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      t0 = tokens.peek(++i);
      if (t0.kind !== SyntaxKind.BoltForeignKeyword) {
        if (KIND_DECLARATION_KEYWORD.indexOf(t0.kind) === -1) {
          throw new ParseError(t0, KIND_DECLARATION_KEYWORD);
        }
      }
    }
    if (t0.kind === SyntaxKind.BoltForeignKeyword) {
      i += 2;
      t0 = tokens.peek(i);
      if (KIND_DECLARATION_KEYWORD.indexOf(t0.kind) === -1) {
        throw new ParseError(t0, KIND_DECLARATION_KEYWORD);
      }
    }
    switch (t0.kind) {
      case SyntaxKind.BoltImplKeyword:
        return this.parseImplDeclaration(tokens);
      case SyntaxKind.BoltTraitKeyword:
        return this.parseTraitDeclaration(tokens);
      case SyntaxKind.BoltTypeKeyword:
        return this.parseTypeAliasDeclaration(tokens);
      case SyntaxKind.BoltFnKeyword:
        return this.parseFunctionDeclaration(tokens);
      case SyntaxKind.BoltLetKeyword:
        return this.parseVariableDeclaration(tokens);
      case SyntaxKind.BoltStructKeyword:
        return this.parseRecordDeclaration(tokens);
      case SyntaxKind.BoltStructKeyword:
        return this.parseVariableDeclaration(tokens);
      default:
        throw new ParseError(t0, KIND_DECLARATION_T0);
      }
  }

  private getFirstTokenAfterModifiers(tokens: BoltTokenStream): BoltToken {
    let mustBeDeclOrImport = false;
    let mustBeFunctionOrVariable = false;
    let i = 1;
    let t0 = tokens.peek(i);
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      mustBeDeclOrImport = true;
      t0 = tokens.peek(++i);
    }
    if (t0.kind === SyntaxKind.BoltForeignKeyword) {
      mustBeFunctionOrVariable = true;
      i += 2;
      t0 = tokens.peek(i);
    }
    if (mustBeFunctionOrVariable
      && t0.kind !== SyntaxKind.BoltStructKeyword
      && t0.kind !== SyntaxKind.BoltFnKeyword) {
      throw new ParseError(t0, [SyntaxKind.BoltStructKeyword, SyntaxKind.BoltFnKeyword]);
    }
    if (mustBeDeclOrImport && KIND_DECLARATION_T0.indexOf(t0.kind) === -1 && t0.kind !== SyntaxKind.BoltImportKeyword) {
      throw new ParseError(t0, KIND_DECLARATION_KEYWORD);
    }
    return t0;
  }

  private lookaheadIsMacroCall(tokens: BoltTokenStream): boolean {
    return tokens.peek(1).kind === SyntaxKind.BoltIdentifier
        && tokens.peek(2).kind === SyntaxKind.BoltExMark;
  }

  public parseSourceElement(tokens: BoltTokenStream): BoltSourceElement {
    if (this.lookaheadIsMacroCall(tokens)) {
      return this.parseMacroCall(tokens);
    }
    const t0 = tokens.peek();
    const t1 = this.getFirstTokenAfterModifiers(tokens);
    if (t1.kind ===  SyntaxKind.BoltImportKeyword) {
      return this.parseImportDirective(tokens);
    } else if (t1.kind === SyntaxKind.BoltModKeyword) {
      return this.parseModuleDeclaration(tokens);
    } else if (KIND_STATEMENT_T0.indexOf(t1.kind) !== -1) {
      return this.parseStatement(tokens);
    } else if (KIND_DECLARATION_KEYWORD.indexOf(t1.kind) !== -1) {
      return this.parseDeclarationLike(tokens);
    } else {
      throw new ParseError(t0, KIND_SOURCEELEMENT_T0);
    }
  }

  public parseFunctionBodyElement(tokens: BoltTokenStream): BoltFunctionBodyElement {
    if (this.lookaheadIsMacroCall(tokens)) {
      return this.parseMacroCall(tokens);
    }
    const t0 = this.getFirstTokenAfterModifiers(tokens);
    if (KIND_STATEMENT_T0.indexOf(t0.kind) !== -1) {
      return this.parseStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltLetKeyword) {
      return this.parseVariableDeclaration(tokens); 
    } else if (t0.kind === SyntaxKind.BoltFnKeyword) {
      return this.parseFunctionDeclaration(tokens);
    } else {
      throw new ParseError(t0, [...KIND_STATEMENT_T0, SyntaxKind.BoltLetKeyword, SyntaxKind.BoltFnKeyword]);
    }
  }

  public parseMacroCall(tokens: BoltTokenStream): BoltMacroCall {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltIdentifier);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltExMark);
    const t2 = tokens.get();
    if (!isBoltPunctuated(t2)) {
      throw new ParseError(t2, [SyntaxKind.BoltBraced, SyntaxKind.BoltParenthesized, SyntaxKind.BoltBracketed]);
    }
    const result = createBoltMacroCall(t0 as BoltIdentifier, t2.text);
    setOrigNodeRange(result, t0, t2);
    return result;
  }

  public parseFunctionBodyElements(tokens: BoltTokenStream): BoltFunctionBodyElement[] {
    const elements: BoltFunctionBodyElement[] = []
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (t0.kind === SyntaxKind.BoltSemi) {
        tokens.get();
        continue;
      }
      elements.push(this.parseFunctionBodyElement(tokens));
    }
    return elements
  }

  public parseSourceElements(tokens: BoltTokenStream): BoltSourceElement[] {
    const elements: BoltSourceElement[] = []
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (t0.kind === SyntaxKind.BoltSemi) {
        tokens.get();
        continue;
      }
      elements.push(this.parseSourceElement(tokens));
    }
    return elements
  }

  public parseSourceFile(tokens: BoltTokenStream, pkg: Package | null = null): BoltSourceFile {
    const elements = this.parseSourceElements(tokens);
    const t1 = tokens.peek();
    assertToken(t1, SyntaxKind.EndOfFile);
    return createBoltSourceFile(
      elements,
      pkg,
      new TextSpan(t1.span!.file, new TextPos(0,1,1), t1.span!.end.clone())
    );
  }

  public parseTraitOrImplElements(tokens: BoltTokenStream): BoltTraitOrImplElement[] {
    const elements: BoltTraitOrImplElement[] = [];
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (t0.kind === SyntaxKind.BoltSemi) {
        tokens.get();
        continue;
      }
      elements.push(this.parseTraitOrImplElement(tokens))
    }
    return elements;
  }

  public parseTraitOrImplElement(tokens: BoltTokenStream): BoltTraitOrImplElement {
    if (this.lookaheadIsMacroCall(tokens)) {
      return this.parseMacroCall(tokens);
    }
    const t0 = tokens.peek();
    switch (t0.kind) {
      case SyntaxKind.BoltFnKeyword:
        return this.parseFunctionDeclaration(tokens);
      case SyntaxKind.BoltTypeKeyword:
        return this.parseTypeAliasDeclaration(tokens);
      default:
        throw new ParseError(t0, [SyntaxKind.BoltFnKeyword, SyntaxKind.BoltTypeAliasDeclaration, SyntaxKind.BoltMacroCall])
    }
  }

  private canParseExpression(tokens: BoltTokenStream): boolean {
    // TODO
    return false;
  }

  private canParseReturnStatement(tokens: BoltTokenStream): boolean {
    const t0 = tokens.get();
    if (t0.kind !== SyntaxKind.BoltReturnKeyword) {
      return false;
    }
    const t1 = tokens.peek();
    if (t1.kind === SyntaxKind.EndOfFile) {
      return true;
    }
    return this.canParseExpression(tokens);
  }

  private canParseLoopStatement(tokens: BoltTokenStream): boolean {
    if (tokens.peek(1).kind !== SyntaxKind.BoltLoopKeyword) {
      return false;
    }
    return true;
  }

  private canParseStatement(tokens: BoltTokenStream): boolean {
    const t0 = tokens.peek();
    switch (t0.kind) {
      case SyntaxKind.BoltReturnKeyword:
        return this.canParseReturnStatement(tokens);
      case SyntaxKind.BoltLoopKeyword:
        return this.canParseLoopStatement(tokens);
      default:
        return this.canParseExpression(tokens)
    }
  }

  private canParseFunctionDeclaration(tokens: BoltTokenStream): boolean {
    let t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      tokens.get();
      t0 = tokens.peek();
    }
    if (t0.kind === SyntaxKind.BoltForeignKeyword) {
      tokens.get();
      const t1 = tokens.get();
      if (t1.kind !== SyntaxKind.BoltStringLiteral) {
        return false;
      }
      t0 = tokens.peek();
    }
    // TODO
    return true;
  }

  private canParseRecordDeclaration(tokens: BoltTokenStream): boolean {
    // TODO
    return true;
  }

  private canParseVariableDeclaration(tokens: BoltTokenStream): boolean {
    // TODO
    return true;
  }

  private canParseDeclaration(tokens: BoltTokenStream): boolean {
    let i = 0;
    let t0 = tokens.peek(i);
    while (isModifierKeyword(t0.kind)) {
      t0 = tokens.peek(++i);
    }
    switch (t0.kind) {
      case SyntaxKind.BoltFnKeyword:
        return this.canParseFunctionDeclaration(tokens);
      case SyntaxKind.BoltStructKeyword:
        return this.canParseRecordDeclaration(tokens);
      default:
        return false;
    }
  }

  private canParseSourceElement(tokens: BoltTokenStream): boolean {
    return this.canParseStatement(tokens)
        || this.canParseDeclaration(tokens)
  }

  private canParseRecordMember(tokens: BoltTokenStream): boolean {
    // TODO
    return true;
  }

  private canParseFunctionBodyElement(tokens: BoltTokenStream): boolean {
    return this.canParseFunctionDeclaration(tokens)
        || this.canParseStatement(tokens)
        || this.canParseVariableDeclaration(tokens);
  }

  private canParseRecordMembers(tokens: BoltTokenStream): boolean {
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (!this.canParseRecordMember(tokens)) {
        return false;
      }
    }
    return true;
  }

  private canParseSourceElements(tokens: BoltTokenStream): boolean {
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (!this.canParseSourceElement(tokens)) {
        return false;
      }
    }
    return true;
  }

  private canParseFunctionBodyElements(tokens: BoltTokenStream): boolean {
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (!this.canParseFunctionBodyElement(tokens)) {
        return false;
      }
    }
    return true;
  }

}
