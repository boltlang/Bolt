import {
  EnumDeclaration,
  Expression,
  ExprOperator,
  Identifier,
  IdentifierAlt,
  LetDeclaration,
  ModuleDeclaration,
  Pattern,
  ReferenceExpression,
  ReferenceTypeExpression,
  SourceFile,
  StructDeclaration,
  Symkind,
  Syntax,
  SyntaxKind,
  TypeExpression,
} from "./cst";
import {
  describeType,
  BindingNotFoudDiagnostic,
  Diagnostics,
  FieldDoesNotExistDiagnostic,
  FieldMissingDiagnostic,
  UnificationFailedDiagnostic,
  KindMismatchDiagnostic,
  ModuleNotFoundDiagnostic,
} from "./diagnostics";
import { assert, isEmpty, MultiMap } from "./util";
import { Analyser } from "./analysis";

const MAX_TYPE_ERROR_COUNT = 5;

export enum TypeKind {
  Arrow,
  Var,
  Con,
  Any,
  Tuple,
  Labeled,
  Record,
  App,
  Nominal,
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
    public paramType: Type,
    public returnType: Type,
    public node: Syntax | null = null,
  ) {
    super();
  }

  public static build(paramTypes: Type[], returnType: Type, node: Syntax | null = null): Type {
    let result = returnType;
    for (let i = paramTypes.length-1; i >= 0; i--) {
      result = new TArrow(paramTypes[i], result, node);
    }
    return result;
  }

  public *getTypeVars(): Iterable<TVar> {
    yield* this.paramType.getTypeVars();
    yield* this.returnType.getTypeVars();
  }

  public shallowClone(): TArrow {
    return new TArrow(
      this.paramType,
      this.returnType,
      this.node,
    )
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newParamType = this.paramType.substitute(sub);
    if (newParamType !== this.paramType) {
      changed = true;
    }
    const newReturnType = this.returnType.substitute(sub);
    if (newReturnType !== this.returnType) {
      changed = true;
    }
    return changed ? new TArrow(newParamType, newReturnType, this.node) : this;
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

  public constructor(
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
    return changed ? new TRecord(newFields, this.node) : this;
  }

}

export class TApp extends TypeBase {

  public readonly kind = TypeKind.App;

  public constructor(
    public left: Type,
    public right: Type,
    public node: Syntax | null = null
  ) {
    super(node);
  }

  public static build(resultType: Type, types: Type[], node: Syntax | null = null): Type {
    for (let i = 0; i < types.length; i++) {
      resultType = new TApp(types[i], resultType, node);
    }
    return resultType;
  }

  public *getTypeVars(): Iterable<TVar> {
     yield* this.left.getTypeVars();
     yield* this.right.getTypeVars();
  }

  public shallowClone() {
    return new TApp(
      this.left,
      this.right,
      this.node
    );
  }

  public substitute(sub: TVSub): Type {
    let changed = false;
    const newOperatorType = this.left.substitute(sub);
    if (newOperatorType !== this.left) {
      changed = true;
    }
    const newArgType = this.right.substitute(sub);
    if (newArgType !== this.right) {
      changed = true;
    }
    return changed ? new TApp(newOperatorType, newArgType, this.node) : this;
  }

}

export class TNominal extends TypeBase {

  public readonly kind = TypeKind.Nominal;

  public constructor(
    public decl: StructDeclaration | EnumDeclaration,
    public node: Syntax | null = null,
  ) {
    super(node);
  }

  public *getTypeVars(): Iterable<TVar> {

  }

  public shallowClone(): Type {
    return new TNominal(
      this.decl,
      this.node,
    );
  }

  public substitute(_sub: TVSub): Type {
    return this;
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
  | TNominal

export const enum KindType {
  Star,
  Arrow,
  Var,
}

class KVSub {

  private mapping = new Map<number, Kind>();

  public set(kv: KVar, kind: Kind): void {
    this.mapping.set(kv.id, kind);
  }

  public get(kv: KVar): Kind | undefined {
    return this.mapping.get(kv.id);
  }

  public has(kv: KVar): boolean {
    return this.mapping.has(kv.id);
  }

  public values(): Iterable<Kind> {
    return this.mapping.values();
  }

}

const enum KindFlags {
  UnificationFailed = 1,
}

abstract class KindBase {

  public flags: KindFlags = 0;
  
  public abstract readonly type: KindType;

  public abstract substitute(sub: KVSub): Kind;

}

class KVar extends KindBase {

  public readonly type = KindType.Var;

  public constructor(
    public id: number,
  ) {
    super();
  }

  public substitute(sub: KVSub): Kind {
    const other = sub.get(this);
    return other === undefined
      ? this : other.substitute(sub);
  }

  public hasFailed(): boolean {
    return true;
  }

}

class KStar extends KindBase {

  public readonly type = KindType.Star;

  public substitute(_sub: KVSub): Kind {
    return this;
  }

  public hasFailed(): boolean {
    return (this.flags & KindFlags.UnificationFailed) > 0;
  }

}

class KArrow extends KindBase {

  public readonly type = KindType.Arrow;

  public constructor(
    public left: Kind,
    public right: Kind,
  ) {
    super();
  }

  public hasFailed(): boolean {
    return (this.flags & KindFlags.UnificationFailed) > 0
        || this.left.hasFailed()
        || this.right.hasFailed();
  }

  public substitute(sub: KVSub): Kind {
    return new KArrow(
      this.left.substitute(sub),
      this.right.substitute(sub),
    );
  }

}

export type Kind
  = KStar
  | KArrow
  | KVar

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
    public typeVars: Iterable<TVar>,
    public constraints: Iterable<Constraint>,
    public type: Type,
  ) {
    super();
  }

}

export type Scheme
  = Forall

type NodeWithReference
  = Identifier
  | IdentifierAlt
  | ExprOperator
  | ReferenceExpression
  | ReferenceTypeExpression

export class TypeEnv {

  private mapping = new MultiMap<string, [Symkind, Scheme]>();

  public constructor(public parent: TypeEnv | null = null) {

  }

  public add(name: string, scheme: Scheme, kind: Symkind): void {
    this.mapping.add(name, [kind, scheme]);
  }

  public get(name: string, expectedKind: Symkind): Scheme | null {
    for (const [kind, scheme] of this.mapping.get(name)) {
      if (kind & expectedKind) {
        return scheme;
      }
    }
    return null;
  }

}

class KindEnv {

  private mapping = new Map<string, Kind>();

  public constructor(public parent: KindEnv | null = null) {

  }

  public get(name: string): Kind | null {
    return this.mapping.get(name) ?? null;
  }

  public set(name: string, kind: Kind): void {
    assert(!this.mapping.has(name));
    this.mapping.set(name, kind);
  }

  public lookup(name: string): Kind | null {
    let curr: KindEnv | null = this;
    do {
      const kind = curr.mapping.get(name);
      if (kind !== undefined) {
        return kind;
      }
      curr = curr.parent;
    } while (curr !== null);
    return null;
  }

}

function splitReferences(node: NodeWithReference): [IdentifierAlt[], Identifier | IdentifierAlt | ExprOperator] {
  let modulePath: IdentifierAlt[];
  let name: Identifier | IdentifierAlt | ExprOperator;
  if (node.kind === SyntaxKind.ReferenceExpression || node.kind === SyntaxKind.ReferenceTypeExpression) {
    modulePath = node.modulePath.map(([name, _dot]) => name);
    name = node.name;
  } else {
    modulePath = [];
    name = node;
  }
  return [modulePath, name]
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
  private nextKindVarId = 0;
  private nextConTypeId = 0;

  private stringType = new TCon(this.nextConTypeId++, [], 'String');
  private intType = new TCon(this.nextConTypeId++, [], 'Int');
  private boolType = new TCon(this.nextConTypeId++, [], 'Bool');

  private contexts: InferContext[] = [];

  private solution = new TVSub();
  private kindSolution = new KVSub();

  public constructor(
    private analyser: Analyser,
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
    this.getContext().typeVars.add(typeVar);
    return typeVar;
  }

  public getContext(): InferContext {
    return this.contexts[this.contexts.length-1];
  }

  private addConstraint(constraint: Constraint): void {
    this.getContext().constraints.push(constraint);
  }

  private pushContext(context: InferContext) {
    this.contexts.push(context);
  }

  private popContext(context: InferContext) {
    assert(this.contexts[this.contexts.length-1] === context);
    this.contexts.pop();
  }

  private lookupKind(env: KindEnv, node: NodeWithReference): Kind | null {
    const [modulePath, name] = splitReferences(node);
    if (modulePath.length > 0) {
      let maxIndex = 0;
      let currUp = node.getEnclosingModule();
      outer: for (;;) {
        let currDown: SourceFile | ModuleDeclaration = currUp;
        for (let i = 0; i < modulePath.length; i++) {
          const moduleName = modulePath[i];
          const nextDown = currDown.resolveModule(moduleName.text);
          if (nextDown === null) {
            if (currUp.kind === SyntaxKind.SourceFile) {
              this.diagnostics.add(
                new ModuleNotFoundDiagnostic(
                  modulePath.slice(maxIndex).map(id => id.text),
                  modulePath[maxIndex],
                )
              );
              return null;
            }
            currUp = currUp.getEnclosingModule();
            continue outer;
          }
          maxIndex = Math.max(maxIndex, i+1);
          currDown = nextDown;
        }
        const found = currDown.kindEnv!.get(name.text);
        if (found !== null) {
          return found;
        }
        this.diagnostics.add(
          new BindingNotFoudDiagnostic(
            modulePath.map(id => id.text),
            name.text,
            name,
          )
        );
        return null;
      }
    } else {
      let curr: KindEnv | null = env;
      do {
        const found = curr.get(name.text);
        if (found !== null) {
          return found;
        }
        curr = curr.parent;
      } while(curr !== null);
      this.diagnostics.add(
        new BindingNotFoudDiagnostic(
          [],
          name.text,
          name,
        )
      );
      return null;
    }
  }

  private lookup(node: NodeWithReference, expectedKind: Symkind): Scheme | null {
    const [modulePath, name] = splitReferences(node);
    if (modulePath.length > 0) {
      let maxIndex = 0;
      let currUp = node.getEnclosingModule();
      outer: for (;;) {
        let currDown: SourceFile | ModuleDeclaration = currUp;
        for (let i = 0; i < modulePath.length; i++) {
          const moduleName = modulePath[i];
          const nextDown = currDown.resolveModule(moduleName.text);
          if (nextDown === null) {
            if (currUp.kind === SyntaxKind.SourceFile) {
              this.diagnostics.add(
                new ModuleNotFoundDiagnostic(
                  modulePath.slice(maxIndex).map(id => id.text),
                  modulePath[maxIndex],
                )
              );
              return null;
            }
            currUp = currUp.getEnclosingModule();
            continue outer;
          }
          maxIndex = Math.max(maxIndex, i+1);
          currDown = nextDown;
        }
        const found = currDown.typeEnv!.get(name.text, expectedKind);
        if (found !== null) {
          return found;
        }
        this.diagnostics.add(
          new BindingNotFoudDiagnostic(
            modulePath.map(id => id.text),
            name.text,
            name,
          )
        );
        return null;
      }
    } else {
      let curr: TypeEnv | null = this.getContext().env;
      do {
        const found = curr.get(name.text, expectedKind);
        if (found !== null) {
          return found;
        }
        curr = curr.parent;
      } while(curr !== null);
      this.diagnostics.add(
        new BindingNotFoudDiagnostic(
          [],
          name.text,
          name,
        )
      );
      return null;
    }
  }

  private getReturnType(): Type {
    const context = this.getContext();
    assert(context.returnType !== null);
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

  private addBinding(name: string, scheme: Scheme, kind: Symkind): void {
    this.getContext().env.add(name, scheme, kind);
  }

  private inferKindFromTypeExpression(node: TypeExpression, env: KindEnv): Kind {
    let kind: Kind;
    switch (node.kind) {
      case SyntaxKind.TupleTypeExpression:
      {
        for (const element of node.elements) {
          this.unifyKind(this.inferKindFromTypeExpression(element, env), new KStar(), node);
        }
        kind = new KStar();
        break;
      }
      case SyntaxKind.ArrowTypeExpression:
      {
        for (const param of node.paramTypeExprs) {
          this.unifyKind(this.inferKindFromTypeExpression(param, env), new KStar(), node);
        }
        this.unifyKind(this.inferKindFromTypeExpression(node.returnTypeExpr, env), new KStar(), node);
        kind = new KStar();
        break;
      }
      case SyntaxKind.ReferenceTypeExpression:
      {
        const matchedKind = this.lookupKind(env, node);
        if (matchedKind === null) {
          // this.diagnostics.add(new BindingNotFoudDiagnostic([], node.name.text, node.name));
          // Create a filler kind variable that still will be able to catch other errors.
          kind = this.createKindVar();
          kind.flags |= KindFlags.UnificationFailed;
        } else {
          kind = matchedKind;
        }
        break;
      }
      case SyntaxKind.VarTypeExpression:
      {
        const matchedKind = this.lookupKind(env, node.name);
        if (matchedKind === null) {
          // this.diagnostics.add(new BindingNotFoudDiagnostic([], node.name.text, node.name));
          // Create a filler kind variable that still will be able to catch other errors.
          kind = this.createKindVar();
          kind.flags |= KindFlags.UnificationFailed;
        } else {
          kind = matchedKind;
        }
        break;
      }
      case SyntaxKind.AppTypeExpression:
      {
        kind = this.inferKindFromTypeExpression(node.operator, env);
        for (const arg of node.args) {
          kind = this.applyKind(kind, this.inferKindFromTypeExpression(arg, env), node);
        }
        break;
      }
      case SyntaxKind.NestedTypeExpression:
      {
        kind = this.inferKindFromTypeExpression(node.typeExpr, env);
        break;
      }
      default:
        throw new Error(`Unexpected ${node}`);
    }
    node.inferredKind = kind;
    return kind;
  }

  private createKindVar(): KVar {
    return new KVar(this.nextKindVarId++);
  }

  private applyKind(operator: Kind, arg: Kind, node: Syntax): Kind {
    switch (operator.type) {
      case KindType.Var:
      {
        const a1 = this.createKindVar();
        const a2 = this.createKindVar();
        const arrow = new KArrow(a1, a2);
        this.unifyKind(arrow, operator, node);
        this.unifyKind(a1, arg, node);
        return a2;
      }
      case KindType.Arrow:
      {
        // Unify the argument to the operator's argument kind and return
        // whatever the operator returns.
        this.unifyKind(operator.left, arg, node);
        return operator.right;
      }
      case KindType.Star:
      {
        this.diagnostics.add(
          new KindMismatchDiagnostic(
            operator,
            new KArrow(
              this.createKindVar(),
              this.createKindVar()
            ),
            node
          )
        );
        // Create a filler kind variable that still will be able to catch other errors.
        const kind = this.createKindVar();
        kind.flags |= KindFlags.UnificationFailed;
        return kind;
      }
    }
  }

  private forwardDeclareKind(node: Syntax, env: KindEnv): void {
    switch (node.kind) {
      case SyntaxKind.ModuleDeclaration:
      {
        const innerEnv = node.kindEnv = new KindEnv(env);
        for (const element of node.elements) {
          this.forwardDeclareKind(element, innerEnv);
        }
        break;
      }
      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.forwardDeclareKind(element, env);
        }
        break;
      }
      case SyntaxKind.TypeDeclaration:
      {
        const innerEnv = new KindEnv(env);
        let kind: Kind = new KStar();
        for (let i = node.varExps.length-1; i >= 0; i--) {
          const varExpr = node.varExps[i];
          const paramKind = this.createKindVar();
          innerEnv.set(varExpr.text, paramKind);
          kind = new KArrow(paramKind, kind);
        }
        env.set(node.name.text, this.inferKindFromTypeExpression(node.typeExpression, innerEnv));
        break;
      }
      case SyntaxKind.StructDeclaration:
      {
        env.set(node.name.text, this.createKindVar());
        break;
      }
      case SyntaxKind.EnumDeclaration:
      {
        env.set(node.name.text, this.createKindVar());
        if (node.members !== null) {
          for (const member of node.members) {
            env.set(member.name.text, this.createKindVar());
          }
        }
        break;
      }
    }
  }
 
  private inferKind(node: Syntax, env: KindEnv): void {
    switch (node.kind) {
      case SyntaxKind.ModuleDeclaration:
      {
        const innerEnv = node.kindEnv!;
        for (const element of node.elements) {
          this.inferKind(element, innerEnv);
        }
        break;
      }
      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.inferKind(element, env);
        }
        break;
      }
      case SyntaxKind.StructDeclaration:
      {
        const declKind = env.lookup(node.name.text)!;
        const innerEnv = new KindEnv(env);
        let kind: Kind = new KStar();
        for (let i = node.varExps.length-1; i >= 0; i--) {
          const varExpr = node.varExps[i];
          const paramKind = this.createKindVar();
          innerEnv.set(varExpr.text, paramKind);
          kind = new KArrow(paramKind, kind);
        }
        this.unifyKind(declKind, kind, node);
        if (node.fields !== null) {
          for (const field of node.fields) {
            this.unifyKind(this.inferKindFromTypeExpression(field.typeExpr, innerEnv), new KStar(), field.typeExpr);
          }
        }
        break;
      }
      case SyntaxKind.EnumDeclaration:
      {
        const declKind = env.lookup(node.name.text)!;
        const innerEnv = new KindEnv(env);
        let kind: Kind = new KStar();
        // FIXME should I go from right to left or left to right?
        for (let i = node.varExps.length-1; i >= 0; i--) {
          const varExpr = node.varExps[i];
          const paramKind = this.createKindVar();
          innerEnv.set(varExpr.text, paramKind);
          kind = new KArrow(paramKind, kind);
        }
        this.unifyKind(declKind, kind, node);
        if (node.members !== null) {
          for (const member of node.members) {
            switch (member.kind) {
              case SyntaxKind.EnumDeclarationTupleElement:
              {
                for (const element of member.elements) {
                  this.unifyKind(this.inferKindFromTypeExpression(element, innerEnv), new KStar(), element);
                }
                break;
              }
              case SyntaxKind.EnumDeclarationStructElement:
              {
                for (const field of member.fields) {
                  this.unifyKind(this.inferKindFromTypeExpression(field.typeExpr, innerEnv), new KStar(), field.typeExpr);
                }
                break;
              }
              default:
                throw new Error(`Unexpected ${member}`);
            }
          }
        }
        break;
      }
      case SyntaxKind.LetDeclaration:
      {
        if (node.typeAssert !== null) {
          this.unifyKind(this.inferKindFromTypeExpression(node.typeAssert.typeExpression, env), new KStar(), node.typeAssert.typeExpression);
        }
        if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
          const innerEnv = new KindEnv(env);
          for (const element of node.body.elements) {
            this.inferKind(element, innerEnv);
          }
        }
        break;
      }
    }
  }

  private unifyKind(a: Kind, b: Kind, node: Syntax): boolean {

    const find = (kind: Kind): Kind => {
      let curr = kind;
      while (curr.type === KindType.Var && this.kindSolution.has(curr)) {
        curr = this.kindSolution.get(curr)!;
      }
      // if (kind.type === KindType.Var && ) {
      //   this.kindSolution.set(kind.id, curr);
      // }
      return curr;
    }

    const solve = (kind: Kind) => kind.substitute(this.kindSolution);

    a = find(a);
    b = find(b);

    if (a.type === KindType.Var) {
      this.kindSolution.set(a, b);
      return true;
    }

    if (b.type === KindType.Var) {
      return this.unifyKind(b, a, node);
    }

    if (a.type === KindType.Star && b.type === KindType.Star) {
      return true;
    }

    if (a.type === KindType.Arrow && b.type === KindType.Arrow) {
      return this.unifyKind(a.left, b.left, node)
          && this.unifyKind(a.right, b.right, node);
    }

    a.flags |= KindFlags.UnificationFailed;
    b.flags |= KindFlags.UnificationFailed;
    this.diagnostics.add(new KindMismatchDiagnostic(solve(a), solve(b), node));
    return false;
  }

  private infer(node: Syntax): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      case SyntaxKind.ModuleDeclaration:
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
          this.addBinding(node.pattern.operator.text, new Forall([], [], type), Symkind.Var);
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

      case SyntaxKind.MatchExpression:
      {
        let exprType;
        if (node.expression !== null) {
          exprType = this.inferExpression(node.expression);
        } else {
          exprType = this.createTypeVar();
        }
        let resultType: Type = this.createTypeVar();
        for (const arm of node.arms) {
          const context = this.getContext();
          const newEnv = new TypeEnv(context.env);
          const newContext: InferContext = {
            constraints: context.constraints,
            typeVars: context.typeVars,
            env: newEnv,
            returnType: context.returnType,
          };
          this.pushContext(newContext);
          this.addConstraint(
            new CEqual(
              this.inferBindings(arm.pattern, [], []),
              exprType,
              arm.pattern,
            )
          );
          this.addConstraint(
            new CEqual(
              resultType,
              this.inferExpression(arm.expression),
              arm.expression
            )
          );
          this.popContext(newContext);
        }
        if (node.expression === null) {
          resultType = new TArrow(exprType, resultType);
        }
        return resultType;
      }

      case SyntaxKind.TupleExpression:
        return new TTuple(node.elements.map(el => this.inferExpression(el)), node);

      case SyntaxKind.ReferenceExpression:
      {
        const scope = node.getScope();
        const target = scope.lookup(node.name.text);
        if (target !== null && target.kind === SyntaxKind.LetDeclaration && target.activeCycle) {
          return target.inferredType!;
        }
        const scheme = this.lookup(node, Symkind.Var);
        if (scheme === null) {
          // this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
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
            TArrow.build(paramTypes, retType),
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

      case SyntaxKind.StructExpression:
      {
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
              const scheme = this.lookup(member.name, Symkind.Var);
              let fieldType;
              if (scheme === null) {
                // this.diagnostics.add(new BindingNotFoudDiagnostic(member.name.text, member.name));
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
        return new TRecord(fields, node);
      }

      case SyntaxKind.InfixExpression:
      {
        const scheme = this.lookup(node.operator, Symkind.Var);
        if (scheme === null) {
          // this.diagnostics.add(new BindingNotFoudDiagnostic(node.operator.text, node.operator));
          return this.createTypeVar();
        }
        const opType = this.instantiate(scheme, node.operator);
        const retType = this.createTypeVar();
        const leftType = this.inferExpression(node.left);
        const rightType = this.inferExpression(node.right);
        this.addConstraint(
          new CEqual(
            new TArrow(leftType, new TArrow(rightType, retType)),
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

    let type;

    if (node.inferredKind!.hasFailed()) {

      type = this.createTypeVar();

    } else {

      switch (node.kind) {

        case SyntaxKind.ReferenceTypeExpression:
        {
          const scheme = this.lookup(node, Symkind.Type);
          if (scheme === null) {
            // this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
            return this.createTypeVar();
          }
          type = this.instantiate(scheme, node.name);
          // It is not guaranteed that `type` is copied during instantiation,
          // so the following check ensures that we really are holding a copy
          // that we can mutate.
          if (type === scheme.type) {
            type = type.shallowClone();
          }
          type.node = node;
          break;
        }

        case SyntaxKind.TupleTypeExpression:
        {
          type = new TTuple(node.elements.map(el => this.inferTypeExpression(el)), node);
          break;
        }

        case SyntaxKind.NestedTypeExpression:
          return this.inferTypeExpression(node.typeExpr, introduceTypeVars);

        case SyntaxKind.VarTypeExpression:
        {
          const scheme = this.lookup(node.name, Symkind.Type);
          if (scheme === null) {
            if (!introduceTypeVars) {
              // this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
            }
            type = this.createTypeVar();
            this.addBinding(node.name.text, new Forall([], [], type), Symkind.Type);
          } else {
            assert(isEmpty(scheme.typeVars));
            assert(isEmpty(scheme.constraints));
            type = scheme.type;
          }
          break;
        }

        case SyntaxKind.AppTypeExpression:
        {
          type = TApp.build(
            this.inferTypeExpression(node.operator, introduceTypeVars),
            node.args.map(arg => this.inferTypeExpression(arg, introduceTypeVars)),
          );
          break;
        }

        case SyntaxKind.ArrowTypeExpression:
        {
          const paramTypes = [];
          for (const paramTypeExpr of node.paramTypeExprs) {
            paramTypes.push(this.inferTypeExpression(paramTypeExpr, introduceTypeVars));
          }
          const returnType = this.inferTypeExpression(node.returnTypeExpr, introduceTypeVars);
          type = TArrow.build(paramTypes, returnType, node);
          break;
        }

        default:
          throw new Error(`Unrecognised ${node}`);

      }

    }

    node.inferredType = type;

    return type;

  }

  public inferBindings(pattern: Pattern, typeVars: Iterable<TVar>, constraints: Iterable<Constraint>): Type {

    switch (pattern.kind) {

      case SyntaxKind.BindPattern:
      {
        const type = this.createTypeVar();
        this.addBinding(pattern.name.text, new Forall(typeVars, constraints, type), Symkind.Var);
        return type;
      }

      case SyntaxKind.LiteralPattern:
      {
        let type;
        switch (pattern.token.kind) {
          case SyntaxKind.Integer:
            type = this.getIntType();
            break;
          case SyntaxKind.StringLiteral:
            type = this.getStringType();
            break;
        }
        type = type.shallowClone();
        type.node = pattern;
        return type;
      }

      case SyntaxKind.DisjunctivePattern:
      {
        const type = this.createTypeVar();
        this.addConstraint(
          new CEqual(
            this.inferBindings(pattern.left, typeVars, constraints),
            type,
            pattern.left
          )
        );
        this.addConstraint(
          new CEqual(
            this.inferBindings(pattern.right, typeVars, constraints),
            type,
            pattern.left
          )
        );
        return type;
      }

      case SyntaxKind.StructPattern:
      {
        const scheme = this.lookup(pattern.name, Symkind.Type);
        let recordType;
        if (scheme === null) {
          // this.diagnostics.add(new BindingNotFoudDiagnostic(pattern.name.text, pattern.name));
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
              this.addBinding(member.name.text, new Forall([], [], fieldType), Symkind.Var);
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

  private initialize(node: Syntax, parentEnv: TypeEnv): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      case SyntaxKind.ModuleDeclaration:
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
        const env = node.typeEnv = new TypeEnv(parentEnv);
        const constraints = new ConstraintSet();
        const typeVars = new TVSet();
        const context: InferContext = {
          typeVars,
          env,
          constraints,
          returnType: null,
        }
        this.pushContext(context);
        const kindArgs = [];
        for (const varExpr of node.varExps) {
          const kindArg = this.createTypeVar();
          env.add(varExpr.text, new Forall([], [], kindArg), Symkind.Type);
          kindArgs.push(kindArg);
        }
        let elementTypes: Type[] = [];
        const type = new TNominal(node, node);
        if (node.members !== null) {
          for (const member of node.members) {
            let elementType;
            switch (member.kind) {
              case SyntaxKind.EnumDeclarationTupleElement:
              {
                const argTypes = member.elements.map(el => this.inferTypeExpression(el));
                elementType = TArrow.build(argTypes, TApp.build(type, kindArgs), member);
                break;
              }
              case SyntaxKind.EnumDeclarationStructElement:
              {
                const fields = new Map();
                for (const field of member.fields) {
                  fields.set(field.name.text, this.inferTypeExpression(field.typeExpr));
                }
                elementType = new TArrow(new TRecord(fields, member), TApp.build(type, kindArgs));
                break;
              }
              default:
                throw new Error(`Unexpected ${member}`);
            }
            parentEnv.add(member.name.text, new Forall(typeVars, constraints, elementType), Symkind.Var);
            elementTypes.push(elementType);
          }
        }
        this.popContext(context);
        parentEnv.add(node.name.text, new Forall(typeVars, constraints, type), Symkind.Type);
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
        const kindArgs = [];
        for (const varExpr of node.varExps) {
          const typeVar = this.createTypeVar();
          kindArgs.push(typeVar);
          env.add(varExpr.text, new Forall([], [], typeVar), Symkind.Type);
        }
        const type = this.inferTypeExpression(node.typeExpression);
        this.popContext(context);
        const scheme = new Forall(typeVars, constraints, TApp.build(type, kindArgs));
        parentEnv.add(node.name.text, scheme, Symkind.Type); 
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
        const kindArgs = [];
        for (const varExpr of node.varExps) {
          const kindArg = this.createTypeVar();
          env.add(varExpr.text, new Forall([], [], kindArg), Symkind.Type);
          kindArgs.push(kindArg);
        }
        const fields = new Map<string, Type>();
        if (node.fields !== null) {
          for (const field of node.fields) {
            fields.set(field.name.text, this.inferTypeExpression(field.typeExpr));
          }
        }
        this.popContext(context);
        const type = new TNominal(node);
        parentEnv.add(node.name.text, new Forall(typeVars, constraints, type), Symkind.Type);
        parentEnv.add(node.name.text, new Forall(typeVars, constraints, new TArrow(new TRecord(fields, node), TApp.build(type, kindArgs))), Symkind.Var);
        break;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  public check(node: SourceFile): void {

    const kenv = new KindEnv();
    kenv.set('Int', new KStar());
    kenv.set('String', new KStar());
    kenv.set('Bool', new KStar());
    const skenv = new KindEnv(kenv);
    this.forwardDeclareKind(node, skenv);
    this.inferKind(node, skenv);

    const typeVars = new TVSet();
    const constraints = new ConstraintSet();
    const env = new TypeEnv();
    const context: InferContext = { typeVars, constraints, env, returnType: null };

    this.pushContext(context);

    const a = this.createTypeVar();
    const b = this.createTypeVar();
    const f = this.createTypeVar();

    env.add('$', new Forall([ f, a ], [], TArrow.build([ a, b, a ], b)), Symkind.Var);
    env.add('String', new Forall([], [], this.stringType), Symkind.Type);
    env.add('Int', new Forall([], [], this.intType), Symkind.Type);
    env.add('Bool', new Forall([], [], this.boolType), Symkind.Type);
    env.add('True', new Forall([], [], this.boolType), Symkind.Var);
    env.add('False', new Forall([], [], this.boolType), Symkind.Var);
    env.add('+', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    env.add('-', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    env.add('*', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    env.add('/', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    env.add('==', new Forall([ a ], [], TArrow.build([ a, a ], this.boolType)), Symkind.Var);
    env.add('not', new Forall([], [], new TArrow(this.boolType, this.boolType)), Symkind.Var);

    this.initialize(node, env);

    this.pushContext({
      typeVars,
      constraints,
      env: node.typeEnv!,
      returnType: null
    });

    const sccs = [...this.analyser.getSortedDeclarations()];

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

        let type = TArrow.build(paramTypes, returnType, node);
        if (node.typeAssert !== null) {
          this.addConstraint(
            new CEqual(
              this.inferTypeExpression(node.typeAssert.typeExpression),
              type,
              node
            )
          );
        }
        node.inferredType = type;

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
          this.addBinding(node.pattern.operator.text, new Forall([], [], ty2), Symkind.Var);
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
            && isFunctionDeclarationLike(element)) {
          if (!this.analyser.isReferencedInParentScope(element)) {
            assert(element.pattern.kind === SyntaxKind.BindPattern);
            const scheme = this.lookup(element.pattern.name, Symkind.Var);
            assert(scheme !== null);
            this.instantiate(scheme, null);
          }
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
        node.activeCycle = true;
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
        node.activeCycle = false;
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
          const unify = (left: Type, right: Type): boolean => {

            const find = (type: Type): Type => {
              while (type.kind === TypeKind.Var && solution.has(type)) {
                type = solution.get(type)!;
              }
              return type;
            }

            left = find(left);
            right = find(right);

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
              return unify(right, left);
            }

            if (left.kind === TypeKind.Arrow && right.kind === TypeKind.Arrow) {
              let success = true;
              if (!unify(left.paramType, right.paramType)) {
                success = false;
              }
              if (!unify(left.returnType, right.returnType)) {
                success = false;
              }
              if (success) {
                TypeBase.join(left, right);
              }
              return success;
            }

            if (left.kind === TypeKind.Tuple && right.kind === TypeKind.Tuple) {
              if (left.elementTypes.length === right.elementTypes.length) {
                let success = false;
                const count = left.elementTypes.length;
                for (let i = 0; i < count; i++) {
                  if (!unify(left.elementTypes[i], right.elementTypes[i])) {
                    success = false;
                  }
                }
                if (success) {
                  TypeBase.join(left, right);
                }
                return success;
              }
            }

            if (left.kind === TypeKind.Con && right.kind === TypeKind.Con) {
              if (left.id === right.id) {
                assert(left.argTypes.length === right.argTypes.length);
                const count = left.argTypes.length;
                let success = true; 
                for (let i = 0; i < count; i++) {
                  if (!unify(left.argTypes[i], right.argTypes[i])) {
                    success = false;
                  }
                }
                if (success) {
                  TypeBase.join(left, right);
                }
                return success;
              }
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
                  if (!unify(root.fields.get(fieldName)!, fieldType)) {
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

            if (left.kind === TypeKind.Nominal && right.kind === TypeKind.Nominal) {
              if (left.decl === right.decl) {
                return true;
              }
              this.diagnostics.add(new UnificationFailedDiagnostic(left, right, [...constraint.getNodes()]));
              return false;
            }

            if (left.kind === TypeKind.App && right.kind === TypeKind.App) {
              return unify(left.left, right.left)
                  && unify(left.right, right.right);
            }

            if (left.kind === TypeKind.Record && right.kind === TypeKind.Record) {
              let success = true;
              const remaining = new Set(right.fields.keys());
              for (const [fieldName, fieldType] of left.fields) {
                if (right.fields.has(fieldName)) {
                  if (!unify(fieldType, right.fields.get(fieldName)!)) {
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

            let leftElement: Type = left;
            while (leftElement.kind === TypeKind.App) {
              leftElement = leftElement.right;
            }
            let rightElement: Type = right;
            while (rightElement.kind === TypeKind.App) {
              rightElement = rightElement.right;
            }

            if (leftElement.kind === TypeKind.Record && right.kind === TypeKind.Labeled) {
              let success = true;
              if (right.fields === undefined) {
                right.fields = new Map([ [ right.name, right.type ] ]);
              }
              for (const [fieldName, fieldType] of right.fields) {
                if (leftElement.fields.has(fieldName)) {
                  if (!unify(fieldType, leftElement.fields.get(fieldName)!)) {
                    success = false;
                  }
                } else {
                  this.diagnostics.add(new FieldMissingDiagnostic(left, fieldName, constraint.node));
                }
              }
              if (success) {
                TypeBase.join(left, right);
              }
              return success;
            }

            if (left.kind === TypeKind.Labeled && right.kind === TypeKind.Record) {
              return unify(right, left);
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

          if (!unify(constraint.left, constraint.right)) {
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

}

