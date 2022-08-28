
import { kMaxLength } from "buffer";
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
  FunctionDeclaration,
  Param,
  Pattern,
  BindPattern,
  LetDeclaration,
  TypeAssert,
  ExprBody,
  BlockBody,
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

const enum OperatorMode {
  None   = 0,
  Prefix = 1,
  InfixL = 2,
  InfixR = 4,
  Suffix = 8,
}

interface OperatorInfo {
  name: string,
  mode: OperatorMode,
  precedence?: number,
}

export class Parser {

  private exprOperators = new MultiDict<string, OperatorInfo>();

  public constructor(
    public tokens: Stream<Token>,
  ) {

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

  private isPrefixOperator(token: Token): boolean {
    const name = token.text;
    for (const operator of this.exprOperators.get(name)) {
      if (operator.mode & OperatorMode.Prefix) {
        return true;
      }
    }
    return false;
  }

  private isBinaryOperator(token: Token): boolean {
    return token.kind === SyntaxKind.CustomOperator;
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

  public parseReferenceExpression(): ReferenceExpression {
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
    return new ReferenceExpression(modulePath, name);
  }

  private parseExpressionWithParens(): Expression {
    const t0 = this.expectToken(SyntaxKind.LParen)
    const t1 = this.peekToken();
    if (t1.kind === SyntaxKind.RParen) {
      this.getToken();
      return new TupleExpression(t0, [], t1);
    }
    if (isConstructor(t1)) {

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
    let out = this.parseExpressionNoOperators()
    const prefixOperators = [];
    for (;;) {
      const t0 = this.peekToken();
      if (!this.isPrefixOperator(t0)) {
        break;
      }
      prefixOperators.push(t0);
      this.getToken()
    }
    for (let i = prefixOperators.length-1; i >= 0; i--) {
      const op = prefixOperators[i];
      out = new PrefixExpression(op, out);
    }
    return out;
  }

  private parseExpressionWithBinaryOperator(lhs: Expression, minPrecedence: number) {
    for (;;) {
      const t0 = this.peekToken();
      if (!this.isBinaryOperator(t0)) {
        break;
      }
    }
    return lhs;
  }

  public parseExpression(): Expression {
    const lhs = this.parseUnaryExpression();
    return this.parseExpressionWithBinaryOperator(lhs, 0);
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

  public parsePattern(): Pattern {
    const t0 = this.peekToken();
    switch (t0.kind) {
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
    for (;;) {
      const t0 = this.peekToken();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      const element = this.parseSourceFileElement();
      elements.push(element);
    }
    return new SourceFile(elements);
  }

}

