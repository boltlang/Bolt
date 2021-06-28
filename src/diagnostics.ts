
import chalk from "chalk"
import {EOF} from "dns";
import {ArrowType, Type} from "./checker";

import { describeSyntaxKind, Token, Syntax, SyntaxKind, Identifier } from "./cst";
import { formatExcerpt, TextFile, TextPosition } from "./text";
import { CompareMode } from "./util";

export abstract class Diagnostic extends Error {

  public abstract format(): string;

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

export class UnexpectedTokenDiagnostic extends Diagnostic {

  public constructor(
    public file: TextFile,
    public actual: Token,
    public expected: ExpectedParse[],
  ) {
    super();
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

  public format({
    addFilePositions = true,
  } = {}): string {
    let out = '';
    if (addFilePositions) {
      out += formatPosition(this.file, this.actual.getStartPos()) + ' '
    }
    out += chalk.bold.red('error:') + ' ' + this.getMessage() + '\n\n';
    out += formatExcerpt({
      text: this.file.getText(), 
      range: this.actual.range!,
    })
    return out;
  }

}

export class UnexpectedCharacterDiagnostic extends Diagnostic {

  public constructor(
    public file: TextFile,
    public actual: string,
    public position: TextPosition,
  ) {
    super();
  }

  public format({
    addFilePositions = true
  } = {}): string {
    let out = '';
    if (addFilePositions) {
      out += formatPosition(this.file, this.position) + ' '
    }
    out += `${chalk.bold.red('error:')} Unexpected character ${formatChar(this.actual)}.\n\n`;
    const nextPos = {
      line: this.position.line,
      column: this.position.column+1,
      offset: this.position.offset+1,
    }
    out += formatExcerpt({
      text: this.file.getText(),
      range: [ this.position, nextPos ],
    });
    return out;
  }

}

// export class UnexpectedIndentationDiagnostic extends Diagnostic {
// 
//   constructor(
//     public file: TextFile,
//     public compareMode: CompareMode,
//     public actualIndentLevel: number,
//     public expectedIndentLevel: number,
//   ) {
//     super();
//   }
// 
//   public getMessage(): string {
//     let out = '';
//     out += `Expected an indentation `
//     switch (this.compareMode) {
//       case CompareMode.Equal:
//         out += 'of exactly';
//         break;
//       case CompareMode.Lesser:
//         out += 'strictly less than'
//         break;
//       case CompareMode.LesserOrEqual:
//         out += 'less than or equal to';
//         break;
//       case CompareMode.Greater:
//         out += 'strictly greater than'
//         break;
//       case CompareMode.GreaterOrEqual:
//         out += 'greater than or equal to';
//         break;
//     }
//     out += ` ${this.expectedIndentLevel} `
//     out += this.expectedIndentLevel === 1 ? 'space' : 'spaces'
//     out += ` but found ${this.actualIndentLevel} `
//     out += this.actualIndentLevel === 1 ? 'space' : 'spaces'
//     out += `.`;
//     return out;
//   }
// 
//   public format(): string {
//     return this.getMessage()
//   }
// 
// }

export class ParamCountMismatchDiagnostic extends Diagnostic {

  public constructor(
    public left: ArrowType,
    public right: ArrowType,
  ) {
    super();
  }

  public format(): string {
    const left = this.left.getSolved();
    const right = this.right.getSolved();
    let out = `${chalk.bold.red('error:')} Type ${chalk.yellow(formatType(left))} requires ${left.paramTypes.length} `;
    out += left.paramTypes.length === 1 ? 'parameter' : 'parameters';
    out += ` while ${chalk.yellow(formatType(right))} requires ${right.paramTypes.length}.`;
    if (left.node !== null) {
      out += '\n\n' + formatExcerpt({
        text: left.node.getSourceText(),
        range: left.node.getRange(),
        message: chalk.yellow(formatType(left)),
      });
    }
    if (right.node !== null) {
      out += '\n\n' + formatExcerpt({
        text: right.node.getSourceText(),
        range: right.node.getRange(),
        message: chalk.yellow(formatType(right))
      });
    }
    return out;
  }

}

export class UnificationFailedDiagnostic extends Diagnostic {

  public constructor(
    public left: Type,
    public right: Type,
  ) {
    super();
  }

  public format(): string {
    const left = this.left.getSolved()
    const right = this.right.getSolved()
    let out = `${chalk.bold.red('error:')} The types ${chalk.yellow(formatType(left))} and ${chalk.yellow(formatType(right))} could not be unified.`;
    if (left.node !== null) {
      out += '\n\n' + formatExcerpt({
        text: left.node.getSourceText(),
        range: left.node.getRange(),
        message: chalk.yellow(formatType(left)),
      });
    }
    if (right.node !== null) {
      out += '\n\n' + formatExcerpt({
        text: right.node.getSourceText(),
        range: right.node.getRange(),
        message: chalk.yellow(formatType(right)),
      });
    }
    return out;
  }

}

export class BindingNotFoundDiagnostic extends Diagnostic {

  public constructor(
    public token: Token,
  ) {
    super();
  }

  public format({
    addFilePositions = true,
  } = {}): string {
    let out = '';
    if (addFilePositions) {
      out += formatPosition(this.token.getSourceFile().getFile(), this.token.getStartPos()) + ' '
    }
    out += `A binding for '${this.token.getText()}' was not found.\n\n`;
    out += formatExcerpt({
      text: this.token.getSourceText(),
      range: this.token.getRange(),
    });
    return out;
  }

}

function formatType(type: Type): string {
  return type.format();
}

function formatChar(ch: string): string {
  if (ch === EOF) {
    return 'end-of-file'
  }
  let escaped;
  switch (ch) {
    case '\a': escaped = '\\n'; break;
    case '\b': escaped = '\\b'; break;
    case '\f': escaped = '\\f'; break;
    case '\n': escaped = '\\n'; break;
    case '\r': escaped = '\\r'; break;
    case '\t': escaped = '\\t'; break;
    case '\v': escaped = '\\v'; break;
    case '\0': escaped = '\\0'; break;
    case '\'': escaped = '\\\''; break;
    case '\\': escaped = '\\\\'; break;
    default:   escaped = ch; break;
  }
  return `'${escaped}'`;
}

function formatPosition(file: TextFile, position: TextPosition): string {
  return chalk.bold.blueBright(`${file.origPath}:${position.line}:${position.column}`)
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

