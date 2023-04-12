
import { TypeKind, type Type, Kind, KindType } from "./checker";
import { ClassConstraint, ClassDeclaration, IdentifierAlt, InstanceDeclaration, Syntax, SyntaxKind, TextFile, TextPosition, TextRange, Token } from "./cst";
import { assertNever, countDigits, IndentWriter } from "./util";

const ANSI_RESET = "\u001b[0m"
const ANSI_BOLD = "\u001b[1m"
const ANSI_UNDERLINE = "\u001b[4m"
const ANSI_REVERSED = "\u001b[7m"

const ANSI_FG_BLACK = "\u001b[30m"
const ANSI_FG_RED = "\u001b[31m"
const ANSI_FG_GREEN = "\u001b[32m"
const ANSI_FG_YELLOW = "\u001b[33m"
const ANSI_FG_BLUE = "\u001b[34m"
const ANSI_FG_CYAN = "\u001b[35m"
const ANSI_FG_MAGENTA = "\u001b[36m"
const ANSI_FG_WHITE = "\u001b[37m"

const ANSI_BG_BLACK = "\u001b[40m"
const ANSI_BG_RED = "\u001b[41m"
const ANSI_BG_GREEN = "\u001b[42m"
const ANSI_BG_YELLOW = "\u001b[43m"
const ANSI_BG_BLUE = "\u001b[44m"
const ANSI_BG_CYAN = "\u001b[45m"
const ANSI_BG_MAGENTA = "\u001b[46m"
const ANSI_BG_WHITE = "\u001b[47m"

const enum Level {
  Debug,
  Verbose,
  Info,
  Warning,
  Error,
  Fatal,
}

const enum DiagnosticKind {
  UnexpectedChar,
  UnexpectedToken,
  KindMismatch,
  TypeMismatch,
  TypeclassNotFound,
  TypeclassDecaredTwice,
  BindingNotFound,
  ModuleNotFound,
  FieldNotFound,
}

interface DiagnosticBase {
  level: Level;
  readonly kind: DiagnosticKind;
}

export class UnexpectedCharDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.UnexpectedChar;

  public level = Level.Error;

  public constructor(
    public file: TextFile,
    public position: TextPosition,
    public actual: string,
  ) {

  }

}


export class UnexpectedTokenDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.UnexpectedToken;

  public level = Level.Error;

  public constructor(
    public file: TextFile,
    public actual: Token,
    public expected: SyntaxKind[],
  ) {

  }

}

export class TypeclassDeclaredTwiceDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.TypeclassDecaredTwice;
  
  public level = Level.Error;

  public constructor(
    public name: IdentifierAlt,
    public origDecl: ClassDeclaration,
  ) {

  } 

}

export class TypeclassNotFoundDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.TypeclassNotFound;

  public level = Level.Error;

  public constructor(
    public name: IdentifierAlt,
    public origin: InstanceDeclaration | ClassConstraint | null = null,
  ) {

  }

}

export class BindingNotFoundDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.BindingNotFound;

  public level = Level.Error;

  public constructor(
    public modulePath: string[],
    public name: string,
    public node: Syntax,
  ) {

  }

}

export class TypeMismatchDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.TypeMismatch;

  public level = Level.Error;

  public constructor(
    public left: Type,
    public right: Type,
    public trace: Syntax[],
    public fieldPath: string[],
  ) {

  }

}

export class FieldNotFoundDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.FieldNotFound;

  public level = Level.Error;

  public constructor(
    public fieldName: string,
    public missing: Syntax | null,
    public present: Syntax | null,
    public cause: Syntax | null = null,
  ) {

  }

}

export class KindMismatchDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.KindMismatch;

  public level = Level.Error;

  public constructor(
    public left: Kind,
    public right: Kind,
    public origin: Syntax | null,
  ) {
  
  }

}

export class ModuleNotFoundDiagnostic implements DiagnosticBase {

  public readonly kind = DiagnosticKind.ModuleNotFound;

  public level = Level.Error;

  public constructor(
    public modulePath: string[],
    public node: Syntax,
  ) {

  }

}

export type Diagnostic
  = UnexpectedCharDiagnostic
  | TypeclassNotFoundDiagnostic
  | TypeclassDeclaredTwiceDiagnostic
  | BindingNotFoundDiagnostic
  | TypeMismatchDiagnostic
  | UnexpectedTokenDiagnostic
  | FieldNotFoundDiagnostic
  | KindMismatchDiagnostic
  | ModuleNotFoundDiagnostic

export interface Diagnostics {
  readonly hasError: boolean;
  readonly hasFatal: boolean;
  add(diagnostic: Diagnostic): void;
}

export class DiagnosticStore implements Diagnostics {

  private storage: Diagnostic[] = [];
  
  public hasError = false;
  public hasFatal = false;

  public add(diagnostic: Diagnostic): void {
    this.storage.push(diagnostic);
    if (diagnostic.level >= Level.Error) {
      this.hasError = true;
    }
    if (diagnostic.level >= Level.Fatal) {
      this.hasFatal = true;
    }
  }

  public getDiagnostics(): Iterable<Diagnostic> {
    return this.storage;
  }

}

export class ConsoleDiagnostics implements Diagnostics {
  
  private writer = new IndentWriter(process.stderr);

  public hasError = false;
  public hasFatal = false;

  public add(diagnostic: Diagnostic): void {

    if (diagnostic.level >= Level.Error) {
      this.hasError = true;
    }
    if (diagnostic.level >= Level.Fatal) {
      this.hasFatal = true;
    }

    switch (diagnostic.level) {
      case Level.Fatal: 
        this.writer.write(ANSI_FG_RED + ANSI_BOLD + 'fatal: ' + ANSI_RESET);
        break;
      case Level.Error:
        this.writer.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET);
        break;
      case Level.Warning:
        this.writer.write(ANSI_FG_RED + ANSI_BOLD + 'warning: ' + ANSI_RESET);
        break;
      case Level.Info:
        this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + 'info: ' + ANSI_RESET);
        break;
      case Level.Verbose:
        this.writer.write(ANSI_FG_CYAN + ANSI_BOLD + 'verbose: ' + ANSI_RESET);
        break;
    }

    switch (diagnostic.kind) {

      case DiagnosticKind.UnexpectedChar:
        const endPos = diagnostic.position.clone();
        endPos.advance(diagnostic.actual);
        this.writer.write(`unexpected character sequence '${diagnostic.actual}'.\n\n`);
        this.writer.write(printExcerpt(diagnostic.file, new TextRange(diagnostic.position, endPos)) + '\n');
        break;

      case DiagnosticKind.UnexpectedToken:
        this.writer.write(`expected ${describeExpected(diagnostic.expected)} but got ${describeActual(diagnostic.actual)}\n\n`);
        this.writer.write(printExcerpt(diagnostic.file, diagnostic.actual.getRange()) + '\n');
        break;

      case DiagnosticKind.TypeclassDecaredTwice:
        this.writer.write(`type class '${diagnostic.name.text}' was already declared somewhere else.\n\n`);
        this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + 'info: ' + ANSI_RESET);
        this.writer.write(`type class '${diagnostic.name.text}' is already declared here\n\n`);
        this.writer.write(printNode(diagnostic.origDecl) + '\n');
        break;

      case DiagnosticKind.TypeclassNotFound:
        this.writer.write(`the type class ${ANSI_FG_MAGENTA + diagnostic.name.text + ANSI_RESET} was not found.\n\n`);
        this.writer.write(printNode(diagnostic.name) + '\n');
        if (diagnostic.origin !== null) {
          this.writer.indent();
          this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + 'info: ' + ANSI_RESET);
          this.writer.write(`${ANSI_FG_MAGENTA + diagnostic.name.text + ANSI_RESET} is required by ${ANSI_FG_MAGENTA + diagnostic.origin.name.text + ANSI_RESET}\n\n`);
          this.writer.write(printNode(diagnostic.origin.name) + '\n');
          this.writer.dedent();
        }
        break;

      case DiagnosticKind.BindingNotFound:
        this.writer.write(`binding '${diagnostic.name}' was not found`);
        if (diagnostic.modulePath.length > 0) {
          this.writer.write(` in module ${ANSI_FG_BLUE + diagnostic.modulePath.join('.') + ANSI_RESET}`);
        }
        this.writer.write(`.\n\n`);
        this.writer.write(printNode(diagnostic.node) + '\n');
        break;

      case DiagnosticKind.TypeMismatch:
        const leftNode = getFirstNodeInTypeChain(diagnostic.left);
        const rightNode = getFirstNodeInTypeChain(diagnostic.right);
        const node = diagnostic.trace[0];
        this.writer.write(`unification of ` + ANSI_FG_GREEN + describeType(diagnostic.left) + ANSI_RESET);
        this.writer.write(' and ' + ANSI_FG_GREEN + describeType(diagnostic.right) + ANSI_RESET + ' failed');
        if (diagnostic.fieldPath.length > 0) {
          this.writer.write(` in field '${diagnostic.fieldPath.join('.')}'`);
        }
        this.writer.write('.\n\n');
        this.writer.write(printNode(node) + '\n');
        for (let i = 1; i < diagnostic.trace.length; i++) {
          const node = diagnostic.trace[i];
          this.writer.write('  ... in an instantiation of the following expression\n\n');
          this.writer.write(printNode(node, { indentation: i === 0 ? '  ' : '    ' }) + '\n');
        }
        if (leftNode !== null) {
          this.writer.indent();
          this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + `info: ` + ANSI_RESET);
          this.writer.write(`type ` + ANSI_FG_GREEN + describeType(diagnostic.left) + ANSI_RESET + ` was inferred from this expression:\n\n`);
          this.writer.write(printNode(leftNode) + '\n');
          this.writer.dedent();
        }
        if (rightNode !== null) {
          this.writer.indent();
          this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + `info: ` + ANSI_RESET);
          this.writer.write(`type ` + ANSI_FG_GREEN + describeType(diagnostic.right) + ANSI_RESET + ` was inferred from this expression:\n\n`);
          this.writer.write(printNode(rightNode) + '\n');
          this.writer.dedent();
        }
        break;

      case DiagnosticKind.KindMismatch:
        this.writer.write(`kind ${describeKind(diagnostic.left)} does not match with ${describeKind(diagnostic.right)}\n\n`);
        if (diagnostic.origin !== null) {
          this.writer.write(printNode(diagnostic.origin) + '\n');
        }
        break;

      case DiagnosticKind.ModuleNotFound:
        this.writer.write(`a module named ${ANSI_FG_BLUE + diagnostic.modulePath.join('.') + ANSI_RESET} was not found.\n\n`);
        this.writer.write(printNode(diagnostic.node) + '\n');
        break;

      case DiagnosticKind.FieldNotFound:
        this.writer.write(`field '${diagnostic.fieldName}' is required in one type but missing in another\n\n`);
        this.writer.indent();
        if (diagnostic.missing !== null) {
          this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + 'info: ' + ANSI_RESET);
          this.writer.write(`field '${diagnostic.fieldName}' is missing in diagnostic construct\n\n`);
          this.writer.write(printNode(diagnostic.missing) + '\n');
        }
        if (diagnostic.present !== null) {
          this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + 'info: ' + ANSI_RESET);
          this.writer.write(`field '${diagnostic.fieldName}' is required in diagnostic construct\n\n`);
          this.writer.write(printNode(diagnostic.present) + '\n');
        }
        if (diagnostic.cause !== null) {
          this.writer.write(ANSI_FG_YELLOW + ANSI_BOLD + 'info: ' + ANSI_RESET);
          this.writer.write(`because of a constraint on diagnostic node:\n\n`);
          this.writer.write(printNode(diagnostic.cause) + '\n');
        }
        this.writer.dedent();
        break;

      default:
        assertNever(diagnostic);

    }

  }

}

const DESCRIPTIONS: Partial<Record<SyntaxKind, string>> = {
  [SyntaxKind.StringLiteral]: 'a string literal',
  [SyntaxKind.Identifier]: "an identifier",
  [SyntaxKind.RArrow]: "'->'",
  [SyntaxKind.RArrowAlt]: '"=>"',
  [SyntaxKind.VBar]: "'|'",
  [SyntaxKind.Comma]: "','",
  [SyntaxKind.Colon]: "':'",
  [SyntaxKind.Integer]: "an integer",
  [SyntaxKind.LParen]: "'('",
  [SyntaxKind.RParen]: "')'",
  [SyntaxKind.LBrace]: "'{'",
  [SyntaxKind.RBrace]: "'}'",
  [SyntaxKind.LBracket]: "'['",
  [SyntaxKind.RBracket]: "']'",
  [SyntaxKind.StructKeyword]: "'struct'",
  [SyntaxKind.EnumKeyword]: "'enum'",
  [SyntaxKind.MatchKeyword]: "'match'",
  [SyntaxKind.TypeKeyword]: "'type'",
  [SyntaxKind.IdentifierAlt]: 'an identifier starting with an uppercase letter',
  [SyntaxKind.ConstantExpression]: 'a constant expression',
  [SyntaxKind.ReferenceExpression]: 'a reference expression',
  [SyntaxKind.LineFoldEnd]: 'the end of the current line-fold',
  [SyntaxKind.TupleExpression]: 'a tuple expression such as (1, 2)',
  [SyntaxKind.ReferenceExpression]: 'a reference to some variable',
  [SyntaxKind.NestedExpression]: 'an expression nested with parentheses',
  [SyntaxKind.ConstantExpression]: 'a constant expression such as 1 or "foo"',
  [SyntaxKind.StructExpression]: 'a struct expression',
  [SyntaxKind.BlockStart]: 'the start of an indented block',
  [SyntaxKind.BlockEnd]: 'the end of an indented block',
  [SyntaxKind.LineFoldEnd]: 'the end of the current line-fold',
  [SyntaxKind.EndOfFile]: 'end-of-file',
}

function describeSyntaxKind(kind: SyntaxKind): string {
  const desc = DESCRIPTIONS[kind];
  if (desc === undefined) {
    throw new Error(`Could not describe SyntaxKind '${kind}'`);
  }
  return desc
}

function describeExpected(expected: SyntaxKind[]) {
  if (expected.length === 0) {
    return 'nothing';
  }
  let out = describeSyntaxKind(expected[0]);
  if (expected.length === 1) {
    return out;
  }
  for (let i = 1; i < expected.length-1; i++) {
    const kind = expected[i];
    out += ', ' + describeSyntaxKind(kind);
  }
  out += ' or ' + describeSyntaxKind(expected[expected.length-1])
  return out;
}

function describeActual(token: Token): string {
  switch (token.kind) {
    case SyntaxKind.BlockStart:
    case SyntaxKind.BlockEnd:
    case SyntaxKind.LineFoldEnd:
    case SyntaxKind.EndOfFile:
      return describeSyntaxKind(token.kind);
    default:
      return `'${token.text}'`;
  }
}

export function describeType(type: Type): string {
  switch (type.kind) {
    case TypeKind.Con:
    {
      let out = type.displayName;
      for (const argType of type.argTypes) {
        out += ' ' + describeType(argType);
      }
      return out;
    }
    case TypeKind.Var:
      return 'a' + type.id;
    case TypeKind.Arrow:
    {
      return describeType(type.paramType) + ' -> ' + describeType(type.returnType);
    }
    case TypeKind.Tuple:
    {
      let out = '(';
      let first = true;
      for (const elementType of type.elementTypes) {
        if (first) first = false;
        else out += ', ';
        out += describeType(elementType);
      }
      return out + ')';
    }
    case TypeKind.Nominal:
    {
      return type.decl.name.text;
    }
    case TypeKind.Field:
    {
      let out = '{ ' + type.name + ': ' + describeType(type.type);
      type = type.restType;
      while (type.kind === TypeKind.Field) {
        out += '; ' + type.name + ': ' + describeType(type.type);
        type = type.restType;
      }

      if (type.kind !== TypeKind.Nil) {
        out += '; ' + describeType(type);
      }
      return out + ' }'
    }
    case TypeKind.App:
    {
      return describeType(type.right) + ' ' + describeType(type.left);
    }
    case TypeKind.Nil:
      return '{}';
    case TypeKind.Absent:
      return 'Abs';
    case TypeKind.Present:
      return describeType(type.type);
  }
}

function describeKind(kind: Kind): string {
  switch (kind.type) {
    case KindType.Var:
      return `k${kind.id}`;
    case KindType.Arrow:
      return describeKind(kind.left) + ' -> ' + describeKind(kind.right);
    case KindType.Star:
      return '*';
  }
}

function getFirstNodeInTypeChain(type: Type): Syntax | null {
  let curr = type.next;
  while (curr !== type && (curr.kind === TypeKind.Var || curr.node === null)) {
    curr = curr.next;
  }
  return curr.node;
}

interface PrintExcerptOptions {
  indentation?: string;
  extraLineCount?: number;
}

interface PrintNodeOptions extends PrintExcerptOptions { }

function printNode(node: Syntax, options?: PrintNodeOptions): string {
  const file = node.getSourceFile().getFile();
  return printExcerpt(file, node.getRange(), options);
}

function printExcerpt(file: TextFile, span: TextRange, { indentation = '  ', extraLineCount = 2 } = {}): string {

  let out = '';

  const content = file.text;
  const startLine = Math.max(0, span.start.line-1-extraLineCount)
  const lines = content.split('\n')
  const endLine = Math.min(lines.length, (span.end !== undefined ? span.end.line : startLine) + extraLineCount)
  const gutterWidth = Math.max(2, countDigits(endLine+1))

  for (let i = startLine; i < endLine; i++) {

    const line = lines[i];

    let j = firstIndexOfNonEmpty(line);

    out +=  indentation + '  ' + ANSI_FG_BLACK + ANSI_BG_WHITE + ' '.repeat(gutterWidth-countDigits(i+1))+(i+1).toString() + ANSI_RESET + ' ' + line + '\n'

    const gutter = indentation + '  ' + ANSI_FG_BLACK + ANSI_BG_WHITE + ' '.repeat(gutterWidth) + ANSI_RESET + ' '

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

    out += gutter + ' '.repeat(j+skip) + ANSI_FG_RED + '~'.repeat(mark-j) + ANSI_RESET + '\n'

  }

  return out;
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

