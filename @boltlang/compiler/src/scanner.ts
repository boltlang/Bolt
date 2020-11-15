
import { EOF, ScanError } from "./common"

import {
  TextFile,
  TextPos,
  TextSpan,
} from "./text"

import {
  BoltToken,
  createBoltImportKeyword,
  createBoltRArrowAlt,
  createEndOfFile,
  createBoltIdentifier,
  createBoltRArrow,
  createBoltOperator,
  createBoltParenthesized,
  createBoltBraced,
  createBoltBracketed,
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
  createBoltLtSign,
  createBoltGtSign,
  createBoltModKeyword,
  createBoltTypeKeyword,
  createBoltForKeyword,
  createBoltTraitKeyword,
  createBoltImplKeyword,
  createBoltMatchKeyword,
  createBoltQuoteKeyword,
  createBoltLetKeyword,
  createBoltVBar,
  createBoltColonColon,
  createBoltExMark,
  createBoltWhereKeyword,
  createBoltIfKeyword,
  createBoltElseKeyword,
} from "./ast"

export enum PunctType {
  Paren,
  Bracket,
  Brace,
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


function isDigit(ch: string) {
  return /[\p{Nd}]/u.test(ch)
}

function isWhiteSpace(ch: string) {
  return /[\n\p{Zs}]/u.test(ch)
}

function isNewLine(ch: string) {
  return ch == '\n'
}

function isIdentStart(ch: string) {
  return /[_\p{ID_Start}]/u.test(ch)
}

function isIdentPart(ch: string) {
  return /[_\p{ID_Continue}]/u.test(ch)
}

function isSymbol(ch: string) {
  return /[=+\/\-*%$!><&^|\.]/.test(ch)
}

export class Scanner {

  private buffer: string[] = [];
  private currPos: TextPos;
  private offset = 0;

  constructor(public file: TextFile, public input: string, startPos = new TextPos(0,1,1)) {
    this.currPos = startPos;
  }

  private readChar() {
    if (this.offset == this.input.length) {
      return EOF
    }
    return this.input[this.offset++]
  }

  private peekChar(count = 1) {
    while (this.buffer.length < count) {
      this.buffer.push(this.readChar());
    }
    return this.buffer[count - 1];
  }

  private getChar() {

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

  private takeWhile(pred: (ch: string) => boolean) {
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

  private scanHexDigit(): number {
    const c0 = this.peekChar();
    let out;
    switch (c0) {
      case '0': out = 0; break;
      case '1': out = 1; break;
      case '2': out = 2; break;
      case '3': out = 3; break;
      case '4': out = 4; break;
      case '5': out = 5; break;
      case '6': out = 6; break;
      case '7': out = 7; break;
      case '8': out = 8; break;
      case '9': out = 9; break;
      case 'a': out = 10; break;
      case 'b': out = 11; break;
      case 'c': out = 12; break;
      case 'd': out = 13; break;
      case 'e': out = 14; break;
      case 'f': out = 15; break;
      case 'A': out = 10; break;
      case 'B': out = 11; break;
      case 'C': out = 12; break;
      case 'D': out = 13; break;
      case 'E': out = 14; break;
      case 'F': out = 15; break;
      default:
        throw new ScanError(this.file, this.currPos.clone(), c0);
    }
    this.getChar();
    return out;
  }

  private scanEscapeSequence(): string {
    this.assertChar('\\')
    const c0 = this.peekChar();
    switch (c0) {
      case 'a': 
        this.getChar();
        return '\n'
      case 'b': 
        this.getChar();
        return '\b'
      case 'f': 
        this.getChar();
        return '\f'
      case 'n': 
        this.getChar();
        return '\n'
      case 'r': 
        this.getChar();
        return '\r'
      case 't': 
        this.getChar();
        return '\t'
      case 'v': 
        this.getChar();
        return '\v'
      case '0': 
        this.getChar();
        return '\0'
      case 'u':
      {
        const d0 = this.scanHexDigit();
        const d1 = this.scanHexDigit();
        const d2 = this.scanHexDigit();
        const d3 = this.scanHexDigit();
        return String.fromCharCode(d0 * (16 ** 3) + d1 * (16 ** 2) + d2 * 16 + d3);
      } 
      case 'x':
      {
        const d0 = this.scanHexDigit();
        const d1 = this.scanHexDigit();
        return String.fromCharCode(d0 * 16 + d1);
      }
      default:
        throw new ScanError(this.file, this.currPos.clone(), c0);
    }
  }

  public scan(): BoltToken {

    while (true) {

      const c0 = this.peekChar();

      if (isWhiteSpace(c0)) {
        this.getChar();
        continue;
      }

      const c1 = this.peekChar(2);
      if (c0 === '/' && c1 == '/') {
        this.scanLineComment();
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
          if (this.peekChar() === ':') {
            this.getChar();
            return createBoltColonColon(new TextSpan(this.file, startPos, this.currPos.clone()));
          }
          return createBoltColon(new TextSpan(this.file, startPos, this.currPos.clone()));
      }

      if (c0 === '"') {

        this.getChar();

        let text = ''

        while (true) {
          const c1 = this.peekChar();
          if (c1 === EOF) {
            throw new ScanError(this.file, this.currPos.clone(), EOF);
          }
          if (c1 === '"') {
            this.getChar();
            break;
          } else if (c1 === '\\') {
            text += this.scanEscapeSequence()
          } else {
            this.getChar();
            text += c1
          }
        }

        const endPos = this.currPos.clone();

        return createBoltStringLiteral(text, new TextSpan(this.file, startPos, endPos))

      } else if (isDigit(c0)) {

        const digits = this.takeWhile(isDigit);
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
          case 'where':   return createBoltWhereKeyword(span);
          case 'return':  return createBoltReturnKeyword(span);
          case 'match':   return createBoltMatchKeyword(span);
          case 'yield':   return createBoltYieldKeyword(span);
          case 'for':     return createBoltForKeyword(span);
          case 'trait':   return createBoltTraitKeyword(span);
          case 'impl':    return createBoltImplKeyword(span);
          case 'type':    return createBoltTypeKeyword(span);
          case 'import':  return createBoltImportKeyword(span);
          case 'foreign': return createBoltForeignKeyword(span);
          case 'let':     return createBoltLetKeyword(span);
          case 'mut':     return createBoltMutKeyword(span);
          case 'struct':  return createBoltStructKeyword(span);
          case 'quote':   return createBoltQuoteKeyword(span);
          case 'enum':    return createBoltEnumKeyword(span);
          case 'if':      return createBoltIfKeyword(span);
          case 'else':    return createBoltElseKeyword(span);
          default:        return createBoltIdentifier(name, span);
        }

      } else if (isSymbol(c0)) {

        const text = this.takeWhile(isSymbol)
        const endPos = this.currPos.clone()
        const span = new TextSpan(this.file, startPos, endPos);

        switch (text) {
          case '!': return createBoltExMark(span);
          case '|': return createBoltVBar(span);
          case '->': return createBoltRArrow(span);
          case '=>': return createBoltRArrowAlt(span);
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

  private assertChar(ch: string): void {
    const c0 = this.peekChar();
    if (c0 !== ch) {
      throw new ScanError(this.file, this.currPos.clone(), ch);
    }
    this.getChar();
  }

  private scanLineComment(): string {
    let text = '';
    this.assertChar('/');
    this.assertChar('/');
    while (true) {
      const c0 = this.peekChar();
      if (c0 === EOF) {
        break;
      }
      if (c0 == '\n') {
        this.getChar();
        const c1 = this.peekChar();
        if (c1 === '\r') {
          this.getChar();
        }
        break;
      }
      text += c0
      this.getChar();
    }
    return text;
  }

}

