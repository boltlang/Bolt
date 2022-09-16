
import { TypeKind, type Type, type TArrow, TRecord, Kind, KindType } from "./checker";
import { Syntax, SyntaxKind, TextFile, TextPosition, TextRange, Token } from "./cst";
import { countDigits, IndentWriter } from "./util";

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

export class UnexpectedCharDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public file: TextFile,
    public position: TextPosition,
    public actual: string,
  ) {

  }

  public format(out: IndentWriter): void {
    const endPos = this.position.clone();
    endPos.advance(this.actual);
    out.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET);
    out.write(`unexpeced character sequence '${this.actual}'.\n\n`);
    out.write(printExcerpt(this.file, new TextRange(this.position, endPos)) + '\n');
  }

}

const DESCRIPTIONS: Partial<Record<SyntaxKind, string>> = {
  [SyntaxKind.StringLiteral]: 'a string literal',
  [SyntaxKind.Identifier]: "an identifier",
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
  [SyntaxKind.NamedTupleExpression]: 'a named tuple expression',
  [SyntaxKind.StructExpression]: 'a struct expression',
  [SyntaxKind.BlockStart]: 'the start of an indented block',
  [SyntaxKind.BlockEnd]: 'the end of an indented block',
  [SyntaxKind.LineFoldEnd]: 'the end of the current line-fold',
  [SyntaxKind.EndOfFile]: 'end-of-file',
  [SyntaxKind.RArrowAlt]: '"=>"',
  [SyntaxKind.VBar]: "'|'",
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

export class UnexpectedTokenDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public file: TextFile,
    public actual: Token,
    public expected: SyntaxKind[],
  ) {

  }

  public format(out: IndentWriter): void {
    out.write(ANSI_FG_RED + ANSI_BOLD + 'fatal: ' + ANSI_RESET);
    out.write(`expected ${describeExpected(this.expected)} but got ${describeActual(this.actual)}\n\n`);
    out.write(printExcerpt(this.file, this.actual.getRange()) + '\n');
  }

}

export class BindingNotFoudDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public name: string,
    public node: Syntax,
  ) {

  }

  public format(out: IndentWriter): void {
    out.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET); 
    out.write(`binding '${this.name}' was not found.\n\n`);
    out.write(printNode(this.node) + '\n');
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
      let out = '(';
      let first = true;
      for (const paramType of type.paramTypes) {
        if (first) first = false;
        else out += ', ';
        out += describeType(paramType);
      }
      out += ') -> ' + describeType(type.returnType);
      return out;
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
      return out;
    }
    case TypeKind.Nominal:
    {
      return type.decl.name.text;
    }
    case TypeKind.Record:
    {
      let out = '{ ';
      let first = true;
      for (const [fieldName, fieldType] of type.fields) {
        if (first) first = false;
        else out += ', ';
        out += fieldName + ': ' + describeType(fieldType);
      }
      return out + ' }';
    }
    case TypeKind.Labeled:
    {
      // FIXME may need to include fields that were added during unification
      return '{ ' + type.name + ': ' + describeType(type.type) + ' }';
    }
    case TypeKind.App:
    {
      return describeType(type.right) + ' ' + describeType(type.left);
    }
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

export class UnificationFailedDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public left: Type,
    public right: Type,
    public nodes: Syntax[],
  ) {

  }

  public format(out: IndentWriter): void {
    const leftNode = getFirstNodeInTypeChain(this.left);
    const rightNode = getFirstNodeInTypeChain(this.right);
    const node = this.nodes[0];
    out.write(ANSI_FG_RED + ANSI_BOLD + `error: ` + ANSI_RESET);
    out.write(`unification of ` + ANSI_FG_GREEN + describeType(this.left) + ANSI_RESET);
    out.write(' and ' + ANSI_FG_GREEN + describeType(this.right) + ANSI_RESET + ' failed.\n\n');
    out.write(printNode(node) + '\n');
    for (let i = 1; i < this.nodes.length; i++) {
      const node = this.nodes[i];
      out.write('  ... in an instantiation of the following expression\n\n');
      out.write(printNode(node, { indentation: i === 0 ? '  ' : '    ' }) + '\n');
    }
    if (leftNode !== null) {
      out.indent();
      out.write(ANSI_FG_YELLOW + ANSI_BOLD + `info: ` + ANSI_RESET);
      out.write(`type ` + ANSI_FG_GREEN + describeType(this.left) + ANSI_RESET + ` was inferred from this expression:\n\n`);
      out.write(printNode(leftNode) + '\n');
      out.dedent();
    }
    if (rightNode !== null) {
      out.indent();
      out.write(ANSI_FG_YELLOW + ANSI_BOLD + `info: ` + ANSI_RESET);
      out.write(`type ` + ANSI_FG_GREEN + describeType(this.right) + ANSI_RESET + ` was inferred from this expression:\n\n`);
      out.write(printNode(rightNode) + '\n');
      out.dedent();
    }
  }

}

export class ArityMismatchDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public left: TArrow,
    public right: TArrow,
  ) {

  }

  public format(out: IndentWriter): void {
    out.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET);
    out.write(ANSI_FG_GREEN + describeType(this.left) + ANSI_RESET);
    out.write(` has ${this.left.paramTypes.length} `);
    out.write(this.left.paramTypes.length === 1 ? 'parameter' : 'parameters');
    out.write(' while ' + ANSI_FG_GREEN + describeType(this.right) + ANSI_RESET);
    out.write(` has ${this.right.paramTypes.length}.\n\n`);
  }

}

export class FieldMissingDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public recordType: Type,
    public fieldName: string,
    public node: Syntax | null,
  ) {

  }

  public format(out: IndentWriter): void {
    out.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET);
    out.write(`field '${this.fieldName}' is missing from `);
    out.write(describeType(this.recordType) + '\n\n');
    if (this.node !== null) {
      out.write(printNode(this.node) + '\n');
    }
  }

}

export class FieldDoesNotExistDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public recordType: TRecord,
    public fieldName: string,
    public node: Syntax | null,
  ) {

  }

  public format(out: IndentWriter): void {
    out.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET);
    out.write(`field '${this.fieldName}' does not exist on type `);
    out.write(describeType(this.recordType) + '\n\n');
    if (this.node !== null) {
      out.write(printNode(this.node) + '\n');
    }
  }

}

export class KindMismatchDiagnostic {

  public readonly level = Level.Error;

  public constructor(
    public left: Kind,
    public right: Kind,
    public node: Syntax | null,
  ) {
  
  }

  public format(out: IndentWriter): void {
    out.write(ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET);
    out.write(`kind ${describeKind(this.left)} does not match with ${describeKind(this.right)}\n\n`);
    if (this.node !== null) {
      out.write(printNode(this.node) + '\n');
    }
  }

}

export type Diagnostic
  = UnexpectedCharDiagnostic
  | BindingNotFoudDiagnostic
  | UnificationFailedDiagnostic
  | UnexpectedTokenDiagnostic
  | ArityMismatchDiagnostic
  | FieldMissingDiagnostic
  | FieldDoesNotExistDiagnostic
  | KindMismatchDiagnostic

export interface Diagnostics {
  add(diagnostic: Diagnostic): void;
}

export class DiagnosticStore {

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

export class ConsoleDiagnostics {

  public add(diagnostic: Diagnostic): void {
    const writer = new IndentWriter(process.stderr);
    diagnostic.format(writer);
  }

}

interface PrintExcerptOptions {
  indentation?: string;
  extraLineCount?: number;
}

function printNode(node: Syntax, options?: PrintExcerptOptions): string {
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

