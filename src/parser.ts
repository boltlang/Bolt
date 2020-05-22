)
import * as acorn from "acorn"

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
  createBoltRecordDeclarationField,
  createBoltImportDeclaration,
  BoltDeclarationModifiers,
  BoltStringLiteral,
  BoltImportSymbol,
  BoltCallExpression,
  BoltExpressionStatement,
  createBoltExpressionStatement,
  BoltVariableDeclaration,
  BoltSyntax,
  createBoltVariableDeclaration,
  BoltReturnStatement,
  createBoltReturnStatement,
  BoltRecordDeclarationField,
  BoltModule,
  createBoltModule,
  BoltTypeAliasDeclaration,
  createBoltTypeAliasDeclaration,
  BoltFunctionDeclaration,
  createBoltFunctionDeclaration,
  createBoltCallExpression,
  BoltSymbol,
  JSSourceElement,
  JSStatement,
  BoltTypeParameter,
  createBoltTypePattern,
  createBoltTypeParameter,
} from "./ast"

import { Scanner } from "./scanner"

import { Stream, setOrigNodeRange, createTokenStream, uniq, FastStringMap } from "./util"

export type BoltTokenStream = Stream<BoltToken>;

export type JSTokenStream = Stream<JSToken>;

function describeKind(kind: SyntaxKind): string {
  switch (kind) {
    case SyntaxKind.BoltIdentifier:
      return "an identifier"
    case SyntaxKind.BoltOperator:
      return "an operator"
    case SyntaxKind.BoltStringLiteral:
      return "a string"
    case SyntaxKind.BoltIntegerLiteral:
      return "an integer"
    case SyntaxKind.BoltFnKeyword:
      return "'fn'"
    case SyntaxKind.BoltForeignKeyword:
      return "'foreign'"
    case SyntaxKind.BoltMatchKeyword:
      return "'match'";
    case SyntaxKind.BoltYieldKeyword:
      return "'yield'";
    case SyntaxKind.BoltReturnKeyword:
      return "'return'";
    case SyntaxKind.BoltPubKeyword:
      return "'pub'"
    case SyntaxKind.BoltLetKeyword:
      return "'let'"
    case SyntaxKind.BoltSemi:
      return "';'"
    case SyntaxKind.BoltColon:
      return "':'"
    case SyntaxKind.BoltDot:
      return "'.'"
    case SyntaxKind.BoltRArrow:
      return "'->'"
    case SyntaxKind.BoltComma:
      return "','"
    case SyntaxKind.BoltModKeyword:
      return "'mod'"
    case SyntaxKind.BoltStructKeyword:
      return "'struct'"
    case SyntaxKind.BoltEnumKeyword:
      return "'enum'"
    case SyntaxKind.BoltTypeKeyword:
      return "'type'";
    case SyntaxKind.BoltBraced:
      return "'{' .. '}'"
    case SyntaxKind.BoltBracketed:
      return "'[' .. ']'"
    case SyntaxKind.BoltParenthesized:
      return "'(' .. ')'"
    case SyntaxKind.EndOfFile:
      return "'}', ')', ']' or end-of-file"
    case SyntaxKind.BoltLtSign:
      return "'<'";
    case SyntaxKind.BoltGtSign:
      return "'<'";
    case SyntaxKind.BoltEqSign:
      return "'='";
    default:
      throw new Error(`failed to describe ${kindToString(kind)}`)
  }
}

function enumerate(elements: string[]) {
  if (elements.length === 1) {
    return elements[0]
  } else {
    return elements.slice(0, elements.length-1).join(', ') + ' or ' + elements[elements.length-1]
  }
}

export class ParseError extends Error {
  constructor(public actual: BoltToken, public expected: SyntaxKind[]) {
    super(`${actual.span!.file.origPath}:${actual.span!.start.line}:${actual.span!.start.column}: expected ${enumerate(expected.map(e => describeKind(e)))} but got ${describeKind(actual.kind)}`)
  }
}

enum OperatorKind {
  Prefix,
  InfixL,
  InfixR,
  Suffix,
}

function isRightAssoc(kind: OperatorKind) {
  return kind === OperatorKind.InfixR;
}

interface OperatorInfo {
  kind: OperatorKind;
  arity: number;
  name: string;
  precedence: number;
}

function assertToken(node: BoltToken, kind: SyntaxKind) {
  if (node.kind !== kind) {
    throw new ParseError(node, [kind]);
  }
}

const KIND_EXPRESSION_T0 = [
  SyntaxKind.BoltStringLiteral,
  SyntaxKind.BoltIntegerLiteral,
  SyntaxKind.BoltIdentifier,
  SyntaxKind.BoltOperator,
  SyntaxKind.BoltMatchKeyword,
  SyntaxKind.BoltYieldKeyword,
]

const KIND_STATEMENT_T0 = uniq([
  SyntaxKind.BoltReturnKeyword,
  ...KIND_EXPRESSION_T0,
])

const KIND_DECLARATION_KEYWORD = [
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

type OperatorTableMatrix = [OperatorKind, number, string][][];

class OperatorTable {

  private operatorsByName = new FastStringMap<string, OperatorInfo>();
  //private operatorsByPrecedence = FastStringMap<number, OperatorInfo>();

  constructor(definitions: OperatorTableMatrix) {
    let i = 0;
    for (const group of definitions) {
      for (const [kind, arity, name] of group) {
        const info = { kind, arity, name, precedence: i }
        this.operatorsByName.set(name, info);
        //this.operatorsByPrecedence[i] = info;
      }
      i++;
    }
  }

  public lookup(name: string): OperatorInfo | null {
    if (!this.operatorsByName.has(name)) {
      return null;
    }
    return this.operatorsByName.get(name);
  }

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

  protected assertEmpty(tokens: BoltTokenStream) {
    const t0 = tokens.peek(1);
    if (t0.kind !== SyntaxKind.EndOfFile) {
      throw new ParseError(t0, [SyntaxKind.EndOfFile]);
    }
  }

  public parse(kind: SyntaxKind, tokens: BoltTokenStream): BoltSyntax {
    return (this as any)['parse' + kindToString(kind).substring('Bolt'.length)](tokens);
  }

  public parseQualName(tokens: BoltTokenStream): BoltQualName {

    const path: BoltIdentifier[] = [];

    while (true) {
      const t0 = tokens.peek(2);
      if (t0.kind !== SyntaxKind.BoltDot) {
        break;
      }
      path.push(tokens.get() as BoltIdentifier)
      tokens.get();
    }

    const name = tokens.get();
    if (name.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(name, [SyntaxKind.BoltIdentifier]);
    }
    const startNode = path.length > 0 ? path[0] : name;
    const endNode = name;
    const node = createBoltQualName(path, name, null);
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

  public parsePattern(tokens: BoltTokenStream): BoltPattern {
    const t0 = tokens.peek(1);
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      return this.parseBindPattern(tokens);
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

  public parseReferenceTypeExpression(tokens: BoltTokenStream) {

    const name = this.parseQualName(tokens)

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
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      return this.parseReferenceTypeExpression(tokens);
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier]);
    }
  }

  private parseTypeExpressionOperators(tokens: BoltTokenStream, lhs: BoltTypeExpression, minPrecedence: number): BoltTypeExpression {
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind !== SyntaxKind.BoltOperator) {
        break;
      }
      let desc0 = this.typeOperatorTable.lookup(t0.text);
      if (desc0 === null || desc0.arity !== 2 || desc0.precedence < minPrecedence) {
        break;
      }
      console.log(desc0)
      tokens.get();
      let rhs = this.parsePrimTypeExpression(tokens);
      while (true) {
        const t1 = tokens.peek()
        if (t1.kind !== SyntaxKind.BoltOperator) {
          break;
        }
        const desc1 = this.typeOperatorTable.lookup(t1.text)
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

  public parseReferenceExpression(tokens: BoltTokenStream): BoltReferenceExpression {
    const name = this.parseQualName(tokens);
    const node = createBoltReferenceExpression(name);
    setOrigNodeRange(node, name, name);
    return node;
  }

  protected parsePrimitiveExpression(tokens: BoltTokenStream): BoltExpression {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltIntegerLiteral || t0.kind === SyntaxKind.BoltStringLiteral) {
      return this.parseConstantExpression(tokens);
    } else if (t0.kind === SyntaxKind.BoltIdentifier) {
      return this.parseReferenceExpression(tokens);
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltStringLiteral, SyntaxKind.BoltIdentifier]);
    }
  }

  //parseSyntax(tokens: TokenStream): Syntax {

  //  // Assuming first token is 'syntax'
  //  const t0 = tokens.get();
  //  assertToken(t0, SyntaxKind.Bolt

  //  const t1 = tokens.get();
  //  if (t1.kind !== SyntaxKind.BoltBraced) {
  //    throw new ParseError(t1, [SyntaxKind.BoltBraced])
  //  }

  //  const innerTokens = t1.toTokenStream();

  //  const pattern = this.parsePattern(innerTokens)

  //  const t2 = innerTokens.get();
  //  if (t2.kind !== SyntaxKind.BoltRArrow) {
  //    throw new ParseError(t2, [SyntaxKind.BoltRArrow]);
  //  }

  //  const body = this.parseBody(innerTokens);

  //  return new Macro(pattern, body)

  //}

  public parseExpression(tokens: BoltTokenStream): BoltExpression {
    return this.parseCallOrPrimitiveExpression(tokens)
  }

  public parseParameter(tokens: BoltTokenStream): BoltParameter {

    let defaultValue = null;
    let typeDecl = null;

    const pattern = this.parsePattern(tokens)

    let t0 = tokens.peek(1);
    let endNode: BoltSyntax = pattern;
    if (t0.kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeDecl = this.parseTypeExpression(tokens);
      endNode = typeDecl;
      t0 = tokens.get();
    }
    if (t0.kind === SyntaxKind.BoltEqSign) {
      tokens.get();
      defaultValue = this.parseExpression(tokens);
      endNode = defaultValue;
    }

    const node = createBoltParameter(0, pattern, typeDecl, defaultValue)
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
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltReturnKeyword) {
      return this.parseReturnStatement(tokens);
    } else if (t0.kind === SyntaxKind.BoltLoopKeyword) {
      return this.parseLoopStatement(tokens);
    } else {
      try {
        return this.parseExpressionStatement(tokens);
      } catch (e) {
        if (!(e instanceof ParseError)) {
          throw e;
        }
        throw new ParseError(t0, KIND_STATEMENT_T0);
      }
    }
  }

  public parseGenericTypeParameter(tokens: BoltTokenStream) {
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

  private parseGenericTypeParameters(tokens: BoltTokenStream) {
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
    const name = createBoltQualName([], t1 as BoltIdentifier);

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

    if (t2.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t2, [SyntaxKind.BoltBraced])
    }

    let fields: BoltRecordDeclarationField[] = [];
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
      fields.push(field);
    }

    const node = createBoltRecordDeclaration(modifiers, name, typeParams, fields);
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

    const name = this.parseQualName(tokens);

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced])
    }
    const sentences = this.parseSourceElementList(createTokenStream(t1));

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
        this.assertEmpty(innerTokens);
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
          body = this.parseStatements(tokens);
          break;
        case "JS":
          const scanner = new Scanner(t3.span!.file, t3.text);
          body = this.parseJSSourceElementList(scanner);
          break;
        default:
          throw new Error(`Unrecognised language: ${target}`);
      }
    }

    const node = createBoltFunctionDeclaration(
      modifiers,
      target,
      name,
      params,
      returnType,
      body
    );
    setOrigNodeRange(node, firstToken, lastToken!);
    return node;

  }

  //public parseModuleDeclaration(tokens: BoltTokenStream): BoltModule {
    //let modifiers = 0;
    //let t0 = tokens.get();
    //if (t0.kind === SyntaxKind.BoltPubKeyword) {
      //modifiers |= BoltDeclarationModifiers.Public;
      //t0 = tokens.get();
    //}
    //assertToken(t0, SyntaxKind.BoltModKeyword);
    //const name = this.parseQualName(tokens);
    //const t1 = tokens.get();
    //assertToken(t1, SyntaxKind.BoltBraced);
    //const elements = this.parseSourceElementList(createTokenStream(t1));
    //const node = createBoltModule(modifiers, name, elements);
    //setOrigNodeRange(node, t0, t1);
    //return node;
  //}

  public parseDeclaration(tokens: BoltTokenStream): BoltDeclaration {
    let t0 = tokens.peek(1);
    let i = 1;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      i += 1;
      t0 = tokens.peek(i);
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

  public parseSourceElement(tokens: BoltTokenStream): BoltSourceElement {
    try {
      return this.parseDeclaration(tokens)
    } catch (e1) {
      if (!(e1 instanceof ParseError)) {
        throw e1;
      }
      try {
        return this.parseStatement(tokens);
      } catch (e2) {
        if (!(e2 instanceof ParseError)) {
          throw e2;
        }
        throw e1;
      }
    }
  }

  public parseSourceElementList(tokens: BoltTokenStream): BoltSourceElement[] {
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

  private parseCallOrPrimitiveExpression(tokens: BoltTokenStream): BoltExpression {

    const operator = this.parsePrimitiveExpression(tokens)

    const t2 = tokens.get();
    if (t2.kind === SyntaxKind.EndOfFile) {
      return operator;
    }
    assertToken(t2, SyntaxKind.BoltParenthesized);

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
      } else if (t4.kind !== SyntaxKind.BoltComma){
        throw new ParseError(t4, [SyntaxKind.BoltComma])
      }
    }

    const node = createBoltCallExpression(operator, args, null)
    setOrigNodeRange(node, operator, t2);
    return node;

  }

  public parseJSStatement(tokens: JSTokenStream): JSStatement {
    return this.parseJSExpressionStatement(tokens);
  }

  public parseJSSourceElementList(tokens: JSTokenStream): JSSourceElement[] {
    const elements: JSSourceElement[] = [];
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      const statement = this.parseJSStatement(tokens)
      elements.push(statement);
    }
    return elements;
  }

}

