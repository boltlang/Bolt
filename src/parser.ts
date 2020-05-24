
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
  BoltQualName,
  BoltPattern,
  createBoltBindPattern,
  BoltImportDeclaration,
  BoltTypeExpression,
  createBoltReferenceTypeExpression,
  createBoltConstantExpression,
  createBoltReferenceExpression,
  createBoltParameter,
  BoltBindPattern,
  createBoltRecordDeclaration,
  createBoltRecordField,
  createBoltImportDeclaration,
  BoltDeclarationModifiers,
  BoltStringLiteral,
  BoltImportSymbol,
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
  setParents,
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
import { Stream, uniq } from "./util"

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

const KIND_EXPRESSION_T0 = [
  SyntaxKind.BoltStringLiteral,
  SyntaxKind.BoltIntegerLiteral,
  SyntaxKind.BoltIdentifier,
  SyntaxKind.BoltOperator,
  SyntaxKind.BoltVBar,
  SyntaxKind.BoltMatchKeyword,
  SyntaxKind.BoltQuoteKeyword,
  SyntaxKind.BoltYieldKeyword,
]

const KIND_STATEMENT_T0 = uniq([
  SyntaxKind.BoltReturnKeyword,
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
  ...KIND_EXPRESSION_T0,
  ...KIND_STATEMENT_T0,
  ...KIND_DECLARATION_T0,
])

function isRightAssoc(kind: OperatorKind): boolean {
  return kind === OperatorKind.InfixR;
}

export class Parser {

  exprOperatorTable = new OperatorTable([
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

  typeOperatorTable = new OperatorTable([
    [
      [OperatorKind.InfixL, 2, '|'],
    ]
  ]);

  public parse(kind: SyntaxKind, tokens: BoltTokenStream): BoltSyntax {
    return (this as any)['parse' + kindToString(kind).substring('Bolt'.length)](tokens);
  }

  public parseNamespacePath(tokens: BoltTokenStream): BoltQualName {

    let modulePath = null;

    if (tokens.peek(2).kind === SyntaxKind.BoltColonColon) {
      modulePath = [];
      while (true) {
        modulePath.push(tokens.get() as BoltIdentifier)
        tokens.get();
        const t0 = tokens.peek(2);
        if (t0.kind !== SyntaxKind.BoltColonColon) {
          break;
        }
      }
    }

    const name = tokens.get();
    assertToken(name, SyntaxKind.BoltIdentifier);
    const startNode = modulePath !== null ? modulePath[0] : name;
    const endNode = name;
    const node = createBoltQualName(modulePath, name as BoltIdentifier, null);
    setOrigNodeRange(node, startNode, endNode);
    return node;
  }

  public parseQualName(tokens: BoltTokenStream): BoltQualName {

    let modulePath = null;

    if (tokens.peek(2).kind === SyntaxKind.BoltDot) {
      modulePath = [];
      while (true) {
        modulePath.push(tokens.get() as BoltIdentifier)
        tokens.get();
        const t0 = tokens.peek(2);
        if (t0.kind !== SyntaxKind.BoltDot) {
          break;
        }
      }
    }

    const name = tokens.get();
    assertToken(name, SyntaxKind.BoltIdentifier);
    const startNode = modulePath !== null ? modulePath[0] : name;
    const endNode = name;
    const node = createBoltQualName(modulePath, name as BoltIdentifier, null);
    setOrigNodeRange(node, startNode, endNode);
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

    const name = this.parseNamespacePath(tokens);
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
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier])
    }
  }

  public parseImportDeclaration(tokens: BoltTokenStream): BoltImportDeclaration {

    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltImportKeyword);

    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltStringLiteral);
    const filename = (t1 as BoltStringLiteral).value;

    const symbols: BoltImportSymbol[] = [];
    // TODO implement grammar and parsing logic for symbols

    const node = createBoltImportDeclaration(filename, symbols);
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

    const name = this.parseNamespacePath(tokens)

    const t1 = tokens.peek();

    let typeArgs: BoltTypeExpression[] | null = null;

    if (t1.kind === SyntaxKind.BoltLtSign) {
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
    }

    const node = createBoltReferenceTypeExpression(name, typeArgs);
    setOrigNodeRange(node, name, name);
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

  private parseTypeExpressionOperators(tokens: BoltTokenStream, lhs: BoltTypeExpression, minPrecedence: number): BoltTypeExpression {
    while (true) {
      const t0 = tokens.peek();
      if (!isBoltOperatorLike(t0)) {
        break;
      }
      let desc0 = this.typeOperatorTable.lookup(emit(t0));
      if (desc0 === null || desc0.arity !== 2 || desc0.precedence < minPrecedence) {
        break;
      }
      console.log(desc0)
      tokens.get();
      let rhs = this.parsePrimTypeExpression(tokens);
      while (true) {
        const t1 = tokens.peek()
        if (!isBoltOperatorLike(t1.kind)) {
          break;
        }
        const desc1 = this.typeOperatorTable.lookup(emit(t1))
        if (desc1 === null || desc1.arity !== 2 || desc1.precedence < desc0.precedence || !isRightAssoc(desc1.kind)) {
          break;
        }
        rhs = this.parseTypeExpressionOperators(tokens, rhs, desc1.precedence);
      }
      const name = createBoltQualName([], t0);
      setOrigNodeRange(name, t0, t0);
      lhs = createBoltReferenceTypeExpression(name, [lhs, rhs]);
      setOrigNodeRange(lhs, t0, rhs);
    }
    return lhs
  }

  public parseTypeExpression(tokens: BoltTokenStream) {
    const lhs = this.parsePrimTypeExpression(tokens);
    return this.parseTypeExpressionOperators(tokens, lhs, 0);
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
    const name = this.parseNamespacePath(tokens);
    const node = createBoltReferenceExpression(name);
    setOrigNodeRange(node, name, name);
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
      const t4 = tokens.peek();
      if (t4.kind === SyntaxKind.EndOfFile) {
        break;
      }
      assertToken(t4, SyntaxKind.BoltComma);
      tokens.get();
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
    const result = createBoltQuoteExpression(scanned);
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

  private parseExpression(tokens: BoltTokenStream): BoltExpression {

    const t0 = tokens.peek();

    let result;
    if (t0.kind === SyntaxKind.BoltVBar) {
      result = this.parseFunctionExpression(tokens);
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

      let t2 = tokens.peek();
      const firstToken = t2;

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
      modifiers |= BoltDeclarationModifiers.Mutable;
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

  public parseStatement(tokens: BoltTokenStream): BoltStatement {
    if (this.lookaheadIsMacroCall(tokens)) {
      return this.parseMacroCall(tokens);
    }
    const t0 = tokens.peek();
    if (KIND_EXPRESSION_T0.indexOf(t0.kind) !== -1) {
      return this.parseExpressionStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltReturnKeyword) {
      return this.parseReturnStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltLoopKeyword) {
      return this.parseLoopStatement(tokens);
    } else {
      throw new ParseError(t0, KIND_STATEMENT_T0);
    }
  }

  public parseGenericTypeParameter(tokens: BoltTokenStream): BoltTypeParameter {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      tokens.get();
      const node = createBoltTypeParameter(0, t0, null)
      setOrigNodeRange(node, t0, t0);
      return node;
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier]);
    }
  }

  private parseGenericTypeParameters(tokens: BoltTokenStream): BoltTypeParameter[] {
    let typeParams: BoltTypeParameter[] = [];
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.BoltLtSign);
    while (true) {
      let t1 = tokens.peek();
      if (t1.kind === SyntaxKind.BoltGtSign) {
        break;
      }
      if (t1.kind === SyntaxKind.EndOfFile) {
        throw new ParseError(t1, [SyntaxKind.BoltGtSign, SyntaxKind.BoltIdentifier, SyntaxKind.BoltComma]);
      }
      if (typeParams.length > 0) {
        tokens.get();
        assertToken(t1, SyntaxKind.BoltComma);
        t1 = tokens.peek();
      }
      if (t1.kind === SyntaxKind.EndOfFile) {
        throw new ParseError(t1, [SyntaxKind.BoltGtSign, SyntaxKind.BoltIdentifier]);
      }
      typeParams.push(this.parseGenericTypeParameter(tokens));
    }
    const t3 = tokens.get();
    assertToken(t3, SyntaxKind.BoltGtSign);
    return typeParams;
  }

  public parseRecordDeclaration(tokens: BoltTokenStream): BoltRecordDeclaration {

    let modifiers = 0;
    let typeParams = null;

    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltDeclarationModifiers.Public;
      t0 = tokens.get();
    }

    if (t0.kind !== SyntaxKind.BoltStructKeyword) {
      throw new ParseError(t0, [SyntaxKind.BoltStructKeyword])
    }

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
      typeParams = this.parseGenericTypeParameters(tokens);
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
        const field = createBoltRecordField(name as BoltIdentifier, type);
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

    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      tokens.get();
      modifiers |= BoltDeclarationModifiers.Public;
      t0 = tokens.peek();
    }

    if (t0.kind !== SyntaxKind.BoltModKeyword) {
      throw new ParseError(t0, [SyntaxKind.BoltModKeyword])
    }

    const name = this.parseNamespacePath(tokens);

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced])
    }
    const sentences = this.parseSourceElements(createTokenStream(t1));

    const node = createBoltModule(modifiers, name, sentences);
    setOrigNodeRange(node, firstToken, t1);
    return node;
  }


  public parseTypeAliasDeclaration(tokens: BoltTokenStream): BoltTypeAliasDeclaration {

    let modifiers = 0;
    let typeParams = null;

    let t0 = tokens.get();
    const firstToken = t0;

    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltDeclarationModifiers.Public;
      t0 = tokens.get();
    }

    assertToken(t0, SyntaxKind.BoltTypeKeyword);

    const name = tokens.get();
    if (name.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(name, [SyntaxKind.BoltIdentifier])
    }

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltLtSign) {
      typeParams = this.parseGenericTypeParameters(tokens);
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
    let lastToken: BoltSyntax;
    const firstToken = k0;

    if (k0.kind === SyntaxKind.BoltPubKeyword) {
      tokens.get();
      modifiers |= BoltDeclarationModifiers.Public;
      k0 = tokens.peek();
    }

    if (k0.kind === SyntaxKind.BoltForeignKeyword) {
      tokens.get();
      modifiers |= BoltDeclarationModifiers.IsForeign;
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

    if (params.length > 0) {
      lastToken = params[params.length-1];
    }

    // Parse return type

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltRArrow) {
      lastToken = t2;
      tokens.get();
      returnType = this.parseTypeExpression(tokens);
    }

    // Parse function body

    const t3 = tokens.peek();
    if (t3.kind === SyntaxKind.BoltBraced) {
      lastToken = t3;
      tokens.get();
      switch (target) {
        case "Bolt":
          body = this.parseStatements(createTokenStream(t3));
          break;
        default:
          body = parseForeignLanguage(target, t3.text, t3.span!.file, t3.span!.start);
          break;
      }
    }

    const result = createBoltFunctionDeclaration(
      modifiers,
      target,
      name,
      params,
      returnType,
      body
    );
    setOrigNodeRange(result, firstToken, lastToken!);
    return result;

  }

  public parseTraitDeclaration(tokens: BoltTokenStream): BoltTraitDeclaration {
    let modifiers = 0;
    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltDeclarationModifiers.Public;
      t0 = tokens.get();
    }
    assertToken(t0, SyntaxKind.BoltTraitKeyword);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltIdentifier);
    let typeParams = null
    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltLtSign) {
      typeParams = this.parseGenericTypeParameters(tokens);
    }
    const t3 = tokens.get();
    assertToken(t3, SyntaxKind.BoltBraced);
    const elements = this.parseSourceElements(createTokenStream(t3));
    const result = createBoltTraitDeclaration(modifiers, t1 as BoltIdentifier, typeParams, elements as BoltDeclaration[]);
    setOrigNodeRange(result, firstToken, t3);
    return result;
  }

  public parseImplDeclaration(tokens: BoltTokenStream): BoltImplDeclaration {
    let modifiers = 0;
    let t0 = tokens.get();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      modifiers |= BoltDeclarationModifiers.Public;
      t0 = tokens.get();
    }
    assertToken(t0, SyntaxKind.BoltImplKeyword);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.BoltIdentifier);
    const t2 = tokens.get();
    assertToken(t2, SyntaxKind.BoltForKeyword);
    const typeExpr = this.parseTypeExpression(tokens);
    let typeParams = null
    const t3 = tokens.peek();
    if (t3.kind === SyntaxKind.BoltLtSign) {
      typeParams = this.parseGenericTypeParameters(tokens);
    }
    const t4 = tokens.get();
    assertToken(t4, SyntaxKind.BoltBraced);
    const elements = this.parseSourceElements(createTokenStream(t4));
    const result = createBoltImplDeclaration(modifiers, t1 as BoltIdentifier, typeExpr, typeParams, elements as BoltDeclaration[]);
    setOrigNodeRange(result, firstToken, t4);
    return result;
  }

  public parseDeclaration(tokens: BoltTokenStream): BoltDeclaration {
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
      case SyntaxKind.BoltModKeyword:
        return this.parseModuleDeclaration(tokens);
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
    let mustBeDecl = false;
    let mustBeFunctionOrVariable = false;
    let i = 1;
    let t0 = tokens.peek(i);
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      mustBeDecl = true;
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
    if (mustBeDecl && KIND_DECLARATION_T0.indexOf(t0.kind) === -1) {
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
    const t0 = this.getFirstTokenAfterModifiers(tokens);
    if (KIND_STATEMENT_T0.indexOf(t0.kind) !== -1) {
      return this.parseStatement(tokens);
    }
    return this.parseDeclaration(tokens);
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

  //parseBinOp(tokens: TokenStream, lhs: Expr , minPrecedence: number) {
  //  let lookahead = tokens.peek(1);
  //  while (true) {
  //    if (lookahead.kind !== SyntaxKind.BoltOperator) {
  //      break;
  //    }
  //    const lookaheadDesc = this.getOperatorDesc(2, lookahead.text);
  //    if (lookaheadDesc === null || lookaheadDesc.precedence < minPrecedence) {
  //      break;
  //    }
  //    const op = lookahead;
  //    const opDesc = this.getOperatorDesc(2, op.text);
  //    tokens.get();
  //    let rhs = this.parsePrimExpr(tokens)
  //    lookahead = tokens.peek()
  //    while (lookaheadDesc.arity === 2 
  //        && ((lookaheadDesc.precedence > opDesc.precedence)
  //          || lookaheadDesc.kind === OperatorKind.InfixR && lookaheadDesc.precedence === opDesc.precedence)) {
  //        rhs = this.parseBinOp(tokens, rhs, lookaheadDesc.precedence)
  //    }
  //    lookahead = tokens.peek();
  //    lhs = new CallExpr(new RefExpr(new QualName(op, [])), [lhs, rhs]);
  //  }
  //  return lhs
  //}

  public parseSourceFile(tokens: BoltTokenStream): BoltSourceFile {
    const elements = this.parseSourceElements(tokens);
    const t1 = tokens.peek();
    assertToken(t1, SyntaxKind.EndOfFile);
    return createBoltSourceFile(
      elements,
      new TextSpan(t1.span!.file, new TextPos(0,1,1), t1.span!.end.clone())
    );
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

import { Scanner } from "./scanner"
import { TextFile, TextSpan, TextPos } from "./text"
import * as fs from "fs"
import {JSScanner} from "./foreign/js/scanner";
import {emit} from "./emitter";

export function parseSourceFile(filepath: string): BoltSourceFile {
  const file = new TextFile(filepath);
  const contents = fs.readFileSync(file.origPath, 'utf8');
  const scanner = new Scanner(file, contents)
  const parser = new Parser();
  const sourceFile = parser.parseSourceFile(scanner);
  setParents(sourceFile);
  return sourceFile;
}

