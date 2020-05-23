
import chalk from "chalk"
import {Syntax} from "./ast";
import {format, MapLike, FormatArg} from "./util";

export const E_TYPE_DECLARATION_NOT_FOUND = "A type declaration named '{name}' was not found."
export const E_DECLARATION_NOT_FOUND = "Reference to an undefined declaration '{name}'.";
export const E_TYPES_NOT_ASSIGNABLE = "Types {left} and {right} are not assignable.";
export const E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL = "Too few arguments for function call. Expected {expected} but got {actual}.";
export const E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL = "Too many arguments for function call. Expected {expected} but got {actual}.";
export const E_INVALID_ARGUMENTS = "Invalid arguments passed to function '{name}'."

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
    switch (diagnostic.severity) {
      case 'error':
        this.hasErrors = true;
        out += chalk.bold.red('error: ');
    }
    if (diagnostic.node !== undefined) {
      // message = this.fileManager.getFileName(diag.location.fileId)+': '+message
      const content = this.fileManager.getContent(diag.location.fileId)
      const startLine = Math.max(0, diag.location.start.line-1-DIAG_NUM_EXTRA_LINES)
      const lines = content.split('\n')
      const endLine = Math.min(lines.length-1, (diag.location.end !== undefined ? diag.location.end.line : startLine)+DIAG_NUM_EXTRA_LINES)
      const gutterWidth = Math.max(2, countDigits(endLine+1))
      for (let i = startLine; i < endLine; i++) {
        console.error('  '+chalk.bgWhite.black(' '.repeat(gutterWidth-countDigits(i+1))+(i+1).toString())+' '+lines[i])
        const gutter = '  '+chalk.bgWhite.black(' '.repeat(gutterWidth))+' '
          if (diag.location.end !== undefined) {
            if (i === diag.location.start.line-1 && i === diag.location.end.line-1) {
              console.error(gutter+' '.repeat(diag.location.start.column-1)+chalk.red('~'.repeat(diag.location.end.column-diag.location.start.column)))
            } else if (i === diag.location.start.line-1) {
              console.error(gutter+' '.repeat(diag.location.start.column-1)+chalk.red('~'.repeat(lines[i].length-diag.location.start.column+1)))
            } else if (i === diag.location.end.line-1) {
              console.error(gutter+chalk.red('~'.repeat(diag.location.end.column-1)))
            } else if (i > diag.location.start.line-1 && i < diag.location.end.line-1) {
              console.error(gutter+chalk.red('~'.repeat(lines[i].length)))
            }
          }
        }
    }
    if (diagnostic.args !== undefined) {
      out += format(diagnostic.message, diagnostic.args);
    } else {
      out += diagnostic.message;
    }
    process.stderr.write(out + '\n');
  }

}

