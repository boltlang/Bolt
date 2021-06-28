
import chalk from "chalk"
import {ArrowType, Type} from "./checker";

import { describeSyntaxKind, Token, Syntax, SyntaxKind } from "./cst";
import { formatExcerpt, TextFile, TextPosition } from "./text";
import { CompareMode } from "./util";

export interface Diagnostic {
  format(): string;
}

export interface Diagnostics {
  add(diagnostic: Diagnostic): void;
}

export class ConsoleDiagnostics implements Diagnostics {

  private diagnostics: Diagnostic[] = [];

  public add(diagnostic: Diagnostic): void {
    if (process.env['BOLT_FORCE_EXCEPTION']) {
      throw new Error(`Got a compile error while BOLT_FORCE_EXCEPTION is set.`);
    }
    this.diagnostics.push(diagnostic);
  }

  public printAll(): void {
    for (const diagnostic of this.diagnostics) {
      console.error(diagnostic.format());
    }

  }


}

export type ExpectedParse = SyntaxKind;

export class UnexpectedTokenDiagnostic implements Diagnostic {

  constructor(
    public file: TextFile,
    public actual: Token,
    public expected: ExpectedParse[],
  ) {

  }

  public getMessage(): string {
    const expectedFragments = this.expected.map(describeSyntaxKind);
    let actualText;
    switch (this.actual.kind) {
      case SyntaxKind.EndOfFile:
      case SyntaxKind.LineFoldStart:
      case SyntaxKind.LineFoldEnd:
      case SyntaxKind.BlockStart:
      case SyntaxKind.BlockEnd:
        actualText = describeSyntaxKind(this.actual.kind);
        break;
      default:
        actualText = `'${this.actual.getText()}'`;
        break;
    }
    return `Expected ${formatSum(expectedFragments)} but got ${actualText}.`;
  }

  public format(): string {
    let out = '';
    out += chalk.bold.red('error:') + ' ' + this.getMessage() + '\n\n';
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

export class ParamCountMismatchDiagnostic implements Diagnostic {

  public constructor(
    public left: ArrowType,
    public right: ArrowType,
  ) {

  }

  public format(): string {
    const left = this.left.getSolved();
    const right = this.right.getSolved();
    let out = `${chalk.bold.red('error:')} Type ${chalk.yellow(formatType(left))} requires ${left.paramTypes.length} `;
    out += left.paramTypes.length === 1 ? 'parameter' : 'parameters';
    out += ` while ${chalk.yellow(formatType(right))} requires ${right.paramTypes.length}.`;
    if (left.node !== null) {
      out += '\n\n';
      out += formatExcerpt({
        text: left.node.getSourceText(),
        range: left.node.getRange(),
        message: chalk.yellow(formatType(left)),
      });
    }
    if (right.node !== null) {
      out += '\n\n';
      out += formatExcerpt({
        text: right.node.getSourceText(),
        range: right.node.getRange(),
        message: chalk.yellow(formatType(right))
      });
    }
    return out;
  }

}

export class UnificationFailedDiagnostic implements Diagnostic {

  public constructor(
    public left: Type,
    public right: Type,
  ) {

  }

  public format(): string {
    const left = this.left.getSolved()
    const right = this.right.getSolved()
    let out = `${chalk.bold.red('error:')} The types ${chalk.yellow(formatType(left))} and ${chalk.yellow(formatType(right))} could not be unified.`;
    if (left.node !== null) {
      out += '\n\n';
      // out += `\n\n  Type ${chalk.yellow(formatType(left))} originated from the following expression.\n\n`;
      out += formatExcerpt({
        text: left.node.getSourceText(),
        range: left.node.getRange(),
        message: chalk.yellow(formatType(left)),
      });
    }
    if (right.node !== null) {
      out += '\n\n';
      out += formatExcerpt({
        text: right.node.getSourceText(),
        range: right.node.getRange(),
        message: chalk.yellow(formatType(right)),
      });
    }
    return out;
  }

}

export class BindingNotFoundDiagnostic implements Diagnostic {

  public constructor(
    public name: string,
  ) {

  }

  public getMessage(): string {
    return `A binding for '${this.name}' was not found.`;
  }

  public format(): string {
    return this.getMessage();
  }

}

export class ExpectedEndOfLineFoldDiagnostic implements Diagnostic {

  constructor(
    public file: TextFile,
    public actual: Token
  ) {

  }

  public getMessage(): string {
    return `expected end of line-fold but got ${describeSyntaxKind(this.actual.kind)}.`;
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
    return `Expected ${describeSyntaxKind(this.actual.kind)} to be placed on a new line.`;
  }

  public format() {
    return this.getMessage();
  }

}

function formatType(type: Type): string {
  return type.format();
}

function formatPosition(file: TextFile, position: TextPosition): string {
  return chalk.bold.yellow(`${file.origPath}:${position.line}:${position.column}`)
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

