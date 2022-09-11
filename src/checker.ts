import {
  Expression,
  LetDeclaration,
  Pattern,
  Scope,
  SourceFile,
  Symkind,
  Syntax,
  SyntaxKind,
  TypeExpression
} from "./cst";
import {
  describeType,
  ArityMismatchDiagnostic,
  BindingNotFoudDiagnostic,
  Diagnostics,
  FieldDoesNotExistDiagnostic,
  FieldMissingDiagnostic,
  UnificationFailedDiagnostic,
  KindMismatchDiagnostic
} from "./diagnostics";
import { assert, isEmpty } from "./util";
import { LabeledDirectedHashGraph, LabeledGraph, strongconnect } from "yagl"

// FIXME Duplicate definitions are not checked

const MAX_TYPE_ERROR_COUNT = 5;

type NodeWithBindings = SourceFile | LetDeclaration;

type ReferenceGraph = LabeledGraph<NodeWithBindings, boolean>;

export enum TypeKind {
  Arrow,
  Var,
  Con,
  Any,
  Tuple,
  Labeled,
  Record,
  App,
}

abstract class TypeBase {

  public abstract readonly kind: TypeKind;

  public next: Type = this as any;

  public constructor(
    public node: Syntax | null = null
  ) {

  }

  public static join(a: Type, b: Type): void {
    const keep = a.next;
    a.next = b;
    b.next = keep;
  }

  public abstract getTypeVars(): Iterable<TVar>;

  public abstract shallowClone(): Type;

  public abstract substitute(sub: TVSub): Type;

  public hasTypeVar(tv: TVar): boolean {
    for (const other of this.getTypeVars()) {
      if (tv.id === other.id) {
        return true;
      }
    }
    return false;
  }

}

class TVar extends TypeBase {

  public readonly kind = TypeKind.Var;

  public constructor(
    public id: number,
    public node: Syntax | null = null,
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    yield this;
  }

  public shallowClone(): TVar {
    return new TVar(this.id, this.node);
  }

  public substitute(sub: TVSub): Type {
    const other = sub.get(this);
    return other === undefined
      ? this : other.substitute(sub);
  }

}

export class TArrow extends TypeBase {

  public readonly kind = TypeKind.Arrow;

  public constructor(
    public paramTypes: Type[],
    public returnType: Type,
    public node: Syntax | null = null,
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const paramType of this.paramTypes) {
      yield* paramType.getTypeVars();
    }
    yield* this.returnType.getTypeVars();
  }

  public shallowClone(): TArrow {
    return new TArrow(
      this.paramTypes,
      this.returnType,
      this.node,
    )
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newParamTypes = [];
    for (const paramType of this.paramTypes) {
      const newParamType = paramType.substitute(sub);
      if (newParamType !== paramType) {
        changed = true;
      }
      newParamTypes.push(newParamType);
    }
    const newReturnType = this.returnType.substitute(sub);
    if (newReturnType !== this.returnType) {
      changed = true;
    }
    return changed ? new TArrow(newParamTypes, newReturnType, this.node) : this;
  }

}

export class TCon extends TypeBase {

  public readonly kind = TypeKind.Con;

  public constructor(
    public id: number,
    public argTypes: Type[],
    public displayName: string,
    public node: Syntax | null = null,
  ) {
    super(node);
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const argType of this.argTypes) {
      yield* argType.getTypeVars();
    }
  }

  public shallowClone(): TCon {
    return new TCon(
      this.id,
      this.argTypes,
      this.displayName,
      this.node,
    );
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newArgTypes = [];
    for (const argType of this.argTypes) {
      const newArgType = argType.substitute(sub);
      if (newArgType !== argType) {
        changed = true;
      }
      newArgTypes.push(newArgType);
    }
    return changed ? new TCon(this.id, newArgTypes, this.displayName, this.node) : this;
  }

}

class TTuple extends TypeBase {

  public readonly kind = TypeKind.Tuple;

  public constructor(
    public elementTypes: Type[],
    public node: Syntax | null = null,
  ) {
    super(node);
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const elementType of this.elementTypes) {
      yield* elementType.getTypeVars();
    }
  }

  public shallowClone(): TTuple {
    return new TTuple(
      this.elementTypes,
      this.node,
    );
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newElementTypes = [];
    for (const elementType of this.elementTypes) {
      const newElementType = elementType.substitute(sub);
      if (newElementType !== elementType) {
        changed = true;
      }
      newElementTypes.push(newElementType);
    }
    return changed ? new TTuple(newElementTypes, this.node) : this;
  }

}

export class TLabeled extends TypeBase {

  public readonly kind = TypeKind.Labeled;

  public fields?: Map<string, Type>;
  public parent: TLabeled | null = null;

  public constructor(
    public name: string,
    public type: Type,
    public node: Syntax | null = null,
  ) {
    super(node);
  }

  public find(): TLabeled {
    let curr: TLabeled | null = this;
    while (curr.parent !== null) {
      curr = curr.parent;
    }
    this.parent = curr;
    return curr;
  }

  public getTypeVars(): Iterable<TVar> {
    return this.type.getTypeVars();
  }

  public shallowClone(): TLabeled {
    return new TLabeled(
      this.name,
      this.type,
      this.node,
    );
  }

  public substitute(sub: TVSub): Type {
    const newType = this.type.substitute(sub);
    return newType !== this.type ? new TLabeled(this.name, newType, this.node) : this;
  }

}

export class TRecord extends TypeBase {

  public readonly kind = TypeKind.Record;

  public nextRecord: TRecord | null = null;

  public constructor(
    public decl: Syntax,
    public fields: Map<string, Type>,
    public node: Syntax | null = null,
  ) {
    super(node);
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const type of this.fields.values()) {
      yield* type.getTypeVars();
    }
  }

  public shallowClone(): TRecord {
    return new TRecord(
      this.decl,
      this.fields,
      this.node
    );
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newFields = new Map();
    for (const [key, type] of this.fields) {
      const newType = type.substitute(sub);
      if (newType !== type) {
        changed = true;
      }
      newFields.set(key, newType);
    }
    return changed ? new TRecord(this.decl, newFields, this.node) : this;
  }

}

export class TApp extends TypeBase {

  public readonly kind = TypeKind.App;

  public constructor(
    public operatorType: Type,
    public argType: Type,
    public node: Syntax | null = null
  ) {
    super(node);
  }

  public static build(operatorType: Type, argTypes: Type[], node: Syntax | null = null): TApp {
    let count = argTypes.length;
    let result = argTypes[count-1];
    for (let i = count-2; i >= 0; i--) {
      result = new TApp(argTypes[i], result, node);
    }
    return new TApp(operatorType, result, node);
  }

  public *getSequence(): Iterable<Type> {
    if (this.operatorType.kind === TypeKind.App) {
      yield* this.operatorType.getSequence();
    } else {
      yield this.operatorType;
    }
    if (this.argType.kind === TypeKind.App) {
      yield* this.argType.getSequence();
    } else {
      yield this.argType;
    }
  }

  public *getTypeVars(): Iterable<TVar> {
     yield* this.operatorType.getTypeVars();
     yield* this.argType.getTypeVars();
  }

  public shallowClone() {
    return new TApp(
      this.operatorType,
      this.argType,
      this.node
    );
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newOperatorType = this.operatorType.substitute(sub);
    if (newOperatorType !== this.operatorType) {
      changed = true;
    }
    const newArgType = this.argType.substitute(sub);
    if (newArgType !== this.argType) {
      changed = true;
    }
    return changed ? new TApp(newOperatorType, newArgType, this.node) : this;
  }

}

export type Type
  = TCon
  | TArrow
  | TVar
  | TTuple
  | TLabeled
  | TRecord
  | TApp

class TVSet {

  private mapping = new Map<number, TVar>();

  public add(tv: TVar): void {
    this.mapping.set(tv.id, tv);
  }
  
  public has(tv: TVar): boolean {
    return this.mapping.has(tv.id);
  }

  public intersectsType(type: Type): boolean {
    for (const tv of type.getTypeVars()) {
      if (this.has(tv)) {
        return true; 
      }
    }
    return false;
  }

  public delete(tv: TVar): void {
    this.mapping.delete(tv.id);
  }

  public [Symbol.iterator](): Iterator<TVar> {
    return this.mapping.values();
  }

}

class TVSub {

  private mapping = new Map<number, Type>();

  public set(tv: TVar, type: Type): void {
    this.mapping.set(tv.id, type);
  }

  public get(tv: TVar): Type | undefined {
    return this.mapping.get(tv.id);
  }

  public has(tv: TVar): boolean {
    return this.mapping.has(tv.id);
  }

  public delete(tv: TVar): void {
    this.mapping.delete(tv.id);
  }

  public values(): Iterable<Type> {
    return this.mapping.values();
  }

}

const enum ConstraintKind {
  Equal,
  Many,
  Shaped,
}

abstract class ConstraintBase {

  public abstract substitute(sub: TVSub): Constraint;

  public constructor(
    public node: Syntax | null = null
  ) {
  }

  public prevInstantiation: Constraint | null = null;

  public *getNodes(): Iterable<Syntax> {
    let curr: Constraint | null = this as any;
    while (curr !== null) {
      if (curr.node !== null) {
        yield curr.node;
      }
      curr = curr.prevInstantiation;
    }
  }

}

class CShaped extends ConstraintBase {

  public readonly kind = ConstraintKind.Shaped;

  public constructor(
    public recordType: TLabeled,
    public type: Type,
  ) {
    super();
  }

  public substitute(sub: TVSub): Constraint {
    return new CShaped(
      this.recordType.substitute(sub) as TLabeled,
      this.type.substitute(sub),
    );
  }

}

class CEqual extends ConstraintBase {

  public readonly kind = ConstraintKind.Equal;

  public constructor(
    public left: Type,
    public right: Type,
    public node: Syntax,
  ) {
    super();
  }

  public substitute(sub: TVSub): Constraint {
    return new CEqual(
      this.left.substitute(sub),
      this.right.substitute(sub),
      this.node,
    );
  }

  public dump(): void {
    console.error(`${describeType(this.left)} ~ ${describeType(this.right)}`);
  }

}

class CMany extends ConstraintBase {

  public readonly kind = ConstraintKind.Many;

  public constructor(
    public elements: Constraint[]
  ) {
    super();
  }

  public substitute(sub: TVSub): Constraint {
    const newElements = [];
    for (const element of this.elements) {
      newElements.push(element.substitute(sub));
    }
    return new CMany(newElements);
  }

}

type Constraint
  = CEqual
  | CMany
  | CShaped

class ConstraintSet extends Array<Constraint> {
}

abstract class SchemeBase {
}

class Forall extends SchemeBase {

  public constructor(
    public typeVars: TVar[],
    public constraints: Constraint[],
    public type: Type,
  ) {
    super();
  }

}

type Scheme
  = Forall

export class TypeEnv {

  private mapping = new Map<string, Scheme>();

  public constructor(public parent: TypeEnv | null = null) {

  }

  public add(name: string, scheme: Scheme): void {
    this.mapping.set(name, scheme);
  }

  public lookup(name: string): Scheme | null {
    let curr: TypeEnv | null = this;
    do {
      const scheme = curr.mapping.get(name);
      if (scheme !== undefined) {
        return scheme;
      }
      curr = curr.parent;
    } while(curr !== null);
    return null;
  }

}

export interface InferContext {
  typeVars: TVSet;
  env: TypeEnv;
  constraints: ConstraintSet;
  returnType: Type | null;
}

function isFunctionDeclarationLike(node: LetDeclaration): boolean {
  return node.pattern.kind === SyntaxKind.BindPattern
      && (node.params.length > 0 || (node.body !== null && node.body.kind === SyntaxKind.BlockBody));
}

export class Checker {

  private nextTypeVarId = 0;
  private nextConTypeId = 0;

  //private graph?: Graph<Syntax>;
  //private currentCycle?: Map<Syntax, Type>;

  private stringType = new TCon(this.nextConTypeId++, [], 'String');
  private intType = new TCon(this.nextConTypeId++, [], 'Int');
  private boolType = new TCon(this.nextConTypeId++, [], 'Bool');

  private contexts: InferContext[] = [];

  private solution = new TVSub();

  public constructor(
    private diagnostics: Diagnostics
  ) {

  }

  public getIntType(): Type {
    return this.intType;
  }

  public getStringType(): Type {
    return this.stringType;
  }

  public getBoolType(): Type {
    return this.boolType;
  }

  private createTypeVar(): TVar {
    const typeVar = new TVar(this.nextTypeVarId++);
    const context = this.contexts[this.contexts.length-1];
    context.typeVars.add(typeVar);
    return typeVar;
  }

  private addConstraint(constraint: Constraint): void {
    this.contexts[this.contexts.length-1].constraints.push(constraint);
  }

  private pushContext(context: InferContext) {
    this.contexts.push(context);
  }

  private popContext(context: InferContext) {
    assert(this.contexts[this.contexts.length-1] === context);
    this.contexts.pop();
  }

  private lookup(name: string): Scheme | null {
    const context = this.contexts[this.contexts.length-1];
    return context.env.lookup(name);
  }

  private getReturnType(): Type {
    const context = this.contexts[this.contexts.length-1];
    assert(context && context.returnType !== null);
    return context.returnType;
  }

  private createSubstitution(scheme: Scheme): TVSub {
    const sub = new TVSub();
    for (const tv of scheme.typeVars) {
      sub.set(tv, this.createTypeVar());
    }
    return sub;
  }

  private instantiate(scheme: Scheme, node: Syntax | null, sub = this.createSubstitution(scheme)): Type {
    for (const constraint of scheme.constraints) {
      const substituted = constraint.substitute(sub);
      substituted.node = node;
      substituted.prevInstantiation = constraint;
      this.addConstraint(substituted);
    }
    return scheme.type.substitute(sub);
  }

  private addBinding(name: string, scheme: Scheme): void {
    const context = this.contexts[this.contexts.length-1];
    context.env.add(name, scheme);
  }

  public infer(node: Syntax): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.infer(element);
        }
        break;
      }

      case SyntaxKind.ExpressionStatement:
      {
        this.inferExpression(node.expression);
        break;
      }

      case SyntaxKind.IfStatement:
      {
        for (const cs of node.cases) {
          if (cs.test !== null) {
            this.addConstraint(
              new CEqual(
                this.inferExpression(cs.test),
                this.getBoolType(),
                cs.test
              )
            );
          }
          for (const element of cs.elements) {
            this.infer(element);
          }
        }
        break;
      }

      case SyntaxKind.ReturnStatement:
      {
        let type;
        if (node.expression === null) {
          type = new TTuple([]);
        } else {
          type = this.inferExpression(node.expression);
        }
        this.addConstraint(
          new CEqual(
            this.getReturnType(),
            type,
            node
          )
        );
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        if (isFunctionDeclarationLike(node)) {
          break;
        }
        let type;
        if (node.pattern.kind === SyntaxKind.WrappedOperator) {
          type = this.createTypeVar();
          this.addBinding(node.pattern.operator.text, new Forall([], [], type));
        } else {
          type = this.inferBindings(node.pattern, [], []);
        }
        if (node.typeAssert !== null) {
          this.addConstraint(
            new CEqual(
              this.inferTypeExpression(node.typeAssert.typeExpression),
              type,
              node
            )
          );
        }
        if (node.body !== null) {
          switch (node.body.kind) {
            case SyntaxKind.ExprBody:
            {
              const type2 = this.inferExpression(node.body.expression);
              this.addConstraint(
                new CEqual(
                  type,
                  type2,
                  node
                )
              );
              break;
            }
            case SyntaxKind.BlockBody:
            {
              // TODO
              assert(false);
            }
          }
        }
        break;
      }

      case SyntaxKind.TypeDeclaration:
      case SyntaxKind.EnumDeclaration:
      case SyntaxKind.StructDeclaration:
        break;

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  public inferExpression(node: Expression): Type {

    switch (node.kind) {

      case SyntaxKind.NestedExpression:
        return this.inferExpression(node.expression);

      case SyntaxKind.ReferenceExpression:
      {
        assert(node.name.modulePath.length === 0);
        const scope = node.getScope();
        const target = scope.lookup(node.name.name.text);
        if (target !== null && target.kind === SyntaxKind.LetDeclaration && target.active) {
          return target.type!;
        }
        const scheme = this.lookup(node.name.name.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.name.text, node.name.name));
          return this.createTypeVar();
        }
        const type = this.instantiate(scheme, node);
        type.node = node;
        return type;
      }

      case SyntaxKind.MemberExpression:
      {
        let type = this.inferExpression(node.expression);
        for (const [_dot, name] of node.path) {
          const newType = this.createTypeVar();
          this.addConstraint(
            new CEqual(
              type,
              new TLabeled(name.text, newType),
              node,
            )
          );
          type = newType;
        }
        return type;
      }

      case SyntaxKind.CallExpression:
      {
        const opType = this.inferExpression(node.func);
        const retType = this.createTypeVar();
        const paramTypes = [];
        for (const arg of node.args) {
          paramTypes.push(this.inferExpression(arg));
        }
        this.addConstraint(
          new CEqual(
            opType,
            new TArrow(paramTypes, retType),
            node
          )
        );
        return retType;
      }

      case SyntaxKind.ConstantExpression:
      {
        let ty;
        switch (node.token.kind) {
          case SyntaxKind.StringLiteral:
            ty = this.getStringType();
            break;
          case SyntaxKind.Integer:
            ty = this.getIntType();
            break;
        }
        ty = ty.shallowClone();
        ty.node = node;
        return ty;
      }

      case SyntaxKind.NamedTupleExpression:
      {
        const scheme = this.lookup(node.name.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          return this.createTypeVar();
        }
        const type = this.instantiate(scheme, node.name);
        assert(type.kind === TypeKind.Con);
        const argTypes = [];
        for (const element of node.elements) {
          argTypes.push(this.inferExpression(element));
        }
        return new TCon(type.id, argTypes, type.displayName, node);
      }

      case SyntaxKind.StructExpression:
      {
        const scope = node.getScope();
        const decl = scope.lookup(node.name.text, Symkind.Constructor);
        if (decl === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          return this.createTypeVar();
        }
        assert(decl.kind === SyntaxKind.StructDeclaration || decl.kind === SyntaxKind.EnumDeclarationStructElement);
        const scheme = decl.scheme;
        const sub = this.createSubstitution(scheme);
        const declType = this.instantiate(scheme, node, sub);
        const argTypes = [];
        for (const typeVar of decl.tvs) {
          argTypes.push(sub.get(typeVar)!);
        }
        const fields = new Map();
        for (const member of node.members) {
          switch (member.kind) {
            case SyntaxKind.StructExpressionField:
            {
              fields.set(member.name.text, this.inferExpression(member.expression));
              break;
            }
            case SyntaxKind.PunnedStructExpressionField:
            {
              const scheme = this.lookup(member.name.text);
              let fieldType;
              if (scheme === null) {
                this.diagnostics.add(new BindingNotFoudDiagnostic(member.name.text, member.name));
                fieldType = this.createTypeVar();
              } else {
                fieldType = this.instantiate(scheme, member);
              }
              fields.set(member.name.text, fieldType);
              break;
            }
            default:
              throw new Error(`Unexpected ${member}`);
          }
        }
        const type = TApp.build(new TRecord(decl, fields, node), argTypes, node);
        this.addConstraint(
          new CEqual(
            TApp.build(declType, argTypes, node),
            type,
            node,
          )
        );
        return type;
      }

      case SyntaxKind.InfixExpression:
      {
        const scheme = this.lookup(node.operator.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.operator.text, node.operator));
          return this.createTypeVar();
        }
        const opType = this.instantiate(scheme, node.operator);
        const retType = this.createTypeVar();
        const leftType = this.inferExpression(node.left);
        const rightType = this.inferExpression(node.right);
        this.addConstraint(
          new CEqual(
            new TArrow([ leftType, rightType ], retType),
            opType,
            node,
          ),
        );
        return retType;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  public inferTypeExpression(node: TypeExpression, introduceTypeVars = false): Type {

    switch (node.kind) {

      case SyntaxKind.ReferenceTypeExpression:
      {
        const scheme = this.lookup(node.name.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          return this.createTypeVar();
        }
        const type = this.instantiate(scheme, node.name);
        type.node = node;
        return type;
      }

      case SyntaxKind.VarTypeExpression:
      {
        const scheme = this.lookup(node.name.text);
        if (scheme === null) {
          if (!introduceTypeVars) {
            this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          }
          const type = this.createTypeVar();
          this.addBinding(node.name.text, new Forall([], [], type));
          return type;
        }
        assert(scheme.typeVars.length === 0);
        assert(scheme.constraints.length === 0);
        return scheme.type;
      }

      case SyntaxKind.AppTypeExpression:
      {
        const operatorType = this.inferTypeExpression(node.operator);
        const argTypes = [];
        for (const argTypeExpr of node.args) {
          argTypes.push(this.inferTypeExpression(argTypeExpr));
        }
        return TApp.build(operatorType, argTypes);
      }

      case SyntaxKind.ArrowTypeExpression:
      {
        const paramTypes = [];
        for (const paramTypeExpr of node.paramTypeExprs) {
          paramTypes.push(this.inferTypeExpression(paramTypeExpr));
        }
        const returnType = this.inferTypeExpression(node.returnTypeExpr);
        return new TArrow(paramTypes, returnType, node);
      }

      default:
        throw new Error(`Unrecognised ${node}`);

    }

  }

  public inferBindings(pattern: Pattern, typeVars: TVar[], constraints: Constraint[]): Type {

    switch (pattern.kind) {

      case SyntaxKind.BindPattern:
      {
        const type = this.createTypeVar();
        this.addBinding(pattern.name.text, new Forall(typeVars, constraints, type));
        return type;
      }

      case SyntaxKind.StructPattern:
      {
        const scheme = this.lookup(pattern.name.text);
        let recordType;
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(pattern.name.text, pattern.name));
          recordType = this.createTypeVar();
        } else {
          recordType = this.instantiate(scheme, pattern.name);
        }
        const type = this.createTypeVar();
        for (const member of pattern.members) {
          switch (member.kind) {
            case SyntaxKind.StructPatternField:
            {
              const fieldType = this.inferBindings(member.pattern, typeVars, constraints);
              this.addConstraint(
                new CEqual(
                  new TLabeled(member.name.text, fieldType),
                  type,
                  member
                )
              );
              break;
            }
            case SyntaxKind.PunnedStructPatternField:
            {
              const fieldType = this.createTypeVar();
              this.addBinding(member.name.text, new Forall([], [], fieldType));
              this.addConstraint(
                new CEqual(
                  new TLabeled(member.name.text, fieldType),
                  type,
                  member
                )
              );
              break;
            }
            default:
              throw new Error(`Unexpected ${member.constructor.name}`);
          }
        }
        this.addConstraint(
          new CEqual(
            recordType,
            type,
            pattern
          )
        );
        return type;
      }

      default:
        throw new Error(`Unexpected ${pattern.constructor.name}`);

    }

  }

  private addReferencesToGraph(graph: ReferenceGraph, node: Syntax, source: LetDeclaration | SourceFile) {

    const addReference = (scope: Scope, name: string) => {
      const target = scope.lookup(name);
      if (target === null || target.kind === SyntaxKind.Param) {
        return;
      }
      assert(target.kind === SyntaxKind.LetDeclaration || target.kind === SyntaxKind.SourceFile);
      graph.addEdge(source, target, true);
    }

    switch (node.kind) {

      case SyntaxKind.ConstantExpression:
        break;

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.addReferencesToGraph(graph, element, source);
        }
        break;
      }

      case SyntaxKind.ReferenceExpression:
      {
        assert(node.name.modulePath.length === 0);
        addReference(node.getScope(), node.name.name.text);
        break;
      }

      case SyntaxKind.MemberExpression:
      {
        this.addReferencesToGraph(graph, node.expression, source);
        break;
      }

      case SyntaxKind.NamedTupleExpression:
      {
        for (const arg of node.elements) {
          this.addReferencesToGraph(graph, arg, source);
        }
        break;
      }

      case SyntaxKind.StructExpression:
      {
        for (const member of node.members) {
          switch (member.kind) {
            case SyntaxKind.PunnedStructExpressionField:
            {
              addReference(node.getScope(), node.name.text);
              break;
            }
            case SyntaxKind.StructExpressionField:
            {
              this.addReferencesToGraph(graph, member.expression, source);
              break;
            };
          }
        }
        break;
      }

      case SyntaxKind.NestedExpression:
      {
        this.addReferencesToGraph(graph, node.expression, source);
        break;
      }

      case SyntaxKind.InfixExpression:
      {
        this.addReferencesToGraph(graph, node.left, source);
        this.addReferencesToGraph(graph, node.right, source);
        break;
      }

      case SyntaxKind.CallExpression:
      {
        this.addReferencesToGraph(graph, node.func, source);
        for (const arg of node.args) {
          this.addReferencesToGraph(graph, arg, source);
        }
        break;
      }

      case SyntaxKind.IfStatement:
      {
        for (const cs of node.cases) {
          if (cs.test !== null) {
            this.addReferencesToGraph(graph, cs.test, source);
          }
          for (const element of cs.elements) {
            this.addReferencesToGraph(graph, element, source);
          }
        }
        break;
      }

      case SyntaxKind.ExpressionStatement:
      {
        this.addReferencesToGraph(graph, node.expression, source);
        break;
      }

      case SyntaxKind.ReturnStatement:
      {
        if (node.expression !== null) {
          this.addReferencesToGraph(graph, node.expression, source);
        }
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        graph.addVertex(node);
        if (node.body !== null) {
          switch (node.body.kind) {
            case SyntaxKind.ExprBody:
            {
              this.addReferencesToGraph(graph, node.body.expression, node);
              break;
            }
            case SyntaxKind.BlockBody:
            {
              for (const element of node.body.elements) {
                this.addReferencesToGraph(graph, element, node);
              }
              break;
            }
          }
        }
        break;
      }

      case SyntaxKind.TypeDeclaration:
      case SyntaxKind.EnumDeclaration:
      case SyntaxKind.StructDeclaration:
        break;

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  private completeReferenceGraph(graph: ReferenceGraph, node: Syntax): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.completeReferenceGraph(graph, element);
        }
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        if (isEmpty(graph.getSourceVertices(node))) {
          const source = node.parent!.getScope().node;
          assert(source.kind === SyntaxKind.LetDeclaration || source.kind === SyntaxKind.SourceFile);
          graph.addEdge(source, node, false);
        }
        if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
          for (const element of node.body.elements) {
            this.completeReferenceGraph(graph, element);
          }
        }
        break;
      }

      case SyntaxKind.IfStatement:
      case SyntaxKind.ReturnStatement:
      case SyntaxKind.ExpressionStatement:
      case SyntaxKind.TypeDeclaration:
      case SyntaxKind.EnumDeclaration:
      case SyntaxKind.StructDeclaration:
        break;

      default:
        throw new Error(`Unexpected ${node}`);

    }

  }

  private initialize(node: Syntax, parentEnv: TypeEnv): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        const env = node.typeEnv = new TypeEnv(parentEnv);
        for (const element of node.elements) {
          this.initialize(element, env);
        }
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        const env = node.typeEnv = new TypeEnv(parentEnv);
        if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
          for (const element of node.body.elements) {
            this.initialize(element, env);
          }
        }
        break;
      }

      case SyntaxKind.IfStatement:
      case SyntaxKind.ExpressionStatement:
      case SyntaxKind.ReturnStatement:
        break;

      case SyntaxKind.EnumDeclaration:
      {
        // TODO complete this
        break;
      }

      case SyntaxKind.TypeDeclaration:
      {
        const env = node.typeEnv = new TypeEnv(parentEnv);
        const constraints = new ConstraintSet();
        const typeVars = new TVSet();
        const context: InferContext = {
          constraints,
          typeVars,
          env,
          returnType: null,
        };
        this.pushContext(context);
        for (const varExpr of node.typeVars) {
          env.add(varExpr.text, new Forall([], [], this.createTypeVar()));
        }
        const type = this.inferTypeExpression(node.typeExpression);
        this.popContext(context);
        const scheme = new Forall(typeVars, constraints, type);
        parentEnv.add(node.name.text, scheme); 
        node.scheme = scheme;
        break;
      }

      case SyntaxKind.StructDeclaration:
      {
        const env = node.typeEnv = new TypeEnv(parentEnv);
        const typeVars = new TVSet();
        const constraints = new ConstraintSet();
        const context: InferContext = {
          constraints,
          typeVars,
          env,
          returnType: null,
        };
        this.pushContext(context);
        const argTypes = [];
        for (const varExpr of node.typeVars) {
          const type = this.createTypeVar();
          env.add(varExpr.text, new Forall([], [], type));
          argTypes.push(type);
        }
        const fields = new Map<string, Type>();
        if (node.members !== null) {
          for (const member of node.members) {
            fields.set(member.name.text, this.inferTypeExpression(member.typeExpr));
          }
        }
        this.popContext(context);
        const type = new TRecord(node, fields, node);
        const scheme = new Forall(typeVars, constraints, type);
        parentEnv.add(node.name.text, scheme);
        node.tvs = argTypes;
        node.scheme = scheme; //new Forall(typeVars, constraints, new TApp(type, argTypes));
        break;
      }

      default:
        throw new Error(`Unexpected ${node}`);

    }

  }

  public check(node: SourceFile): void {

    const typeVars = new TVSet();
    const constraints = new ConstraintSet();
    const env = new TypeEnv();
    const context: InferContext = { typeVars, constraints, env, returnType: null };

    this.pushContext(context);

    const a = this.createTypeVar();

    env.add('String', new Forall([], [], this.stringType));
    env.add('Int', new Forall([], [], this.intType));
    env.add('True', new Forall([], [], this.boolType));
    env.add('False', new Forall([], [], this.boolType));
    env.add('+', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.add('-', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.add('*', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.add('/', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.add('==', new Forall([ a ], [], new TArrow([ a, a ], this.boolType)));
    env.add('not', new Forall([], [], new TArrow([ this.boolType ], this.boolType)));

    const graph = new LabeledDirectedHashGraph<NodeWithBindings, boolean>();
    this.addReferencesToGraph(graph, node, node);
    this.completeReferenceGraph(graph, node);

    this.initialize(node, env);

    this.pushContext({
      typeVars,
      constraints,
      env: node.typeEnv!,
      returnType: null
    });

    const sccs = [...strongconnect(graph)];

    for (const nodes of sccs) {

      if (nodes.some(n => n.kind === SyntaxKind.SourceFile)) {
        assert(nodes.length === 1);
        continue;
      }

      const typeVars = new TVSet();
      const constraints = new ConstraintSet();

      for (const node of nodes) {

        assert(node.kind === SyntaxKind.LetDeclaration);

        if (!isFunctionDeclarationLike(node)) {
          continue;
        }

        const env = node.typeEnv!;
        const context: InferContext = {
          typeVars,
          constraints,
          env,
          returnType: null,
        };
        node.context = context;

        this.contexts.push(context);

        const returnType = this.createTypeVar();
        context.returnType = returnType;

        const paramTypes = [];
        for (const param of node.params) {
          const paramType = this.inferBindings(param.pattern, [], []);
          paramTypes.push(paramType);
        }

        let type = new TArrow(paramTypes, returnType);
        if (node.typeAssert !== null) {
          this.addConstraint(
            new CEqual(
              this.inferTypeExpression(node.typeAssert.typeExpression),
              type,
              node
            )
          );
        }
        node.type = type;

        this.contexts.pop();

        // FIXME get rid of all this useless stack manipulation
        const parentDecl = node.parent!.getScope().node;
        const bindCtx = {
          typeVars: context.typeVars,
          constraints: context.constraints,
          env: parentDecl.typeEnv!,
          returnType: null,
        };
        this.contexts.push(bindCtx)
        let ty2;
        if (node.pattern.kind === SyntaxKind.WrappedOperator) {
          ty2 = this.createTypeVar();
          this.addBinding(node.pattern.operator.text, new Forall([], [], ty2));
        } else {
          ty2 = this.inferBindings(node.pattern, typeVars, constraints);
        }
        this.addConstraint(new CEqual(ty2, type, node));
        this.contexts.pop();
      }

    }

    const visitElements = (elements: Syntax[]) => {
      for (const element of elements) {
        if (element.kind === SyntaxKind.LetDeclaration
            && isFunctionDeclarationLike(element)
            && graph.hasEdge(node, element, false)) {
          assert(element.pattern.kind === SyntaxKind.BindPattern);
          const scheme = this.lookup(element.pattern.name.text);
          assert(scheme !== null);
          this.instantiate(scheme, null);
        } else {
          this.infer(element);
        }
      }
    }

    for (const nodes of sccs) {

      if (nodes.some(n => n.kind === SyntaxKind.SourceFile)) {
        assert(nodes.length === 1);
        continue;
      }

      for (const node of nodes) {
        assert(node.kind === SyntaxKind.LetDeclaration);
        node.active = true;
      }

      for (const node of nodes) {

        assert(node.kind === SyntaxKind.LetDeclaration);

        if (!isFunctionDeclarationLike(node)) {
          continue;
        }

        const context = node.context!;
        const returnType = context.returnType!;
        this.contexts.push(context);

        if (node.body !== null) {
          switch (node.body.kind) {
            case SyntaxKind.ExprBody:
            {
              this.addConstraint(
                new CEqual(
                  this.inferExpression(node.body.expression),
                  returnType,
                  node.body.expression
                )
              );
              break;
            }
            case SyntaxKind.BlockBody:
            {
              visitElements(node.body.elements);
              break;
            }
          }
        }

        this.contexts.pop();
      }

      for (const node of nodes) {
        assert(node.kind === SyntaxKind.LetDeclaration);
        node.active = false;
      }

    }
 
    visitElements(node.elements);

    this.contexts.pop();
    this.popContext(context);

    this.solve(new CMany(constraints), this.solution);
  }

  private solve(constraint: Constraint, solution: TVSub): void {

    const queue = [ constraint ];

    let errorCount = 0;

    while (queue.length > 0) {

      const constraint = queue.shift()!;

      switch (constraint.kind) {

        case ConstraintKind.Many:
        {
          for (const element of constraint.elements) {
            queue.push(element);
          }
          break;
        }

        case ConstraintKind.Equal:
        {
          // constraint.dump();
          if (!this.unify(constraint.left, constraint.right, solution, constraint)) {
            errorCount++;
            if (errorCount === MAX_TYPE_ERROR_COUNT) {
              return;
            }
          }
          break;
        }
      }

    }

  }

  private unify(left: Type, right: Type, solution: TVSub, constraint: CEqual): boolean {

    while (left.kind === TypeKind.Var && solution.has(left)) {
      left = solution.get(left)!;
    }
    while (right.kind === TypeKind.Var && solution.has(right)) {
      right = solution.get(right)!;
    }

    if (left.kind === TypeKind.Var) {
      if (right.hasTypeVar(left)) {
        // TODO occurs check diagnostic
        return false;
      }
      solution.set(left, right);
      TypeBase.join(left, right);
      return true;
    }

    if (right.kind === TypeKind.Var) {
      return this.unify(right, left, solution, constraint);
    }

    if (left.kind === TypeKind.Arrow && right.kind === TypeKind.Arrow) {
      if (left.paramTypes.length !== right.paramTypes.length) {
        this.diagnostics.add(new ArityMismatchDiagnostic(left, right));
        return false;
      }
      let success = true;
      const count = left.paramTypes.length;
      for (let i = 0; i < count; i++) {
        if (!this.unify(left.paramTypes[i], right.paramTypes[i], solution, constraint)) {
          success = false;
        }
      }
      if (!this.unify(left.returnType, right.returnType, solution, constraint)) {
        success = false;
      }
      if (success) {
        TypeBase.join(left, right);
      }
      return success;
    }

    if (left.kind === TypeKind.Arrow && left.paramTypes.length === 0) {
      return this.unify(left.returnType, right, solution, constraint);
    }

    if (right.kind === TypeKind.Arrow) {
      return this.unify(right, left, solution, constraint);
    }

    if (left.kind === TypeKind.Con && right.kind === TypeKind.Con) {
      if (left.id === right.id) {
        assert(left.argTypes.length === right.argTypes.length);
        const count = left.argTypes.length;
        let success = true; 
        for (let i = 0; i < count; i++) {
          if (!this.unify(left.argTypes[i], right.argTypes[i], solution, constraint)) {
            success = false;
          }
        }
        if (success) {
          TypeBase.join(left, right);
        }
        return success;
      }
    }

    if (left.kind === TypeKind.App && right.kind === TypeKind.App) {
      let leftElements = [...left.getSequence()];
      let rightElements = [...right.getSequence()];
      if (leftElements.length !== rightElements.length) {
        this.diagnostics.add(new KindMismatchDiagnostic(leftElements.length-1, rightElements.length-1, constraint.node));
        return false;
      }
      const count = leftElements.length;
      let success = true;
      for (let i = 0; i < count; i++) {
        if (!this.unify(leftElements[i], rightElements[i], solution, constraint)) {
          success = false;
        }
      }
      return success;
    }

    if (left.kind === TypeKind.Labeled && right.kind === TypeKind.Labeled) {
      let success = false;
      // This works like an ordinary union-find algorithm where an additional
      // property 'fields' is carried over from the child nodes to the
      // ever-changing root node.
      const root = left.find();
      right.parent = root;
      if (root.fields === undefined) {
        root.fields = new Map([ [ root.name, root.type ] ]);
      }
      if (right.fields === undefined) {
        right.fields = new Map([ [ right.name, right.type ] ]);
      }
      for (const [fieldName, fieldType] of right.fields) {
        if (root.fields.has(fieldName)) {
          if (!this.unify(root.fields.get(fieldName)!, fieldType, solution, constraint)) {
            success = false;
          }
        } else {
          root.fields.set(fieldName, fieldType);
        }
      }
      delete right.fields;
      if (success) {
        TypeBase.join(left, right);
      }
      return success;
    }

    if (left.kind === TypeKind.Record && right.kind === TypeKind.Record) {
      if (left.decl !== right.decl) {
        this.diagnostics.add(new UnificationFailedDiagnostic(left, right, [...constraint.getNodes()]));
        return false;
      }
      let success = true;
      const remaining = new Set(right.fields.keys());
      for (const [fieldName, fieldType] of left.fields) {
        if (right.fields.has(fieldName)) {
          if (!this.unify(fieldType, right.fields.get(fieldName)!, solution, constraint)) {
            success = false;
          }
          remaining.delete(fieldName);
        } else {
          this.diagnostics.add(new FieldMissingDiagnostic(right, fieldName, constraint.node));
          success = false;
        }
      }
      for (const fieldName of remaining) {
        this.diagnostics.add(new FieldDoesNotExistDiagnostic(left, fieldName, constraint.node));
      }
      if (success) {
        TypeBase.join(left, right);
      }
      return success;
    }

    if (left.kind === TypeKind.Record && right.kind === TypeKind.Labeled) {
      let success = true;
      if (right.fields === undefined) {
        right.fields = new Map([ [ right.name, right.type ] ]);
      }
      for (const [fieldName, fieldType] of right.fields) {
        if (left.fields.has(fieldName)) {
          if (!this.unify(fieldType, left.fields.get(fieldName)!, solution, constraint)) {
            success = false;
          }
        } else {
          this.diagnostics.add(new FieldMissingDiagnostic(left, fieldName));
        }
      }
      if (success) {
        TypeBase.join(left, right);
      }
      return success;
    }

    if (left.kind === TypeKind.Labeled && right.kind === TypeKind.Record) {
      return this.unify(right, left, solution, constraint);
    }

    this.diagnostics.add(
      new UnificationFailedDiagnostic(
        left.substitute(solution),
        right.substitute(solution),
        [...constraint.getNodes()],
      )
    );
    return false;
  }

}

