
import { TextPosition } from "./text";
import { Token, Identifier, SimpleToken, TokenType, EndOfFile, DecimalInteger, CustomOperator, Assignment } from "./token";
import { BufferedStream, hasOwnProperty } from "./util";

const EOF = '\uFFFF';

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
  return /[+\-*/%&^|<>=?!]/.test(ch);
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

const KEYWORDS: Record<string, TokenType> = {
  'return': TokenType.ReturnKeyword,
  'struct': TokenType.StructKeyword,
  'type': TokenType.TypeKeyword,
  'import': TokenType.ImportKeyword,
  'pub': TokenType.PubKeyword,
  'mut': TokenType.MutKeyword,
  'let': TokenType.LetKeyword,
  'perform': TokenType.PerformKeyword,
  'yield': TokenType.YieldKeyword,
  'resume': TokenType.ResumeKeyword,
  'match': TokenType.MatchKeyword,
}

export class ScanError extends Error {

  constructor(
    public actual: string,
    public offset: number,
  ) {
    super(`Unexpected ${describeChar(actual)}`);
  }

}

export class Scanner extends BufferedStream<Token> {

  constructor(
    public text: string,
    public textOffset = 0,
    private atBlankLine = true,
    private currIndentLevel = 0,
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
      this.currIndentLevel = 0;
      this.atBlankLine = true;
    } else {
      if (this.atBlankLine) {
        if (isWhiteSpace(ch)) {
          ++this.currIndentLevel;
        } else {
          this.atBlankLine = false;
        }
      }
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
    for (;;) {
      const c0 = this.peekChar()
      if (c0 === '#') {
        this.getChar();
        for (;;) {
          const c1 = this.getChar()
          if (c1 === EOF || c1 === '\n') {
            break;
          }
        }
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

  private scanChar(kind: TokenType): Token {
    const startPos = this.getPosition();
    this.getChar();
    const endPos = this.getPosition();
    // @ts-ignore Unification fails due to `kind` being ambiguous
    return new SimpleToken(kind, this.currIndentLevel, [startPos, endPos]);
  }

  public read(): Token {

    for (;;) {

      this.skipAllWhiteSpace();

      const c0 = this.peekChar()

      if (c0 === EOF) {
        return new EndOfFile([ this.getPosition(), this.getPosition() ]);
      }

      switch (c0) {
        case '(': return this.scanChar(TokenType.LParen);
        case ')': return this.scanChar(TokenType.RParen);
        case '{': return this.scanChar(TokenType.LBrace);
        case '}': return this.scanChar(TokenType.RBrace);
        case '[': return this.scanChar(TokenType.LBracket);
        case ']': return this.scanChar(TokenType.RBracket);
        case '.': return this.scanChar(TokenType.DotSign);
        case ':': return this.scanChar(TokenType.ColonSign);
        case '=': return this.scanChar(TokenType.EqualSign);
        case ',': return this.scanChar(TokenType.CommaSign);
        case '\\': return this.scanChar(TokenType.BSlashSign);
      }

      if (isOperator(c0)) {
        const startPos = this.getPosition();
        this.getChar()
        const text = c0 + this.takeWhile(isOperator);
        const endPos = this.getPosition();
        if (text.endsWith('=') && text[text.length-2] !== '=') {
          return new Assignment(text.substring(0, text.length-1), this.currIndentLevel, [startPos, endPos]);
        }
        return new CustomOperator(text, this.currIndentLevel, [startPos, endPos]);
      }

      if (isDigit(c0)) {
        const startPos = this.getPosition()
        this.getChar()
        const digits = c0 + this.takeWhile(isDigit)
        const endPos = this.getPosition();
        return new DecimalInteger(
          countLeadingChars(digits, '0'),
          BigInt(digits),
          this.currIndentLevel,
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
          return new SimpleToken(KEYWORDS[name], this.currIndentLevel, [startPos, endPos]);
        }
        return new Identifier(name, this.currIndentLevel, [startPos, endPos]);
      }

      throw new ScanError(c0, this.textOffset);

    }

  }

}


