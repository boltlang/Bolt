
import XRegExp from "xregexp"

import {
  TextFile,
  TextPos,
  TextSpan,
} from "./text"

import {
  SyntaxKind,
  BoltToken,
  BoltSentence,
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
  createBoltEOS,
  createBoltDot,
  createBoltEqSign,
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

function isOperatorStart(ch: string) {
  return /[+\-*\/%$!><]/.test(ch)
}

function isOperatorPart(ch: string) {
  return /[=+\-*\/%$!><]/.test(ch)

}

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
        return createBoltEOS(new TextSpan(this.file, startPos, startPos));
      }

      switch (c0) {
        case '.':
          this.getChar();
          return createBoltDot(new TextSpan(this.file, startPos, this.currPos.clone()));
        case '=':
          this.getChar();
          return createBoltEqSign(new TextSpan(this.file, startPos, this.currPos.clone()));
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
        return createBoltIdentifier(name, new TextSpan(this.file, startPos, endPos))

      } else if (isOperatorStart(c0)) {

        const text = this.takeWhile(isOperatorPart)
        const endPos = this.currPos.clone()
        const span = new TextSpan(this.file, startPos, endPos);

        if (text === '->') {
          return createBoltRArrow(span);
        } else {
          return createBoltOperator(text, span);
        }

      } else {

        throw new ScanError(this.file, this.currPos.clone(), c0);

      }

    }

  }

  peek(count = 1): BoltToken {
    while (this.scanned.length < count) {
      this.scanned.push(this.scanToken());
    }
    return this.scanned[count - 1];
  }

  get(): BoltToken {
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
        if (token.kind === SyntaxKind.BoltEOS) {
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

  scan() {
    const startPos = this.currPos.clone();
    const elements = this.scanTokens();
    const endPos = this.currPos.clone();
    return createBoltSourceFile(elements, new TextSpan(this.file, startPos, endPos));
  }

}
