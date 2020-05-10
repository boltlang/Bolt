
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
  BoltTypeNode,
  createBoltReferenceTypeNode,
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
  BoltNewTypeDeclaration,
  createBoltNewTypeDeclaration,
  BoltFunctionDeclaration,
  createBoltFunctionDeclaration,
  createBoltCallExpression,
} from "./ast"

import { BoltTokenStream, setOrigNodeRange } from "./util"

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
    case SyntaxKind.BoltNewTypeKeyword:
      return "'newtype'";
    case SyntaxKind.BoltBraced:
      return "'{' .. '}'"
    case SyntaxKind.BoltBracketed:
      return "'[' .. ']'"
    case SyntaxKind.BoltParenthesized:
      return "'(' .. ')'"
    case SyntaxKind.BoltEOS:
      return "'}', ')', ']' or end-of-file"
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

const KIND_STATEMENT_T0 = [
  SyntaxKind.BoltReturnKeyword,
  ...KIND_EXPRESSION_T0,
]

const KIND_DECLARATION_KEYWORD = [
  SyntaxKind.BoltFnKeyword,
  SyntaxKind.BoltEnumKeyword,
  SyntaxKind.BoltLetKeyword,
  SyntaxKind.BoltNewTypeKeyword,
  SyntaxKind.BoltModKeyword,
  SyntaxKind.BoltStructKeyword,
]

const KIND_DECLARATION_T0 = [
  SyntaxKind.BoltPubKeyword,
  SyntaxKind.BoltForeignKeyword,
  ...KIND_DECLARATION_KEYWORD,
]

const KIND_SOURCEELEMENT_T0 = [
  SyntaxKind.BoltModKeyword,
  ...KIND_EXPRESSION_T0,
  ...KIND_STATEMENT_T0,
  ...KIND_DECLARATION_T0,
]

export class Parser {

  operatorTable = [
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
      [OperatorKind.Prefix, '!']
    ],
  ];

  protected assertEmpty(tokens: BoltTokenStream) {
    const t0 = tokens.peek(1);
    if (t0.kind !== SyntaxKind.BoltEOS) {
      throw new ParseError(t0, [SyntaxKind.BoltEOS]);
    }
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

  public parseReferenceTypeNode(tokens: BoltTokenStream) {

    const name = this.parseQualName(tokens)

    const t1 = tokens.peek();

    let typeArgs: BoltTypeNode[] | null = null;

    if (t1.kind === SyntaxKind.BoltLtSign) {
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
        typeArgs!.push(this.parseTypeNode(tokens));
      }
      const t4 = tokens.get();
      assertToken(t4, SyntaxKind.BoltGtSign);
    }

    const node = createBoltReferenceTypeNode(name, typeArgs);
    setOrigNodeRange(node, name, name);
    return node;
  }

  public parseTypeNode(tokens: BoltTokenStream): BoltTypeNode {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      return this.parseReferenceTypeNode(tokens);
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier]);
    }
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
    return this.parsePrimitiveExpression(tokens)
  }

  public parseParameter(tokens: BoltTokenStream): BoltParameter {

    let defaultValue = null;
    let typeDecl = null;

    const pattern = this.parsePattern(tokens)

    let t0 = tokens.peek(1);
    let endNode: BoltSyntax = pattern;
    if (t0.kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeDecl = this.parseTypeNode(tokens);
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
      lastNode = typeDecl = this.parseTypeNode(tokens);
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
    if (t1.kind !== SyntaxKind.BoltEOS) { 
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

  public parseRecordDeclaration(tokens: BoltTokenStream): BoltRecordDeclaration {

    let modifiers = 0;

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

    const t2 = tokens.get();

    if (t2.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t2, [SyntaxKind.BoltBraced])
    }

    let fields: BoltRecordDeclarationField[] = [];
    const innerTokens = createTokenStream(t2);

    while (true) {
      const t3 = innerTokens.get();
      if (t3.kind === SyntaxKind.BoltEOS) {
        break;
      }
      const name = innerTokens.get();
      assertToken(name, SyntaxKind.BoltIdentifier);
      const t4 = innerTokens.get();
      assertToken(t4, SyntaxKind.BoltColon);
      const type = this.parseTypeNode(innerTokens);
      const field = createBoltRecordDeclarationField(name as BoltIdentifier, type);
      setOrigNodeRange(field, name, type);
      fields.push(field);
    }

    const node = createBoltRecordDeclaration(modifiers, name, fields);
    setOrigNodeRange(node, firstToken, t2);
    return node;
  }

  public parseStatements(tokens: BoltTokenStream): BoltStatement[] {
    const statements: BoltStatement[] = [];
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.BoltEOS) {
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

    if (t0.kind !== SyntaxKind.BoltIdentifier || t0.text !== 'mod') {
      throw new ParseError(t0, [SyntaxKind.BoltModKeyword])
    }

    const name = this.parseQualName(tokens);

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced])
    }
    const sentences = this.parseSentences(createTokenStream(t1));

    const node = createBoltModule(modifiers, name, sentences);
    setOrigNodeRange(node, firstToken, t1);
    return node;
  }


  public parseNewTypeDeclaration(tokens: BoltTokenStream): BoltNewTypeDeclaration {

    let modifiers = 0;

    let t0 = tokens.get();
    const firstToken = t0;

    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      tokens.get();
      modifiers |= BoltDeclarationModifiers.Public;
      t0 = tokens.peek();
      if (t0.kind !== SyntaxKind.BoltIdentifier) {
        throw new ParseError(t0, [SyntaxKind.BoltNewTypeKeyword])
      }
    }

    if (t0.kind !== SyntaxKind.BoltNewTypeKeyword) {
      throw new ParseError(t0, [SyntaxKind.BoltNewTypeKeyword])
    }

    const name = tokens.get();
    if (name.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(name, [SyntaxKind.BoltIdentifier])
    }

    const node = createBoltNewTypeDeclaration(modifiers, name)
    setOrigNodeRange(node, firstToken, name);
    return node;
  }

  private parseFunctionDeclaration(tokens: BoltTokenStream): BoltFunctionDeclaration {

    let target = "Bolt";
    let modifiers = 0;

    let k0 = tokens.peek();
    let lastToken: BoltSyntax;
    const firstToken = k0;

    if (k0.kind !== SyntaxKind.BoltPubKeyword) {
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

    let name: BoltQualName;
    let returnType = null;
    let body = null;
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

      name = createBoltQualName([], t0);
      setOrigNodeRange(name, t0, t0);
      tokens.get();
      params.push(parseParamLike(tokens))

    } else if (isParamLike(t0) && t1.kind == SyntaxKind.BoltOperator) {

      params.push(parseParamLike(tokens));
      name = createBoltQualName([], t1);
      setOrigNodeRange(name, t1, t1);
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

      name = this.parseQualName(tokens)
      const t2 = tokens.get();
      if (t2.kind === SyntaxKind.BoltParenthesized) {
        const innerTokens = createTokenStream(t2);
        while (true) {
          const t3 = innerTokens.peek();
          if (t3.kind === SyntaxKind.BoltEOS) {
            break;
          }
          params.push(this.parseParameter(innerTokens, i++))
          const t4 = innerTokens.get();
          if (t4.kind === SyntaxKind.BoltComma) {
            continue;
          } else if (t4.kind === SyntaxKind.BoltEOS) {
            break;
          } else {
            throw new ParseError(t4, [SyntaxKind.BoltComma, SyntaxKind.BoltEOS])
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
      returnType = this.parseTypeNode(tokens);
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
          // TODO
          //body = acorn.parse(t3.text).body;
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

  public parseDeclaration(tokens: BoltTokenStream): BoltDeclaration {
    let t0 = tokens.peek(1);
    let i = 1;
    if (t0.kind === SyntaxKind.BoltPubKeyword) {
      t0 = tokens.peek(i++);
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
      case SyntaxKind.BoltNewTypeKeyword:
        return this.parseNewTypeDeclaration(tokens);
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
    const t0 = tokens.peek();
    try {
      return this.parseDeclaration(tokens)
    } catch (e) {
      if (!(e instanceof ParseError)) {
        throw e;
      }
      try {
        return this.parseStatement(tokens);
      } catch (e) {
        if (!(e instanceof ParseError)) {
          throw e;
        }
        throw new ParseError(t0, KIND_SOURCEELEMENT_T0)
      }
    }
  }

  protected getOperatorDesc(seekArity: number, seekName: string): OperatorInfo {
    for (let i = 0; i < this.operatorTable.length; ++i) {
      for (const [kind, arity, name] of this.operatorTable[i]) {
        if (arity == seekArity && name === seekName) {
          return {
            kind,
            name,
            arity,
            precedence: i
          }
        }
      }
    }
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

  public parseCallExpression(tokens: BoltTokenStream): BoltCallExpression {

    const operator = this.parsePrimitiveExpression(tokens)
    const args: BoltExpression[] = []

    const t2 = tokens.get();
    assertToken(t2, SyntaxKind.BoltParenthesized);

    const innerTokens = createTokenStream(t2);

    while (true) {
      const t3 = innerTokens.peek();
      if (t3.kind === SyntaxKind.BoltEOS) {
        break; 
      }
      args.push(this.parseExpression(innerTokens))
      const t4 = innerTokens.get();
      if (t4.kind === SyntaxKind.BoltEOS) {
        break
      } else if (t4.kind !== SyntaxKind.BoltComma){
        throw new ParseError(t4, [SyntaxKind.BoltComma])
      }
    }

    return createBoltCallExpression(operator, args, null)

  }

}

