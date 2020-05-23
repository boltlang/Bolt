
import chalk from "chalk"
import {Syntax} from "./ast";
import {format, MapLike, FormatArg} from "./util";

export const E_TYPE_DECLARATION_NOT_FOUND = "A type declaration named '{name}' was not found."
export const E_DECLARATION_NOT_FOUND = "Reference to an undefined declaration '{name}'.";
export const E_TYPES_NOT_ASSIGNABLE = "Types {left} and {right} are not assignable.";
export const E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL = "Too few arguments for function call. Expected {expected} but got {actual}.";
export const E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL = "Too many arguments for function call. Expected {expected} but got {actual}.";
export const E_INVALID_ARGUMENTS = "Invalid arguments passed to function '{name}'."

const DIAG_NUM_EXTRA_LINES = 1;

export interface Diagnostic {
  message: string;
  severity: string;
  args?: MapLike<FormatArg>;
  node?: Syntax;
}

export function countDigits(num: number) {
  if (num === 0) {
    return 1
  }
  return Math.ceil(Math.log10(num+1))
}

export class DiagnosticPrinter {

  public hasErrors = false;

  public add(diagnostic: Diagnostic): void {
    let out = ''
    if (diagnostic.node !== undefined) {
      const span = diagnostic.node.span!;
      const content = span.file.getText();
      const startLine = Math.max(0, span.start.line-1-DIAG_NUM_EXTRA_LINES)
      const lines = content.split('\n')
      const endLine = Math.min(lines.length-1, (span.end !== undefined ? span.end.line : startLine)+DIAG_NUM_EXTRA_LINES)
      const gutterWidth = Math.max(2, countDigits(endLine+1))
      for (let i = startLine; i < endLine; i++) {
        out += '  '+chalk.bgWhite.black(' '.repeat(gutterWidth-countDigits(i+1))+(i+1).toString())+' '+lines[i]+'\n'
        const gutter = '  '+chalk.bgWhite.black(' '.repeat(gutterWidth))+' '
        if (span.end !== undefined) {
          if (i === span.start.line-1 && i === span.end.line-1) {
            out += gutter+' '.repeat(span.start.column-1)+chalk.red('~'.repeat(span.end.column-span.start.column)) + '\n'
          } else if (i === span.start.line-1) {
            out += gutter+' '.repeat(span.start.column-1)+chalk.red('~'.repeat(lines[i].length-span.start.column+1)) + '\n'
          } else if (i === span.end.line-1) {
            out += gutter+chalk.red('~'.repeat(span.end.column-1)) + '\n'
          } else if (i > span.start.line-1 && i < span.end.line-1) {
            out += gutter+chalk.red('~'.repeat(lines[i].length)) + '\n'
          }
        }
      }
      out += '\n'
      out += chalk.bold.yellow(`${span.file.origPath}:${span.start.line}:${span.start.column}: `);
    }
    switch (diagnostic.severity) {
      case 'error':
        this.hasErrors = true;
        out += chalk.bold.red('error: ');
    }
    if (diagnostic.args !== undefined) {
      out += format(diagnostic.message, diagnostic.args) + '\n';
    } else {
      out += diagnostic.message + '\n';
    }
    out += '\n'
    process.stderr.write(out);
  }

}

