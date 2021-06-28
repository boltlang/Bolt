import { Scheme, Type, TypeEnv} from "./checker";
import { TextFile, TextPosition } from "./text";
import { ColonSign, DecimalInteger, DotSign, EqualSign, Identifier, LetKeyword, LParen, MatchKeyword, PubKeyword, RArrowSign, ReturnKeyword, RParen, StructKeyword, TildeSign, Token } from "./token";

export enum SyntaxKind {

  // Module-level nodes
  Declaration,
  VariableDefinition,
  FunctionDefinition,

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

  // Declarations
  RecordDeclaration,

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
  | FunctionDefinition
  | VariableDefinition
  | Declaration
  | RecordDeclaration

let nextNodeId = 0;

abstract class SyntaxBase {

  public readonly kind!: SyntaxKind;

  constructor(kind: SyntaxKind) {
    Object.defineProperties(this, {
      id: { value: nextNodeId++ },
      kind: { value: kind },
    })
  }

  public abstract getTokens(): Iterable<Token>;

  public abstract getFirstToken(): Token;

  public abstract getLastToken(): Token;

}

export class QualName extends SyntaxBase {

  public readonly kind!: SyntaxKind.QualName;

  public constructor(
    public modulePath: Array<[Identifier, DotSign]> = [],
    public name: Identifier,
  ) {
    super(SyntaxKind.QualName);
  }

  public *getTokens(): Iterable<Token> {
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

  public *getTokens(): Iterable<Token> {
    yield this.lparen;
    yield* this.expression.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield* this.pattern.getTokens();
    yield this.equalSign;
    yield* this.expression.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield this.matchKeyword;
    yield* this.expression.getTokens()
    yield this.dotSign;
    for (const arm of this.arms) {
      yield* arm.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield* this.lhs.getTokens();
    yield this.operator;
    yield* this.rhs.getTokens();
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

  public *getTokens(): Iterable<Token> {
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

  public *getTokens(): Generator<Token> {
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

  public *getTokens(): Iterable<Token> {
    yield* this.operator.getTokens()
    for (const arg of this.args) {
      yield* arg.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield this.returnKeyword;
    if (this.expression !== null) {
      yield* this.expression.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield* this.expression.getTokens();
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

export class TypeReferenceExpression extends SyntaxBase {

  public readonly kind!: SyntaxKind.TypeReferenceExpression;

  public constructor(
    public name: QualName,
    public typeArgs: TypeExpression[] = [],
  ) {
    super(SyntaxKind.QualName);
  }

  public *getTokens(): Iterable<Token> {
    yield* this.name.getTokens();
    for (const typeArg of this.typeArgs) {
      yield* typeArg.getTokens();
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

export class TypeParameter extends SyntaxBase {

  public readonly kind!: SyntaxKind.TypeParameter;

  public constructor(
    public name: Identifier,
  ) {
    super(SyntaxKind.TypeParameter);
  }

  public *getTokens(): Iterable<Token> {
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

  public *getTokens(): Iterable<Token> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.structKeyword;
    yield this.name;
    for (const typeParam of this.typeParams) {
      yield* typeParam.getTokens();
    }
    if (this.body !== null) {
      yield this.body[0];
      for (const element of this.body[1]) {
        yield* element.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield this.name;
    yield this.colonSign;
    yield* this.typeExpr.getTokens();
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

  public getTokens(): Iterable<Token> {
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
  = Statement
  | RecordDeclaration
  | Declaration
  | FunctionDefinition
  | VariableDefinition

export type FunctionBodyElement
  = Statement
  | Expression

export type ParameterDefaultValue = [TildeSign, Expression];

export class Parameter extends SyntaxBase {

  public readonly kind!: SyntaxKind.Parameter;

  public type: Type | null = null;

  public constructor(
    public pattern: Pattern,
    public typeExpr: TypeExpression | null = null,
    public defaultValue: ParameterDefaultValue | null = null,
  ) {
    super(SyntaxKind.Parameter);
  }

  public *getTokens(): Iterable<Token> {
    yield* this.pattern.getTokens();
  }

  public getFirstToken(): Token {
    return this.pattern.getFirstToken();
  }

  public getLastToken(): Token {
    return this.pattern.getLastToken();
  }

}

export class Declaration extends SyntaxBase {

  public readonly kind!: SyntaxKind.Declaration;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public name: Identifier,
    public colonSign: ColonSign,
    public paramTypes: Array<[TypeExpression, RArrowSign]>,
    public returnType: TypeExpression,
  ) {
    super(SyntaxKind.Declaration);
  }

  public *getTokens(): Iterable<Token> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.name;
    yield this.colonSign;
    for (const [typeExpr, rarrow] of this.paramTypes) {
      yield* typeExpr.getTokens();
      yield rarrow;
    }
    yield* this.returnType.getTokens();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.name;
  }

  public getLastToken(): Token {
    return this.returnType.getLastToken();
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

  public *getTokens(): Iterable<Token> {
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

  public *getTokens(): Iterable<Token> {
    yield this.lparen;
    for (const element of this.elements) {
      yield* element.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield this.dotSign;
    for (const element of this.elements) {
      yield* element.getTokens();
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

  public *getTokens(): Iterable<Token> {
    yield this.equalSign;
    yield* this.expression.getTokens();
  }

  public getFirstToken(): Token {
    return this.equalSign;
  }

  public getLastToken(): Token {
    return this.expression.getLastToken();
  }

}

export class FunctionDefinition extends SyntaxBase {

  public readonly kind!: SyntaxKind.FunctionDefinition;

  public typeEnv: TypeEnv | null = null;
  public scheme: Scheme | null = null;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public name: Identifier,
    public params: Parameter[],
    public body: DefinitionBody,
  ) {
    super(SyntaxKind.FunctionDefinition);
  }

  public *getTokens(): Iterable<Token> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.letKeyword;
    yield this.name;
    for (const param of this.params) {
      yield* param.getTokens();
    }
    yield* this.body.getTokens();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.letKeyword;
  }

  public getLastToken(): Token {
    return this.body.getLastToken();
  }

}


export class VariableDefinition extends SyntaxBase {

  public readonly kind!: SyntaxKind.VariableDefinition;

  public constructor(
    public pubKeyword: PubKeyword | null,
    public letKeyword: LetKeyword,
    public pattern: Pattern,
    public body: DefinitionBody,
  ) {
    super(SyntaxKind.VariableDefinition);
  }

  public *getTokens(): Iterable<Token> {
    if (this.pubKeyword !== null) {
      yield this.pubKeyword;
    }
    yield this.letKeyword;
    yield* this.pattern.getTokens();
    yield* this.body.getTokens();
  }

  public getFirstToken(): Token {
    if (this.pubKeyword !== null) {
      return this.pubKeyword;
    }
    return this.letKeyword;
  }

  public getLastToken(): Token {
    return this.body.getLastToken();
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

  public *getTokens(): Iterable<Token> {
    for (const element of this.elements) {
      yield* element.getTokens();
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

  public getLastToken(): Token {
    if (this.elements.length === 0) {
      throw new Error(`Can not get last token of an empty SourceFile.`); 
    }
    return this.elements[this.elements.length-1].getLastToken();
  }

}

