
import { EOF, TextFile, TextPosition } from "./text";
import {
  Token,
  Identifier,
  SimpleToken,
  EndOfFile,
  DecimalInteger,
  CustomOperator,
  Assignment,
  LineFoldEnd,
  BlockStart,
  BlockEnd,
  SyntaxKind,
  TokenSyntaxKind,
} from "./cst";
import { BufferedStream, hasOwnProperty, Stream } from "./util";
import { UnexpectedCharacterDiagnostic } from "./diagnostics";

function isWhiteSpace(ch: string): boolean {
  return /[\n\t\r ]/.test(ch);
}

function isIdentStart(ch: string): boolean {
  return /[a-zA-Z_]/.test(ch);
}

function isIdentPart(ch: string): boolean {
  return /[a-zA-Z0-9_]/.test(ch);
}

function isDigit(ch: string): boolean {
  return /[0-9]/.test(ch);
}

function isOperator(ch: string): boolean {
  return /[$+\-*/%&^|<>=?!]/.test(ch);
}

function countLeadingChars(text: string, ch: string): number {
  let i = 0;
  for (const ch2 of text) {
    if (ch2 !== ch) {
      break;
    }
    i++;
  }
  return i;
}

const ASCII_ESCAPE_CHARS: Record<string, string> = {
  '\a': '\\a',
  '\b': '\\b',
  '\f': '\\f',
  '\n': '\\n',
  '\r': '\\r',
  '\t': '\\t',
  '\v': '\\v',
  '\0': '\\0',
  '\'': '\\\'',
}

function describeChar(ch: string): string {
  if (ch === EOF) {
    return 'end-of-file'
  }
  if (ch in ASCII_ESCAPE_CHARS) {
    ch = ASCII_ESCAPE_CHARS[ch];
  }
  return `character '${ch}'`
}

const KEYWORDS: Record<string, TokenSyntaxKind> = {
  'return': SyntaxKind.ReturnKeyword,
  'struct': SyntaxKind.StructKeyword,
  'enum': SyntaxKind.EnumKeyword,
  'type': SyntaxKind.TypeKeyword,
  'import': SyntaxKind.ImportKeyword,
  'pub': SyntaxKind.PubKeyword,
  'mut': SyntaxKind.MutKeyword,
  'let': SyntaxKind.LetKeyword,
  'perform': SyntaxKind.PerformKeyword,
  'yield': SyntaxKind.YieldKeyword,
  'resume': SyntaxKind.ResumeKeyword,
  'match': SyntaxKind.MatchKeyword,
  'class': SyntaxKind.ClassKeyword,
  'instance': SyntaxKind.InstanceKeyword,
}

export class Scanner extends BufferedStream<Token> {

  constructor(
    private file: TextFile,
    public text: string = file.getText(),
    public textOffset = 0,
    private currLine = 1,
    private currColumn = 1,
  ) {
    super();
  }

  private peekChar(offset = 1): string {
    const textOffset = this.textOffset + offset - 1;
    return textOffset < this.text.length
        ? this.text[textOffset]
        : EOF;
  }

  private getChar(): string {
    const ch = this.textOffset < this.text.length
        ? this.text[this.textOffset++]
        : EOF;
    if (ch === '\n') {
      ++this.currLine;
      this.currColumn = 1;
    } else {
      ++this.currColumn;
    }
    return ch;
  }

  private takeWhile(pred: (ch: string) => boolean): string {
    let out = '';
    for (;;) {
      const c0 = this.peekChar()
      if (c0 === EOF || !pred(c0)) {
        break;
      }
      out += c0;
      this.getChar()
    }
    return out;
  }

  private skipAllWhiteSpace() {
    outer: for (;;) {
      const c0 = this.peekChar()
      if (c0 === '#') {
        this.getChar();
        for (;;) {
          const c1 = this.getChar()
          if (c1 === EOF || c1 === '\n') {
            break;
          }
        }
        continue outer;
      }
      if (!isWhiteSpace(c0)) {
        break;
      }
      this.getChar();
    }
  }

  private getPosition(): TextPosition {
    return {
      line: this.currLine,
      column: this.currColumn,
      offset: this.textOffset,
    }
  }

  private scanChar(kind: SyntaxKind): Token {
    const startPos = this.getPosition();
    this.getChar();
    const endPos = this.getPosition();
    // @ts-ignore Unification fails due to `kind` being ambiguous
    return new SimpleToken(kind, [startPos, endPos]);
  }

  public read(): Token {

    for (;;) {

      this.skipAllWhiteSpace();

      const c0 = this.peekChar()

      if (c0 === EOF) {
        return new EndOfFile([ this.getPosition(), this.getPosition() ]);
      }

      switch (c0) {
        case '(': return this.scanChar(SyntaxKind.LParen);
        case ')': return this.scanChar(SyntaxKind.RParen);
        case '{': return this.scanChar(SyntaxKind.LBrace);
        case '}': return this.scanChar(SyntaxKind.RBrace);
        case '[': return this.scanChar(SyntaxKind.LBracket);
        case ']': return this.scanChar(SyntaxKind.RBracket);
        case '.': return this.scanChar(SyntaxKind.DotSign);
        case ':': return this.scanChar(SyntaxKind.ColonSign);
        case ',': return this.scanChar(SyntaxKind.CommaSign);
        case '\\': return this.scanChar(SyntaxKind.BSlashSign);
      }

      if (isOperator(c0)) {
        const startPos = this.getPosition();
        this.getChar()
        const text = c0 + this.takeWhile(isOperator);
        const endPos = this.getPosition();
        if (text === '=') {
          return new SimpleToken(SyntaxKind.EqualSign, [ startPos, endPos ]);
        }
        if (text.endsWith('=') && text[text.length-2] !== '=') {
          return new Assignment(text.substring(0, text.length-1), [startPos, endPos]);
        }
        switch (text) {
          case '->': return new SimpleToken(SyntaxKind.RArrowSign, [startPos, endPos]);
        }
        return new CustomOperator(text, [startPos, endPos]);
      }

      if (isDigit(c0)) {
        const startPos = this.getPosition()
        this.getChar()
        const digits = c0 + this.takeWhile(isDigit)
        const endPos = this.getPosition();
        const numLeadingZeroes = countLeadingChars(digits, '0');
        const value = BigInt(digits);
        return new DecimalInteger(
          value === BigInt(0) ? numLeadingZeroes-1 : numLeadingZeroes,
          value,
          [ startPos, endPos ]
        );
      }

      if (isIdentStart(c0)) {
        const startPos = this.getPosition();
        this.getChar();
        const name = c0 + this.takeWhile(isIdentPart);
        const endPos = this.getPosition();
        if (hasOwnProperty(KEYWORDS, name)) {
          // @ts-ignore Unification fails due to KEYWORDS[name] being ambiguous
          return new SimpleToken(KEYWORDS[name], [startPos, endPos]);
        }
        return new Identifier(name, [startPos, endPos]);
      }

      throw new UnexpectedCharacterDiagnostic(this.file, c0, this.getPosition());

    }

  }

}

const enum FrameType {
  Block,
  LineFold,
}

const INIT_POS = { line: 0, column: 0, offset: 0 }

export class Punctuator extends BufferedStream<Token> {

  private referencePositions: TextPosition[] = [ INIT_POS ];
  private frameTypes: FrameType[] = [ FrameType.Block ];

  constructor(private tokens: Stream<Token>) {
    super();
  }

  public read(): Token {

    const refPos = this.referencePositions[this.referencePositions.length-1];
    const frameType = this.frameTypes[this.frameTypes.length-1];
    const t0 = this.tokens.peek(1);

    if (t0.kind === SyntaxKind.EndOfFile) {
      if (this.frameTypes.length === 1) {
        return t0;
      }
      this.frameTypes.pop();
      switch (frameType) {
        case FrameType.LineFold:
          return new LineFoldEnd(t0.range);
        case FrameType.Block:
          return new BlockEnd(t0.range);
      }
    }

    switch (frameType) {

      case FrameType.LineFold:
      {

        // This important check verifies we're still inside the line-fold. If
        // we aren't, we need to clean up the stack a bit and eventually return
        // a token that indicates the line-fold ended.
        if (t0.getStartLine() > refPos.line
          && t0.getStartColumn() <= refPos.column) {
          this.frameTypes.pop();
          this.referencePositions.pop();
          return new LineFoldEnd(t0.range);
        }

        const t1 = this.tokens.peek(2);
        if (t0.kind === SyntaxKind.DotSign && t0.getEndLine() < t1.getStartLine()) {
          this.tokens.get();
          this.frameTypes.push(FrameType.Block);
          return new BlockStart(t0);
        }

        // If we got here, this is an ordinary token that is part of the
        // line-fold. Make sure to consume it and return it to the caller.
        this.tokens.get();
        return t0;
      }

      case FrameType.Block:
      {

        if (t0.getStartColumn() <= refPos.column) {

          // We only get here if the current token is less indented than the
          // current reference token. Pop the block indicator and leave the
          // reference position be for the edge case where the parent line-fold
          // continues after the block.
          this.frameTypes.pop();
          return new BlockEnd(t0.range);

        }

        this.frameTypes.push(FrameType.LineFold);
        this.referencePositions.push(t0.getStartPos());

        // In theory, we could explictly issue a LineFoldStart and let all
        // tokens be passed through in the FrameType.LineFold case. It does add
        // more logic to the parser for no real benefit, which is why it was
        // omitted.
        this.tokens.get();
        return t0;
      }

    }

  }

}

