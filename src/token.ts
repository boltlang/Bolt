
import { TextPosition, TextRange } from "./text";

export enum TokenType {
  EndOfFile,
  EndOfIndent,
  Identifier,
  CustomOperator,
  Assignment,
  DecimalInteger,
  StructKeyword,
  ReturnKeyword,
  ImportKeyword,
  PubKeyword,
  LetKeyword,
  MutKeyword,
  TypeKeyword,
  PerformKeyword,
  ResumeKeyword,
  MatchKeyword,
  YieldKeyword,
  DotSign,
  DotDotSign,
  ColonSign,
  EqualSign,
  RArrowSign,
  TildeSign,
  CommaSign,
  BSlashSign,
  LBracket,
  RBracket,
  LBrace,
  RBrace,
  LParen,
  RParen,
}

export type Token
  = EndOfFile
  | EndOfIndent
  | Identifier
  | RArrowSign
  | DecimalInteger
  | CustomOperator
  | Assignment
  | StructKeyword
  | ReturnKeyword
  | ImportKeyword
  | PubKeyword
  | LetKeyword
  | MutKeyword
  | TypeKeyword
  | PerformKeyword
  | ResumeKeyword
  | MatchKeyword
  | YieldKeyword
  | DotSign
  | DotDotSign
  | ColonSign
  | EqualSign
  | TildeSign
  | CommaSign
  | BSlashSign
  | LBracket
  | RBracket
  | LBrace
  | RBrace
  | LParen
  | RParen

export abstract class TokenBase {

  public readonly type!: TokenType;

  public indentLevel!: number;
  public range!: TextRange | null;

  public constructor(
    type: TokenType,
    indentLevel: number,
    range: TextRange | null,
  ) {
    Object.defineProperties(this, {
      type: {
        value: type,
      },
      indentLevel: {
        writable: true,
        value: indentLevel,
      },
      range: {
        writable: true,
        value: range,
      },
    })
  }

  public abstract getText(): string;

  public getRange(): TextRange {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range;
  }

  public getStartPos(): TextPosition {
    return this.getRange()[0];
  }

  public getStartLine(): number {
    return this.getRange()[0].line;
  }

  public getStartColumn(): number {
    return this.getRange()[0].column;
  }

  public getEndPos(): TextPosition {
    return this.getRange()[1];
  }

  public getEndLine(): number {
    return this.getRange()[1].line;
  }

  public getEndColumn(): number {
    return this.getRange()[1].column;
  }

}

export class EndOfFile extends TokenBase {

  public readonly type!: TokenType.EndOfFile;

  public constructor(
    range: TextRange | null = null,
  ) {
    super(TokenType.EndOfFile, 0, range);
  }

  public getText(): string {
    return '';
  }

}

export class EndOfIndent extends TokenBase {

  public readonly type!: TokenType.EndOfIndent;

  public constructor(
    public token: Token
  ) {
    super(TokenType.EndOfIndent, token.indentLevel, token.range);
  }

  public getText(): string {
    return this.token.getText();
  }

}

export class Identifier extends TokenBase {

  public readonly type!: TokenType.Identifier;

  public constructor(
    public text: string,
    indentLevel: number,
    range: TextRange | null = null,
  ) {
    super(TokenType.Identifier, indentLevel, range);
  }

  public getText(): string {
    return this.text;
  }

}

export class CustomOperator extends TokenBase {

  public readonly type!: TokenType.CustomOperator;

  public constructor(
    public text: string,
    indentLevel: number,
    range: TextRange | null = null,
  ) {
    super(TokenType.CustomOperator, indentLevel, range);
  }

  public getText(): string {
    return this.text;
  }

}

export class Assignment extends TokenBase {

  public readonly type!: TokenType.Assignment;

  public constructor(
    public text: string,
    indentLevel: number,
    range: TextRange | null = null,
  ) {
    super(TokenType.Assignment, indentLevel, range);
  }

  public getText(): string {
    return this.text + '=';
  }

}

export class DecimalInteger extends TokenBase {

  public readonly type!: TokenType.DecimalInteger;

  constructor(
    public numLeadingZeroes: number,
    public value: bigint,
    indentLevel: number,
    range: TextRange | null = null,
  ) {
    super(TokenType.DecimalInteger, indentLevel, range);
  }

  public getText(): string {
    return '0'.repeat(this.numLeadingZeroes) + this.value.toString();
  }

}

const TOKEN_TEXT: Partial<Record<TokenType, string>> = {
  [TokenType.LetKeyword]: 'let',
  [TokenType.PubKeyword]: 'pub',
  [TokenType.MutKeyword]: 'mut',
  [TokenType.PerformKeyword]: 'perform',
  [TokenType.YieldKeyword]: 'yield',
  [TokenType.ResumeKeyword]: 'resume',
  [TokenType.StructKeyword]: 'struct',
  [TokenType.MatchKeyword]: 'match',
  [TokenType.ReturnKeyword]: 'return',
  [TokenType.ImportKeyword]: 'import',
  [TokenType.TypeKeyword]: 'type',
  [TokenType.DotSign]: '.',
  [TokenType.DotDotSign]: '..',
  [TokenType.ColonSign]: ':',
  [TokenType.TildeSign]: '~',
  [TokenType.RArrowSign]: '->',
  [TokenType.CommaSign]: ',',
  [TokenType.BSlashSign]: '\\',
  [TokenType.EqualSign]: '=',
  [TokenType.LBracket]: '{',
  [TokenType.RBracket]: '}',
  [TokenType.LBrace]: '{',
  [TokenType.RBrace]: '}',
  [TokenType.LParen]: '(',
  [TokenType.RParen]: ')',
}

export class SimpleToken<K extends TokenType> extends TokenBase {

  public readonly type!: K;

  constructor(
    type: K,
    indentLevel: number,
    range: TextRange | null = null,
  ) {
    super(type, indentLevel, range);
  }

  public getText(): string {
    return TOKEN_TEXT[this.type]!;
  }

}

export type DotSign = SimpleToken<TokenType.DotSign>;
export type DotDotSign = SimpleToken<TokenType.DotDotSign>;
export type ColonSign = SimpleToken<TokenType.ColonSign>;
export type EqualSign = SimpleToken<TokenType.EqualSign>;
export type TildeSign = SimpleToken<TokenType.TildeSign>;
export type RArrowSign = SimpleToken<TokenType.RArrowSign>;
export type CommaSign = SimpleToken<TokenType.CommaSign>;
export type BSlashSign = SimpleToken<TokenType.BSlashSign>;
export type LBracket = SimpleToken<TokenType.LBracket>;
export type RBracket = SimpleToken<TokenType.RBracket>;
export type LParen = SimpleToken<TokenType.LParen>;
export type RParen = SimpleToken<TokenType.RParen>;
export type LBrace = SimpleToken<TokenType.LBrace>;
export type RBrace = SimpleToken<TokenType.RBrace>;

export type ReturnKeyword = SimpleToken<TokenType.ReturnKeyword>;
export type MatchKeyword = SimpleToken<TokenType.MatchKeyword>;
export type StructKeyword = SimpleToken<TokenType.StructKeyword>;
export type ImportKeyword = SimpleToken<TokenType.ImportKeyword>;
export type PubKeyword = SimpleToken<TokenType.PubKeyword>;
export type LetKeyword = SimpleToken<TokenType.LetKeyword>;
export type MutKeyword = SimpleToken<TokenType.MutKeyword>;
export type TypeKeyword = SimpleToken<TokenType.TypeKeyword>;
export type PerformKeyword = SimpleToken<TokenType.PerformKeyword>;
export type ResumeKeyword = SimpleToken<TokenType.ResumeKeyword>;
export type YieldKeyword = SimpleToken<TokenType.YieldKeyword>;

export function describeTokenType(type: TokenType): string {
  if (type in TOKEN_TEXT) {
    return `'${TOKEN_TEXT[type]!}'`
  }
  switch (type) {
    case TokenType.EndOfFile: return 'end-of-file';
    case TokenType.Identifier: return 'an identifier';
    case TokenType.CustomOperator: return 'an operator';
    case TokenType.DecimalInteger: return 'a decimal integer';
    case TokenType.EndOfIndent: return 'the end of a line fold';
    default:
      throw new Error(`Could not describe TokenType: value went by unhandled.`);
  }
}

