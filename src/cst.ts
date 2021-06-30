import { Constraint, Scheme, Type, TypeEnv } from "./checker";
import { TextFile, TextPosition, TextRange } from "./text";

export enum SyntaxKind {

  // Tokens
  EndOfFile,
  BlockStart,
  BlockEnd,
  LineFoldStart,
  LineFoldEnd,
  Identifier,
  CustomOperator,
  Assignment,
  DecimalInteger,
  ClassKeyword,
  InstanceKeyword,
  StructKeyword,
  ReturnKeyword,
  ImportKeyword,
  PubKeyword,
  LetKeyword,
  MutKeyword,
  TypeKeyword,
  PerformKeyword,
  ResumeKeyword,
  MatchKeyword,
  YieldKeyword,
  DotSign,
  DotDotSign,
  ColonSign,
  EqualSign,
  RArrowSign,
  RArrowAltSign,
  TildeSign,
  CommaSign,
  BSlashSign,
  LBracket,
  RBracket,
  LBrace,
  RBrace,
  LParen,
  RParen,

  // Other nodes
  Module,
  SourceFile,
  MacroInvocation,

  // Purely syntactic nodes
  QualName,
  Parameter,
  TypeParameter,
  RecordDeclarationField,
  BlockDefinitionBody,
  InlineDefinitionBody,
  MatchArm,
  ConstraintSignature,

  // Patterns
  BindPattern,
  RecordPattern,
  TuplePattern,

  // Expressions
  ReferenceExpression,
  ConstantExpression,
  MatchExpression,
  CallExpression,
  BinaryExpression,
  NestedExpression,

  // Statements
  ReturnStatement,
  ExpressionStatement,

  // Type expressions
  TypeReferenceExpression,
  NestedTypeExpression,
  ArrowTypeExpression,

  // Constraint expressions
  IsInstanceConstraintExpression,

  // Declarations
  RecordDeclaration,
  ClassDeclaration,
  InstanceDeclaration,
  VariableDefinition,
  FunctionDefinition,
}

export type Syntax
  = SourceFile
  | TypeParameter
  | DefinitionBody
  | Parameter
  | QualName
  | Statement
  | Pattern
  | Expression
  | TypeExpression
  | Definition
  | RecordDeclaration
  | Token
  | MatchArm
  | RecordDeclarationElement
  | ConstraintSignature
  | ConstraintExpression
  | ClassDeclaration
  | InstanceDeclaration

let nextNodeId = 0;

abstract class SyntaxBase {

  public readonly kind!: SyntaxKind;

  public parentNode!: Syntax | null;

  public associatedType?: Type;
  public associatedConstraints?: Constraint[];

  constructor(kind: SyntaxKind) {
    Object.defineProperties(this, {
      id: { value: nextNodeId++ },
      kind: { value: kind },
      parentNode: { value: null, writable: true },
    })
  }

  public abstract getChildren(): Iterable<Syntax>;

  public *getTokens(): Iterable<Token> {
    for (const child of this.getChildren()) {
      if (isToken(child)) {
        yield child;
      } else {
        yield* child.getTokens();
      }
    }
  }

  public setParents(parentNode: Syntax | null = null): void {
    this.parentNode = parentNode;
    for (const child of this.getChildren()) {
      child.setParents(this as unknown as Syntax);
    }
  }

  public abstract getFirstToken(): Token;

  public abstract getLastToken(): Token;

  public getParentOfKind<K extends SyntaxKind>(kind: K): Syntax & { kind: K } | null {
    let currNode: Syntax | null = this.parentNode;
    while (currNode !== null) {
      if (currNode.kind === kind) {
        return currNode as Syntax & { kind: K };
      }
      currNode = currNode.parentNode;
    }
    return null;
  }

  public getSourceFile(): SourceFile {
    const sourceFile = this.getParentOfKind(SyntaxKind.SourceFile);
    if (sourceFile === null) {
      throw new Error(`Could not extract the SourceFile out of the given node. This is most likely due to 'parentNode' not being correctly set.`);
    }
    return sourceFile;
  }

  public getSourceText(): string {
    return this.getSourceFile().getFile().getText();
  }

  public getRange(): TextRange {
    return [
      this.getFirstToken().getStartPos(),
      this.getLastToken().getEndPos(),
    ];
  }

}

export type Token
  = EndOfFile
  | BlockStart
  | BlockEnd
  | LineFoldStart
  | LineFoldEnd
  | Identifier
  | RArrowSign
  | RArrowAltSign
  | DecimalInteger
  | CustomOperator
  | Assignment
  | StructKeyword
  | ReturnKeyword
  | ImportKeyword
  | PubKeyword
  | LetKeyword
  | MutKeyword
  | TypeKeyword
  | PerformKeyword
  | ResumeKeyword
  | MatchKeyword
  | YieldKeyword
  | ClassKeyword
  | InstanceKeyword
  | DotSign
  | DotDotSign
  | ColonSign
  | EqualSign
  | TildeSign
  | CommaSign
  | BSlashSign
  | LBracket
  | RBracket
  | LBrace
  | RBrace
  | LParen
  | RParen


export function isToken(node: Syntax): node is Token {
  return node.kind === SyntaxKind.EndOfFile
      || node.kind === SyntaxKind.BlockStart
      || node.kind === SyntaxKind.BlockEnd
      || node.kind === SyntaxKind.LineFoldStart
      || node.kind === SyntaxKind.LineFoldEnd
      || node.kind === SyntaxKind.Identifier
      || node.kind === SyntaxKind.RArrowSign
      || node.kind === SyntaxKind.RArrowAltSign
      || node.kind === SyntaxKind.DecimalInteger
      || node.kind === SyntaxKind.CustomOperator
      || node.kind === SyntaxKind.Assignment
      || node.kind === SyntaxKind.StructKeyword
      || node.kind === SyntaxKind.ReturnKeyword
      || node.kind === SyntaxKind.ImportKeyword
      || node.kind === SyntaxKind.PubKeyword
      || node.kind === SyntaxKind.LetKeyword
      || node.kind === SyntaxKind.MutKeyword
      || node.kind === SyntaxKind.TypeKeyword
      || node.kind === SyntaxKind.PerformKeyword
      || node.kind === SyntaxKind.ResumeKeyword
      || node.kind === SyntaxKind.MatchKeyword
      || node.kind === SyntaxKind.YieldKeyword
      || node.kind === SyntaxKind.ClassKeyword
      || node.kind === SyntaxKind.InstanceKeyword
      || node.kind === SyntaxKind.DotSign
      || node.kind === SyntaxKind.DotDotSign
      || node.kind === SyntaxKind.ColonSign
      || node.kind === SyntaxKind.EqualSign
      || node.kind === SyntaxKind.TildeSign
      || node.kind === SyntaxKind.CommaSign
      || node.kind === SyntaxKind.BSlashSign
      || node.kind === SyntaxKind.LBracket
      || node.kind === SyntaxKind.RBracket
      || node.kind === SyntaxKind.LBrace
      || node.kind === SyntaxKind.RBrace
      || node.kind === SyntaxKind.LParen
      || node.kind === SyntaxKind.RParen
}

export type TokenSyntaxKind = Token['kind'];

export abstract class TokenBase extends SyntaxBase {

  public range!: TextRange | null;

  public constructor(
    kind: TokenSyntaxKind,
    range: TextRange | null,
  ) {
    super(kind);
    Object.defineProperties(this, {
      range: {
        writable: true,
        value: range,
      },
    })
  }

  public abstract getText(): string;

  public *getChildren(): Iterable<Syntax> {

  }

  public getRange(): TextRange {
    if (this.range === null) {
      throw new Error(`The 'range'-property was not set on a Token object.`)
    }
    return this.range;
  }

  public getFirstToken(): Token {
    throw new Error(`A token object does not have children.`);
  }

  public getLastToken(): Token {
    throw new Error(`A token object does not have children.`);
  }

  public getStartPos(): TextPosition {
    return this.getRange()[0];
  }

  public getStartLine(): number {
    return this.getRange()[0].line;
  }

  public getStartColumn(): number {
    return this.getRange()[0].column;
  }

  public getEndPos(): TextPosition {
    return this.getRange()[1];
  }

  public getEndLine(): number {
    return this.getRange()[1].line;
  }

  public getEndColumn(): number {
    return this.getRange()[1].column;
  }

}

export class EndOfFile extends TokenBase {

  public readonly kind!: SyntaxKind.EndOfFile;

  public constructor(
    range: TextRange | null = null,
  ) {
    super(SyntaxKind.EndOfFile, range);
  }

  public getText(): string {
    return '';
  }

}

export class LineFoldStart extends TokenBase {

  public readonly kind!: SyntaxKind.LineFoldStart;

  public constructor(range: TextRange | null) {
    super(SyntaxKind.LineFoldStart, range);
  }

  public getText(): string {
    return '';
  }

}

export class LineFoldEnd extends TokenBase {

  public readonly kind!: SyntaxKind.LineFoldEnd;

  public constructor(range: TextRange | null) {
    super(SyntaxKind.LineFoldEnd, range);
  }

  public getText(): string {
    return '';
  }

}

export class BlockStart extends TokenBase {

  public readonly kind!: SyntaxKind.BlockStart;

  public constructor(
    public dotSign: DotSign,
  ) {
    super(SyntaxKind.BlockStart, null);
  }

  public getRange(): TextRange {
    return this.dotSign.getRange();
  }

  public getText(): string {
    return this.dotSign.getText();
  }

}

export class BlockEnd extends TokenBase {

  public readonly kind!: SyntaxKind.BlockEnd;

  public constructor(range: TextRange | null) {
    super(SyntaxKind.BlockEnd, range);
  }

  public getText(): string {
    return '';
  }

}

export class Identifier extends TokenBase {

  public readonly kind!: SyntaxKind.Identifier;

  public constructor(
    public text: string,
    range: TextRange | null = null,
  ) {
    super(SyntaxKind.Identifier, range);
  }

  public getText(): string {
    return this.text;
  }

}

export type Operator
  = CustomOperator

export function isOperator(node: Syntax): node is Operator {
  return node.kind === SyntaxKind.CustomOperator;
}

export class CustomOperator extends TokenBase {

  public readonly kind!: SyntaxKind.CustomOperator;

  public constructor(
    public text: string,
    range: TextRange | null = null,
  ) {
    super(SyntaxKind.CustomOperator, range);
  }

  public getText(): string {
    return this.text;
  }

}

export class Assignment extends TokenBase {

  public readonly kind!: SyntaxKind.Assignment;

  public constructor(
    public text: string,
    range: TextRange | null = null,
  ) {
    super(SyntaxKind.Assignment, range);
  }

  public getText(): string {
    return this.text + '=';
  }

}

export class DecimalInteger extends TokenBase {

  public readonly kind!: SyntaxKind.DecimalInteger;

  constructor(
    public numLeadingZeroes: number,
    public value: bigint,
    range: TextRange | null = null,
  ) {
    super(SyntaxKind.DecimalInteger, range);
  }

  public getText(): string {
    return '0'.repeat(this.numLeadingZeroes) + this.value.toString();
  }

}

const TOKEN_TEXT: Partial<Record<TokenSyntaxKind, string>> = {
  [SyntaxKind.LetKeyword]: 'let',
  [SyntaxKind.PubKeyword]: 'pub',
  [SyntaxKind.MutKeyword]: 'mut',
  [SyntaxKind.PerformKeyword]: 'perform',
  [SyntaxKind.YieldKeyword]: 'yield',
  [SyntaxKind.ResumeKeyword]: 'resume',
  [SyntaxKind.StructKeyword]: 'struct',
  [SyntaxKind.MatchKeyword]: 'match',
  [SyntaxKind.ReturnKeyword]: 'return',
  [SyntaxKind.ImportKeyword]: 'import',
  [SyntaxKind.ClassKeyword]: 'class',
  [SyntaxKind.InstanceKeyword]: 'instance',
  [SyntaxKind.TypeKeyword]: 'type',
  [SyntaxKind.DotSign]: '.',
  [SyntaxKind.DotDotSign]: '..',
  [SyntaxKind.ColonSign]: ':',
  [SyntaxKind.TildeSign]: '~',
  [SyntaxKind.RArrowSign]: '->',
  [SyntaxKind.RArrowAltSign]: '=>',
  [SyntaxKind.CommaSign]: ',',
  [SyntaxKind.BSlashSign]: '\\',
  [SyntaxKind.EqualSign]: '=',
  [SyntaxKind.LBracket]: '{',
  [SyntaxKind.RBracket]: '}',
  [SyntaxKind.LBrace]: '{',
  [SyntaxKind.RBrace]: '}',
  [SyntaxKind.LParen]: '(',
  [SyntaxKind.RParen]: ')',
}

export class SimpleToken<K extends TokenSyntaxKind> extends TokenBase {

  public readonly kind!: K;

  constructor(
    kind: K,
    range: TextRange | null = null,
  ) {
    super(kind, range);
  }

  public getText(): string {
    return TOKEN_TEXT[this.kind]!;
  }

}

export type DotSign = SimpleToken<SyntaxKind.DotSign>;
export type DotDotSign = SimpleToken<SyntaxKind.DotDotSign>;
export type ColonSign = SimpleToken<SyntaxKind.ColonSign>;
export type EqualSign = SimpleToken<SyntaxKind.EqualSign>;
export type TildeSign = SimpleToken<SyntaxKind.TildeSign>;
export type RArrowSign = SimpleToken<SyntaxKind.RArrowSign>;
export type RArrowAltSign = SimpleToken<SyntaxKind.RArrowAltSign>;
export type CommaSign = SimpleToken<SyntaxKind.CommaSign>;
export type BSlashSign = SimpleToken<SyntaxKind.BSlashSign>;
export type LBracket = SimpleToken<SyntaxKind.LBracket>;
export type RBracket = SimpleToken<SyntaxKind.RBracket>;
export type LParen = SimpleToken<SyntaxKind.LParen>;
export type RParen = SimpleToken<SyntaxKind.RParen>;
export type LBrace = SimpleToken<SyntaxKind.LBrace>;
export type RBrace = SimpleToken<SyntaxKind.RBrace>;

export type ReturnKeyword = SimpleToken<SyntaxKind.ReturnKeyword>;
export type MatchKeyword = SimpleToken<SyntaxKind.MatchKeyword>;
export type StructKeyword = SimpleToken<SyntaxKind.StructKeyword>;
export type ImportKeyword = SimpleToken<SyntaxKind.ImportKeyword>;
export type PubKeyword = SimpleToken<SyntaxKind.PubKeyword>;
export type LetKeyword = SimpleToken<SyntaxKind.LetKeyword>;
export type MutKeyword = SimpleToken<SyntaxKind.MutKeyword>;
export type TypeKeyword = SimpleToken<SyntaxKind.TypeKeyword>;
export type PerformKeyword = SimpleToken<SyntaxKind.PerformKeyword>;
export type ResumeKeyword = SimpleToken<SyntaxKind.ResumeKeyword>;
export type YieldKeyword = SimpleToken<SyntaxKind.YieldKeyword>;
export type ClassKeyword = SimpleToken<SyntaxKind.ClassKeyword>;
export type InstanceKeyword = SimpleToken<SyntaxKind.InstanceKeyword>;

export function describeSyntaxKind(kind: SyntaxKind): string {
  if (kind in TOKEN_TEXT) {
    return `'${TOKEN_TEXT[kind as TokenSyntaxKind]!}'`
  }
  switch (kind) {
    case SyntaxKind.EndOfFile: return 'end-of-file';
    case SyntaxKind.Identifier: return 'an identifier';
    case SyntaxKind.CustomOperator: return 'an operator';
    case SyntaxKind.DecimalInteger: return 'a decimal integer';
    case SyntaxKind.BlockStart: return `a new indented block started with '.'`
    case SyntaxKind.BlockEnd: return `the end of an indented block`
    case SyntaxKind.LineFoldStart: return 'the start of a new line-fold';
    case SyntaxKind.LineFoldEnd: return 'the end of the current line-fold';
    default:
      throw new Error(`Could not describe SyntaxKind ${SyntaxKind[kind]}: value went by unhandled.`);
  }
}


export class QualName extends SyntaxBase {

  public readonly kind!: SyntaxKind.QualName;

  public constructor(
    public modulePath: Array<[Identifier, DotSign]> = [],
    public name: Identifier,
  ) {
    super(SyntaxKind.QualName);
  }

  public *getChildren(): Iterable<Syntax> {
    for (const [name, dotSign] of this.modulePath) {
      yield name;
      yield dotSign
    }
    yield this.name;
  }

  public getFirstToken(): Token {
    if (this.modulePath.length > 0) {
      return this.modulePath[0][0];
    }
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export type Expression
  = ReferenceExpression
  | ConstantExpression
  | CallExpression
  | BinaryExpression
  | NestedExpression
  | MatchExpression

export class NestedExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.NestedExpression;

  public constructor(
    public lparen: LParen,
    public expression: Expression,
    public rparen: RParen,
  ) {
    super(SyntaxKind.NestedExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.lparen;
    yield this.expression;
    yield this.rparen;
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export class MatchArm extends SyntaxBase {

  public readonly kind!: SyntaxKind.MatchArm;

  public constructor(
    public pattern: Pattern,
    public equalSign: EqualSign,
    public expression: Expression,
  ) {
    super(SyntaxKind.MatchArm);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.pattern;
    yield this.equalSign;
    yield this.expression;
  }

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

export class MatchExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.MatchExpression;

  public constructor(
    public matchKeyword: MatchKeyword,
    public expression: Expression,
    public dotSign: DotSign,
    public arms: MatchArm[],
  ) {
    super(SyntaxKind.MatchExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.matchKeyword;
    yield this.expression
    yield this.dotSign;
    for (const arm of this.arms) {
      yield arm;
    }
  }

  public getFirstToken(): Token {
    return this.matchKeyword;
  }

  public getLastToken(): Token {
    if (this.arms.length > 0) {
      return this.arms[this.arms.length-1].getLastToken();
    }
    return this.dotSign;
  }

}

export class BinaryExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.BinaryExpression;

  public constructor(
    public lhs: Expression,
    public operator: Token,
    public rhs: Expression,
  ) {
    super(SyntaxKind.BinaryExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.lhs;
    yield this.operator;
    yield this.rhs;
  }

  public getFirstToken(): Token {
    return this.lhs.getFirstToken();
  }

  public getLastToken(): Token {
    return this.rhs.getLastToken();
  }

}

export class ReferenceExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.ReferenceExpression;

  public constructor(
    public name: Identifier,
  ) {
    super(SyntaxKind.ReferenceExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.name;
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export class ConstantExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.ConstantExpression;

  public constructor(
    public value: DecimalInteger,
  ) {
    super(SyntaxKind.ConstantExpression);
  }

  public *getChildren(): Generator<Token> {
    yield this.value;
  }

  public getFirstToken(): Token {
    return this.value;
  }

  public getLastToken(): Token {
    return this.value;
  }

}

export class CallExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.CallExpression;

  public constructor(
    public operator: Expression,
    public args: Expression[],
  ) {
    super(SyntaxKind.CallExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.operator
    for (const arg of this.args) {
      yield arg;
    }
  }

  public getFirstToken(): Token {
    return this.operator.getFirstToken();
  }

  public getLastToken(): Token {
    if (this.args.length > 0) {
      return this.args[this.args.length-1].getLastToken();
    }
    return this.operator.getLastToken();
  }

}

export class ReturnStatement extends SyntaxBase {

  public readonly kind!: SyntaxKind.ReturnStatement;

  public constructor(
    public returnKeyword: ReturnKeyword,
    public expression: Expression | null = null,
  ) {
    super(SyntaxKind.ReturnStatement);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.returnKeyword;
    if (this.expression !== null) {
      yield this.expression;
    }
  }

  public getFirstToken(): Token {
    return this.returnKeyword;
  }

  public getLastToken(): Token {
    if (this.expression !== null) {
      return this.expression.getLastToken()
    }
    return this.returnKeyword;
  }

}

export class ExpressionStatement extends SyntaxBase {

  public readonly kind!: SyntaxKind.ExpressionStatement;

  public constructor(
    public expression: Expression,
  ) {
    super(SyntaxKind.ExpressionStatement);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.expression;
  }

  public getFirstToken(): Token {
    return this.expression.getFirstToken();
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

export type Statement
  = ReturnStatement

export type TypeExpression
  = TypeReferenceExpression
  | NestedTypeExpression
  | ArrowTypeExpression

export class ArrowTypeExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.ArrowTypeExpression;

  public constructor(
    public paramTypes: Array<[TypeExpression, RArrowSign]>,
    public returnType: TypeExpression,
  ) {
    super(SyntaxKind.ArrowTypeExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    for (const [param, rarrowSign] of this.paramTypes) {
      yield param;
      yield rarrowSign;
    }
    yield this.returnType;
  }

  public getFirstToken(): Token {
    if (this.paramTypes.length > 0) {
      return this.paramTypes[0][0].getFirstToken();
    }
    return this.returnType.getFirstToken();
  }

  public getLastToken(): Token {
    return this.returnType.getLastToken();
  }

}

export class TypeReferenceExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.TypeReferenceExpression;

  public constructor(
    public name: QualName,
    public typeArgs: TypeExpression[] = [],
  ) {
    super(SyntaxKind.TypeReferenceExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.name;
    for (const typeArg of this.typeArgs) {
      yield typeArg;
    }
  }

  public getFirstToken(): Token {
    return this.name.getFirstToken();
  }

  public getLastToken(): Token {
    return this.typeArgs.length > 0
      ? this.typeArgs[this.typeArgs.length-1].getLastToken()
      : this.name.getLastToken();
  }

}

export class NestedTypeExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.NestedTypeExpression;

  public constructor(
    public lparen: LParen,
    public expression: TypeExpression,
    public rparen: RParen,
  ) {
    super(SyntaxKind.NestedTypeExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.lparen;
    yield this.expression;
    yield this.rparen;
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export class TypeParameter extends SyntaxBase {

  public readonly kind!: SyntaxKind.TypeParameter;

  public constructor(
    public name: Identifier,
  ) {
    super(SyntaxKind.TypeParameter);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.name;
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export type RecordDeclarationBody = [DotSign, Array<RecordDeclarationElement>];

export class RecordDeclaration extends SyntaxBase {

  public readonly kind!: SyntaxKind.RecordDeclaration;

  public constructor(
    public pubKeyword: PubKeyword | null = null,
    public structKeyword: StructKeyword,
    public name: Identifier,
    public typeParams: TypeParameter[] = [],
    public body: RecordDeclarationBody | null = null,
  ) {
    super(SyntaxKind.RecordDeclaration);
  }

  public *getChildren(): Iterable<Syntax> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.structKeyword;
    yield this.name;
    for (const typeParam of this.typeParams) {
      yield typeParam;
    }
    if (this.body !== null) {
      yield this.body[0];
      for (const element of this.body[1]) {
        yield element;
      }
    }
  }

  public getFirstToken(): Token {
    return this.pubKeyword !== null
        ? this.pubKeyword
        : this.structKeyword;
  }

  public getLastToken(): Token {
    if (this.body !== null) {
      const [dotSign, elements] = this.body;
      return elements.length > 0
          ? elements[elements.length-1].getLastToken()
          : dotSign;
    }
    if (this.typeParams.length > 0) {
      return this.typeParams[this.typeParams.length-1].getLastToken();
    }
    return this.name;
  }

}

export type RecordDeclarationElement
  = RecordDeclarationField
  | MacroInvocation

export class RecordDeclarationField extends SyntaxBase {

  public readonly kind!: SyntaxKind.RecordDeclarationField;

  public constructor(
    public name: Identifier,
    public colonSign: ColonSign,
    public typeExpr: TypeExpression,
  ) {
    super(SyntaxKind.RecordDeclarationField);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.name;
    yield this.colonSign;
    yield this.typeExpr;
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.typeExpr.getLastToken();
  }

}

export class MacroInvocation extends SyntaxBase {

  public readonly kind!: SyntaxKind.MacroInvocation;

  public constructor(
    public text: string,
  ) {
    super(SyntaxKind.MacroInvocation);
  }

  public getChildren(): Iterable<Syntax> {
    throw new Error(`Can not extract tokens from a macro invocation.`);
  }

  public getFirstToken(): Token {
    throw new Error(`Can not extract tokens from a macro invocation.`);
  }

  public getLastToken(): Token {
    throw new Error(`Can not extract tokens from a macro invocation.`);
  }

}

export type SourceElement
  = Expression
  | Statement
  | RecordDeclaration
  | Definition
  | ClassDeclaration
  | InstanceDeclaration

export type FunctionBodyElement
  = Statement
  | Expression
  | Definition

export type ParameterDefaultValue = [TildeSign, Expression];

export class Parameter extends SyntaxBase {

  public readonly kind!: SyntaxKind.Parameter;

  public constructor(
    public pattern: Pattern,
    public typeExpr: TypeExpression | null = null,
    public defaultValue: ParameterDefaultValue | null = null,
  ) {
    super(SyntaxKind.Parameter);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.pattern;
  }

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.pattern.getLastToken();
  }

}

export type Pattern
  = BindPattern
  | TuplePattern
  | ConstantExpression

export class BindPattern extends SyntaxBase {

  public readonly kind!: SyntaxKind.BindPattern;

  public constructor(
    public name: Identifier,
  ) {
    super(SyntaxKind.BindPattern);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.name;
  }

  public getFirstToken(): Token {
    return this.name;
  }

  public getLastToken(): Token {
    return this.name;
  }

}

export class TuplePattern extends SyntaxBase {

  public readonly kind!: SyntaxKind.TuplePattern;

  public constructor(
    public lparen: LParen,
    public elements: Pattern[],
    public rparen: RParen,
  ) {
    super(SyntaxKind.TuplePattern);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.lparen;
    for (const element of this.elements) {
      yield element;
    }
    yield this.rparen;
  }

  public getFirstToken(): Token {
    return this.lparen;
  }

  public getLastToken(): Token {
    return this.rparen;
  }

}

export type DefinitionBody
  = BlockDefinitionBody
  | InlineDefinitionBody

export class BlockDefinitionBody extends SyntaxBase {

  public readonly kind!: SyntaxKind.BlockDefinitionBody;

  public constructor(
    public dotSign: DotSign,
    public elements: FunctionBodyElement[],
  ) {
    super(SyntaxKind.BlockDefinitionBody);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.dotSign;
    for (const element of this.elements) {
      yield element;
    }
  }

  public getFirstToken(): Token {
    return this.dotSign;
  }

  public getLastToken(): Token {
    if (this.elements.length > 0) {
      return this.elements[this.elements.length-1].getLastToken();
    }
    return this.dotSign;
  }

}

export class InlineDefinitionBody extends SyntaxBase {

  public readonly kind!: SyntaxKind.InlineDefinitionBody;

  public constructor(
    public equalSign: EqualSign,
    public expression: Expression,
  ) {
    super(SyntaxKind.InlineDefinitionBody);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.equalSign;
    yield this.expression;
  }

  public getFirstToken(): Token {
    return this.equalSign;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

export type Definition
  = FunctionDefinition
  | VariableDefinition

export class FunctionDefinition extends SyntaxBase {

  public readonly kind!: SyntaxKind.FunctionDefinition;

  public typeEnv?: TypeEnv;
  public scheme?: Scheme;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public name: Identifier | Operator,
    public params: Parameter[],
    public typeExpr: TypeExpression | null,
    public body: DefinitionBody | null,
  ) {
    super(SyntaxKind.FunctionDefinition);
  }

  public *getChildren(): Iterable<Syntax> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.letKeyword;
    yield this.name;
    for (const param of this.params) {
      yield param;
    }
    if (this.typeExpr !== null) {
      yield this.typeExpr
    }
    if (this.body !== null) {
      yield this.body;
    }
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.letKeyword;
  }

  public getLastToken(): Token {
    if (this.body !== null) {
      return this.body.getLastToken();
    }
    if (this.typeExpr !== null) {
      return this.typeExpr.getLastToken()
    }
    if (this.params.length > 0) {
      return this.params[this.params.length-1].getLastToken();
    }
    return this.name;
  }

}


export class VariableDefinition extends SyntaxBase {

  public readonly kind!: SyntaxKind.VariableDefinition;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public mutKeyword: MutKeyword | null,
    public pattern: Pattern,
    public typeExpr: TypeExpression | null,
    public body: DefinitionBody | null,
  ) {
    super(SyntaxKind.VariableDefinition);
  }

  public *getChildren(): Iterable<Syntax> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.letKeyword;
    if (this.mutKeyword !== null) {
      yield this.mutKeyword;
    }
    yield this.pattern;
    if (this.typeExpr !== null) {
      yield this.typeExpr;
    }
    if (this.body !== null) {
      yield this.body;
    }
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.letKeyword;
  }

  public getLastToken(): Token {
    if (this.body !== null) {
      return this.body.getLastToken();
    }
    if (this.typeExpr !== null) {
      return this.typeExpr.getLastToken()
    }
    return this.pattern.getLastToken()
  }

}

export type ConstraintExpression
  = IsInstanceConstraintExpression

export class IsInstanceConstraintExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.IsInstanceConstraintExpression;

  public constructor(
    public name: QualName,
    public types: TypeExpression[],
  ) {
    super(SyntaxKind.IsInstanceConstraintExpression);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.name;
    for (const typeExpr of this.types) {
      yield typeExpr;
    }
  }

  public getFirstToken(): Token {
    return this.name.getFirstToken();
  }

  public getLastToken(): Token {
    return this.types[this.types.length-1].getLastToken();
  }

}

export class ConstraintSignature extends SyntaxBase {

  public readonly kind!: SyntaxKind.ConstraintSignature;

  public constructor(
    public expressions: Array<[ConstraintExpression, CommaSign | null]>,
    public rarrowAltSign: RArrowAltSign,
  ) {
    super(SyntaxKind.ConstraintSignature);
  }

  public *getChildren(): Iterable<Syntax> {
    for (const [expression, commaSign] of this.expressions) {
      yield expression;
      if (commaSign !== null) {
        yield commaSign;
      }
    }
    yield this.rarrowAltSign;
  }

  public getFirstToken(): Token {
    return this.expressions[0][0].getFirstToken();
  }

  public getLastToken(): Token {
    return this.rarrowAltSign;
  }

}

export class ClassDeclaration extends SyntaxBase {

  public readonly kind!: SyntaxKind.ClassDeclaration;

  public typeEnv?: TypeEnv;

  public constructor(
    public classKeyword: ClassKeyword,
    public constraints: ConstraintSignature | null,
    public name: Identifier,
    public typeParams: TypeExpression[],
    public dotSign: DotSign,
    public definitions: Definition[],
  ) {
    super(SyntaxKind.ClassDeclaration);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.classKeyword;
    if (this.constraints !== null) {
      yield this.constraints;
    }
    yield this.name;
    for (const typeExpr of this.typeParams) {
      yield typeExpr;
    }
    yield this.dotSign;
    for (const definition of this.definitions) {
      yield definition;
    }
  }

  public getFirstToken(): Token {
    return this.classKeyword;
  }

  public getLastToken(): Token {
    if (this.definitions.length > 0) {
      return this.definitions[this.definitions.length-1].getLastToken();
    }
    return this.dotSign;
  }

}

export class InstanceDeclaration extends SyntaxBase {

  public readonly kind!: SyntaxKind.InstanceDeclaration;

  public typeEnv?: TypeEnv;

  public constructor(
    public instanceKeyword: InstanceKeyword,
    public constraints: ConstraintSignature | null,
    public name: Identifier,
    public typeParams: TypeExpression[],
    public dotSign: DotSign,
    public definitions: Definition[],
  ) {
    super(SyntaxKind.InstanceDeclaration);
  }

  public *getChildren(): Iterable<Syntax> {
    yield this.instanceKeyword
    if (this.constraints !== null) {
      yield this.constraints;
    }
    yield this.name;
    for (const typeExpr of this.typeParams) {
      yield typeExpr;
    }
    yield this.dotSign;
    for (const definition of this.definitions) {
      yield definition;
    }
  }

  public getFirstToken(): Token {
    return this.instanceKeyword;
  }

  public getLastToken(): Token {
    if (this.definitions.length > 0) {
      return this.definitions[this.definitions.length-1].getLastToken();
    }
    return this.dotSign;
  }

}

const INIT_POS = {
  line: 1,
  column: 1,
  offset: 0,
}

export class SourceFile extends SyntaxBase {

  public readonly kind!: SyntaxKind.SourceFile;

  public file!: TextFile | null;

  private endPosition!: TextPosition;

  public constructor(
    public elements: SourceElement[],
    file: TextFile | null = null,
    endPosition: TextPosition | null = null,
  ) {
    super(SyntaxKind.SourceFile);
    Object.defineProperties(this, {
      file: {
        writable: true,
        value: file,
      },
      endPosition: {
        writable: true,
        value: endPosition,
      },
    });
  }

  public *getChildren(): Iterable<Syntax> {
    for (const element of this.elements) {
      yield element;
    }
  }

  public getStartPos(): TextPosition {
    return INIT_POS;
  }

  public getEndPos(): TextPosition {
    if (this.endPosition === null) {
      throw new Error(`SourceFile has no information about its end position.`);
    }
    return this.endPosition;
  }

  public getFirstToken(): Token {
    if (this.elements.length === 0) {
      throw new Error(`Can not get first token of an empty SourceFile.`); 
    }
    return this.elements[0].getFirstToken();
  }

  public getFile(): TextFile {
    if (this.file === null) {
      throw new Error(`Could not get the associated TextFile of a SourceFile. Most likely someone forgot to set the property.`);
    }
    return this.file;
  }

  public getLastToken(): Token {
    if (this.elements.length === 0) {
      throw new Error(`Can not get last token of an empty SourceFile.`); 
    }
    return this.elements[this.elements.length-1].getLastToken();
  }

}

