
import {
  SyntaxKind,
  Token,
  Identifier,
  StringLiteral,
  EndOfFile,
  BlockStart,
  BlockEnd,
  LineFoldEnd,
  PubKeyword,
  MutKeyword,
  LetKeyword,
  ImportKeyword,
  TypeKeyword,
  TextPosition,
  Colon,
  Comma,
  Equals,
  LParen,
  RParen,
  LBrace,
  LBracket,
  RBrace,
  RBracket,
  ReturnKeyword,
  CustomOperator,
  IdentifierAlt,
  Integer,
  TextFile,
  Dot,
  DotDot,
  Assignment,
  ElifKeyword,
  ElseKeyword,
  IfKeyword,
  StructKeyword,
  RArrow,
  EnumKeyword,
  MatchKeyword,
  RArrowAlt,
  VBar,
  ForeignKeyword,
  ModKeyword,
} from "./cst"
import { Diagnostics, UnexpectedCharDiagnostic } from "./diagnostics"
import { Stream, BufferedStream, assert } from "./util";

const EOF = '\uFFFF'

function isUpper(ch: string): boolean {
  return ch.toUpperCase() === ch;
}

function isWhiteSpace(ch: string): boolean {
  return /[\r\n\t ]/.test(ch);
}

function isIdentPart(ch: string): boolean {
  return /[a-zA-Z0-9_]/.test(ch);
}

function isIdentStart(ch: string): boolean {
  return /[a-zA-Z_]/.test(ch)
}

function isDecimalDigit(ch: string): boolean {
  return /[0-9]/.test(ch);
}

function toDecimal(ch: string): number {
  const code = ch.charCodeAt(0);
  assert(code >= 48 && code <= 57);
  return code - 48;
}

function isOperatorPart(ch: string): boolean {
  return /[+\-*\/%^&|$<>!?=]/.test(ch);
}

export class ScanError extends Error {

  public constructor(
    public file: TextFile,
    public position: TextPosition,
    public actual: string,
  ) {
    super(`Uncaught scanner error`);
  }

}

export class Scanner extends BufferedStream<Token> {

  private currLine = 1;
  private currColumn = 1;

  public constructor(
    public text: string,
    public textOffset: number = 0,
    public diagnostics: Diagnostics,
    private file: TextFile,
  ) {
    super();
  }

  private peekChar(offset = 1): string {
    const i = this.textOffset + offset - 1;
    return i < this.text.length ? this.text[i] : EOF;
  }

  private getChar(): string {
    const ch = this.textOffset < this.text.length
      ? this.text[this.textOffset++]
      : EOF
    if (ch === '\n') {
      this.currLine++;
      this.currColumn = 1;
    } else {
      this.currColumn++;
    }
    return ch;
  }

  private takeWhile(pred: (ch: string) => boolean): string {
    let out = ''
    for (;;) {
      const c0 = this.peekChar()
      if (!pred(c0)) {
        break;
      }
      this.getChar()
      out += c0;
    }
    return out;
  }

  private getCurrentPosition(): TextPosition {
    return new TextPosition(
      this.textOffset,
      this.currLine,
      this.currColumn
    );
  }

  public read(): Token {

    let c0: string;

    // Skip whitespace and comments
    for (;;) {

      for (;;) {
        c0 = this.peekChar();
        if (isWhiteSpace(c0)) {
          this.getChar();
          continue;
        }
        if (c0 === '#') {
          this.getChar();
          for (;;) {
            const c1 = this.getChar();
            if (c1 === '\n' || c1 === EOF) {
              break;
            }
          }
          continue;
        }

        // We failed to match a newline or line comment, so there's nothing to skip
        break;

      }

      const startPos = this.getCurrentPosition();
      this.getChar();

      switch (c0) {

        case '"':
        {
          const startPos = this.getCurrentPosition();
          let contents = '';
          let escaping = false;
          for (;;) {
            if (escaping) {
              const startPos = this.getCurrentPosition();
              const c1 = this.getChar();
              switch (c1) {
                case 'a': contents += '\a'; break;
                case 'b': contents += '\b'; break;
                case 'f': contents += '\f'; break;
                case 'n': contents += '\n'; break;
                case 'r': contents += '\r'; break;
                case 't': contents += '\t'; break;
                case 'v': contents += '\v'; break;
                case '0': contents += '\0'; break;
                case '\'': contents += '\''; break;
                case '\"': contents += '\"'; break;
                default:
                  throw new ScanError(this.file, startPos, c1);
              }
              escaping = false;
            } else {
              const c1 = this.getChar();
              if (c1 === '"') {
                break;
              } else {
                contents += c1;
              }
            }
          }
          return new StringLiteral(contents, startPos);
        }

        case EOF:
        {
          return new EndOfFile(startPos);
        }

        case '(': return new LParen(startPos);
        case ')': return new RParen(startPos);
        case '[': return new LBracket(startPos);
        case ']': return new RBracket(startPos);
        case '{': return new LBrace(startPos);
        case '}': return new RBrace(startPos);
        case ',': return new Comma(startPos);
        case ':': return new Colon(startPos);
        case '.': {
          const dots = c0 + this.takeWhile(ch => ch === '.');
          if (dots === '.') {
            return new Dot(startPos);
          } else if (dots === '..') {
            return new DotDot(startPos);
          } else {
            throw new ScanError(this.file, startPos, dots);
          }
        }

        case '+':
        case '-':
        case '*':
        case '/':
        case '%':
        case '&':
        case '^':
        case '|':
        case '$':
        case '<':
        case '>':
        case '=':
        case '!':
        case '?':
        {
          const text = c0 + this.takeWhile(isOperatorPart);
          if (text === '->') {
            return new RArrow(startPos);
          } else if (text === '=>') {
            return new RArrowAlt(startPos);
          } else if (text === '|') {
            return new VBar(startPos);
          } else if (text === '=') {
            return new Equals(startPos);
          } else if (text.endsWith('=') && text[text.length-2] !== '=') {
            return new Assignment(text, startPos);
          } else {
            return new CustomOperator(text, startPos);
          }
        }

        case '0':
        {
          const c1 = this.peekChar();
          switch (c1) {
            case 'x': // TODO
            case 'o': // TODO
            case 'b': // TODO
          }
        }
        case '1':
        case '2':
        case '3':
        case '4':
        case '5':
        case '6':
        case '7':
        case '8':
        case '9':
        {
          let value = BigInt(toDecimal(c0));
          for (;;) {
            const c1 = this.peekChar();
            if (!isDecimalDigit(c1)) {
              break;
            }
            this.getChar();
            value = value * BigInt(10) + BigInt(toDecimal(c1));
          }
          return new Integer(value, 10, startPos);
        }

        case 'a':
        case 'b':
        case 'c':
        case 'd':
        case 'e':
        case 'f':
        case 'g':
        case 'h':
        case 'i':
        case 'j':
        case 'k':
        case 'l':
        case 'm':
        case 'n':
        case 'o':
        case 'p':
        case 'q':
        case 'r':
        case 's':
        case 't':
        case 'u':
        case 'v':
        case 'w':
        case 'x':
        case 'y':
        case 'z':
        case 'A':
        case 'B':
        case 'C':
        case 'D':
        case 'E':
        case 'F':
        case 'G':
        case 'H':
        case 'I':
        case 'J':
        case 'K':
        case 'L':
        case 'M':
        case 'N':
        case 'O':
        case 'P':
        case 'Q':
        case 'R':
        case 'S':
        case 'T':
        case 'U':
        case 'V':
        case 'W':
        case 'X':
        case 'Y':
        case 'Z':
        case '_':
        {
          const text = c0 + this.takeWhile(isIdentPart);
          switch (text) {
            case 'import': return new ImportKeyword(startPos);
            case 'pub': return new PubKeyword(startPos);
            case 'mut': return new MutKeyword(startPos);
            case 'let': return new LetKeyword(startPos);
            case 'import': return new ImportKeyword(startPos);
            case 'return': return new ReturnKeyword(startPos);
            case 'type': return new TypeKeyword(startPos);
            case 'if': return new IfKeyword(startPos);
            case 'else': return new ElseKeyword(startPos);
            case 'elif': return new ElifKeyword(startPos);
            case 'struct': return new StructKeyword(startPos);
            case 'enum': return new EnumKeyword(startPos);
            case 'match': return new MatchKeyword(startPos);
            case 'foreign': return new ForeignKeyword(startPos);
            case 'mod': return new ModKeyword(startPos);
            default:
              if (isUpper(text[0])) {
                return new IdentifierAlt(text, startPos);
              } else {
                return new Identifier(text, startPos);
              }
          }
        }

        default:

          // Nothing matched, so the current character is unrecognisable
          throw new ScanError(this.file, startPos, c0);
        }

    }

  }

}

const enum FrameType {
  Block,
  LineFold,
  Fallthrough,
}

const INIT_POS = new TextPosition(0, 0, 0);

export class Punctuator extends BufferedStream<Token> {

  private referencePositions: TextPosition[] = [ INIT_POS ];

  private frameTypes: FrameType[] = [ FrameType.Block ];

  public constructor(
    private tokens: Stream<Token>,
  ) {
    super();
  }

  public read(): Token {

    const t0 = this.tokens.peek(1);

    switch (t0.kind) {
      case SyntaxKind.LBrace:
        this.frameTypes.push(FrameType.Fallthrough);
        break;
      case SyntaxKind.EndOfFile:
      {
        if (this.frameTypes.length === 1) {
          return t0;
        }
        const frameType = this.frameTypes.pop()!;
        switch (frameType) {
          case FrameType.LineFold:
            return new LineFoldEnd(t0.getStartPosition());
          case FrameType.Block:
            return new BlockEnd(t0.getStartPosition());
        }
      }
    }

    const refPos = this.referencePositions[this.referencePositions.length-1];
    const frameType = this.frameTypes[this.frameTypes.length-1];

    switch (frameType) {

      case FrameType.Fallthrough:
      {
        if (t0.kind === SyntaxKind.RBrace) {
          this.frameTypes.pop()!;
        }
        this.tokens.get();
        return t0;
      }

      case FrameType.LineFold:
      {

        // This important check verifies we're still inside the line-fold. If
        // we aren't, we need to clean up the stack a bit and eventually return
        // a token that indicates the line-fold ended.
        if (t0.getStartLine() > refPos.line
          && t0.getStartColumn() <= refPos.column) {
          this.frameTypes.pop();
          this.referencePositions.pop();
          return new LineFoldEnd(t0.getStartPosition());
        }

        const t1 = this.tokens.peek(2);
        if (t0.kind === SyntaxKind.Dot && t0.getEndLine() < t1.getStartLine()) {
          this.tokens.get();
          this.frameTypes.push(FrameType.Block);
          return new BlockStart(t0.getStartPosition());
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
          return new BlockEnd(t0.getStartPosition());

        }

        this.frameTypes.push(FrameType.LineFold);
        this.referencePositions.push(t0.getStartPosition());

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


