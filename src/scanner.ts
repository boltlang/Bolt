
import { EOF, ScanError } from "./util"

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
  createBoltForKeyword,
  createBoltTraitDeclaration,
  createBoltTraitKeyword,
  createBoltImplKeyword,
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
  return /[_\p{L}]/u.test(ch)
}

function isIdentPart(ch: string) {
  return /[_\p{L}\p{Nd}]/u.test(ch)
}

function isSymbol(ch: string) {
  return /[=+\/\-*%$!><&^|]/.test(ch)
}

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
          case 'for':     return createBoltForKeyword(span);
          case 'trait':   return createBoltTraitKeyword(span);
          case 'impl':    return createBoltImplKeyword(span);
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

