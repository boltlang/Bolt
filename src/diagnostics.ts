import {Syntax, SyntaxKind} from "./cst";
import { formatExcerpt, TextFile } from "./text";
import { describeTokenType, Token, TokenType } from "./token";
import { CompareMode } from "./util";

interface Diagnostic {
  getMessage(): string;
  format(): string;
}

export interface Diagnostics {
  add(diagnostic: Diagnostic): void;
}

export class ConsoleDiagnostics implements Diagnostics {

  public add(diagnostic: Diagnostic): void {
    if (process.env['BOLT_FORCE_EXCEPTION']) {
      throw new Error(diagnostic.getMessage());
    }
    console.error(diagnostic.format());
  }

}

export type ExpectedParse = TokenType;

export class UnexpectedTokenDiagnostic implements Diagnostic {

  constructor(
    public file: TextFile,
    public actual: Token,
    public expected: ExpectedParse[],
  ) {

  }

  public getMessage(): string {
    const expectedFragments = this.expected.map(describeTokenType);
    if (this.actual.type === TokenType.EndOfIndent) {
      return `got a wrongly indented '${this.actual.getText()}' while expecting ${formatSum(expectedFragments)}.`;
    }
    const actualText = this.actual.type === TokenType.EndOfFile ? 'end-of-file' : `'${this.actual.getText()}'`;
    return `got ${actualText} but expected ${formatSum(expectedFragments)}.`;
  }

  public format(): string {
    let out = '';
    out += this.getMessage() + '\n\n';
    out += formatExcerpt(this.file.getText(), this.actual.range!)
    return out;
  }

}

export class UnexpectedConstructDiagnostic implements Diagnostic {

  public constructor(
    public node: Syntax,
  ) {

  }

  public getMessage(): string {
    return `unexpected ${SyntaxKind[this.node.kind]}.`;
  }

  public format(): string {
    return this.getMessage();
  }

}

export class UnexpectedIndentationDiagnostic implements Diagnostic {

  constructor(
    public file: TextFile,
    public compareMode: CompareMode,
    public actualIndentLevel: number,
    public expectedIndentLevel: number,
  ) {

  }

  public getMessage(): string {
    let out = `expected an indentation `
    switch (this.compareMode) {
      case CompareMode.Equal:
        out += 'of exactly';
        break;
      case CompareMode.Lesser:
        out += 'strictly less than'
        break;
      case CompareMode.LesserOrEqual:
        out += 'less than or equal to';
        break;
      case CompareMode.Greater:
        out += 'strictly greater than'
        break;
      case CompareMode.GreaterOrEqual:
        out += 'greater than or equal to';
        break;
    }
    out += ` ${this.expectedIndentLevel} `
    out += this.expectedIndentLevel === 1 ? 'space' : 'spaces'
    out += ` but found ${this.actualIndentLevel} `
    out += this.actualIndentLevel === 1 ? 'space' : 'spaces'
    out += `.`;
    return out;
  }

  public format(): string {
    return this.getMessage()
  }

}

export class ExpectedEndOfLineFoldDiagnostic implements Diagnostic {

  constructor(
    public file: TextFile,
    public actual: Token
  ) {

  }

  public getMessage(): string {
    return `expected end of line-fold but got ${describeTokenType(this.actual.type)}.`;
  }

  public format(): string {
    let out = '';
    out += this.getMessage() + '\n\n';
    out += formatExcerpt(this.file.getText(), this.actual.getRange());
    return out;
  }

}

export class NewLineRequiredDiagnostic implements Diagnostic {

  constructor(
    public file: TextFile,
    public actual: Token,
  ) {

  }

  public getMessage(): string {
    return `expected ${describeTokenType(this.actual.type)} to be placed on a new line.`;
  }

  public format() {
    return this.getMessage();
  }

}


function formatSum(elements: string[]) {
  if (elements.length === 0) {
    return 'nothing';
  }
  if (elements.length === 1) {
    return elements[0];
  }
  let out = elements[0];
  for (let i = 1; i < elements.length-1; i++) {
    out += ', ' + elements[i];
  }
  out += ' or ' + elements[elements.length-1];
  return out
}

