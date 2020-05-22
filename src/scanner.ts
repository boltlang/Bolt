
import XRegExp from "xregexp"

import {
  TextFile,
  TextPos,
  TextSpan,
} from "./text"

import {
  setParents,
  SyntaxKind,
  BoltToken,
  BoltSentence,
  createEndOfFile,
  createBoltSentence,
  createBoltIdentifier,
  createBoltRArrow,
  createBoltOperator,
  createBoltParenthesized,
  createBoltBraced,
  createBoltBracketed,
  createBoltSourceFile,
  createBoltSemi,
  createBoltComma,
  createBoltStringLiteral,
  createBoltIntegerLiteral,
  createBoltColon,
  createBoltDot,
  createBoltEqSign,
  createBoltPubKeyword,
  createBoltMutKeyword,
  createBoltStructKeyword,
  createBoltEnumKeyword,
  createBoltForeignKeyword,
  createBoltAssignment,
  createBoltYieldKeyword,
  createBoltReturnKeyword,
  createBoltFnKeyword,
  createBoltLArrow,
  createBoltDotDot,
  createJSIdentifier,
  JSToken,
  createBoltLtSign,
  createBoltGtSign,
  createBoltModKeyword,
  createBoltTypeKeyword,
} from "./ast"

export enum PunctType {
  Paren,
  Bracket,
  Brace,
}

function escapeChar(ch: string) {
  switch (ch) {
    case '\a': return '\\a';
    case '\b': return '\\b';
    case '\f': return '\\f';
    case '\n': return '\\n';
    case '\r': return '\\r';
    case '\t': return '\\t';
    case '\v': return '\\v';
    case '\0': return '\\0';
    case '\'': return '\\\'';
    default:
      const code = ch.charCodeAt(0);
      if (code >= 0x20 && code <= 0x7E)  {
        return ch
      } else if (code < 0x7F) {
        return `\\x${code.toString(16).padStart(2, '0')}`
      } else {
        return `\\u${code.toString(16).padStart(4, '0')}`
      }
  }
}

function getPunctType(ch: string) {
  switch (ch) {
    case '(':
    case ')':
      return PunctType.Paren;
    case '[':
    case ']':
      return PunctType.Bracket;
    case '{':
    case '}':
      return PunctType.Brace;
    default:
      return null;
  }
}

function isClosePunct(ch: string) {
  switch (ch) {
    case '}':
    case ']':
    case ')':
      return true;
    default:
      return false;
  }
}

function isOpenPunct(ch: string) {
  switch (ch) {
    case '{':
    case '(':
    case '[':
      return true;
    default:
      return false;
  }
}

class ScanError extends Error {
  constructor(public file: TextFile, public position: TextPos, public char: string) {
    super(`${file.origPath}:${position.line}:${position.column}: unexpected char '${escapeChar(char)}'`)
  }
}

function isDigit(ch: string) {
  return XRegExp('\\p{Nd}').test(ch)
}

function isWhiteSpace(ch: string) {
  return ch == '\n' || XRegExp('\\p{Zs}').test(ch)
}

function isNewLine(ch: string) {
  return ch == '\n'
}

function isIdentStart(ch: string) {
  return ch == '_' || XRegExp('\\p{L}').test(ch)
}

function isIdentPart(ch: string) {
  return ch == '_' || XRegExp('\\p{L}').test(ch)
}

function isSymbol(ch: string) {
  return /[=+\/\-*%$!><&^|]/.test(ch)
}


function isJSWhiteSpace(ch: string): boolean {
  return ch === '\u0009'
      || ch === '\u000B'
      || ch === '\u000C'
      || ch === '\u0020'
      || ch === '\u00A0'
      || ch === '\u000B'
      || ch === '\uFEFF'
      || XRegExp('\\p{Zs}').test(ch)
}

function isJSIdentStart(ch: string): boolean {
  return XRegExp('[\\p{ID_Start}$_\\]').test(ch)
}

function isJSIdentPart(ch: string): boolean {
  return XRegExp('[\u200C\u200D\\p{ID_Continue}$\\]').test(ch)
}

//function isOperatorPart(ch: string) {
  //return /[=+\-*\/%$!><]/.test(ch)
//}

const EOF = ''

export class Scanner {

  protected buffer: string[] = [];
  protected scanned: BoltToken[] = [];
  protected currPos: TextPos;
  protected offset = 0;

  constructor(public file: TextFile, public input: string, startPos = new TextPos(0,1,1)) {
    this.currPos = startPos;
  }

  protected readChar() {
    if (this.offset == this.input.length) {
      return EOF
    }
    return this.input[this.offset++]
  }

  protected peekChar(count = 1) {
    while (this.buffer.length < count) {
      this.buffer.push(this.readChar());
    }
    return this.buffer[count - 1];
  }

  protected getChar() {

    const ch = this.buffer.length > 0
      ? this.buffer.shift()!
      : this.readChar()

    if (ch == EOF) {
      return EOF
    }

    if (isNewLine(ch)) {
      this.currPos.line += 1;
      this.currPos.column = 1;
    } else {
      this.currPos.column += 1;
    }
    this.currPos.offset += 1;

    return ch
  }

  protected takeWhile(pred: (ch: string) => boolean) {
    let text = this.getChar();
    while (true) {
      const c0 = this.peekChar();
      if (!pred(c0)) {
        break;
      }
      this.getChar()
      text += c0;
    }
    return text;
  }

  scanToken(): BoltToken {

    while (true) {

      const c0 = this.peekChar();

      if (isWhiteSpace(c0)) {
        this.getChar();
        continue;
      }

      const startPos = this.currPos.clone()

      if (c0 == EOF) {
        return createEndOfFile(new TextSpan(this.file, startPos, startPos));
      }

      switch (c0) {
        case ';':
          this.getChar();
          return createBoltSemi(new TextSpan(this.file, startPos, this.currPos.clone()));
        case ',':
          this.getChar();
          return createBoltComma(new TextSpan(this.file, startPos, this.currPos.clone()));
        case ':':
          this.getChar();
          return createBoltColon(new TextSpan(this.file, startPos, this.currPos.clone()));
      }

      if (c0 === '"') {

        this.getChar();

        let text = ''

        while (true) {
          const c1 = this.getChar();
          if (c1 === EOF) {
            throw new ScanError(this.file, this.currPos.clone(), EOF);
          }
          if (c1 === '"') {
            break;
          } else if (c1 === '\\') {
            this.scanEscapeSequence()
          } else {
            text += c1
          }
        }

        const endPos = this.currPos.clone();

        return createBoltStringLiteral(text, new TextSpan(this.file, startPos, endPos))

      } else if (isDigit(c0)) {

        const digits = this.takeWhile(isDigit)
        const endPos = this.currPos.clone();
        return createBoltIntegerLiteral(BigInt(digits), new TextSpan(this.file, startPos, endPos));

      } else if (isOpenPunct(c0)) {

        this.getChar();

        const punctType = getPunctType(c0);
        let punctCount = 1;
        let text = ''

        while (true) {

          const c1 = this.getChar();

          if (c1 === EOF) {
            throw new ScanError(this.file, this.currPos.clone(), EOF)
          }

          if (punctType == getPunctType(c1)) {
            if (isClosePunct(c1)) {
              punctCount--;
              if (punctCount === 0)
                break;
            } else {
              punctCount++;
            }
          }

          text += c1

        }

        const endPos = this.currPos.clone();

        switch (punctType) {
          case PunctType.Brace:
            return createBoltBraced(text, new TextSpan(this.file, startPos, endPos));
          case PunctType.Paren:
            return createBoltParenthesized(text, new TextSpan(this.file, startPos, endPos));
          case PunctType.Bracket:
            return createBoltBracketed(text, new TextSpan(this.file, startPos, endPos));
          default:
            throw new Error("Got an invalid state.")
        }

      } else if (isIdentStart(c0)) {

        const name = this.takeWhile(isIdentPart);
        const endPos = this.currPos.clone();
        const span = new TextSpan(this.file, startPos, endPos);
        switch (name) {
          case 'pub':     return createBoltPubKeyword(span);
          case 'mod':     return createBoltModKeyword(span);
          case 'fn':      return createBoltFnKeyword(span);
          case 'return':  return createBoltReturnKeyword(span);
          case 'yield':   return createBoltYieldKeyword(span);
          case 'type':    return createBoltTypeKeyword(span);
          case 'foreign': return createBoltForeignKeyword(span);
          case 'let':     return createBoltPubKeyword(span);
          case 'mut':     return createBoltMutKeyword(span);
          case 'struct':  return createBoltStructKeyword(span);
          case 'enum':    return createBoltEnumKeyword(span);
          default:        return createBoltIdentifier(name, span);
        }

      } else if (isSymbol(c0)) {

        const text = this.takeWhile(isSymbol)
        const endPos = this.currPos.clone()
        const span = new TextSpan(this.file, startPos, endPos);

        switch (text) {
          case '->': return createBoltRArrow(span);
          case '<-': return createBoltLArrow(span);
          case '<':  return createBoltLtSign(span);
          case '>':  return createBoltGtSign(span);
          case '.':  return createBoltDot(span);
          case '..': return createBoltDotDot(span);
          case '=':  return createBoltEqSign(span);
          case '==': return createBoltOperator(text, span);
        }

        if (text.endsWith('=')) {
          const operator = text.substring(0, text.length-1);
          return createBoltAssignment(operator.length === 0 ? null : operator, span);
        }

        return createBoltOperator(text, span);

      } else {

        throw new ScanError(this.file, this.currPos.clone(), c0);

      }

    }

  }

  public peek(count = 1): BoltToken {
    while (this.scanned.length < count) {
      this.scanned.push(this.scanToken());
    }
    return this.scanned[count - 1];
  }

  public get(): BoltToken {
    return this.scanned.length > 0
      ? this.scanned.shift()!
      : this.scanToken();
  }

  scanTokens() {

    const elements: BoltSentence[] = []

    outer: while (true) {

      const tokens: BoltToken[] = [];

      inner: while (true) {
        const token = this.scanToken();
        if (token.kind === SyntaxKind.EndOfFile) {
          if (tokens.length === 0) {
            break outer;
          } else {
            break inner;
          }  
        }
        if (token.kind === SyntaxKind.BoltSemi) {
          break;
        }
        tokens.push(token)
        if (token.kind === SyntaxKind.BoltBraced) {
          break;
        }
      }

      if (tokens.length > 0) {
        elements.push(
          createBoltSentence(
            tokens,
            new TextSpan(this.file, tokens[0].span!.start.clone(), tokens[tokens.length-1].span!.end.clone())
          )
        )
      }

    }

    return elements
  }

  public scan() {
    const startPos = this.currPos.clone();
    const elements = this.scanTokens();
    const endPos = this.currPos.clone();
    const sourceFile = createBoltSourceFile(elements, new TextSpan(this.file, startPos, endPos));
    setParents(sourceFile);
    return sourceFile;
  }

}

export class JSScanner {

  private buffer: string[] = [];
  private scanned: JSToken[] = [];
  private offset = 0;

  constructor(
    private file: TextFile,
    private input: string,
    private currPos: TextPos = new TextPos(0,1,1),
  ) {
    
  }

  protected readChar() {
    if (this.offset == this.input.length) {
      return EOF
    }
    return this.input[this.offset++]
  }

  protected peekChar(count = 1) {
    while (this.buffer.length < count) {
      this.buffer.push(this.readChar());
    }
    return this.buffer[count - 1];
  }

  protected getChar() {

    const ch = this.buffer.length > 0
      ? this.buffer.shift()!
      : this.readChar()

    if (ch == EOF) {
      return EOF
    }

    if (isNewLine(ch)) {
      this.currPos.line += 1;
      this.currPos.column = 1;
    } else {
      this.currPos.column += 1;
    }
    this.currPos.offset += 1;

    return ch
  }

  private assertChar(expected: string) {
    const actual = this.getChar();
    if (actual !== expected) {
      throw new ScanError(this.file, this.currPos.clone(), actual);
    }
  }

  private scanLineComment(): string {
    let text = '';
    this.assertChar('/');
    this.assertChar('/')
    while (true) {
      const c2 = this.peekChar();
      if (c2 === '\n') {
        this.getChar();
        if (this.peekChar() === '\r') {
          this.getChar();
        }
        break;
      }
      if (c2 === EOF) {
        break;
      }
      text += this.getChar();
    }
    return text;
  }

  private scanMultiLineComment(): string {
    let text = '';
    while (true) {
      const c2 = this.getChar();
      if (c2 === '*') {
        const c3 = this.getChar();
        if (c3 === '/') {
          break;
        }
        text += c2 + c3;
      } else if (c2 === EOF) {
        throw new ScanError(this.file, this.currPos.clone(), c2);
      } else {
        text += c2;
      }
    }
    return text;
  }

  private skipComments() {
    while (true) {
      const c0 = this.peekChar();
      if (c0 === '/') {
        const c1 = this.peekChar(2);
        if (c1 == '/') {
          this.scanLineComment();
        } else if (c1 === '*') {
          this.scanMultiLineComment();
        } else {
          break;
        }
      } else if (isWhiteSpace(c0)) {
        this.getChar();
      } else {
        break;
      }
    }
  }

  private scanHexDigit(): number {
    const startPos = this.currPos.clone();
    const c0 = this.getChar();
    switch (c0.toLowerCase()) {
      case '0': return 0;
      case '1': return 1;
      case '2': return 2;
      case '3': return 3;
      case '4': return 4;
      case '5': return 5;
      case '6': return 6;
      case '7': return 7;
      case '8': return 8;
      case '9': return 0;
      case 'a': return 10;
      case 'b': return 11;
      case 'c': return 12;
      case 'd': return 13;
      case 'e': return 14;
      case 'f': return 15;
      default:
        throw new ScanError(this.file, startPos, c0);
    }
  }

  private scanUnicodeEscapeSequence() {
    throw new Error(`Scanning unicode escape sequences is not yet implemented.`);
  }

  public scan(): JSToken {
    this.skipComments();
    const c0 = this.peekChar();
    const startPos = this.currPos.clone();
    if (isJSIdentStart(c0)) {
      let name = '';
      while (true) {
        const c0 = this.peekChar();
        if (!isJSIdentPart(c0)) {
          break;
        }
        if (c0 === '\\') {
          name += this.scanUnicodeEscapeSequence();
        } else {
          name += this.getChar();
        }
      }
      const endPos = this.currPos.clone();
      return createJSIdentifier(name, new TextSpan(this.file, startPos, endPos))
    } else {
      throw new ScanError(this.file, this.currPos.clone(), c0);
    }
  }

  public peek(count = 1): JSToken {
    while (this.scanned.length < count) {
      this.scanned.push(this.scan());
    }
    return this.scanned[count - 1];
  }

  public get(): JSToken {
    return this.scanned.length > 0
      ? this.scanned.shift()!
      : this.scan();
  }

}

