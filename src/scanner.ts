
import { createIdentifier, createSimpleToken, SyntaxKind, TextPosition, Token, TokenSyntaxKind } from "./cst";
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
  'type': SyntaxKind.TypeKeyword,
  'import': SyntaxKind.ImportKeyword,
  'pub': SyntaxKind.PubKeyword,
  'mut': SyntaxKind.MutKeyword,
  'let': SyntaxKind.LetKeyword,
  'perform': SyntaxKind.PerformKeyword,
  'yield': SyntaxKind.YieldKeyword,
  'resume': SyntaxKind.ResumeKeyword,
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

  private scanChar(kind: TokenSyntaxKind): Token {
    const startPos = this.getPosition();
    this.getChar();
    const endPos = this.getPosition();
    // @ts-ignore Unification fails due to `kind` being ambiguous
    return createSimpleToken(kind, this.currIndentLevel, [startPos, endPos]);
  }

  public read(): Token {

    for (;;) {

      this.skipAllWhiteSpace();

      const c0 = this.peekChar()

      if (c0 === EOF) {
        return createSimpleToken(SyntaxKind.EndOfFile, 0);
      }

      switch (c0) {
        case '(': return this.scanChar(SyntaxKind.LParen);
        case '(': return this.scanChar(SyntaxKind.RParen);
        case '{': return this.scanChar(SyntaxKind.LBrace);
        case '}': return this.scanChar(SyntaxKind.RBrace);
        case '[': return this.scanChar(SyntaxKind.LBracket);
        case ']': return this.scanChar(SyntaxKind.RBracket);
        case '.': return this.scanChar(SyntaxKind.DotSign);
        case ':': return this.scanChar(SyntaxKind.ColonSign);
        case '=': return this.scanChar(SyntaxKind.EqualSign);
      }

      if (isIdentStart(c0)) {
        const startPos = this.getPosition();
        this.getChar();
        const name = c0 + this.takeWhile(isIdentPart);
        const endPos = this.getPosition();
        if (hasOwnProperty(KEYWORDS, name)) {
          // @ts-ignore Unification fails due to KEYWORDS[name] being ambiguous
          return createSimpleToken(KEYWORDS[name], this.currIndentLevel, [startPos, endPos]);
        }
        return createIdentifier(name, this.currIndentLevel, [startPos, endPos]);
      }

      throw new ScanError(c0, this.textOffset);

    }

  }

}


