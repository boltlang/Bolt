
import chalk from "chalk"
import {Syntax} from "./ast";
import {format, MapLike, FormatArg} from "./util";

export const E_TYPE_DECLARATION_NOT_FOUND = "A type declaration named '{name}' was not found."
export const E_DECLARATION_NOT_FOUND = "Reference to an undefined declaration '{name}'.";
export const E_TYPES_NOT_ASSIGNABLE = "Types {left} and {right} are not assignable.";

export interface Diagnostic {
  message: string;
  severity: string;
  args?: MapLike<FormatArg>;
  node?: Syntax;
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
    if (diagnostic.args !== undefined) {
      out += format(diagnostic.message, diagnostic.args);
    } else {
      out += diagnostic.message;
    }
    process.stderr.write(out + '\n');
  }

}

