
import {
  createRecordDeclaration,
  createRecordDeclarationField,
  createSimpleToken,
  createSourceFile,
  createTypeParameter,
  describeToken,
  RecordDeclaration,
  SourceElement,
  SourceFile,
  SyntaxKind,
  TextPosition,
  Token,
  TypeExpression
} from "./cst";
import { Stream, BufferedStream } from "./util"

enum CompareMode {
  Greater,
  GreaterOrEqual,
  Lesser,
  LesserOrEqual,
  Equal,
}

export class ParseError extends Error {

  constructor(
    public actual: Token,
    public expected: ExpectedParse[],
  ) {
    super(`Got ${describeToken(actual.kind)} but expected something else`);
  }

}

function formatUnexpectedIndentationError(
  compareMode: CompareMode,
  actualIndentLevel: number,
  expectedIndentLevel: number,
) {
  let out = `Expected an indentation `
  switch (compareMode) {
    case CompareMode.Equal:
      out += 'of exactly';
      break;
    case CompareMode.Lesser:
      out += 'strictly less than'
      break;
    case CompareMode.LesserOrEqual:
      out += 'less than or equal to';
      break;
    case CompareMode.Greater:
      out += 'strictly greater than'
      break;
    case CompareMode.GreaterOrEqual:
      out += 'greater than or equal to';
      break;
  }
  out += ` ${expectedIndentLevel} `
  if (expectedIndentLevel === 1) {
    out += 'space';
  } else {
    out += 'spaces';
  }
  out += ` but found ${actualIndentLevel} `
  if (actualIndentLevel === 1) {
    out += 'space';
  } else {
    out += 'spaces';
  }
  out += '.'
  return out;
}

export class UnexpectedIndentationError extends Error {

  constructor(
    public compareMode: CompareMode,
    public actualIndentLevel: number,
    public expectedIndentLevel: number,
  ) {
    super(formatUnexpectedIndentationError(compareMode, actualIndentLevel, expectedIndentLevel));
  }

}

export class EndOfLineFoldExpected extends Error {

  constructor(
    public actual: Token
  ) {
    super(`Expected end of line-fold but got ${describeToken(actual.kind)}.`);
  }

}

export class NewLineRequiredError extends Error {

  constructor(
    public actual: Token
  ) {
    super(`Expected ${describeToken(actual.kind)} to be placed on a new line.`)
  }

}

export class HardParseError extends ParseError {

}

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
    if (token.kind === SyntaxKind.EndOfFile ||
      (token.getStartLine() > this.referencePosition.line
        && token.getStartColumn() <= this.referencePosition.column)) {
      return createSimpleToken(SyntaxKind.EndOfFile, 0);
    }
    return this.tokens.get();
  }

}

export class Parser {

  public parseTypeExpression(tokens: TokenStream): TypeExpression {
    tokens.get()
  }

  public parseStructDeclaration(tokens: TokenStream): RecordDeclaration {
    let pubKeyword = null;
    const typeParams = [];
    let body = null;
    let t0 = tokens.peek();
    const firstToken = t0;
    if (t0.kind === SyntaxKind.PubKeyword) {
      pubKeyword = t0;
      tokens.get();
      t0 = tokens.peek();
    }
    if (t0.kind !== SyntaxKind.StructKeyword) {
      throw new ParseError(t0, [ SyntaxKind.StructKeyword ]);
    }
    tokens.get();
    const t1 = tokens.peek()
    if (t1.kind !== SyntaxKind.Identifier) {
      throw new HardParseError(t1, [ SyntaxKind.Identifier ]);
    }
    tokens.get();
    let t2 = tokens.peek();
    for (;;) {
      if (t2.kind !== SyntaxKind.Identifier) {
        break;
      }
      tokens.get()
      typeParams.push(createTypeParameter(t2));
      t2 = tokens.peek();
    }
    const dotSign = tokens.peek()
    if (dotSign.kind === SyntaxKind.DotSign) {
      const elements = [];
      tokens.get()
      let lastRefToken = tokens.peek();
      for (;;) {
        const t3 = tokens.peek()
        if (t3.kind === SyntaxKind.EndOfFile) {
          break;
        }
        if (t3.getStartLine() === dotSign.getStartLine()) {
          throw new NewLineRequiredError(t3);
        }
        if (t3.getStartColumn() < lastRefToken.getStartColumn()) {
          if (t3.getStartColumn() > firstToken.getStartColumn()) {
            throw new UnexpectedIndentationError(CompareMode.Equal, t3.getStartColumn()-1, lastRefToken.getStartColumn()-1);
          }
          break;
        }
        if (t3.kind !== SyntaxKind.Identifier) {
          throw new HardParseError(t3, [ SyntaxKind.Identifier ]);
        }
        lastRefToken = t3;
        tokens.get()
        const bodyTokens = new LineFoldTokenStream(tokens, t3.getStartPos());
        const t4 = bodyTokens.peek()
        if (t4.kind !== SyntaxKind.ColonSign) {
          throw new HardParseError(t4, [ SyntaxKind.ColonSign ]);
        }
        bodyTokens.get()
        const typeExpr = this.parseTypeExpression(tokens);
        elements.push(
          createRecordDeclarationField(
            t3,
            t4,
            typeExpr
          )
        );
        const t5 = bodyTokens.peek()
        if (t5.kind !== SyntaxKind.EndOfFile) {
          throw new EndOfLineFoldExpected(t5);
        }
      }
      body = { dotSign, elements };
    }
    return createRecordDeclaration(
      pubKeyword,
      t0,
      t1,
      typeParams,
      body,
    )
  }

  public parseSourceElement(tokens: TokenStream): SourceElement {
    const t0 = tokens.peek();
    switch (t0.kind) {
      case SyntaxKind.StructKeyword:
        return this.parseStructDeclaration(tokens)
      default:
        throw new ParseError(t0, [ SyntaxKind.StructKeyword ]);
    }
  }

  public parseSourceFile(tokens: TokenStream): SourceFile {
    const elements = [];
    for (;;) {
      const t0 = tokens.peek()
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      elements.push(this.parseSourceElement(tokens));
    }
    return createSourceFile(elements);
  }

}

