
import { privateDecrypt } from "crypto";
import {
  ReferenceTypeExpression,
  SourceFile,
  SourceFileElement,
  StructDeclaration,
  StructDeclarationField,
  SyntaxKind,
  Token,
  TokenKind,
  Expression,
  TypeExpression,
  ConstantExpression,
  ReferenceExpression,
  Dot,
  Identifier,
  TupleExpression,
  PrefixExpression,
  ExpressionStatement,
  ImportDeclaration,
  Param,
  Pattern,
  BindPattern,
  LetDeclaration,
  TypeAssert,
  ExprBody,
  BlockBody,
  QualifiedName,
  NestedExpression,
  NamedTuplePattern,
  StructPattern,
  VariadicStructPatternElement,
  PunnedFieldStructPatternElement,
  FieldStructPatternElement,
  TuplePattern,
  InfixExpression,
} from "./cst"
import { Stream, MultiDict } from "./util";

const DESCRIPTIONS: Record<SyntaxKind, string> = {
  [SyntaxKind.StringLiteral]: 'a string literal',
  [SyntaxKind.Identifier]: "an identifier",
  [SyntaxKind.Comma]: "','",
  [SyntaxKind.Colon]: "':'",
  [SyntaxKind.Integer]: "an integer",
  [SyntaxKind.LParen]: "'('",
  [SyntaxKind.RParen]: "')'",
  [SyntaxKind.LBrace]: "'{'",
  [SyntaxKind.RBrace]: "'}'",
  [SyntaxKind.LBracket]: "'['",
  [SyntaxKind.RBracket]: "']'",
  [SyntaxKind.ConstantExpression]: 'a constant expression',
  [SyntaxKind.ReferenceExpression]: 'a reference expression',
  [SyntaxKind.LineFoldEnd]: 'the end of the current line-fold',
  [SyntaxKind.TupleExpression]: 'a tuple expression such as (1, 2)',
  [SyntaxKind.ReferenceExpression]: 'a reference to some variable',
  [SyntaxKind.NestedExpression]: 'an expression nested with parentheses',
  [SyntaxKind.ConstantExpression]: 'a constant expression such as 1 or "foo"',
}

function describeSyntaxKind(kind: SyntaxKind): string {
  const desc = DESCRIPTIONS[kind];
  if (desc === undefined) {
    throw new Error(`Could not describe SyntaxKind '${kind}'`);
  }
  return desc
}

function describeExpected(expected: SyntaxKind[]) {
  if (expected.length === 0) {
    return 'nothing';
  }
  let out = describeSyntaxKind(expected[0]);
  if (expected.length === 1) {
    return out;
  }
  for (let i = 1; i < expected.length-1; i++) {
    const kind = expected[i];
    out += ', ' + describeSyntaxKind(kind);
  }
  out += ' or ' + describeSyntaxKind(expected[expected.length-1])
  return out;
}

class ParseError extends Error {

  public constructor(
    public actual: Token,
    public expected: SyntaxKind[],
  ) {
    super(`got '${actual.text}' but expected ${describeExpected(expected)}`);
  }

}

function isConstructor(token: Token): boolean {
  return token.kind === SyntaxKind.Identifier
      && token.text[0].toUpperCase() === token.text[0];
}

function isBinaryOperatorLike(token: Token): boolean {
  return token.kind === SyntaxKind.CustomOperator;
}

function isPrefixOperatorLike(token: Token): boolean {
  return token.kind === SyntaxKind.CustomOperator;
}

const enum OperatorMode {
  None   = 0,
  Prefix = 1,
  InfixL = 2,
  InfixR = 4,
  Infix = 6,
  Suffix = 8,
}

interface OperatorInfo {
  name: string,
  mode: OperatorMode,
  precedence: number,
}

const EXPR_OPERATOR_TABLE: Array<[string, OperatorMode, number]> = [
  ["**", OperatorMode.InfixR, 11],
  ["*", OperatorMode.InfixL, 8],
  ["/", OperatorMode.InfixL, 8],
  ["+", OperatorMode.InfixL, 7],
  ["-", OperatorMode.InfixL, 7],
  ["<", OperatorMode.InfixL, 6],
  [">", OperatorMode.InfixL, 6],
  ["<=", OperatorMode.InfixL, 5],
  [">=", OperatorMode.InfixL, 5],
  ["==", OperatorMode.InfixL, 5],
  ["!=", OperatorMode.InfixL, 5],
  ["<*", OperatorMode.InfixL, 4],
  [":", OperatorMode.InfixL, 3],
  ["<|>", OperatorMode.InfixL, 2],
  ["<?>", OperatorMode.InfixL, 1],
  ["$", OperatorMode.InfixR, 0]
];

export class Parser {

  private prefixExprOperators = new Set<string>();
  private binaryExprOperators = new Map<string, OperatorInfo>();
  private suffixExprOperators = new Set<string>();

  public constructor(
    public tokens: Stream<Token>,
  ) {
    for (const [name, mode, precedence] of EXPR_OPERATOR_TABLE) {
      this.binaryExprOperators.set(name, { name, mode, precedence });
    }
  }

  private getToken(): Token {
    return this.tokens.get();
  }

  private peekToken(offset = 1): Token {
    return this.tokens.peek(offset);
  }

  private assertToken<K extends Token['kind']>(token: Token, expectedKind: K): void {
    if (token.kind !== expectedKind) {
      this.raiseParseError(token, [ expectedKind ]);
    }
  }

  private expectToken<K extends TokenKind>(expectedKind: K): Token & { kind: K } {
    const token = this.getToken();
    if (token.kind !== expectedKind) {
      this.raiseParseError(token, [ expectedKind ])
    }
    return token as Token & { kind: K };
  }

  private raiseParseError(actual: Token, expected: SyntaxKind[]): never {
    throw new ParseError(actual, expected);
  }

  private peekTokenAfterModifiers(): Token {
    let t0;
    for (let i = 1;;i++) {
      t0 = this.peekToken(i);
      if (t0.kind !== SyntaxKind.PubKeyword) {
        break;
      }
    }
    return t0;
  }

  public parseReferenceTypeExpression(): ReferenceTypeExpression {
    const name = this.expectToken(SyntaxKind.Identifier);
    return new ReferenceTypeExpression([], name);
  }

  public parseTypeExpression(): TypeExpression {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.Identifier:
        return this.parseReferenceTypeExpression();
      default:
        throw new ParseError(t0, [ SyntaxKind.Identifier ]);
    }
  }

  public parseConstantExpression(): ConstantExpression {
    const token = this.getToken()
    if (token.kind !== SyntaxKind.StringLiteral
      && token.kind !== SyntaxKind.Integer) {
      this.raiseParseError(token, [ SyntaxKind.StringLiteral, SyntaxKind.Integer ])
    }
    return new ConstantExpression(token);
  }

  public parseQualifiedName(): QualifiedName {
    const modulePath: Array<[Identifier, Dot]> = [];
    let name = this.expectToken(SyntaxKind.Identifier)
    for (;;) {
      const t1 = this.peekToken()
      if (t1.kind !== SyntaxKind.Dot) {
        break;
      }
      modulePath.push([name, t1]);
      name = this.expectToken(SyntaxKind.Identifier)
    }
    return new QualifiedName(modulePath, name);
  }

  public parseReferenceExpression(): ReferenceExpression {
    return new ReferenceExpression(this.parseQualifiedName());
  }

  private parseExpressionWithParens(): Expression {
    const lparen = this.expectToken(SyntaxKind.LParen)
    const t1 = this.peekToken();
    if (t1.kind === SyntaxKind.RParen) {
      this.getToken();
      return new TupleExpression(lparen, [], t1);
    } else if (t1.kind === SyntaxKind.Constructor) {
    } else {
      const expression = this.parseExpression();
      const t2 = this.expectToken(SyntaxKind.RParen);
      return new NestedExpression(lparen, expression, t2);
    }
  }

  private parseExpressionNoOperators(): Expression {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.LParen:
        return this.parseExpressionWithParens();
      case SyntaxKind.Identifier:
        return this.parseReferenceExpression();
      case SyntaxKind.Integer:
      case SyntaxKind.StringLiteral:
        return this.parseConstantExpression();
      default:
        this.raiseParseError(t0, [
          SyntaxKind.TupleExpression,
          SyntaxKind.NestedExpression,
          SyntaxKind.ConstantExpression,
          SyntaxKind.ReferenceExpression
        ]);
    }
  }

  private parseUnaryExpression(): Expression {
    let result = this.parseExpressionNoOperators()
    const prefixes = [];
    for (;;) {
      const t0 = this.peekToken();
      if (!isPrefixOperatorLike(t0)) {
        break;
      }
      if (!this.prefixExprOperators.has(t0.text)) {
        break;
      }
      prefixes.push(t0);
      this.getToken()
    }
    for (let i = prefixes.length-1; i >= 0; i--) {
      const operator = prefixes[i];
      result = new PrefixExpression(operator, result);
    }
    return result;
  }

  private parseBinaryOperatorAfterExpr(lhs: Expression, minPrecedence: number) {
    for (;;) {
      const t0 = this.peekToken();
      if (!isBinaryOperatorLike(t0)) {
        break;
      }
      const info0 = this.binaryExprOperators.get(t0.text);
      if (info0 === undefined || info0.precedence < minPrecedence) {
        break;
      }
      this.getToken();
      let rhs = this.parseUnaryExpression();
      for (;;) {
        const t1 = this.peekToken();
        if (!isBinaryOperatorLike(t1)) {
          break;
        }
        const info1 = this.binaryExprOperators.get(t1.text);
        if (info1 === undefined
          || info1.precedence < info0.precedence
          || (info1.precedence === info0.precedence && (info1.mode & OperatorMode.InfixR) === 0)) {
          break;
        }
        rhs = this.parseBinaryOperatorAfterExpr(rhs, info0.precedence);
      }
      lhs = new InfixExpression(lhs, t0, rhs);
    }
    return lhs;
  }

  public parseExpression(): Expression {
    const lhs = this.parseUnaryExpression();
    return this.parseBinaryOperatorAfterExpr(lhs, 0);
  }

  public parseStructDeclaration(): StructDeclaration {
    const structKeyword = this.expectToken(SyntaxKind.StructKeyword);
    const name = this.expectToken(SyntaxKind.Identifier);
    const t2 = this.peekToken()
    let members = null;
    if (t2.kind === SyntaxKind.BlockStart) {
      this.getToken();
      members = [];
      for (;;) {
        const name = this.expectToken(SyntaxKind.Identifier);
        const colon = this.expectToken(SyntaxKind.Colon);
        const typeExpr = this.parseTypeExpression();
        const member = new StructDeclarationField(name, colon, typeExpr);
        members.push(member);
      }
    } else {
      this.assertToken(t2, SyntaxKind.LineFoldEnd);
    }
    return new StructDeclaration(structKeyword, name, members);
  }

  private parsePatternStartingWithConstructor() {
    const name = this.expectToken(SyntaxKind.Constructor);
    const t2 = this.peekToken();
    if (t2.kind === SyntaxKind.LBrace) {
      this.getToken();
      const fields = [];
      let rbrace;
      for (;;) {
        const t3 = this.peekToken();
        if (t3.kind === SyntaxKind.RBrace) {
          rbrace = t3;
          break;
        } else if (t3.kind === SyntaxKind.Identifier) {
          this.getToken();
          const t4 = this.peekToken();
          if (t4.kind === SyntaxKind.Equals) {
            this.getToken();
            const pattern = this.parsePattern();
            fields.push(new FieldStructPatternElement(t3, t4, pattern));
          } else {
            fields.push(new PunnedFieldStructPatternElement(t3));
          }
        } else if (t3.kind === SyntaxKind.DotDot) {
          this.getToken();
          fields.push(new VariadicStructPatternElement(t3, null));
        } else {
          this.raiseParseError(t3, [ SyntaxKind.Identifier, SyntaxKind.DotDot ]);
        }
        const t5 = this.peekToken();
        if (t5.kind === SyntaxKind.Comma) {
          this.getToken();
        } else if (t5.kind === SyntaxKind.RBrace) {
          rbrace = t5;
          break;
        } else {
          this.raiseParseError(t5, [ SyntaxKind.Comma, SyntaxKind.RBrace ]);
        }
      }
      return new StructPattern(name, t2, fields, rbrace);
    } else {
      const patterns = [];
      for (;;) {
        const t3 = this.peekToken();
        if (t3.kind === SyntaxKind.RParen) {
          break;
        }
        patterns.push(this.parsePattern());
      }
      return new NamedTuplePattern(name, patterns);
    }
  }

  public parseTuplePattern(): TuplePattern {
    const lparen = this.expectToken(SyntaxKind.LParen);
    const elements = [];
    let rparen;
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.RParen) {
        rparen = t1;
        break;
      }
      elements.push(this.parsePattern());
      const t2 = this.peekToken();
      if (t2.kind === SyntaxKind.Comma) {
        this.getToken();
      } else if (t2.kind === SyntaxKind.RParen) {
        rparen = t2;
        break;
      } else {
        this.raiseParseError(t2, [ SyntaxKind.Comma, SyntaxKind.RParen ]);
      }
    }
    this.getToken();
    return new TuplePattern(lparen, elements, rparen);
  }

  public parsePattern(): Pattern {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.LParen:
      {
        this.getToken();
        const t1 = this.peekToken();
        if (t1.kind === SyntaxKind.Constructor) {
          return this.parsePatternStartingWithConstructor();
        } else {
          return this.parseTuplePattern();
        }
      }
      case SyntaxKind.Identifier:
        this.getToken();
        return new BindPattern(t0);
      default:
        this.raiseParseError(t0, [ SyntaxKind.Identifier ]);
    }
  }

  public parseParam(): Param {
    const pattern = this.parsePattern();
    return new Param(pattern);
  }

  public parseDeclartionWithLetKeyword(): LetDeclaration {
    let t0 = this.getToken();
    let pubKeyword = null;
    let mutKeyword = null;
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    if (t0.kind !== SyntaxKind.LetKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.LetKeyword ]);
    }
    const t1 = this.peekToken();
    if (t1.kind === SyntaxKind.MutKeyword) {
      this.getToken();
      mutKeyword = t1;
    }
    const pattern = this.parsePattern();
    const params = [];
    for (;;) {
      const t2 = this.peekToken();
      if (t2.kind === SyntaxKind.Colon
        || t2.kind === SyntaxKind.BlockStart
        || t2.kind === SyntaxKind.Equals
        || t2.kind === SyntaxKind.LineFoldEnd) {
        break;
      }
      params.push(this.parseParam());
    }
    let typeAssert = null;
    let t3 = this.getToken();
    if (t3.kind === SyntaxKind.Colon) {
      const typeExpression = this.parseTypeExpression();
      typeAssert = new TypeAssert(t3, typeExpression);
      t3 = this.getToken();
    }
    let body = null;
    switch (t3.kind) {
      case SyntaxKind.BlockStart:
      {
        const elements = [];
        for (;;) {
          const t4 = this.peekToken();
          if (t4.kind === SyntaxKind.BlockEnd) {
            break;
          }
          elements.push(this.parseLetBodyElement());
        }
        body = new BlockBody(t3, elements);
        t3 = this.getToken();
        break;
      }
      case SyntaxKind.Equals:
      {
        const expression = this.parseExpression();
        body = new ExprBody(t3, expression);
        t3 = this.getToken();
        break;
      }
      case SyntaxKind.LineFoldEnd:
        break;
    }
    if (t3.kind !== SyntaxKind.LineFoldEnd) {
      this.raiseParseError(t3, [ SyntaxKind.LineFoldEnd ]);
    }
    return new LetDeclaration(
      pubKeyword,
      t0,
      mutKeyword,
      pattern,
      params,
      typeAssert,
      body
    );
  }

  public parseExpressionStatement(): ExpressionStatement {
    const expression = this.parseExpression();
    this.expectToken(SyntaxKind.LineFoldEnd)
    return new ExpressionStatement(expression);
  }

  public parseImportDeclaration(): ImportDeclaration {
    const importKeyword = this.expectToken(SyntaxKind.ImportKeyword);
    const importSource = this.expectToken(SyntaxKind.StringLiteral);
    return new ImportDeclaration(importKeyword, importSource);
  }

  public parseSourceFileElement(): SourceFileElement {

    const t0 = this.peekTokenAfterModifiers();
    switch (t0.kind) {
      case SyntaxKind.LetKeyword:
        return this.parseDeclartionWithLetKeyword();
      case SyntaxKind.ImportKeyword:
        return this.parseImportDeclaration();
      case SyntaxKind.StructKeyword:
        return this.parseStructDeclaration();
      default:
        return this.parseExpressionStatement();
    }
  }

  public parseSourceFile(): SourceFile {
    const elements = [];
    let eof;
    for (;;) {
      const t0 = this.peekToken();
      if (t0.kind === SyntaxKind.EndOfFile) {
        eof = t0;
        break;
      }
      const element = this.parseSourceFileElement();
      elements.push(element);
    }
    return new SourceFile(elements, eof);
  }

}

