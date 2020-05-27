
import chalk from "chalk"
import {Syntax} from "./ast";
import {format, MapLike, FormatArg, countDigits} from "./util";

export const E_MAY_NOT_RETURN_A_VALUE = "Returning a value inside a function that does not return values."
export const E_MUST_RETURN_A_VALUE = "The function must return a value on all control paths.";;;;
export const E_FILE_NOT_FOUND = "A file named {filename} was not found.";
export const E_FIELD_HAS_INVALID_VERSION_NUMBER = "Field '{name}' contains an invalid version nunmber."
export const E_FIELD_MUST_BE_STRING = "Field '{name}' must be a string."
export const E_FIELD_NOT_PRESENT = "Field '{name}' is not present."
export const E_TYPE_DECLARATION_NOT_FOUND = "A type declaration named '{name}' was not found."
export const E_DECLARATION_NOT_FOUND = "Reference to an undefined declaration '{name}'.";
export const E_TYPES_NOT_ASSIGNABLE = "Types {left} and {right} are not assignable.";
export const E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL = "Too few arguments for function call. Expected {expected} but got {actual}.";
export const E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL = "Too many arguments for function call. Expected {expected} but got {actual}.";
export const E_INVALID_ARGUMENTS = "Invalid arguments passed to function '{name}'."

const BOLT_HARD_ERRORS = process.env['BOLT_HARD_ERRORS']

const DIAG_NUM_EXTRA_LINES = 1;

export interface Diagnostic {
  message: string;
  severity: string;
  args?: MapLike<FormatArg>;
  node?: Syntax;
}

function firstIndexOfNonEmpty(str: string) {
  let j = 0;
  for (; j < str.length; j++) {
    const ch = str[j];
    if (ch !== ' ' && ch !== '\t') {
      break;
    }
  }
  return j
}

export class DiagnosticPrinter {

  public hasErrors = false;

  public add(diagnostic: Diagnostic): void {

    if (BOLT_HARD_ERRORS && (diagnostic.severity === 'error' || diagnostic.severity === 'fatal')) {
      let out = ''
      if (diagnostic.args !== undefined) {
        out += format(diagnostic.message, diagnostic.args);
      } else {
        out += diagnostic.message;
      }
      throw new Error(out);
    }

    let out = ''

    switch (diagnostic.severity) {
      case 'error':
        this.hasErrors = true;
        out += chalk.bold.red('error: ');
        break;
      case 'warning':
        this.hasErrors = true;
        out += chalk.bold.red('warning: ');
        break;
      default:
        throw new Error(`Unkown severity for diagnostic message.`);
    }

    if (diagnostic.node !== undefined) {
      const span = diagnostic.node.span!;
      out += chalk.bold.yellow(`${span.file.origPath}:${span.start.line}:${span.start.column}: `);
    }
    if (diagnostic.args !== undefined) {
      out += format(diagnostic.message, diagnostic.args) + '\n';
    } else {
      out += diagnostic.message + '\n';
    }

    if (diagnostic.node !== undefined) {
      out += '\n'
      const span = diagnostic.node.span!;
      const content = span.file.getText();
      const startLine = Math.max(0, span.start.line-1-DIAG_NUM_EXTRA_LINES)
      const lines = content.split('\n')
      const endLine = Math.min(lines.length-1, (span.end !== undefined ? span.end.line : startLine)+DIAG_NUM_EXTRA_LINES)
      const gutterWidth = Math.max(2, countDigits(endLine+1))
      for (let i = startLine; i < endLine; i++) {
        const line = lines[i];
        let j = firstIndexOfNonEmpty(line);
        out += '  '+chalk.bgWhite.black(' '.repeat(gutterWidth-countDigits(i+1))+(i+1).toString())+' '+line+'\n'
        const gutter = '  '+chalk.bgWhite.black(' '.repeat(gutterWidth))+' '
        let mark: number;
        let skip: number;
        if (i === span.start.line-1 && i === span.end.line-1) {
          skip = span.start.column-1;
          mark = span.end.column-span.start.column;
        } else if (i === span.start.line-1) {
          skip = span.start.column-1;
          mark = line.length-span.start.column+1;
        } else if (i === span.end.line-1) {
          skip = 0;
          mark = span.end.column-1;
        } else if (i > span.start.line-1 && i < span.end.line-1) {
          skip = 0;
          mark = line.length;
        } else {
          continue;
        }
        if (j <= skip) {
          j = 0;
        }
        out += gutter+' '.repeat(j+skip)+chalk.red('~'.repeat(mark-j)) + '\n'
      }
      out += '\n'
    }

    process.stderr.write(out);
  }

}

