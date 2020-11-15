
import * as fs from "fs"
import chalk from "chalk"
import { Syntax } from "./ast";
import {format, MapLike, FormatArg, countDigits, mapValues, prettyPrint, assert, isPlainObject, isPrimitive, JsonObject, Json} from "./util";
import { BOLT_DIAG_NUM_EXTRA_LINES } from "./constants";
import { TextPos, TextFile, TextSpan } from "./text";

export const E_ASSIGN_TUPLE_LENGTH_MISMATCH = "Trying to assign a tuple of length {rhsLength} to a tuple of length {lhsLength}"
export const E_TYPES_NOT_ASSIGNABLE = "Trying to assign to an expression of type {lhs}, which is incompatible with expression of type {rhs}"

export const E_MAY_NOT_RETURN_BECAUSE_TYPE_RESOLVES_TO_VOID = "May not return a value because the function's return type resolves to '()'"
export const E_MUST_RETURN_BECAUSE_TYPE_DOES_NOT_RESOLVE_TO_VOID = "Must return a value because the function's return type does not resolve to '()'"

export const E_INVALID_TEST_COMPARE = "The given test results cannot be compared because they use different specifications."
export const E_TESTS_DO_NOT_COMPARE = "This test does not compare with its expected output."
export const E_NO_BOLTFILE_FOUND_IN_PATH_OR_PARENT_DIRS = 'No Boltfile found in {path} or any of its parent directories.'
export const E_SCAN_ERROR = "Got an unexpected {char}"
export const E_STDLIB_NOT_FOUND = "Package 'stdlib' is required to build the current source set but it was not found. Use --no-std if you know what you are doing."
export const E_PARSE_ERROR = "Expected {expected:enum} but got {actual}"
export const E_MAY_NOT_RETURN_A_VALUE = "Returning a value inside a function that does not return values."
export const E_FILE_NOT_FOUND = "A file named {filename} was not found.";
export const E_FIELD_HAS_INVALID_VERSION_NUMBER = "Field '{name}' contains an invalid version nunmber."
export const E_FIELD_MUST_BE_STRING = "Field '{name}' must be a string."
export const E_FIELD_NOT_PRESENT = "Field '{name}' is not present."
export const E_FIELD_MUST_BE_BOOLEAN = "Field '{name}' must be a either 'true' or 'false'."
export const E_TYPE_DECLARATION_NOT_FOUND = "A type declaration named '{name}' was not found."
export const E_DECLARATION_NOT_FOUND = "Reference to an undefined declaration '{name}'.";
export const E_TYPE_MISMATCH = "Types '{left}' and '{right}' are not compatible with one another.";
export const E_THIS_NODE_CAUSED_INVALID_TYPE = "This expression resolved to the type {type}, which is incompatible with {origType}."
export const E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL = "Too few arguments for function call. Expected {expected} but got {actual}.";
export const E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL = "Too many arguments for function call. Expected {expected} but got {actual}.";
export const E_NOT_CALLABLE = "The result of this expression is not callable."
export const E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER = "Candidate function requires this parameter."
export const E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER = "This argument is missing a corresponding parameter."
export const E_INVALID_ARGUMENTS = "Invalid arguments passed to function '{name}'"
export const E_RECORD_MISSING_MEMBER = "Record {name} does not have a member declaration named {memberName}"
export const E_TYPE_NEVER_MATCHES = "Type '{type}' never matches anything."
export const E_TYPES_MISSING_MEMBER = "Not all types resolve to a record with the a member named '{name}'."
export const E_NODE_DOES_NOT_CONTAIN_MEMBER = "This node does not contain the the member '{name}'."
export const E_ARGUMENT_TYPE_NOT_ASSIGNABLE = "This argument's type '{argType}' is not assignable to the function's parameter type '{paramType}'."
export const E_PARAMETER_DECLARED_HERE = "The parameter was declared here with type {type}."
export const E_BUILTIN_TYPE_MISSING = "A built-in type named '{name}' was not found in the prelude."

export const TYPE_ERROR_MESSAGES = [
  E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
  E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
  E_ARGUMENT_TYPE_NOT_ASSIGNABLE,
  E_TYPE_MISMATCH,
]

const BOLT_HARD_ERRORS = process.env['BOLT_HARD_ERRORS']

export interface Diagnostic {
  message: string;
  severity: string;
  args?: MapLike<FormatArg>;
  node?: { span: TextSpan | null };
  nested?: Diagnostic[];
  position?: TextPos,
  file?: TextFile,
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

export interface Diagnostics {
  add(diagnostic: Diagnostic): void;
}

export class DiagnosticIndex {

  private diagnostics = new Array<Diagnostic>();

  public add(diagnostic: Diagnostic) {
    this.diagnostics.push(diagnostic);
  }

  public getAllDiagnostics(): IterableIterator<Diagnostic> {
    return this.diagnostics[Symbol.iterator]();
  }

}

export class DiagnosticPrinter {

  public hasErrors = false
  public hasFatal = false;

  private indent = 0;

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

    const indentation = ' '.repeat(this.indent);

    let out = indentation;

    switch (diagnostic.severity) {
      case 'error':
        this.hasErrors = true;
        out += chalk.bold.red('error: ');
        break;
      case 'fatal':
        this.hasFatal = true;
        out += chalk.bold.red('fatal: ') 
        break;
      case 'warning':
        out += chalk.bold.red('warning: ');
        break;
      case 'info':
        out += chalk.bold.yellow('info: ')
        break;
      default:
        throw new Error(`Unkown severity for diagnostic message.`);
    }

    let span = null;
    if (diagnostic.node !== undefined) {
      span = diagnostic.node.span!;
    } else if (diagnostic.position !== undefined) {
      assert(diagnostic.file !== undefined);
      span = new TextSpan(diagnostic.file!, diagnostic.position, diagnostic.position)
    }

    if (span !== null) {
      out += chalk.bold.yellow(`${span.file.origPath}:${span.start.line}:${span.start.column}: `);
    }
    if (diagnostic.args !== undefined) {
      out += format(diagnostic.message, diagnostic.args) + '\n';
    } else {
      out += diagnostic.message + '\n';
    }

    if (span !== null) {
      out += '\n'
      const content = span.file.getText();
      const startLine = Math.max(0, span.start.line-1-BOLT_DIAG_NUM_EXTRA_LINES)
      const lines = content.split('\n')
      const endLine = Math.min(lines.length, (span.end !== undefined ? span.end.line : startLine)+BOLT_DIAG_NUM_EXTRA_LINES)
      const gutterWidth = Math.max(2, countDigits(endLine+1))
      for (let i = startLine; i < endLine; i++) {
        const line = lines[i];
        let j = firstIndexOfNonEmpty(line);
        out +=  indentation + '  '+chalk.bgWhite.black(' '.repeat(gutterWidth-countDigits(i+1))+(i+1).toString())+' '+line+'\n'
        const gutter = indentation + '  '+chalk.bgWhite.black(' '.repeat(gutterWidth))+' '
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

    if (diagnostic.nested !== undefined) {
      this.indent += 2;
      for (const nested of diagnostic.nested) {
        this.add(nested);
      }
      this.indent -= 2;
    }

  }

}

