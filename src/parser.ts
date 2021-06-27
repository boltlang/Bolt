
import {
  BindPattern,
  CallExpression,
  ConstantExpression,
  Expression,
  Parameter,
  QualName,
  RecordDeclaration,
  RecordDeclarationField,
  ReferenceExpression,
  SourceElement,
  SourceFile,
  TypeExpression,
  TypeParameter,
  TypeReferenceExpression,
  FunctionBodyElement,
  InlineDefinitionBody,
  BlockDefinitionBody,
  VariableDefinition,
  FunctionDefinition,
  SyntaxKind,
  TuplePattern,
  Pattern,
  BinaryExpression,
  NestedExpression,
  MatchExpression,
  MatchArm,
  RecordDeclarationBody
} from "./cst";
import {
  Diagnostics,
  ExpectedParse,
  UnexpectedTokenDiagnostic,
} from "./diagnostics";
import { TextFile } from "./text";
import { DotSign, Identifier, Token, TokenType } from "./token";
import { Stream } from "./util"

type TokenStream = Stream<Token>;

function isOperator(token: Token): boolean {
  return token.type === TokenType.CustomOperator
}

const enum OperatorMode {
  Prefix = 1,
  InfixL = 2,
  InfixR = 3,
  Suffix = 4,
  Infix  = InfixL | InfixR,
}

interface OperatorInfo {
  text: string;
  mode: OperatorMode;
  precedence: number;
}

const DEFAULT_OPERATORS: Array<[string, OperatorMode, number]> = [
  ['*', OperatorMode.InfixL, 1],
  ['/', OperatorMode.InfixL, 1],
  ['+', OperatorMode.InfixL, 2],
  ['-', OperatorMode.InfixL, 2],
  ['$', OperatorMode.InfixR, 10],
];

export class ParseError extends Error {

  constructor(
    public file: TextFile,
    public actual: Token,
    public expected: ExpectedParse[],
  ) {
    super(`Uncaught parse error`);
  }

  public getDiagnostic() {
    return new UnexpectedTokenDiagnostic(
      this.file,
      this.actual,
      this.expected
    );
  }

}

export class Parser {

  private exprOperatorTable: OperatorInfo[] = [];

  constructor(
    public file: TextFile,
    public diagnostics: Diagnostics,
    public tokens: TokenStream,
  ) {
    for (const [text, mode, precedence] of DEFAULT_OPERATORS) {
      this.exprOperatorTable.push({
        text,
        mode,
        precedence,
      });
    }
  }

  private getToken(): Token {
    return this.tokens.get();
  }

  private peekToken(offset = 1): Token {
    return this.tokens.peek(offset);
  }

  private raiseParseError(actual: Token, expected: ExpectedParse[]): never {
    throw new ParseError(this.file, actual, expected);
  }

  private expectToken<T extends TokenType>(type: T): Token & { type: T } {
    const t0 = this.peekToken();
    if (t0.type !== type) {
      this.raiseParseError(t0, [ type ]);
    }
    this.getToken();
    return t0 as any;
  }

  public parseQualName(): QualName {
    const t0 = this.peekToken()
    if (t0.type !== TokenType.Identifier) {
      this.raiseParseError(t0, [ TokenType.Identifier ]);
    }
    this.getToken();
    let name = t0;
    const modulePath: Array<[Identifier, DotSign]> = []
    for (;;) {
      const t1 = this.peekToken(1)
      const t2 = this.peekToken(2);
      if (t1.type !== TokenType.DotSign || t2.getStartLine() > t1.getStartLine()) {
        break;
      }
      if (t2.type !== TokenType.Identifier) {
        this.raiseParseError(t2, [ TokenType.Identifier ]);
      }
      modulePath.push([name, t1]);
      name = t2;
      this.getToken();
      this.getToken();
    }
    return new QualName(modulePath, name);
  }

  public parseMatchExpression(): MatchExpression {

    const matchKeyword = this.peekToken()
    if (matchKeyword.type !== TokenType.MatchKeyword) {
      this.raiseParseError(matchKeyword, [ TokenType.MatchKeyword ]);
    }
    this.getToken();

    const expression = this.parseExpression()

    const t1 = this.peekToken();
    if (t1.type !== TokenType.BlockStart) {
      this.raiseParseError(t1, [ TokenType.BlockStart ]);
    }
    this.getToken();

    const arms = [];

    for (;;) {
      const t2 = this.peekToken();
      if (t2.type === TokenType.BlockEnd) {
        this.getToken();
        break;
      }
      const pattern = this.parsePattern();
      const equalSign = this.peekToken()
      if (equalSign.type !== TokenType.EqualSign) {
        this.raiseParseError(equalSign, [ TokenType.EqualSign ]);
      }
      this.getToken();
      const expression = this.parseExpression();
      arms.push(new MatchArm(pattern, equalSign, expression));
      const t3 = this.peekToken()
      if (t3.type !== TokenType.LineFoldEnd) {
        this.raiseParseError(t3, [ TokenType.LineFoldEnd ]);
      }
      this.getToken();
    }

    return new MatchExpression(
      matchKeyword,
      expression,
      t1.dotSign,
      arms,
    );

  }

  public parsePrimitiveExpression(): Expression {
    const t0 = this.peekToken();
    switch (t0.type) {
      case TokenType.MatchKeyword:
        return this.parseMatchExpression();
      case TokenType.Identifier:
        this.getToken();
        return new ReferenceExpression(t0);
      case TokenType.DecimalInteger:
        this.getToken();
        return new ConstantExpression(t0);
      case TokenType.LParen:
        this.getToken();
        const expression = this.parseExpression();
        const t1 = this.peekToken()
        if (t1.type !== TokenType.RParen) {
          this.raiseParseError(t1, [ TokenType.RParen ]);
        }
        this.getToken();
        return new NestedExpression(t0, expression, t1);
      default:
        this.raiseParseError(t0, [ TokenType.Identifier ]);
    }
  }

  public parseCallExpression(): Expression {
    const expr0 = this.parsePrimitiveExpression();
    const args = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.type === TokenType.LineFoldEnd
        || t1.type === TokenType.BlockEnd
        || t1.type === TokenType.BlockStart
        || t1.type === TokenType.RParen
        || isOperator(t1)) {
        break;
      }
      const arg = this.parsePrimitiveExpression()
      args.push(arg);
    }
    if (args.length === 0) {
      return expr0;
    }
    return new CallExpression(expr0, args);
  }

  public parseUnaryExpression(): Expression {
    const stack = [];
    for (;;) {
      const t0 = this.peekToken()
      if (!isOperator(t0)) {
        break;
      }
      const info0 = this.getOperatorInfo(OperatorMode.Prefix, t0.getText());
      if (info0 === null) {
        break;
      }
      stack.push(t0);
    }
    // TODO create PrefixExpression using the operator stack
    return this.parseCallExpression();
  }

  private getOperatorInfo(mode: OperatorMode, text: string): OperatorInfo | null {
    for (const info of this.exprOperatorTable) {
      if (info.mode & mode && info.text === text) {
        return info;
      }
    }
    return null;
  }

  public parseOperatorsAfterExpression(lhs: Expression, minPrecedence: number): Expression {
    for (;;) {
      const t0 = this.peekToken()
      if (!isOperator(t0)) {
        break;
      }
      const info0 = this.getOperatorInfo(OperatorMode.Infix, t0.getText());
      if (info0 === null || info0.precedence < minPrecedence) {
        break;
      }
      this.getToken();
      let rhs = this.parseUnaryExpression();
      if (typeof(rhs) === 'number') {
        return rhs
      }
      for (;;) {
        const t1 = this.peekToken()
        if (!isOperator(t1)) {
          break;
        }
        const info1 = this.getOperatorInfo(OperatorMode.Infix, t1.getText());
        if (info1 === null
          || (info1.precedence > info0.precedence 
            || (info1.precedence === info0.precedence && info1.mode === OperatorMode.InfixR))) {
          break;
        }
        const result = this.parseOperatorsAfterExpression(lhs, info1.precedence);
        if (typeof(result) === 'number') {
          return result;
        }
        rhs = result;
      }
      lhs = new BinaryExpression(lhs, t0, rhs);
    }
    return lhs;
  }

  public parseExpression(): Expression {
    const lhs = this.parseUnaryExpression()
    return this.parseOperatorsAfterExpression(lhs, 0);
  }

  public parseTypeExpression(): TypeExpression {
    const typeArgs = [];
    const name = this.parseQualName();
    if (typeof(name) === 'number') {
      return name;
    }
    for (;;) {
      const t1 = this.peekToken()
      if (t1.type !== TokenType.Identifier) {
        break;
      }
      const typeArg = this.parseTypeExpression();
      if (typeof(typeArg) === 'number') {
        return typeArg;
      }
      typeArgs.push(typeArg);
    }
    return new TypeReferenceExpression(
      name,
      typeArgs
    );
  }

  public parseStructDeclaration(): RecordDeclaration {

    let pubKeyword = null;
    const typeParams = [];
    let body: RecordDeclarationBody | null = null;

    let t0 = this.peekToken();

    if (t0.type === TokenType.PubKeyword) {
      pubKeyword = t0;
      this.getToken();
      t0 = this.peekToken();
    }

    if (t0.type !== TokenType.StructKeyword) {
      this.raiseParseError(t0, [ TokenType.StructKeyword ]);
    }
    this.getToken();

    const t1 = this.peekToken()
    if (t1.type !== TokenType.Identifier) {
      this.raiseParseError(t1, [ TokenType.Identifier ]);
    }
    this.getToken();

    let t2 = this.peekToken();

    for (;;) {
      if (t2.type !== TokenType.Identifier) {
        break;
      }
      this.getToken()
      typeParams.push(new TypeParameter(t2));
      t2 = this.peekToken();
    }

    const blockStart = this.peekToken()

    if (blockStart.type === TokenType.BlockStart) {

      const elements = [];

      this.getToken()

      for (;;) {

        const t3 = this.peekToken()

        if (t3.type === TokenType.BlockEnd) {
          this.getToken()
          break;
        }

        const name = this.expectToken(TokenType.Identifier);
        const colonSign = this.expectToken(TokenType.ColonSign);
        const typeExpr = this.parseTypeExpression();

        elements.push(
          new RecordDeclarationField(
            name,
            colonSign,
            typeExpr
          )
        );

        const t5 = this.peekToken()
        if (t5.type !== TokenType.LineFoldEnd) {
          this.raiseParseError(t5, [ TokenType.LineFoldEnd ]);
        }
        this.getToken()
      }

      body = [blockStart.dotSign, elements];

    }

    this.expectToken(TokenType.LineFoldEnd)

    return new RecordDeclaration(
      pubKeyword,
      t0,
      t1,
      typeParams,
      body,
    )

  }

  public parseFunctionBodyElement(): FunctionBodyElement {
    // FIXME
    return this.parseExpression();
  }

  public parseTuplePattern(): TuplePattern {
    const t0 = this.peekToken()
    if (t0.type !== TokenType.LParen) {
      this.raiseParseError(t0, [ TokenType.LParen ]);
    }
    this.getToken();
    let elements = [];
    for (;;) {
      const t1 = this.peekToken()
      if (t1.type === TokenType.RParen) {
        break;
      }
      const element = this.parsePattern();
      elements.push(element);
      const t2 = this.peekToken();
      if (t2.type === TokenType.CommaSign) {
        this.getToken();
      } else {
        break;
      }
    }
    const t3 = this.peekToken()
    if (t3.type !== TokenType.RParen) {
      this.raiseParseError(t0, [ TokenType.RParen ]);
    }
    this.getToken();
    return new TuplePattern(t0, elements, t3);
  }

  public parsePattern(): Pattern {
    const t0 = this.peekToken();
    switch (t0.type) {
      case TokenType.Identifier:
        this.getToken();
        return new BindPattern(t0);
      case TokenType.LParen:
        return this.parseTuplePattern();
      case TokenType.DecimalInteger:
        this.getToken();
        return new ConstantExpression(t0);
      default:
        this.raiseParseError(t0, [ TokenType.LParen, TokenType.DecimalInteger, TokenType.Identifier ]);
    }
  }

  public parseDefinition(): FunctionDefinition | VariableDefinition {

    let pubKeyword = null;
    let body = null;
    const params = [];

    let t0 = this.peekToken();

    // Parse the 'pub' keyword, if present

    if (t0.type === TokenType.PubKeyword) {
      this.getToken();
      pubKeyword = t0;
      t0 = this.peekToken()
    }

    // Parse the 'let' keyword

    if (t0.type !== TokenType.LetKeyword) {
      this.raiseParseError(t0, [ TokenType.LetKeyword ]);
    }
    this.getToken();

    // Parse the name of the definition

    const pattern = this.parsePattern();

    // Parse the parameters

    let t2;

    for (;;) {
      t2 = this.peekToken();
      if (t2.type === TokenType.EndOfFile) {
        this.raiseParseError(t2, [ TokenType.Identifier, TokenType.EqualSign, TokenType.DotSign ]);
      }
      if (t2.type === TokenType.EqualSign || t2.type === TokenType.BlockStart) {
        break;
      }
      const pattern = this.parsePattern();
      if (typeof(pattern) === 'number') {
        return pattern;
      }
      params.push(new Parameter(pattern));
    }

    // Parse the function's body

    switch (t2.type) {

      case TokenType.EqualSign:
      {
        this.getToken();
        const equalSign = t2;
        const expression = this.parseExpression();
        const t3 = this.peekToken();
        if (t3.type !== TokenType.LineFoldEnd) {
          this.raiseParseError(t3, [ TokenType.LineFoldEnd ]);
        }
        body = new InlineDefinitionBody(
          equalSign,
          expression
        );
        break;
      }

      case TokenType.BlockStart:
      {
        this.getToken();
        const dotSign = t2.dotSign;
        const elements = [];
        for (;;) {
          const t3 = this.peekToken()
          if (t3.type === TokenType.BlockEnd) {
            break;
          }
          const element = this.parseFunctionBodyElement();
          if (typeof(element) === 'number') {
            return element;
          }
          elements.push(element);
          const t4 = this.peekToken();
          if (t4.type !== TokenType.LineFoldEnd) {
            this.raiseParseError(t4, [ TokenType.LineFoldEnd ]);
          }
          this.getToken();
        }
        body = new BlockDefinitionBody(
          dotSign,
          elements
        );
        break;
      }

      default:
        this.raiseParseError(t2, [ TokenType.EqualSign, TokenType.DotSign ]);

    }

    this.expectToken(TokenType.LineFoldEnd);

    if (params.length > 0) {
      if (pattern.kind !== SyntaxKind.BindPattern) {
        this.raiseParseError(pattern);
      }
      return new FunctionDefinition(
        pubKeyword,
        t0,
        pattern.name,
        params,
        body
      );
    }

    return new VariableDefinition(
      pubKeyword,
      t0,
      pattern,
      body,
    );

  }

  public parseSourceElement(): SourceElement {
    const t0 = this.peekToken();
    switch (t0.type) {
      case TokenType.LetKeyword:
        return this.parseDefinition();
      case TokenType.StructKeyword:
        return this.parseStructDeclaration()
      default:
        this.raiseParseError(t0, [ TokenType.StructKeyword, TokenType.LetKeyword ]);
    }
  }

  public parseSourceFile(): SourceFile {
    const elements = [];
    for (;;) {
      const t0 = this.peekToken()
      if (t0.type === TokenType.EndOfFile) {
        break;
      }
      const element = this.parseSourceElement();
      if (typeof(element) === 'number') {
        return element;
      }
      elements.push(element);
    }
    return new SourceFile(elements, this.file);
  }

}

