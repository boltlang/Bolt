
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
  NamedPattern,
  LetDeclaration,
  TypeAssert,
  ExprBody,
  BlockBody,
  NestedExpression,
  NamedTuplePattern,
  StructPattern,
  VariadicStructPatternElement,
  PunnedStructPatternField,
  StructPatternField,
  TuplePattern,
  InfixExpression,
  TextFile,
  CallExpression,
  LetBodyElement,
  ReturnStatement,
  StructExpression,
  StructExpressionField,
  PunnedStructExpressionField,
  IfStatementCase,
  IfStatement,
  MemberExpression,
  IdentifierAlt,
  WrappedOperator,
  ArrowTypeExpression,
  EnumDeclarationStructElement,
  EnumDeclaration,
  EnumDeclarationTupleElement,
  VarTypeExpression,
  TypeDeclaration,
  AppTypeExpression,
  NestedPattern,
  NestedTypeExpression,
  MatchArm,
  MatchExpression,
  LiteralPattern,
  DisjunctivePattern,
  TupleTypeExpression,
  ModuleDeclaration,
  isExprOperator,
  ClassConstraint,
  ClassDeclaration,
  ClassKeyword,
  InstanceDeclaration,
  ClassConstraintClause,
  AssignStatement,
} from "./cst"
import { Stream } from "./util";

export class ParseError extends Error {

  public constructor(
    public file: TextFile,
    public actual: Token,
    public expected: SyntaxKind[],
  ) {
    super(`Uncaught parse error`);
  }

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
    public file: TextFile,
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

  private assertToken<K extends Token['kind']>(token: Token, expectedKind: K): asserts token is Token & { kind: K } {
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
    throw new ParseError(this.file, actual, expected);
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
    const modulePath = [];
    for (;;) {
      const t1 = this.peekToken(1);
      const t2 = this.peekToken(2);
      if (t1.kind !== SyntaxKind.IdentifierAlt || t2.kind !== SyntaxKind.Dot) {
        break;
      }
      this.getToken();
      this.getToken();
      modulePath.push([t1, t2] as [IdentifierAlt, Dot]);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    return new ReferenceTypeExpression(modulePath, name);
  }

  public parseVarTypeExpression(): VarTypeExpression {
    const name = this.expectToken(SyntaxKind.Identifier);
    return new VarTypeExpression(name);
  }

  public parsePrimitiveTypeExpression(): TypeExpression {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.Identifier:
        return this.parseVarTypeExpression();
      case SyntaxKind.IdentifierAlt:
        return this.parseReferenceTypeExpression();
      case SyntaxKind.LParen:
      {
        this.getToken();
        const elements = [];
        let rparen;
        for (;;) {
          const t2 = this.peekToken();
          if (t2.kind === SyntaxKind.RParen) {
            this.getToken();
            rparen = t2;
            break;
          }
          const typeExpr = this.parseTypeExpression();
          elements.push(typeExpr);
          const t3 = this.getToken();
          if (t3.kind === SyntaxKind.RParen) {
            rparen = t3;
            break;
          } else if (t3.kind === SyntaxKind.Comma) {
            continue;
          } else {
            this.raiseParseError(t3, [ SyntaxKind.Comma, SyntaxKind.RParen ]);
          }
        }
        if (elements.length === 1) {
          return new NestedTypeExpression(t0, elements[0], rparen);
        }
        return new TupleTypeExpression(t0, elements, rparen);
      }
      default:
        this.raiseParseError(t0, [ SyntaxKind.IdentifierAlt ]);
    }
  }

  private parseAppTypeExpressionOrBelow(): TypeExpression {
    const operator = this.parsePrimitiveTypeExpression();
    const args = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.RParen
          || t1.kind === SyntaxKind.RBrace
          || t1.kind === SyntaxKind.RBracket
          || t1.kind === SyntaxKind.Comma
          || t1.kind === SyntaxKind.Equals
          || t1.kind === SyntaxKind.BlockStart
          || t1.kind === SyntaxKind.LineFoldEnd
          || t1.kind === SyntaxKind.RArrow) {
        break;
      }
      args.push(this.parsePrimitiveTypeExpression());
    }
    if (args.length === 0) {
      return operator;
    }
    return new AppTypeExpression(operator, args);
  }

  public parseTypeExpression(): TypeExpression {
    let returnType = this.parseAppTypeExpressionOrBelow();
    const paramTypes = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind !== SyntaxKind.RArrow) {
        break;
      }
      this.getToken();
      paramTypes.push(returnType);
      returnType = this.parseAppTypeExpressionOrBelow();
    }
    if (paramTypes.length === 0) {
      return returnType;
    }
    return new ArrowTypeExpression(paramTypes, returnType);
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
    const modulePath: Array<[IdentifierAlt, Dot]> = [];
    for (;;) {
      const t0 = this.peekToken(1);
      const t1 = this.peekToken(2);
      if (t0.kind !== SyntaxKind.IdentifierAlt || t1.kind !== SyntaxKind.Dot) {
        break;
      }
      this.getToken();
      this.getToken();
      modulePath.push([t0, t1]);
    }
    const name = this.getToken();
    if (name.kind !== SyntaxKind.Identifier && name.kind !== SyntaxKind.IdentifierAlt) {
      this.raiseParseError(name, [ SyntaxKind.Identifier, SyntaxKind.IdentifierAlt ]);
    }
    return new ReferenceExpression(modulePath, name);
  }

  private parseExpressionWithParens(): Expression {
    const elements = [];
    const lparen = this.expectToken(SyntaxKind.LParen)
    let rparen;
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.RParen) {
        rparen = t1;
        this.getToken();
        break;
      }
      const expression = this.parseExpression();
      elements.push(expression);
      const t2 = this.getToken();
      if (t2.kind === SyntaxKind.Comma) {
        continue;
      } else if (t2.kind === SyntaxKind.RParen) {
        rparen = t2;
        break;
      } else {
        this.raiseParseError(t2, [ SyntaxKind.Comma, SyntaxKind.RParen ]);
      }
    }
    if (elements.length === 1) {
      return new NestedExpression(lparen, elements[0], rparen);
    }
    return new TupleExpression(lparen, elements, rparen);
  }

  private parsePrimitiveExpression(): Expression {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.LParen:
        return this.parseExpressionWithParens();
      case SyntaxKind.Identifier:
      case SyntaxKind.IdentifierAlt:
        return this.parseReferenceExpression();
      case SyntaxKind.Integer:
      case SyntaxKind.StringLiteral:
        return this.parseConstantExpression();
      case SyntaxKind.MatchKeyword:
      {
        this.getToken();
        let expression = null
        const t1 = this.peekToken();
        if (t1.kind !== SyntaxKind.BlockStart) {
          expression = this.parseExpression();
        }
        this.expectToken(SyntaxKind.BlockStart);
        const arms = [];
        for (;;) {
          const t2 = this.peekToken();
          if (t2.kind === SyntaxKind.BlockEnd) {
            this.getToken();
            break;
          }
          const pattern = this.parsePattern();
          const rarrowAlt = this.expectToken(SyntaxKind.RArrowAlt);
          const expression = this.parseExpression();
          arms.push(new MatchArm(pattern, rarrowAlt, expression));
          this.expectToken(SyntaxKind.LineFoldEnd);
        }
        return new MatchExpression(t0, expression, arms);
      }
      case SyntaxKind.LBrace:
      {
        this.getToken();
        const fields = [];
        let rbrace;
        for (;;) {
          const t2 = this.peekToken();
          if (t2.kind === SyntaxKind.RBrace) {
            this.getToken();
            rbrace = t2;
            break;
          }
          let field;
          const t3 = this.getToken();
          if (t3.kind === SyntaxKind.Identifier) {
            const t4 = this.peekToken();
            if (t4.kind === SyntaxKind.Equals) {
              this.getToken();
              const expression = this.parseExpression();
              field = new StructExpressionField(t3, t4, expression);
            } else {
              field = new PunnedStructExpressionField(t3);
            }
          } else {
            // TODO add spread fields
            this.raiseParseError(t3, [ SyntaxKind.Identifier ]);
          }
          fields.push(field);
          const t5 = this.peekToken();
          if (t5.kind === SyntaxKind.Comma) {
            this.getToken();
            continue;
          } else if (t5.kind === SyntaxKind.RBrace) {
            this.getToken();
            rbrace = t5;
            break;
          }
        }
        return new StructExpression(t0, fields, rbrace);
      }
      default:
        this.raiseParseError(t0, [
          SyntaxKind.TupleExpression,
          SyntaxKind.NestedExpression,
          SyntaxKind.ConstantExpression,
          SyntaxKind.ReferenceExpression
        ]);
    }
  }

  private tryParseMemberExpression(): Expression {
    const expression = this.parsePrimitiveExpression();
    const path: Array<[Dot, Identifier]> = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind !== SyntaxKind.Dot) {
        break;
      }
      this.getToken();
      const name = this.expectToken(SyntaxKind.Identifier);
      path.push([t1, name]);
    }
    if (path.length === 0) {
      return expression;
    }
    return new MemberExpression(expression, path);
  }

  private tryParseCallExpression(): Expression {
    const func = this.tryParseMemberExpression();
    const args = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.LineFoldEnd
        || t1.kind === SyntaxKind.RBrace
        || t1.kind === SyntaxKind.RBracket
        || t1.kind === SyntaxKind.RParen
        || t1.kind === SyntaxKind.BlockStart
        || t1.kind === SyntaxKind.Comma
        || isExprOperator(t1)) {
        break;
      }
      args.push(this.tryParseMemberExpression());
    }
    if (args.length === 0) {
      return func
    }
    return new CallExpression(func, args);
  }

  private parseUnaryExpression(): Expression {
    let result = this.tryParseCallExpression()
    const prefixes = [];
    for (;;) {
      const t0 = this.peekToken();
      if (!isExprOperator(t0)) {
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
      if (!isExprOperator(t0)) {
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
        if (!isExprOperator(t1)) {
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

  public parseTypeDeclaration(): TypeDeclaration {
    let pubKeyword = null;
    let t0 = this.getToken();
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    if (t0.kind !== SyntaxKind.TypeKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.TypeKeyword ]);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    const typeVars = [];
    let t1 = this.getToken();
    while (t1.kind === SyntaxKind.Identifier) {
      typeVars.push(t1);
      t1 = this.getToken();
    }
    if (t1.kind !== SyntaxKind.Equals) {
      this.raiseParseError(t1, [ SyntaxKind.Equals ]);
    }
    const typeExpr = this.parseTypeExpression();
    this.expectToken(SyntaxKind.LineFoldEnd);
    return new TypeDeclaration(pubKeyword, t0, name, typeVars, t1, typeExpr);
  }

  public parseEnumDeclaration(): EnumDeclaration {
    let pubKeyword = null;
    let t0 = this.getToken();
    if (t0.kind == SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    if (t0.kind !== SyntaxKind.EnumKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.EnumKeyword ]);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    let t1 = this.getToken();
    const varExps = [];
    while (t1.kind === SyntaxKind.Identifier) {
      varExps.push(t1);
      t1 = this.getToken();
    }
    let members = null;
    if (t1.kind === SyntaxKind.BlockStart) {
      members = [];
      for (;;) {
        const t2 = this.peekToken();
        if (t2.kind === SyntaxKind.BlockEnd) {
          this.getToken();
          break;
        }
        const name = this.expectToken(SyntaxKind.IdentifierAlt);
        const t3 = this.peekToken();
        let member;
        if (t3.kind === SyntaxKind.BlockStart) {
          this.getToken();
          const members = [];
          for (;;) {
            const name = this.expectToken(SyntaxKind.Identifier);
            const colon = this.expectToken(SyntaxKind.Colon);
            const typeExpr = this.parseTypeExpression();
            this.expectToken(SyntaxKind.LineFoldEnd);
            members.push(new StructDeclarationField(name, colon, typeExpr));
            const t4 = this.peekToken();
            if (t4.kind === SyntaxKind.BlockEnd) {
              this.getToken();
              break;
            }
          }
          member = new EnumDeclarationStructElement(name, t3, members);
        } else {
          const typeExps = [];
          for (;;) {
            const t3 = this.peekToken();
            if (t3.kind === SyntaxKind.LineFoldEnd) {
              break;
            }
            const typeExpr = this.parsePrimitiveTypeExpression();
            typeExps.push(typeExpr);
          }
          member = new EnumDeclarationTupleElement(name, typeExps);
        }
        members.push(member);
        this.expectToken(SyntaxKind.LineFoldEnd);
      }
      t1 = this.getToken();
    }
    if (t1.kind !== SyntaxKind.LineFoldEnd) {
      this.raiseParseError(t1, [ SyntaxKind.Identifier, SyntaxKind.BlockStart, SyntaxKind.LineFoldEnd ]);
    }
    return new EnumDeclaration(pubKeyword, t0, name, varExps, members);
  }

  public parseStructDeclaration(): StructDeclaration {
    let pubKeyword = null;
    let t0 = this.getToken();
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    if (t0.kind !== SyntaxKind.StructKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.StructKeyword ]);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    let t2 = this.getToken();
    const typeVars = [];
    while (t2.kind === SyntaxKind.Identifier) {
      typeVars.push(t2);
      t2 = this.getToken();
    }
    let members = null;
    if (t2.kind === SyntaxKind.BlockStart) {
      members = [];
      for (;;) {
        const t3 = this.peekToken();
        if (t3.kind === SyntaxKind.BlockEnd) {
          this.getToken();
          break;
        }
        const name = this.expectToken(SyntaxKind.Identifier);
        const colon = this.expectToken(SyntaxKind.Colon);
        const typeExpr = this.parseTypeExpression();
        this.expectToken(SyntaxKind.LineFoldEnd);
        const member = new StructDeclarationField(name, colon, typeExpr);
        members.push(member);
      }
      t2 = this.getToken();
    }
    if (t2.kind !== SyntaxKind.LineFoldEnd) {
      this.raiseParseError(t2, [ SyntaxKind.LineFoldEnd, SyntaxKind.BlockStart, SyntaxKind.Identifier ]);
    }
    return new StructDeclaration(pubKeyword, t0, name, typeVars, members);
  }

  private parsePatternStartingWithConstructor() {
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
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

  public parseStructPattern(): StructPattern {
    const t2 = this.expectToken(SyntaxKind.LBrace);
    const fields = [];
    let rbrace;
    for (;;) {
      const t3 = this.peekToken();
      if (t3.kind === SyntaxKind.RBrace) {
        this.getToken();
        rbrace = t3;
        break;
      } else if (t3.kind === SyntaxKind.Identifier) {
        this.getToken();
        const t4 = this.peekToken();
        if (t4.kind === SyntaxKind.Equals) {
          this.getToken();
          const pattern = this.parsePattern();
          fields.push(new StructPatternField(t3, t4, pattern));
        } else {
          fields.push(new PunnedStructPatternField(t3));
        }
      } else if (t3.kind === SyntaxKind.DotDot) {
        this.getToken();
        const t4 = this.peekToken();
        let rest = null;
        if (t4.kind !== SyntaxKind.RBrace) {
          rest = this.parsePattern();
        }
        fields.push(new VariadicStructPatternElement(t3, rest));
      } else {
        this.raiseParseError(t3, [ SyntaxKind.Identifier, SyntaxKind.DotDot ]);
      }
      const t5 = this.peekToken();
      if (t5.kind === SyntaxKind.Comma) {
        this.getToken();
      } else if (t5.kind === SyntaxKind.RBrace) {
        this.getToken();
        rbrace = t5;
        break;
      } else {
        this.raiseParseError(t5, [ SyntaxKind.Comma, SyntaxKind.RBrace ]);
      }
    }
    return new StructPattern(t2, fields, rbrace);
  }

  public parsePrimitivePattern(): Pattern {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.LBrace:
        return this.parseStructPattern();
      case SyntaxKind.LParen:
      {
        const t1 = this.peekToken(2);
        if (t1.kind === SyntaxKind.IdentifierAlt) {
          this.getToken();
          const pattern = this.parsePatternStartingWithConstructor();
          const t3 = this.expectToken(SyntaxKind.RParen);
          return new NestedPattern(t0, pattern, t3);
        } else {
          return this.parseTuplePattern();
        }
      }
      case SyntaxKind.IdentifierAlt:
      {
        this.getToken();
        return new NamedTuplePattern(t0, []);
      }
      case SyntaxKind.Identifier:
      {
        this.getToken();
        return new NamedPattern(t0);
      }
      case SyntaxKind.StringLiteral:
      case SyntaxKind.Integer:
      {
        this.getToken();
        return new LiteralPattern(t0);
      }
      default:
        this.raiseParseError(t0, [ SyntaxKind.Identifier ]);
    }
  }

  public parsePattern(): Pattern {
    let result: Pattern = this.parsePrimitivePattern();
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind !== SyntaxKind.VBar) {
        break;
      }
      this.getToken();
      const right = this.parsePrimitivePattern();
      result = new DisjunctivePattern(result, t1, right);
    }
    return result;
  }

  public parseParam(): Param {
    const pattern = this.parsePattern();
    return new Param(pattern);
  }

  private lookaheadIsAssignment(): boolean {
    for (let i = 1;; i++) {
      const t0 = this.peekToken(i);
      switch (t0.kind) {
        case SyntaxKind.LineFoldEnd:
        case SyntaxKind.BlockStart:
          return false;
        case SyntaxKind.Assignment:
          return true;
      }
    }
  }

  public parseAssignStatement(): AssignStatement {
    const pattern = this.parsePattern();
    const operator = this.expectToken(SyntaxKind.Assignment);
    const expression = this.parseExpression();
    this.expectToken(SyntaxKind.LineFoldEnd);
    return new AssignStatement(pattern, operator, expression);
  }

  public parseLetBodyElement(): LetBodyElement {
    const t0 = this.peekTokenAfterModifiers();
    switch (t0.kind) {
      case SyntaxKind.LetKeyword:
        return this.parseLetDeclaration();
      case SyntaxKind.ReturnKeyword:
        return this.parseReturnStatement();
      case SyntaxKind.IfKeyword:
        return this.parseIfStatement();
      default:
        if (this.lookaheadIsAssignment()) {
          return this.parseAssignStatement();
        }
        // TODO convert parse errors to include LetKeyword and ReturnKeyword
        return this.parseExpressionStatement();
    }
  }

  public parseLetDeclaration(): LetDeclaration {
    let t0 = this.getToken();
    let pubKeyword = null;
    let mutKeyword = null;
    let foreignKeyword = null;
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    if (t0.kind !== SyntaxKind.LetKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.LetKeyword ]);
    }
    let t1 = this.peekToken();
    if (t1.kind === SyntaxKind.ForeignKeyword) {
      this.getToken();
      foreignKeyword = t1;
      t1 = this.peekToken();
    }
    if (t1.kind === SyntaxKind.MutKeyword) {
      this.getToken();
      mutKeyword = t1;
      t1 = this.peekToken();
    }
    const t2 = this.peekToken(2);
    const t3 = this.peekToken(3);
    let pattern;
    if (t1.kind === SyntaxKind.LParen && t2.kind === SyntaxKind.CustomOperator && t3.kind === SyntaxKind.RParen) {
      this.getToken()
      this.getToken();
      this.getToken();
      pattern = new WrappedOperator(t1, t2, t3);
    } else {
      pattern = this.parsePattern();
    }
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
    let t5 = this.getToken();
    if (t5.kind === SyntaxKind.Colon) {
      const typeExpression = this.parseTypeExpression();
      typeAssert = new TypeAssert(t5, typeExpression);
      t5 = this.getToken();
    }
    let body = null;
    switch (t5.kind) {
      case SyntaxKind.BlockStart:
      {
        const elements = [];
        for (;;) {
          const t4 = this.peekToken();
          if (t4.kind === SyntaxKind.BlockEnd) {
            this.getToken();
            break;
          }
          elements.push(this.parseLetBodyElement());
        }
        body = new BlockBody(t5, elements);
        t5 = this.getToken();
        break;
      }
      case SyntaxKind.Equals:
      {
        const expression = this.parseExpression();
        body = new ExprBody(t5, expression);
        t5 = this.getToken();
        break;
      }
      case SyntaxKind.LineFoldEnd:
        break;
    }
    if (t5.kind !== SyntaxKind.LineFoldEnd) {
      this.raiseParseError(t5, [ SyntaxKind.LineFoldEnd ]);
    }
    return new LetDeclaration(
      pubKeyword,
      t0,
      foreignKeyword,
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

  public parseIfStatement(): IfStatement {
    const ifKeyword = this.expectToken(SyntaxKind.IfKeyword);
    const test = this.parseExpression();
    const blockStart = this.expectToken(SyntaxKind.BlockStart);
    const elements = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.BlockEnd) {
        this.getToken();
        break;
      }
      elements.push(this.parseLetBodyElement());
    }
    this.expectToken(SyntaxKind.LineFoldEnd);
    const cases = [];
    cases.push(new IfStatementCase(ifKeyword, test, blockStart, elements));
    for (;;) {
      const t2 = this.peekToken();
      if (t2.kind === SyntaxKind.ElseKeyword) {
        this.getToken();
        const blockStart = this.expectToken(SyntaxKind.BlockStart);
        const elements = [];
        for (;;) {
          const t3 = this.peekToken();
          if (t3.kind === SyntaxKind.BlockEnd) {
            this.getToken();
            break;
          }
          elements.push(this.parseLetBodyElement());
        }
        this.expectToken(SyntaxKind.LineFoldEnd);
        cases.push(new IfStatementCase(t2, null, blockStart, elements));
        break;
      } else if (t2.kind === SyntaxKind.ElifKeyword) {
        this.getToken();
        const test = this.parseExpression();
        const blockStart = this.expectToken(SyntaxKind.BlockStart);
        for (;;) {
          const t4 = this.peekToken();
          if (t4.kind === SyntaxKind.BlockEnd) {
            this.getToken();
            break;
          }
          elements.push(this.parseLetBodyElement());
        }
        this.expectToken(SyntaxKind.LineFoldEnd);
        cases.push(new IfStatementCase(t2, test, blockStart, elements));
      } else if (t2.kind === SyntaxKind.LineFoldEnd) {
        this.getToken();
        break;
      } else {
        this.raiseParseError(t2, [ SyntaxKind.ElifKeyword, SyntaxKind.ElseKeyword, SyntaxKind.LineFoldEnd ]); 
      }
    }
    return new IfStatement(cases);
  }

  public parseReturnStatement(): ReturnStatement {
    const returnKeyword = this.expectToken(SyntaxKind.ReturnKeyword);
    let expression = null;
    const t1 = this.peekToken();
    if (t1.kind !== SyntaxKind.LineFoldEnd) {
      expression = this.parseExpression();
    }
    this.expectToken(SyntaxKind.LineFoldEnd);
    return new ReturnStatement(returnKeyword, expression);
  }

  public parseImportDeclaration(): ImportDeclaration {
    const importKeyword = this.expectToken(SyntaxKind.ImportKeyword);
    const importSource = this.expectToken(SyntaxKind.StringLiteral);
    return new ImportDeclaration(importKeyword, importSource);
  }

  public parseModuleDeclaration(): ModuleDeclaration {
    let pubKeyword = null;
    let t0 = this.getToken();
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    if (t0.kind !== SyntaxKind.ModKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.ModKeyword ]);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    const blockStart = this.expectToken(SyntaxKind.BlockStart);
    const elements = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.BlockEnd) {
        this.getToken();
        break;
      }
      elements.push(this.parseSourceFileElement());
    }
    this.expectToken(SyntaxKind.LineFoldEnd);
    return new ModuleDeclaration(pubKeyword, t0, name, blockStart, elements);
  }

  private currentLineFoldHasToken(expectedKind: SyntaxKind): boolean {
    for (let i = 1;; i++) {
      const t0 = this.peekToken(i);
      switch (t0.kind) { 
        case SyntaxKind.BlockStart:
        case SyntaxKind.LineFoldEnd:
        case SyntaxKind.EndOfFile:
          return false;
        case expectedKind:
          return true;
      }
    }
  }

  private parseClassConstraint(): ClassConstraint {
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    const types = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.Comma
        || t1.kind === SyntaxKind.RArrowAlt
        || t1.kind === SyntaxKind.BlockStart
        || t1.kind === SyntaxKind.LineFoldEnd) {
          break;
      }
      types.push(this.parsePrimitiveTypeExpression());
    }
    return new ClassConstraint(name, types);
  }

  public parseInstanceDeclaration(): InstanceDeclaration {
    let pubKeyword = null;
    let t0 = this.getToken();
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    this.assertToken(t0, SyntaxKind.InstanceKeyword);
    let clause = null;
    if (this.currentLineFoldHasToken(SyntaxKind.RArrowAlt)) {
      let rarrowAlt;
      const constraints = [];
      for (;;) {
        constraints.push(this.parseClassConstraint());
        const t2 = this.getToken();
        if (t2.kind === SyntaxKind.RArrowAlt) {
          rarrowAlt = t2;
          break;
        } else if (t2.kind !== SyntaxKind.Comma) {
          this.raiseParseError(t2, [ SyntaxKind.RArrowAlt, SyntaxKind.Comma ])
        }
      }
      clause = new ClassConstraintClause(constraints, rarrowAlt);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    const types = [];
    for (;;) {
      const t3 = this.peekToken();
      if (t3.kind === SyntaxKind.BlockStart || t3.kind === SyntaxKind.LineFoldEnd) {
        break;
      }
      const type = this.parseTypeExpression();
      types.push(type);
    }
    this.expectToken(SyntaxKind.BlockStart);
    const elements = [];
    loop: for (;;) {
      const t4 = this.peekToken();
      let element;
      switch (t4.kind) {
        case SyntaxKind.BlockEnd:
          this.getToken();
          break loop;
        case SyntaxKind.LetKeyword:
          element = this.parseLetDeclaration();
          break;
        case SyntaxKind.TypeKeyword:
          element = this.parseTypeDeclaration();
          break;
        default:
          this.raiseParseError(t4, [ SyntaxKind.LetKeyword, SyntaxKind.TypeKeyword, SyntaxKind.BlockEnd ]);
      }
      elements.push(element);
    }
    this.expectToken(SyntaxKind.LineFoldEnd);
    return new InstanceDeclaration(pubKeyword, t0, clause, name, types, elements);
  }

  public parseClassDeclaration(): ClassDeclaration {
    let pubKeyword = null;
    let t0 = this.getToken();
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      t0 = this.getToken();
    }
    this.assertToken(t0, SyntaxKind.ClassKeyword);
    let clause = null;
    if (this.currentLineFoldHasToken(SyntaxKind.RArrowAlt)) {
      let rarrowAlt;
      const constraints = [];
      for (;;) {
        constraints.push(this.parseClassConstraint());
        const t2 = this.getToken();
        if (t2.kind === SyntaxKind.RArrowAlt) {
          rarrowAlt = t2;
          break;
        } else if (t2.kind !== SyntaxKind.Comma) {
          this.raiseParseError(t2, [ SyntaxKind.RArrowAlt, SyntaxKind.Comma ])
        }
      }
      clause = new ClassConstraintClause(constraints, rarrowAlt);
    }
    const name = this.expectToken(SyntaxKind.IdentifierAlt);
    const types = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.Identifier) {
        const type = this.parseVarTypeExpression();
        types.push(type);
      } else {
        break;
      }
    }
    this.expectToken(SyntaxKind.BlockStart);
    const elements = [];
    loop: for (;;) {
      const t3 = this.peekToken();
      let element;
      switch (t3.kind) {
        case SyntaxKind.BlockEnd:
          this.getToken();
          break loop;
        case SyntaxKind.LetKeyword:
          element = this.parseLetDeclaration();
          break;
        case SyntaxKind.TypeKeyword:
          element = this.parseTypeDeclaration();
          break;
        default:
          this.raiseParseError(t3, [ SyntaxKind.LetKeyword, SyntaxKind.TypeKeyword, SyntaxKind.BlockEnd ]);
      }
      elements.push(element);
    }
    this.expectToken(SyntaxKind.LineFoldEnd);
    return new ClassDeclaration(pubKeyword, t0 as ClassKeyword, clause, name, types, elements);
  }

  public parseSourceFileElement(): SourceFileElement {
    const t0 = this.peekTokenAfterModifiers();
    switch (t0.kind) {
      case SyntaxKind.LetKeyword:
        return this.parseLetDeclaration();
      case SyntaxKind.ModKeyword:
        return this.parseModuleDeclaration();
      case SyntaxKind.ImportKeyword:
        return this.parseImportDeclaration();
      case SyntaxKind.StructKeyword:
        return this.parseStructDeclaration();
      case SyntaxKind.InstanceKeyword:
        return this.parseInstanceDeclaration();
      case SyntaxKind.ClassKeyword:
        return this.parseClassDeclaration();
      case SyntaxKind.EnumKeyword:
        return this.parseEnumDeclaration();
      case SyntaxKind.TypeKeyword:
        return this.parseTypeDeclaration();
      case SyntaxKind.IfKeyword:
        return this.parseIfStatement();
      default:
        if (this.lookaheadIsAssignment()) {
          return this.parseAssignStatement();
        }
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
    return new SourceFile(this.file, elements, eof);
  }

}

