
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
  RecordDeclarationBody,
  DotSign,
  Identifier,
  Token,
  TokenSyntaxKind,
} from "./cst";
import {
  Diagnostics,
  ExpectedParse,
  UnexpectedTokenDiagnostic,
} from "./diagnostics";
import { TextFile } from "./text";
import { Stream } from "./util"

type TokenStream = Stream<Token>;

function isOperator(token: Token): boolean {
  return token.kind === SyntaxKind.CustomOperator
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
  ['==', OperatorMode.InfixR, 5],
  ['$', OperatorMode.InfixR, 10],
];

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
    throw new UnexpectedTokenDiagnostic(this.file, actual, expected);
  }

  private expectToken<T extends TokenSyntaxKind>(type: T): Token & { kind: T } {
    const t0 = this.peekToken();
    if (t0.kind !== type) {
      this.raiseParseError(t0, [ type ]);
    }
    this.getToken();
    return t0 as any;
  }

  public parseQualName(): QualName {
    const t0 = this.peekToken()
    if (t0.kind !== SyntaxKind.Identifier) {
      this.raiseParseError(t0, [ SyntaxKind.Identifier ]);
    }
    this.getToken();
    let name = t0;
    const modulePath: Array<[Identifier, DotSign]> = []
    for (;;) {
      const t1 = this.peekToken(1)
      const t2 = this.peekToken(2);
      if (t1.kind !== SyntaxKind.DotSign || t2.getStartLine() > t1.getStartLine()) {
        break;
      }
      if (t2.kind !== SyntaxKind.Identifier) {
        this.raiseParseError(t2, [ SyntaxKind.Identifier ]);
      }
      modulePath.push([name, t1]);
      name = t2;
      this.getToken();
      this.getToken();
    }
    return new QualName(modulePath, name);
  }

  public parseMatchExpression(): MatchExpression {

    const matchKeyword = this.expectToken(SyntaxKind.MatchKeyword)
    const expression = this.parseExpression()
    const blockStart = this.expectToken(SyntaxKind.BlockStart);

    const arms = [];

    for (;;) {
      const t2 = this.peekToken();
      if (t2.kind === SyntaxKind.BlockEnd) {
        this.getToken();
        break;
      }
      const pattern = this.parsePattern();
      const equalSign = this.expectToken(SyntaxKind.EqualSign);
      const expression = this.parseExpression();
      arms.push(new MatchArm(pattern, equalSign, expression));
      this.expectToken(SyntaxKind.LineFoldEnd)
    }

    return new MatchExpression(
      matchKeyword,
      expression,
      blockStart.dotSign,
      arms,
    );

  }

  public parsePrimitiveExpression(): Expression {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.MatchKeyword:
        return this.parseMatchExpression();
      case SyntaxKind.Identifier:
        this.getToken();
        return new ReferenceExpression(t0);
      case SyntaxKind.DecimalInteger:
        this.getToken();
        return new ConstantExpression(t0);
      case SyntaxKind.LParen:
        this.getToken();
        const expression = this.parseExpression();
        const t1 = this.peekToken()
        if (t1.kind !== SyntaxKind.RParen) {
          this.raiseParseError(t1, [ SyntaxKind.RParen ]);
        }
        this.getToken();
        return new NestedExpression(t0, expression, t1);
      default:
        this.raiseParseError(t0, [ SyntaxKind.Identifier ]);
    }
  }

  public parseCallExpression(): Expression {
    const expr0 = this.parsePrimitiveExpression();
    const args = [];
    for (;;) {
      const t1 = this.peekToken();
      if (t1.kind === SyntaxKind.LineFoldEnd
        || t1.kind === SyntaxKind.BlockEnd
        || t1.kind === SyntaxKind.BlockStart
        || t1.kind === SyntaxKind.RParen
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
      if (t1.kind !== SyntaxKind.Identifier) {
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

    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      this.getToken();
      t0 = this.peekToken();
    }

    if (t0.kind !== SyntaxKind.StructKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.StructKeyword ]);
    }
    this.getToken();

    const name = this.expectToken(SyntaxKind.Identifier)

    let t2 = this.peekToken();

    for (;;) {
      if (t2.kind !== SyntaxKind.Identifier) {
        break;
      }
      this.getToken()
      typeParams.push(new TypeParameter(t2));
      t2 = this.peekToken();
    }

    const blockStart = this.peekToken()

    if (blockStart.kind === SyntaxKind.BlockStart) {

      const elements = [];

      this.getToken()

      for (;;) {

        const t3 = this.peekToken()

        if (t3.kind === SyntaxKind.BlockEnd) {
          this.getToken()
          break;
        }

        const name = this.expectToken(SyntaxKind.Identifier);
        const colonSign = this.expectToken(SyntaxKind.ColonSign);
        const typeExpr = this.parseTypeExpression();

        elements.push(
          new RecordDeclarationField(
            name,
            colonSign,
            typeExpr
          )
        );

        this.expectToken(SyntaxKind.LineFoldEnd);
      }

      body = [blockStart.dotSign, elements];

    }

    this.expectToken(SyntaxKind.LineFoldEnd)

    return new RecordDeclaration(
      pubKeyword,
      t0,
      name,
      typeParams,
      body,
    )

  }

  public parseFunctionBodyElement(): FunctionBodyElement {
    // FIXME Add support for return statements, etc.
    const expression = this.parseExpression();
    this.expectToken(SyntaxKind.LineFoldEnd);
    return expression;
  }

  public parseTuplePattern(): TuplePattern {
    const lparen = this.expectToken(SyntaxKind.LParen);
    let elements = [];
    for (;;) {
      const t1 = this.peekToken()
      if (t1.kind === SyntaxKind.RParen) {
        break;
      }
      const element = this.parsePattern();
      elements.push(element);
      const t2 = this.peekToken();
      if (t2.kind === SyntaxKind.CommaSign) {
        this.getToken();
      } else {
        break;
      }
    }
    const rparen = this.expectToken(SyntaxKind.RParen)
    return new TuplePattern(lparen, elements, rparen);
  }

  public parsePattern(): Pattern {
    const t0 = this.peekToken();
    switch (t0.kind) {
      case SyntaxKind.Identifier:
        this.getToken();
        return new BindPattern(t0);
      case SyntaxKind.LParen:
        return this.parseTuplePattern();
      case SyntaxKind.DecimalInteger:
        this.getToken();
        return new ConstantExpression(t0);
      default:
        this.raiseParseError(t0, [ SyntaxKind.LParen, SyntaxKind.DecimalInteger, SyntaxKind.Identifier ]);
    }
  }

  public parseDefinition(): FunctionDefinition | VariableDefinition {

    let pubKeyword = null;
    let body = null;
    const params = [];

    let t0 = this.peekToken();

    // Parse the 'pub' keyword, if present

    if (t0.kind === SyntaxKind.PubKeyword) {
      this.getToken();
      pubKeyword = t0;
      t0 = this.peekToken()
    }

    // Parse the 'let' keyword

    if (t0.kind !== SyntaxKind.LetKeyword) {
      this.raiseParseError(t0, [ SyntaxKind.LetKeyword ]);
    }
    this.getToken();

    // Parse the name of the definition

    const pattern = this.parsePattern();

    // Parse the parameters

    let t2;

    for (;;) {
      t2 = this.peekToken();
      if (t2.kind === SyntaxKind.EndOfFile) {
        this.raiseParseError(t2, [ SyntaxKind.Identifier, SyntaxKind.EqualSign, SyntaxKind.DotSign ]);
      }
      if (t2.kind === SyntaxKind.EqualSign || t2.kind === SyntaxKind.BlockStart) {
        break;
      }
      const pattern = this.parsePattern();
      if (typeof(pattern) === 'number') {
        return pattern;
      }
      params.push(new Parameter(pattern));
    }

    // Parse the function's body

    switch (t2.kind) {

      case SyntaxKind.EqualSign:
      {
        this.getToken();
        const equalSign = t2;
        const expression = this.parseExpression();
        const t3 = this.peekToken();
        if (t3.kind !== SyntaxKind.LineFoldEnd) {
          this.raiseParseError(t3, [ SyntaxKind.LineFoldEnd ]);
        }
        body = new InlineDefinitionBody(
          equalSign,
          expression
        );
        break;
      }

      case SyntaxKind.BlockStart:
      {
        this.getToken();
        const dotSign = t2.dotSign;
        const elements = [];
        for (;;) {
          const t3 = this.peekToken()
          if (t3.kind === SyntaxKind.BlockEnd) {
            this.getToken()
            break;
          }
          const element = this.parseFunctionBodyElement();
          if (typeof(element) === 'number') {
            return element;
          }
          elements.push(element);
          this.expectToken(SyntaxKind.LineFoldEnd)
        }
        body = new BlockDefinitionBody(
          dotSign,
          elements
        );
        break;
      }

      default:
        this.raiseParseError(t2, [ SyntaxKind.EqualSign, SyntaxKind.DotSign ]);

    }

    this.expectToken(SyntaxKind.LineFoldEnd);

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
    switch (t0.kind) {
      case SyntaxKind.LetKeyword:
        return this.parseDefinition();
      case SyntaxKind.StructKeyword:
        return this.parseStructDeclaration()
      default:
        return this.parseFunctionBodyElement();
        // this.raiseParseError(t0, [ SyntaxKind.StructKeyword, SyntaxKind.LetKeyword ]);
    }
  }

  public parseSourceFile(): SourceFile {
    const elements = [];
    for (;;) {
      const t0 = this.peekToken()
      if (t0.kind === SyntaxKind.EndOfFile) {
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

