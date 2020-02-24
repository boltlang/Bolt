
import XRegExp from "xregexp"

import {
  TextFile,
  TextPos,
  TextSpan,
  Identifier,
  Operator,
  PunctType,
  Token,
  Punctuated,
} from "./ast"

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
      throw new Error(`given character is not a valid punctuator`)
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
    super(`${file.path}:${position.line}:${position.column}: unexpected char '${escapeChar(char)}'`)
  }
}

interface Stream<T> {
  read(): T
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

function isOperatorStart(ch: string) {
  return /[+\-*\/%$!><]/.test(ch)
}

function isOperatorPart(ch: string) {
  return /[=+\-*\/%$!><]/.test(ch)

}

const EOF = ''

export class Scanner {

  protected buffer: string[] = [];
  protected currPos = new TextPos(0,1,1);
  protected offset = 0;

  constructor(public file: TextFile, public input: string) {
    
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

  scanToken() {

    while (true) {

      const c0 = this.peekChar();

      if (isWhiteSpace(c0)) {
        this.getChar();
        continue;
      }

      if (c0 == EOF) {
        return null;
      }

      const startPos = this.currPos.clone()

      if (isOpenPunct(c0)) {

        this.getChar();

        const punctType = getPunctType(c0);
        const elements: Token[] = [];

        while (true) {

          const c1 = this.peekChar();

          if (c1 === EOF) {
            throw new ScanError(this.file, this.currPos.clone(), EOF)
          }

          if (isClosePunct(c1)) {
            if (punctType == getPunctType(c1)) {
              this.getChar();
              break;
            } else {
              throw new ScanError(this.file, this.currPos, c1);
            }
          }

          const token = this.scanToken();
          if (token === null) {
            throw new ScanError(this.file, this.currPos.clone(), EOF)
          }
          elements.push(token!);

        }

        const endPos = this.currPos.clone();

        return new Punctuated(punctType, elements, new TextSpan(this.file, startPos, endPos));

      } else if (isIdentStart(c0)) {

        const name = this.takeWhile(isIdentPart);
        const endPos = this.currPos.clone();
        return new Identifier(name, new TextSpan(this.file, startPos, endPos))

      } else if (isOperatorStart(c0)) {

        const text = this.takeWhile(isOperatorPart)
        const endPos = this.currPos.clone()
        return new Operator(text, new TextSpan(this.file, startPos, endPos));

      } else {

        throw new ScanError(this.file, this.currPos.clone(), c0);

      }

    }

  }

  scanTokenList() {

  }

}

