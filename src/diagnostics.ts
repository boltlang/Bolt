
import { TypeKind, Type } from "./checker";
import { Syntax, SyntaxKind, TextFile, TextPosition, TextRange, Token } from "./cst";
import { countDigits } from "./util";

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

export class UnexpectedCharDiagnostic {

  public constructor(
    public file: TextFile,
    public position: TextPosition,
    public actual: string,
  ) {

  }

  public format(): string {
    const endPos = this.position.clone();
    endPos.advance(this.actual);
    return ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET 
         + `unexpeced character '${this.actual}'.\n\n`
         + printExcerpt(this.file, new TextRange(this.position, endPos)) + '\n';
  }

}

const DESCRIPTIONS: Record<SyntaxKind, string> = {
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
  [SyntaxKind.ConstantExpression]: 'a constant expression',
  [SyntaxKind.ReferenceExpression]: 'a reference expression',
  [SyntaxKind.LineFoldEnd]: 'the end of the current line-fold',
  [SyntaxKind.TupleExpression]: 'a tuple expression such as (1, 2)',
  [SyntaxKind.ReferenceExpression]: 'a reference to some variable',
  [SyntaxKind.NestedExpression]: 'an expression nested with parentheses',
  [SyntaxKind.ConstantExpression]: 'a constant expression such as 1 or "foo"',
  [SyntaxKind.NamedTupleExpression]: 'a named tuple expression',
  [SyntaxKind.StructExpression]: 'a struct expression',
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

export class UnexpectedTokenDiagnostic {

  public constructor(
    public file: TextFile,
    public actual: Token,
    public expected: SyntaxKind[],
  ) {

  }

  public format(): string {
    return ANSI_FG_RED + ANSI_BOLD + 'fatal: ' + ANSI_RESET
         + `expected ${describeExpected(this.expected)} but got '${this.actual.text}'\n\n`
         + printExcerpt(this.file, this.actual.getRange()) + '\n';
  }

}

export class BindingNotFoudDiagnostic {

  public constructor(
    public name: string,
    public node: Syntax,
  ) {

  }

  public format(): string {
    const file = this.node.getSourceFile().getFile();
    return ANSI_FG_RED + ANSI_BOLD + 'error: ' + ANSI_RESET 
         + `binding '${this.name}' was not found.\n\n`
         + printExcerpt(file, this.node.getRange()) + '\n';
  }

}

function describeType(type: Type): string {
  switch (type.kind) {
    case TypeKind.Any:
      return 'Any';
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
  }
}

export class UnificationFailedDiagnostic {

  public constructor(
    public left: Type,
    public right: Type,
    public node: Syntax,
  ) {

  }

  public format(): string {
    const file = this.node.getSourceFile().getFile();
    return ANSI_FG_RED + ANSI_BOLD + `error: ` + ANSI_RESET
        + `unification of ` + ANSI_FG_GREEN + describeType(this.left) + ANSI_RESET
        + ' and ' + ANSI_FG_GREEN + describeType(this.right) + ANSI_RESET + ' failed.\n\n'
        + printExcerpt(file, this.node.getRange()) + '\n';
  }

}

export type Diagnostic
  = UnexpectedCharDiagnostic
  | BindingNotFoudDiagnostic
  | UnificationFailedDiagnostic
  | UnexpectedTokenDiagnostic

export class Diagnostics {

  private savedDiagnostics: Diagnostic[] = [];

  public add(diagnostic: Diagnostic): void {
    this.savedDiagnostics.push(diagnostic);
    process.stderr.write(diagnostic.format());
  }

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

