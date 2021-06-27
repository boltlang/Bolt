
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
  NestedExpression
} from "./cst";
import {
  Diagnostics,
  ExpectedEndOfLineFoldDiagnostic,
  NewLineRequiredDiagnostic,
  UnexpectedConstructDiagnostic,
  UnexpectedIndentationDiagnostic,
  UnexpectedTokenDiagnostic
} from "./diagnostics";
import { TextFile, TextPosition } from "./text";
import { DotSign, EndOfIndent, Identifier, Token, TokenType } from "./token";
import { Stream, BufferedStream, CompareMode } from "./util"

type TokenStream = Stream<Token>;

class LineFoldTokenStream extends BufferedStream<Token> {

  constructor(
    private tokens: TokenStream,
    private referencePosition: TextPosition
  ) {
    super();
  }

  protected read(): Token {
    const token = this.tokens.peek();
    if (token.type === TokenType.EndOfIndent) {
      return token;
    }
    if (token.type === TokenType.EndOfFile ||
      (token.getStartLine() > this.referencePosition.line
        && token.getStartColumn() <= this.referencePosition.column)) {
      return new EndOfIndent(token);
    }
    return this.tokens.get();
  }

}

interface ParseContext {
  enableDiagnostics: boolean;
  file: TextFile;
}

function isOperator(token: Token): boolean {
  return token.type === TokenType.CustomOperator
}

type ParseResult<T> = T | number;

const MAY_BACKTRACK = 1;
const NO_BACKTRACK  = 2;

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

export class Parser {

  private exprOperatorTable: OperatorInfo[] = [];

  constructor(public diagnostics: Diagnostics) {
    for (const [text, mode, precedence] of DEFAULT_OPERATORS) {
      this.exprOperatorTable.push({
        text,
        mode,
        precedence,
      });
    }
  }

  public parseQualName(tokens: TokenStream, ctx: ParseContext): ParseResult<QualName> {
    const t0 = tokens.peek()
    if (t0.type !== TokenType.Identifier) {
      if (ctx.enableDiagnostics) {
        this.diagnostics.add(
          new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.Identifier ])
        );
      }
      return MAY_BACKTRACK;
    }
    tokens.get();
    let name = t0;
    const modulePath: Array<[Identifier, DotSign]> = []
    for (;;) {
      const t1 = tokens.peek(1)
      const t2 = tokens.peek(2);
      if (t1.type !== TokenType.DotSign || t2.getStartLine() > t1.getStartLine()) {
        break;
      }
      if (t2.type !== TokenType.Identifier) {
        if (ctx.enableDiagnostics) {
          this.diagnostics.add(
            new UnexpectedTokenDiagnostic(ctx.file, t2, [ TokenType.Identifier ])
          );
        }
        return MAY_BACKTRACK;
      }
      modulePath.push([name, t1]);
      name = t2;
      tokens.get();
      tokens.get();
    }
    return new QualName(modulePath, name);
  }

  public parsePrimitiveExpression(tokens: TokenStream, ctx: ParseContext): ParseResult<Expression> {
    const t0 = tokens.peek();
    switch (t0.type) {
      case TokenType.Identifier:
        tokens.get();
        return new ReferenceExpression(t0);
      case TokenType.DecimalInteger:
        tokens.get();
        return new ConstantExpression(t0);
      case TokenType.LParen:
        tokens.get();
        const expression = this.parseExpression(tokens, ctx);
        if (typeof(expression) === 'number') {
          return expression;
        }
        const t1 = tokens.peek()
        if (t1.type !== TokenType.RParen) {
          if (ctx.enableDiagnostics) {
            this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t1, [ TokenType.RParen ]));
          }
          return MAY_BACKTRACK;
        }
        tokens.get();
        return new NestedExpression(t0, expression, t1);
      default:
        if (ctx.enableDiagnostics) {
          this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.Identifier ]));
        }
        return MAY_BACKTRACK;
    }
  }

  public parseCallExpression(tokens: TokenStream, ctx: ParseContext): ParseResult<Expression> {
    const expr0 = this.parsePrimitiveExpression(tokens, ctx);
    if (typeof(expr0) === 'number') {
      return expr0;
    }
    const args = [];
    for (;;) {
      const t1 = tokens.peek();
      if (t1.type === TokenType.RParen || isOperator(t1) || t1.type === TokenType.EndOfIndent) {
        break;
      }
      const arg = this.parsePrimitiveExpression(tokens, ctx)
      if (typeof(arg) === 'number') {
        return arg;
      }
      args.push(arg);
    }
    if (args.length === 0) {
      return expr0;
    }
    return new CallExpression(expr0, args);
  }

  public parseUnaryExpression(tokens: TokenStream, ctx: ParseContext): ParseResult<Expression> {
    const stack = [];
    for (;;) {
      const t0 = tokens.peek()
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
    return this.parseCallExpression(tokens, ctx);
  }

  private getOperatorInfo(mode: OperatorMode, text: string): OperatorInfo | null {
    for (const info of this.exprOperatorTable) {
      if (info.mode & mode && info.text === text) {
        return info;
      }
    }
    return null;
  }

  public parseOperatorsAfterExpression(tokens: TokenStream, ctx: ParseContext, lhs: Expression, minPrecedence: number): ParseResult<Expression> {
    for (;;) {
      const t0 = tokens.peek()
      if (!isOperator(t0)) {
        break;
      }
      const info0 = this.getOperatorInfo(OperatorMode.Infix, t0.getText());
      if (info0 === null || info0.precedence < minPrecedence) {
        break;
      }
      tokens.get();
      let rhs = this.parseUnaryExpression(tokens, ctx);
      if (typeof(rhs) === 'number') {
        return rhs
      }
      for (;;) {
        const t1 = tokens.peek()
        if (!isOperator(t1)) {
          break;
        }
        const info1 = this.getOperatorInfo(OperatorMode.Infix, t1.getText());
        if (info1 === null
          || (info1.precedence > info0.precedence 
            || (info1.precedence === info0.precedence && info1.mode === OperatorMode.InfixR))) {
          break;
        }
        const result = this.parseOperatorsAfterExpression(tokens, ctx, lhs, info1.precedence);
        if (typeof(result) === 'number') {
          return result;
        }
        rhs = result;
      }
      lhs = new BinaryExpression(lhs, t0, rhs);
    }
    return lhs;
  }

  public parseExpression(tokens: TokenStream, ctx: ParseContext): ParseResult<Expression> {
    const lhs = this.parseUnaryExpression(tokens, ctx)
    if (typeof(lhs) === 'number') {
      return lhs;
    }
    return this.parseOperatorsAfterExpression(tokens, ctx, lhs, 0);
  }

  public parseTypeExpression(tokens: TokenStream, ctx: ParseContext): ParseResult<TypeExpression> {
    const typeArgs = [];
    const name = this.parseQualName(tokens, ctx);
    if (typeof(name) === 'number') {
      return name;
    }
    for (;;) {
      const t1 = tokens.peek()
      if (t1.type !== TokenType.Identifier) {
        break;
      }
      const typeArg = this.parseTypeExpression(tokens, ctx);
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

  public parseStructDeclaration(tokens: TokenStream, ctx: ParseContext): ParseResult<RecordDeclaration> {
    let pubKeyword = null;
    const typeParams = [];
    let body = null;
    let t0 = tokens.peek();
    const firstToken = t0;
    if (t0.type === TokenType.PubKeyword) {
      pubKeyword = t0;
      tokens.get();
      t0 = tokens.peek();
    }
    if (t0.type !== TokenType.StructKeyword) {
      if (ctx.enableDiagnostics) {
        this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.StructKeyword ]));
      }
      return MAY_BACKTRACK;
    }
    tokens.get();
    const t1 = tokens.peek()
    if (t1.type !== TokenType.Identifier) {
      this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t1, [ TokenType.Identifier ]));
      return NO_BACKTRACK;
    }
    tokens.get();
    let t2 = tokens.peek();
    for (;;) {
      if (t2.type !== TokenType.Identifier) {
        break;
      }
      tokens.get()
      typeParams.push(new TypeParameter(t2));
      t2 = tokens.peek();
    }
    const dotSign = tokens.peek()
    if (dotSign.type === TokenType.DotSign) {
      const elements = [];
      tokens.get()
      let lastRefToken = tokens.peek();
      for (;;) {
        const t3 = tokens.peek()
        if (t3.type === TokenType.EndOfIndent) {
          break;
        }
        if (t3.getStartLine() === dotSign.getStartLine()) {
          this.diagnostics.add(new NewLineRequiredDiagnostic(ctx.file, t3));
          return NO_BACKTRACK;
        }
        if (t3.getStartColumn() < lastRefToken.getStartColumn()) {
          if (t3.getStartColumn() > firstToken.getStartColumn()) {
            this.diagnostics.add(new UnexpectedIndentationDiagnostic(ctx.file, CompareMode.Equal, t3.getStartColumn()-1, lastRefToken.getStartColumn()-1)) ;
            return NO_BACKTRACK;
          }
          break;
        }
        if (t3.type !== TokenType.Identifier) {
          this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t3, [ TokenType.Identifier ]));
          return NO_BACKTRACK;
        }
        lastRefToken = t3;
        tokens.get()
        const bodyTokens = new LineFoldTokenStream(tokens, t3.getStartPos());
        const t4 = bodyTokens.peek()
        if (t4.type !== TokenType.ColonSign) {
          this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t4, [ TokenType.ColonSign ]));
          return NO_BACKTRACK;
        }
        bodyTokens.get()
        const typeExpr = this.parseTypeExpression(bodyTokens, ctx);
        if (typeof(typeExpr) === 'number') {
          return typeExpr;
        }
        elements.push(
          new RecordDeclarationField(
            t3,
            t4,
            typeExpr
          )
        );
        const t5 = bodyTokens.peek()
        if (t5.type !== TokenType.EndOfIndent) {
          this.diagnostics.add(new ExpectedEndOfLineFoldDiagnostic(ctx.file, t5));
          return NO_BACKTRACK;
        }
      }
      body = { dotSign, elements };
    }
    return new RecordDeclaration(
      pubKeyword,
      t0,
      t1,
      typeParams,
      body,
    )
  }

  public parseFunctionBodyElement(tokens: TokenStream, ctx: ParseContext): ParseResult<FunctionBodyElement> {
    // FIXME
    return this.parseExpression(tokens, ctx);
  }

  public parseTuplePattern(tokens: TokenStream, ctx: ParseContext): ParseResult<TuplePattern> {
    const t0 = tokens.peek()
    if (t0.type !== TokenType.LParen) {
      if (ctx.enableDiagnostics) {
        this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.LParen ]));
      }
      return MAY_BACKTRACK;
    }
    tokens.get();
    let elements = [];
    for (;;) {
      const t1 = tokens.peek()
      if (t1.type === TokenType.RParen) {
        break;
      }
      const element = this.parsePattern(tokens, ctx);
      if (typeof(element) === 'number') {
        return element;
      }
      elements.push(element);
      const t2 = tokens.peek();
      if (t2.type === TokenType.CommaSign) {
        tokens.get();
      } else {
        break;
      }
    }
    const t3 = tokens.peek()
    if (t3.type !== TokenType.RParen) {
      if (ctx.enableDiagnostics) {
        this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.RParen ]));
      }
      return MAY_BACKTRACK;
    }
    tokens.get();
    return new TuplePattern(t0, elements, t3);
  }

  public parsePattern(tokens: TokenStream, ctx: ParseContext): ParseResult<Pattern> {
    const t0 = tokens.peek();
    switch (t0.type) {
      case TokenType.Identifier:
        tokens.get();
        return new BindPattern(t0);
      case TokenType.LParen:
        return this.parseTuplePattern(tokens, ctx);
      case TokenType.DecimalInteger:
        tokens.get();
        return new ConstantExpression(t0);
      default:
        if (ctx.enableDiagnostics) {
          this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.LParen, TokenType.DecimalInteger, TokenType.Identifier ]));
        }
        return MAY_BACKTRACK;
    }
  }

  public parseDefinition(tokens: TokenStream, ctx: ParseContext): ParseResult<FunctionDefinition | VariableDefinition> {

    let pubKeyword = null;
    let body = null;
    const params = [];

    let t0 = tokens.peek();
    const firstToken = t0;

    // Parse the 'pub' keyword, if present

    if (t0.type === TokenType.PubKeyword) {
      tokens.get();
      pubKeyword = t0;
      t0 = tokens.peek()
    }

    // Parse the 'let' keyword

    if (t0.type !== TokenType.LetKeyword) {
      if (ctx.enableDiagnostics) {
        this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.LetKeyword ]));
      }
      return MAY_BACKTRACK;
    }
    tokens.get();

    // Parse the name of the definition

    // const t1 = tokens.peek()
    // if (t1.type !== TokenType.Identifier) {
    //   this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t1, [ TokenType.Identifier ]));
    //   return NO_BACKTRACK;
    // }
    // tokens.get();
    const afterLetCtx = { file: ctx.file, enableDiagnostics: true };
    const pattern = this.parsePattern(tokens, afterLetCtx);
    if (typeof(pattern) === 'number') {
      return NO_BACKTRACK;
    }

    // Parse the parameters

    let t2;

    for (;;) {
      t2 = tokens.peek();
      if (t2.type === TokenType.EndOfFile) {
        this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t2, [ TokenType.Identifier, TokenType.EqualSign, TokenType.DotSign ]));
        return NO_BACKTRACK;
      }
      if (t2.type === TokenType.EqualSign || t2.type === TokenType.DotSign) {
        break;
      }
      const pattern = this.parsePattern(tokens, ctx);
      if (typeof(pattern) === 'number') {
        return pattern;
      }
      params.push(new Parameter(pattern));
    }

    // Parse the function's body

    switch (t2.type) {

      case TokenType.EqualSign:
      {
        tokens.get();
        const equalSign = t2;
        const exprTokens = new LineFoldTokenStream(tokens, firstToken.getStartPos());
        const expression = this.parseExpression(exprTokens, afterLetCtx);
        if (typeof(expression) === 'number') {
          return expression;
        }
        const t3 = exprTokens.peek();
        if (t3.type !== TokenType.EndOfIndent) {
          this.diagnostics.add(new ExpectedEndOfLineFoldDiagnostic(ctx.file, t3));
          return NO_BACKTRACK;
        }
        body = new InlineDefinitionBody(
          equalSign,
          expression
        );
        break;
      }

      case TokenType.DotSign:
      {
        tokens.get();
        const dotSign = t2;
        const elements = [];
        let lastRefToken = tokens.peek();
        for (;;) {
          const t3 = tokens.peek();
          if (t3.type === TokenType.EndOfIndent) {
            break;
          }
          if (t3.getStartLine() === dotSign.getStartLine()) {
            this.diagnostics.add(new NewLineRequiredDiagnostic(ctx.file, t3));
            return NO_BACKTRACK;
          }
          if (t3.getStartColumn() < lastRefToken.getStartColumn()) {
            if (t3.getStartColumn() > firstToken.getStartColumn()) {
              this.diagnostics.add(new UnexpectedIndentationDiagnostic(ctx.file, CompareMode.Equal, t3.getStartColumn()-1, lastRefToken.getStartColumn()-1)) ;
              return NO_BACKTRACK;
            }
            break;
          }
          lastRefToken = t3;
          const elementTokens = new LineFoldTokenStream(tokens, t3.getStartPos());
          const element = this.parseFunctionBodyElement(elementTokens, afterLetCtx);
          if (typeof(element) === 'number') {
            return element;
          }
          elements.push(element);
          const t4 = elementTokens.peek();
          if (t4.type !== TokenType.EndOfIndent) {
            this.diagnostics.add(new ExpectedEndOfLineFoldDiagnostic(ctx.file, t4));
            return NO_BACKTRACK;
          }
        }
        body = new BlockDefinitionBody(
          dotSign,
          elements
        );
        break;
      }

      default:
        this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t2, [ TokenType.EqualSign, TokenType.DotSign ]));
        return NO_BACKTRACK;

    }

    if (params.length > 0) {
      if (pattern.kind !== SyntaxKind.BindPattern) {
        this.diagnostics.add(new UnexpectedConstructDiagnostic(pattern));
        return NO_BACKTRACK;
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

  public parseSourceElement(tokens: TokenStream, ctx: ParseContext): ParseResult<SourceElement> {
    const t0 = tokens.peek();
    switch (t0.type) {
      case TokenType.LetKeyword:
        return this.parseDefinition(tokens, ctx);
      case TokenType.StructKeyword:
        return this.parseStructDeclaration(tokens, ctx)
      default:
        if (ctx.enableDiagnostics) {
          this.diagnostics.add(new UnexpectedTokenDiagnostic(ctx.file, t0, [ TokenType.StructKeyword, TokenType.LetKeyword ]));
        }
        return MAY_BACKTRACK;
    }
  }

  public parseSourceFile(tokens: TokenStream, ctx: ParseContext): ParseResult<SourceFile> {
    const elements = [];
    for (;;) {
      const t0 = tokens.peek()
      if (t0.type === TokenType.EndOfFile) {
        break;
      }
      const element = this.parseSourceElement(tokens, ctx);
      if (typeof(element) === 'number') {
        return element;
      }
      elements.push(element);
    }
    return new SourceFile(elements, ctx.file);
  }

}

