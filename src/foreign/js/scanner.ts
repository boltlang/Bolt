
import { TextPos, TextSpan, TextFile } from "../../text"
import { EOF, ScanError } from "../../common"

import {
  JSToken,
  createJSIdentifier,
  createJSDot,
  createJSDotDotDot,
  createJSOpenBracket,
  createJSCloseBracket,
  createJSCloseParen,
  createJSOpenParen,
  createJSOpenBrace,
  createJSCloseBrace,
  createJSSemi,
  createJSComma,
  createEndOfFile,
  createJSMulOp,
  createJSNotOp,
  createJSBOrOp,
  createJSBNotOp,
  createJSBXorOp,
  createJSBAndOp,
  createJSGtOp,
  createJSLtOp,
  createJSDivOp,
  createJSSubOp,
  createJSAddOp,
  createJSLetKeyword,
  createJSWhileKeyword,
  createJSForKeyword,
  createJSFunctionKeyword,
  createJSExportKeyword,
  createJSImportKeyword,
  createJSConstKeyword,
  createJSAsKeyword,
  createJSReturnKeyword,
  createJSCatchKeyword,
  createJSFromKeyword,
  createJSString,
  createJSTryKeyword,
  createJSInteger,
} from "../../ast"

function isWhiteSpace(ch: string): boolean {
  return /[\u0009\u000B\u000C\u0020\u00A0\u000B\uFEFF\p{Zs}]/u.test(ch)
}

function isLineTerminator(ch: string): boolean {
  return ch === '\u000A'
      || ch === '\u000D'
      || ch === '\u2028'
      || ch === '\u2029';;
}

function isOperator(ch: string): boolean {
  return /[-+*/&^|%!<>=]/.test(ch)
}

function isIdentStart(ch: string): boolean {
  return /[\p{ID_Start}$_\\]/u.test(ch)
}

function isIdentPart(ch: string): boolean {
  return /[\u200C\u200D\p{ID_Continue}$\\]/u.test(ch)
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
    if (this.offset === this.input.length) {
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

    if (isLineTerminator(ch)) {
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
      if (isLineTerminator(c2)) {
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
      } else if (isWhiteSpace(c0) || isLineTerminator(c0)) {
        this.getChar();
      } else {
        break;
      }
    }
  }

  private scanHexDigit(): number {
    const startPos = this.currPos.clone();
    const c0 = this.getChar();
    switch (c0) {
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
      case 'A': return 10;
      case 'B': return 11;
      case 'C': return 12;
      case 'D': return 13;
      case 'E': return 14;
      case 'F': return 15;
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

  protected takeWhile(pred: (ch: string) => boolean) {
    let text = this.getChar();
    while (true) {
      const c0 = this.peekChar();
      if (c0 === EOF) {
        break;
      }
      if (!pred(c0)) {
        break;
      }
      this.getChar()
      text += c0;
    }
    return text;
  }

  public scan(): JSToken {

    this.skipComments();

    const c0 = this.peekChar();

    if (c0 === EOF) {
      return createEndOfFile(new TextSpan(this.file, this.currPos.clone(), this.currPos.clone()))
    }

    const startPos = this.currPos.clone();

    if (c0 === '"' || c0 === "'") {
      // FIXME
      this.getChar();
      const value = this.takeWhile(ch => ch !== c0)
      this.getChar();
      const endPos = this.currPos.clone();
      return createJSString(value, new TextSpan(this.file, startPos, endPos))
    }

    if (/[,;()\[\]{}]/.test(c0)) {
      this.getChar();
      const span = new TextSpan(this.file, startPos, this.currPos.clone());
      switch (c0) {
        case '(': return createJSOpenParen(span);
        case ')': return createJSCloseParen(span);
        case '[': return createJSOpenBracket(span);
        case ']': return createJSCloseBracket(span);
        case '{': return createJSOpenBrace(span);
        case '}': return createJSCloseBrace(span);
        case ',': return createJSComma(span);
        case ';': return createJSSemi(span);
      }
    }

    let i = 0;
    let ch = c0;
    while (ch === '.') {
      this.getChar();
      ch = this.peekChar();
      i++;
    }

    if (i > 0) {
      if (i === 1) {
        return createJSDot(new TextSpan(this.file, startPos, this.currPos.clone()));
      } else if (i === 3) {
        return createJSDotDotDot(new TextSpan(this.file, startPos, this.currPos.clone()));
      } else {
        throw new ScanError(this.file, startPos, c0);
      }
    }

    if (c0 === '0') {
      this.getChar();
      const endPos = this.currPos.clone();
      return createJSInteger(0, new TextSpan(this.file, startPos, endPos));
    }

    if (isOperator(c0)) {
      const text = this.takeWhile(isOperator)
      const span = new TextSpan(this.file, startPos, this.currPos.clone());
      switch (text) {
        case '+': return createJSAddOp(span);
        case '-': return createJSSubOp(span);
        case '*': return createJSMulOp(span);
        case '/': return createJSDivOp(span);
        case '<': return createJSLtOp(span);
        case '>': return createJSGtOp(span);
        case '&': return createJSBAndOp(span);
        case '^': return createJSBXorOp(span);
        case '~': return createJSBNotOp(span);
        case '|': return createJSBOrOp(span);
        case '!': return createJSNotOp(span);
      }
    }

    if (isIdentStart(c0)) {
      let name = '';
      while (true) {
        const c0 = this.peekChar();
        if (!isIdentPart(c0)) {
          break;
        }
        if (c0 === '\\') {
          name += this.scanUnicodeEscapeSequence();
        } else {
          name += this.getChar();
        }
      }
      const endPos = this.currPos.clone();
      const span = new TextSpan(this.file, startPos, endPos);
      switch (name) {
        case 'return':   return createJSReturnKeyword(span);
        case 'try':      return createJSTryKeyword(span);
        case 'catch':    return createJSCatchKeyword(span);
        case 'from':     return createJSFromKeyword(span);
        case 'let':      return createJSLetKeyword(span);
        case 'const':    return createJSConstKeyword(span);
        case 'import':   return createJSImportKeyword(span);
        case 'export':   return createJSExportKeyword(span);
        case 'as':       return createJSAsKeyword(span);
        case 'function': return createJSFunctionKeyword(span);
        case 'for':      return createJSForKeyword(span);
        case 'while':    return createJSWhileKeyword(span);
        default:         return createJSIdentifier(name, span)
      }
    } else {
      throw new ScanError(this.file, startPos, c0);
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

