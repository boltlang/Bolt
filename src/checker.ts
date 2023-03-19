
// TODO support rigid vs free variables
//      https://www.reddit.com/r/haskell/comments/d4v83/comment/c0xmc3r/

import {
  ClassDeclaration,
  EnumDeclaration,
  Expression,
  ExprOperator,
  Identifier,
  IdentifierAlt,
  InstanceDeclaration,
  LetDeclaration,
  ModuleDeclaration,
  Pattern,
  ReferenceExpression,
  ReferenceTypeExpression,
  SourceFile,
  StructDeclaration,
  StructPattern,
  Syntax,
  SyntaxKind,
  TypeExpression,
} from "./cst";
import { Symkind } from "./scope"
import {
  describeType,
  BindingNotFoundDiagnostic,
  Diagnostics,
  FieldNotFoundDiagnostic,
  TypeMismatchDiagnostic,
  KindMismatchDiagnostic,
  ModuleNotFoundDiagnostic,
  TypeclassNotFoundDiagnostic,
  TypeclassDeclaredTwiceDiagnostic,
} from "./diagnostics";
import { assert, isDebug, assertNever, first, isEmpty, last, MultiMap, customInspectSymbol, InspectFn } from "./util";
import { Analyser } from "./analysis";
import { CustomInspectFunction, inspect, InspectOptions } from "util";

const MAX_TYPE_ERROR_COUNT = 5;

export enum TypeKind {
  Arrow,
  Var,
  Con,
  Tuple,
  App,
  Nominal,
  Field,
  Nil,
  Absent,
  Present,
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return describeType(this as any);
  }

}

class TVar extends TypeBase {

  public readonly kind = TypeKind.Var;

  public context = new Set<ClassDeclaration>();

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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return 'a' + this.id;
  }

}

export class TNil extends TypeBase {

  public readonly kind = TypeKind.Nil;

  public substitute(_sub: TVSub): Type {
    return this;
  }

  public shallowClone(): Type {
    return new TNil(this.node);
  }

  public *getTypeVars(): Iterable<TVar> {
    
  }

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return '{}'
  }

}

export class TAbsent extends TypeBase {

  public readonly kind = TypeKind.Absent;

  public substitute(_sub: TVSub): Type {
    return this;
  }

  public shallowClone(): Type {
    return new TAbsent(this.node);
  }

  public *getTypeVars(): Iterable<TVar> {
    
  }

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return 'Abs';
  }

}

export class TPresent extends TypeBase {

  public readonly kind = TypeKind.Present;

  public constructor(
    public type: Type,
    node: Syntax | null = null,
  ) {
    super(node);
  }

  public substitute(sub: TVSub): Type {
    return new TPresent(this.type.substitute(sub), this.node);
  }

  public getTypeVars(): Iterable<TVar> {
    return this.type.getTypeVars();
  }

  public shallowClone(): Type {
    return new TPresent(this.type, this.node);
  }

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return inspect(this.type);
  }

}

export class TArrow extends TypeBase {

  public readonly kind = TypeKind.Arrow;

  public constructor(
    public paramType: Type,
    public returnType: Type,
    node: Syntax | null = null,
  ) {
    super(node);
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return inspect(this.paramType) + ' -> ' + inspect(this.returnType);
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    let out = this.displayName;
    for (const argType of this.argTypes) {
      out += ' ' + inspect(argType);
    }
    return out;
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    let out = '(';
    let first = true;
    for (const elementType of this.elementTypes) {
      if (first) first = false;
      else out += ', ';
      out += inspect(elementType);
    }
    return out + ')';
  }

}

export class TField extends TypeBase {

  public readonly kind = TypeKind.Field;

  public constructor(
    public name: string,
    public type: Type,
    public restType: Type,
    public node: Syntax | null = null,
  ) {
    super(node);
  }

  public getTypeVars(): Iterable<TVar> {
    return this.type.getTypeVars();
  }

  public shallowClone(): TField {
    return new TField(
      this.name,
      this.type,
      this.restType,
      this.node,
    );
  }

  public static sort(type: Type): Type {
    const fields = new Map<string, TField>();
    while (type.kind === TypeKind.Field) {
      fields.set(type.name, type);
      type = type.restType;
    }
    const keys = [...fields.keys()].sort().reverse();
    let out: Type = type;
    for (const key of keys) {
      const field = fields.get(key)!;
      out = new TField(key, field.type, out, field.node);
    }
    return out
  }

  public substitute(sub: TVSub): Type {
    const newType = this.type.substitute(sub);
    const newRestType = this.restType.substitute(sub);
    return newType !== this.type || newRestType !== this.restType
      ? new TField(this.name, newType, newRestType, this.node) : this;
  }

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    let out = '{ ' + this.name + ': ' + inspect(this.type);
    let type = this.restType;
    while (type.kind === TypeKind.Field) {
      out += '; ' + type.name + ': ' + inspect(type.type);
      type = type.restType;
    }
    if (type.kind !== TypeKind.Nil) {
      out += '; ' + inspect(type);
    }
    return out + ' }'
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return inspect(this.left) + ' ' + inspect(this.right);
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return this.decl.name.text;
  }

}

export type Type
  = TCon
  | TArrow
  | TVar
  | TTuple
  | TApp
  | TNominal
  | TField
  | TNil
  | TPresent
  | TAbsent

export class Qual {

  public constructor(
    public preds: Pred[],
    public type: Type,
  ) {

  }

  public substitute(sub: TVSub): Qual {
    return new Qual(
      this.preds.map(pred => pred.substitute(sub)),
      this.type.substitute(sub),
    );
  }

  public *getTypeVars() {
    for (const pred of this.preds) {
      yield* pred.type.getTypeVars();
    }
    yield* this.type.getTypeVars();
  }

}

class IsInPred {

  public constructor(
    public id: string,
    public type: Type,
  ) {

  }

  public substitute(sub: TVSub): Pred {
    return new IsInPred(this.id, this.type.substitute(sub));

  }

}

type Pred = IsInPred;

export const enum KindType {
  Star,
  Arrow,
  Var,
  Row,
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

abstract class KindBase {

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

class KType extends KindBase {

  public readonly type = KindType.Star;

  public substitute(_sub: KVSub): Kind {
    return this;
  }

}

class KRow extends KindBase {

  public readonly type = KindType.Row;

  public substitute(_sub: KVSub): Kind {
    return this;
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

  public substitute(sub: KVSub): Kind {
    return new KArrow(
      this.left.substitute(sub),
      this.right.substitute(sub),
    );
  }

}

const kindOfTypes = new KType();
const kindOfRows = new KRow();

export type Kind
  = KType
  | KArrow
  | KVar
  | KRow

class TVSet {

  private mapping = new Map<number, TVar>();

  public constructor(iterable?: Iterable<TVar>) {
    if (iterable !== undefined) {
      for (const tv of iterable) {
        this.add(tv);
      }
    }
  }

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

  public get size(): number {
    return this.mapping.size;
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
  Class,
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

  public get lastNode(): Syntax | null {
    return last(this.getNodes()[Symbol.iterator]()) ?? null;
  }

  public get firstNode(): Syntax | null {
    return first(this.getNodes()[Symbol.iterator]()) ?? null;
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return `${inspect(this.left)} ~ ${inspect(this.right)}`;
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

  public [customInspectSymbol](depth: number, options: InspectOptions, inspect: InspectFn): string {
    return this.elements.map(el => inspect(el)).join('\n');
  }

}

type Constraint
  = CEqual
  | CMany

class ConstraintSet extends Array<Constraint> {
}

abstract class SchemeBase {
}

class Forall extends SchemeBase {

  public typeVars: TVSet;

  public constructor(
    typeVars: Iterable<TVar>,
    public constraints: Iterable<Constraint>,
    public type: Type,
  ) {
    super();
    this.typeVars = new TVSet();
    const allowed = new TVSet(type.getTypeVars());
    for (const tv of typeVars) {
      if (allowed.has(tv)) {
        this.typeVars.add(tv);
      }
    }
  }

  protected [customInspectSymbol](depth: number, inspectOptions: InspectOptions, inspect: InspectFn): string {
     let out = 'forall';
     if (this.typeVars.size > 0) {
       out += ' ' + [...this.typeVars].map(tv => inspect(tv)).join(' ');
     }
     out += '. ' + inspect(this.type);
     return out;
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

function validateScheme(scheme: Scheme): void {
  const isMonoVar = scheme.type.kind === TypeKind.Var && scheme.typeVars.size === 0;
  if (!isMonoVar) {
    const tvs = new TVSet(scheme.type.getTypeVars())
    for (const tv of tvs) {
      if (!scheme.typeVars.has(tv)) {
        throw new Error(`Type variable ${describeType(tv)} is free because does not appear in the scheme's type variable list`);
      }
    }
    for (const tv of scheme.typeVars) {
      if (!tvs.has(tv)) {
        throw new Error(`Polymorphic type variable ${describeType(tv)} does not occur anywhere in scheme's type ${describeType(scheme.type)}`);
      }
    }
  }
}

class TypeEnv {

  private mapping = new MultiMap<string, [Symkind, Scheme]>();

  public constructor(public parent: TypeEnv | null = null) {

  }

  public add(name: string, scheme: Scheme, kind: Symkind): void {
    if (isDebug) {
      validateScheme(scheme);
    }
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

export type { KindEnv, TypeEnv };

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
  return node.pattern.kind === SyntaxKind.NamedPattern
      && (node.params.length > 0 || (node.body !== null && node.body.kind === SyntaxKind.BlockBody));
}

export class Checker {

  private nextTypeVarId = 0;
  private nextKindVarId = 0;
  private nextConTypeId = 0;

  private stringType = this.createTCon([], 'String');
  private intType = this.createTCon([], 'Int');
  private boolType = this.createTCon([], 'Bool');

  private contexts: InferContext[] = [];

  private classDecls = new Map<string, ClassDeclaration>();
  private globalKindEnv = new KindEnv();
  private globalTypeEnv = new TypeEnv();

  private solution = new TVSub();
  private kindSolution = new KVSub();

  public constructor(
    private analyser: Analyser,
    private diagnostics: Diagnostics
  ) {

    this.globalKindEnv.set('Int', new KType());
    this.globalKindEnv.set('String', new KType());
    this.globalKindEnv.set('Bool', new KType());

    const a = new TVar(this.nextTypeVarId++);
    const b = new TVar(this.nextTypeVarId++);

    this.globalTypeEnv.add('$', new Forall([ a, b ], [], new TArrow(new TArrow(new TArrow(a, b), a), b)), Symkind.Var);
    this.globalTypeEnv.add('String', new Forall([], [], this.stringType), Symkind.Type);
    this.globalTypeEnv.add('Int', new Forall([], [], this.intType), Symkind.Type);
    this.globalTypeEnv.add('Bool', new Forall([], [], this.boolType), Symkind.Type);
    this.globalTypeEnv.add('True', new Forall([], [], this.boolType), Symkind.Var);
    this.globalTypeEnv.add('False', new Forall([], [], this.boolType), Symkind.Var);
    this.globalTypeEnv.add('+', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('-', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('*', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('/', new Forall([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('==', new Forall([ a ], [], TArrow.build([ a, a ], this.boolType)), Symkind.Var);
    this.globalTypeEnv.add('not', new Forall([], [], new TArrow(this.boolType, this.boolType)), Symkind.Var);

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

  private createTCon(types: Type[], name: string): TCon {
    return new TCon(this.nextConTypeId++, types, name);
  }

  private createTypeVar(node: Syntax | null = null): TVar {
    const typeVar = new TVar(this.nextTypeVarId++, node);
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

  private lookupKind(env: KindEnv, node: NodeWithReference, emitDiagnostic = true): Kind | null {
    const [modulePath, name] = splitReferences(node);
    if (modulePath.length > 0) {
      let maxIndex = 0;
      let currUp = node.getEnclosingModule();
      outer: for (;;) {
        let currDown = currUp;
        for (let i = 0; i < modulePath.length; i++) {
          const moduleName = modulePath[i];
          const nextDown = currDown.resolveModule(moduleName.text);
          if (nextDown === null) {
            if (currUp.kind === SyntaxKind.SourceFile) {
              if (emitDiagnostic) {
                this.diagnostics.add(
                  new ModuleNotFoundDiagnostic(
                    modulePath.slice(maxIndex).map(id => id.text),
                    modulePath[maxIndex],
                  )
                );
              }
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
        if (emitDiagnostic) {
          this.diagnostics.add(
            new BindingNotFoundDiagnostic(
              modulePath.map(id => id.text),
              name.text,
              name,
            )
          );
        }
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
      if (emitDiagnostic) {
        this.diagnostics.add(
          new BindingNotFoundDiagnostic(
            [],
            name.text,
            name,
          )
        );
      }
      return null;
    }
  }

  private lookup(node: NodeWithReference, expectedKind: Symkind): Scheme | null {
    const [modulePath, name] = splitReferences(node);
    if (modulePath.length > 0) {
      let maxIndex = 0;
      let currUp = node.getEnclosingModule();
      outer: for (;;) {
        let currDown = currUp;
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
          new BindingNotFoundDiagnostic(
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
        new BindingNotFoundDiagnostic(
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
    const tvs = [...scheme.typeVars]
    for (const tv of tvs) {
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

  private unifyKindMany(first: Kind, rest: Kind[], node: TypeExpression): boolean {
    return rest.every(kind => this.unifyKind(kind, first, node));
  }

  private inferKindFromTypeExpression(node: TypeExpression, env: KindEnv): Kind {

    // Store the resluting kind in this variable whenever we didn't encounter
    // any errors and wish to proceed with type inference on this node.
    let kind: Kind | undefined;

    switch (node.kind) {

      case SyntaxKind.TupleTypeExpression:
      {
        if (this.unifyKindMany(kindOfTypes, node.elements.map(el => this.inferKindFromTypeExpression(el, env)), node)) {
          kind = kindOfTypes;
        }
        break;
      }

      case SyntaxKind.ArrowTypeExpression:
      {
        if (node.paramTypeExprs.every(param => this.unifyKind(kindOfTypes, this.inferKindFromTypeExpression(param, env), node))
            && this.unifyKind(kindOfTypes, this.inferKindFromTypeExpression(node.returnTypeExpr, env), node)) {
          kind = kindOfTypes;
        }
        break;
      }

      case SyntaxKind.ReferenceTypeExpression:
      {
        const matchedKind = this.lookupKind(env, node);
        if (matchedKind !== null) {
          kind = matchedKind;
        }
        break;
      }

      case SyntaxKind.VarTypeExpression:
      {
        const matchedKind = this.lookupKind(env, node.name, false);
        // If no kind is associated to the type variable with the given name,
        // we can assign a fresh kind variable to the type variable. Next time,
        // the type variable will remember whatever unified with it in-between.
        if (matchedKind === null) {
          kind = this.createKindVar();
          env.set(node.name.text, kind);
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

    // We store the kind on the node so there is a one-to-one correspondence
    // and this way the kind can be refrieved very efficiently.
    // Note that at this point `kind` may be undefined. This signals further
    // inference logic that this node should be skipped because it already contains errors.
    node.inferredKind = kind;

    // Set a filler default for the node in a way that allows other unification
    // errors to be caught.
    if (kind === undefined) {
      kind = this.createKindVar();
    }

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
      default:
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
        return this.createKindVar();
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
        let kind: Kind = new KType();
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

      case SyntaxKind.ClassDeclaration:
      case SyntaxKind.InstanceDeclaration:
      {
        if (node.constraintClause !== null) {
          for (const constraint of node.constraintClause.constraints) {
            for (const typeExpr of constraint.types) {
              this.unifyKind(this.inferKindFromTypeExpression(typeExpr, env), new KType(), typeExpr);
            }
          }
        }
        for (const typeExpr of node.types) {
          this.unifyKind(this.inferKindFromTypeExpression(typeExpr, env), new KType(), typeExpr);
        }
        for (const element of node.elements) {
          this.inferKind(element, env);
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
        let kind: Kind = new KType();
        for (let i = node.varExps.length-1; i >= 0; i--) {
          const varExpr = node.varExps[i];
          const paramKind = this.createKindVar();
          innerEnv.set(varExpr.text, paramKind);
          kind = new KArrow(paramKind, kind);
        }
        this.unifyKind(declKind, kind, node);
        if (node.fields !== null) {
          for (const field of node.fields) {
            this.unifyKind(this.inferKindFromTypeExpression(field.typeExpr, innerEnv), new KType(), field.typeExpr);
          }
        }
        break;
      }

      case SyntaxKind.EnumDeclaration:
      {
        const declKind = env.lookup(node.name.text)!;
        const innerEnv = new KindEnv(env);
        let kind: Kind = new KType();
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
                  this.unifyKind(this.inferKindFromTypeExpression(element, innerEnv), new KType(), element);
                }
                break;
              }
              case SyntaxKind.EnumDeclarationStructElement:
              {
                for (const field of member.fields) {
                  this.unifyKind(this.inferKindFromTypeExpression(field.typeExpr, innerEnv), new KType(), field.typeExpr);
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
          this.unifyKind(this.inferKindFromTypeExpression(node.typeAssert.typeExpression, env), new KType(), node.typeAssert.typeExpression);
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

      case SyntaxKind.ClassDeclaration:
      {
        for (const element of node.elements) {
          this.infer(element);
        }
        break;
      }

      case SyntaxKind.InstanceDeclaration:
      {
        const cls = node.getScope().lookup(node.name.text, Symkind.Typeclass) as ClassDeclaration | null;
        if (cls === null) {
          this.diagnostics.add(new TypeclassNotFoundDiagnostic(node.name));
        }
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
          const newFieldType = this.createTypeVar(name);
          const newRestType = this.createTypeVar();
          this.addConstraint(
            new CEqual(
              type,
              new TField(name.text, new TPresent(newFieldType), newRestType, name),
              node,
            )
          );
          type = newFieldType;
        }
        return type;
      }

      case SyntaxKind.CallExpression:
      {
        const opType = this.inferExpression(node.func);
        const retType = this.createTypeVar(node);
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
        let type: Type = new TNil(node);
        for (const member of node.members) {
          switch (member.kind) {
            case SyntaxKind.StructExpressionField:
            {
              type = new TField(member.name.text, new TPresent(this.inferExpression(member.expression)), type, node);
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
              type = new TField(member.name.text, new TPresent(fieldType), type, node);
              break;
            }
            default:
              throw new Error(`Unexpected ${member}`);
          }
        }
        return TField.sort(type);
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

    if (!node.inferredKind) {

      type = this.createTypeVar();

    } else {

      switch (node.kind) {

        case SyntaxKind.ReferenceTypeExpression:
        {
          const scheme = this.lookup(node, Symkind.Type);
          if (scheme === null) {
            // this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
            type = this.createTypeVar();
          } else {
            type = this.instantiate(scheme, node.name);
            // It is not guaranteed that `type` is copied during instantiation,
            // so the following check ensures that we really are holding a copy
            // that we can mutate.
            if (type === scheme.type) {
              type = type.shallowClone();
            }
            type.node = node;
          }
          break;
        }

        case SyntaxKind.TupleTypeExpression:
        {
          type = new TTuple(node.elements.map(el => this.inferTypeExpression(el, introduceTypeVars)), node);
          break;
        }

        case SyntaxKind.NestedTypeExpression:
          type = this.inferTypeExpression(node.typeExpr, introduceTypeVars);
          break;

        case SyntaxKind.VarTypeExpression:
        {
          const scheme = this.lookup(node.name, Symkind.Type);
          if (scheme === null) {
            if (!introduceTypeVars) {
              this.diagnostics.add(new BindingNotFoundDiagnostic([], node.name.text, node.name));
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

      case SyntaxKind.NamedPattern:
      {
        const type = this.createTypeVar();
        this.addBinding(pattern.name.text, new Forall(typeVars, constraints, type), Symkind.Var);
        return type;
      }

      case SyntaxKind.NestedPattern:
        return this.inferBindings(pattern.pattern, typeVars, constraints);

      case SyntaxKind.NamedTuplePattern:
      {
        const scheme = this.lookup(pattern.name, Symkind.Type);
        if (scheme === null) {
          return this.createTypeVar();
        }
        let tupleType = pattern.elements.map(p => this.inferBindings(p, typeVars, constraints));
        // FIXME not tested
        return TApp.build(
          new TNominal(scheme.type.node as StructDeclaration | EnumDeclaration, pattern),
          tupleType
        );
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
        const variadicMember = getVariadicMember(pattern);
        let type: Type;
        if (variadicMember === null) {
          type = new TNil(pattern);
        } else if (variadicMember.pattern === null) {
          type = this.createTypeVar();
        } else {
          type = this.inferBindings(variadicMember.pattern, typeVars, constraints);
        }
        for (const member of pattern.members) {
          switch (member.kind) {
            case SyntaxKind.StructPatternField:
            {
              const fieldType = this.inferBindings(member.pattern, typeVars, constraints);
              type = new TField(member.name.text, new TPresent(fieldType), type, pattern);
              break;
            }
            case SyntaxKind.PunnedStructPatternField:
            {
              const fieldType = this.createTypeVar();
              this.addBinding(member.name.text, new Forall([], [], fieldType), Symkind.Var);
              type = new TField(member.name.text, new TPresent(fieldType), type, pattern);
              break;
            }
            case SyntaxKind.VariadicStructPatternElement:
              break;
            default:
              assertNever(member);
          }
        }
        return TField.sort(type);
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

      case SyntaxKind.ClassDeclaration:
      {
        const other = this.classDecls.get(node.name.text);
        if (other !== undefined) {
          this.diagnostics.add(new TypeclassDeclaredTwiceDiagnostic(node.name, other));
        } else {
          if (node.constraintClause !== null) {
            for (const constraint of node.constraintClause.constraints) {
              if (!this.classDecls.has(constraint.name.text)) {
                this.diagnostics.add(new TypeclassNotFoundDiagnostic(constraint.name));
              }
            }
          }
          this.classDecls.set(node.name.text, node);
        }
        const env = node.typeEnv = new TypeEnv(parentEnv);
        for (const tv of node.types) {
          assert(tv.kind === SyntaxKind.VarTypeExpression);
          env.add(tv.name.text, new Forall([], [], this.createTypeVar(tv)), Symkind.Type);
        }
        for (const element of node.elements) {
          this.initialize(element, env);
        }
        break;
      }

      case SyntaxKind.InstanceDeclaration:
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
        for (const name of node.varExps) {
          const kindArg = this.createTypeVar();
          env.add(name.text, new Forall([], [], kindArg), Symkind.Type);
          kindArgs.push(kindArg);
        }
        const type = TApp.build(new TNominal(node, node), kindArgs);
        parentEnv.add(node.name.text, new Forall(typeVars, constraints, type), Symkind.Type);
        let elementTypes: Type[] = [];
        if (node.members !== null) {
          for (const member of node.members) {
            let ctorType, elementType;
            switch (member.kind) {
              case SyntaxKind.EnumDeclarationTupleElement:
              {
                const argTypes = member.elements.map(el => this.inferTypeExpression(el, false));
                elementType = new TTuple(argTypes, member);
                ctorType = TArrow.build(argTypes, type, member);
                break;
              }
              case SyntaxKind.EnumDeclarationStructElement:
              {
                elementType = new TNil(member);
                for (const field of member.fields) {
                  elementType = new TField(field.name.text, new TPresent(this.inferTypeExpression(field.typeExpr, false)), elementType, member);
                }
                elementType = TField.sort(elementType);
                ctorType = new TArrow(elementType, type);
                break;
              }
              default:
                throw new Error(`Unexpected ${member}`);
            }
            // FIXME `typeVars` may contain too much irrelevant type variables
            parentEnv.add(member.name.text, new Forall(typeVars, constraints, ctorType), Symkind.Var);
            elementTypes.push(elementType);
          }
        }
        this.popContext(context);
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
        let type: Type = new TNil(node);
        if (node.fields !== null) {
          for (const field of node.fields) {
            type = new TField(field.name.text, new TPresent(this.inferTypeExpression(field.typeExpr)), type, node);
          }
        }
        this.popContext(context);
        parentEnv.add(node.name.text, new Forall(typeVars, constraints, TField.sort(type)), Symkind.Type);
        //parentEnv.add(node.name.text, new Forall(typeVars, constraints, new TArrow(type, TApp.build(type, kindArgs))), Symkind.Var);
        break;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  public check(node: SourceFile): void {

    const kenv = new KindEnv(this.globalKindEnv);
    this.forwardDeclareKind(node, kenv);
    this.inferKind(node, kenv);

    const typeVars = new TVSet();
    const constraints = new ConstraintSet();
    const env = new TypeEnv(this.globalTypeEnv);
    const context: InferContext = { typeVars, constraints, env, returnType: null };

    this.pushContext(context);

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
        const inner: InferContext = {
          typeVars,
          constraints,
          env,
          returnType: null,
        };
        node.context = inner;

        this.contexts.push(inner);

        const returnType = this.createTypeVar();
        inner.returnType = returnType;

        const paramTypes = node.params.map(
          param => this.inferBindings(param.pattern, [], [])
        );

        let type = TArrow.build(paramTypes, returnType, node);
        if (node.typeAssert !== null) {
          this.addConstraint(
            new CEqual(
              this.inferTypeExpression(node.typeAssert.typeExpression, true),
              type,
              node
            )
          );
        }
        node.inferredType = type;

        // if (node.parent!.kind === SyntaxKind.InstanceDeclaration) {
        //   const inst = node.parent!;
        //   const cls = inst.getScope().lookup(node.parent!.constraint.name.text, Symkind.Typeclass) as ClassDeclaration; 
        //   const other = cls.lookup(node)! as LetDeclaration;
        //   assert(other.pattern.kind === SyntaxKind.BindPattern);
        //   console.log(describeType(type));
        //   const otherScheme = this.lookup(other.pattern.name, Symkind.Var)!;
        //   addAll(otherScheme.typeVars, typeVars);
        //   constraints.push(...otherScheme.constraints);
        //   this.addConstraint(new CEqual(type, other.inferredType!, node));
        // }

        this.contexts.pop();

        if (node.parent!.kind !== SyntaxKind.InstanceDeclaration) {
          const scopeDecl = node.parent!.getScope().node;
          const outer = {
            typeVars: inner.typeVars,
            constraints: inner.constraints,
            env: scopeDecl.typeEnv!,
            returnType: null,
          };
          this.contexts.push(outer)
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

    }

    const visitElements = (elements: Syntax[]) => {
      for (const element of elements) {
        if (element.kind === SyntaxKind.LetDeclaration
            && isFunctionDeclarationLike(element)) {
          if (!this.analyser.isReferencedInParentScope(element)) {
            assert(element.pattern.kind === SyntaxKind.NamedPattern);
            const scheme = this.lookup(element.pattern.name, Symkind.Var);
            assert(scheme !== null);
            this.instantiate(scheme, null);
          }
        } else {
          const elementHasTypeEnv = hasTypeEnv(element);
          if (elementHasTypeEnv) {
            this.pushContext({ ...this.getContext(), env: element.typeEnv! });
          }
          this.infer(element);
          if(elementHasTypeEnv) {
            this.contexts.pop();
          }
        }
      }
    }

    for (const nodes of sccs) {

      if (nodes[0].kind === SyntaxKind.SourceFile) {
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

  private lookupClass(name: string): ClassDeclaration | null {
    return this.classDecls.get(name) ?? null;
  }

  private *findInstanceContext(type: TCon, clazz: ClassDeclaration): Iterable<ClassDeclaration[]> {
    for (const instance of clazz.getInstances()) {
      assert(instance.types.length === 1);
      const instTy0 = instance.types[0];
      if ((instTy0.kind === SyntaxKind.AppTypeExpression
          && instTy0.operator.kind === SyntaxKind.ReferenceTypeExpression
          && instTy0.operator.name.text === type.displayName)
         || (instTy0.kind === SyntaxKind.ReferenceTypeExpression
          && instTy0.name.text === type.displayName)) {
        if (instance.constraintClause === null) {
          return;
        }
        for (const argType of type.argTypes) {
          const classes = [];
          for (const constraint of instance.constraintClause.constraints) {
            assert(constraint.types.length === 1);
            const classDecl = this.lookupClass(constraint.name.text);
            if (classDecl === null) {
              this.diagnostics.add(new TypeclassNotFoundDiagnostic(constraint.name));
            } else {
              classes.push(classDecl);
            }
          }
          yield classes;
        }
      }
    }
  }

  private solve(constraint: Constraint, solution: TVSub): void {

    const queue = [ constraint ];

    let errorCount = 0;

    const find = (type: Type): Type => {
      while (type.kind === TypeKind.Var && solution.has(type)) {
        type = solution.get(type)!;
      }
      return type;
    }

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
          let path: string[] = [];

          const unifyField = (left: Type, right: Type): boolean => {

            const swap = () => { [right, left] = [left, right]; }

            if (left.kind === TypeKind.Absent && right.kind === TypeKind.Absent) {
              return true;
            }

            if (right.kind === TypeKind.Absent) {
              swap();
            }

            if (left.kind === TypeKind.Absent) {
              assert(right.kind === TypeKind.Present);
              const fieldName = path[path.length-1];
              this.diagnostics.add(
                new FieldNotFoundDiagnostic(fieldName, left.node, right.type.node, constraint.firstNode)
              );
              return false;
            }

            assert(left.kind === TypeKind.Present && right.kind === TypeKind.Present);
            return unify(left.type, right.type);
          }

          const unifyPred = (left: Pred, right: Pred) => {
            if (left.id === right.id) {
              return unify(left.type, right.type);
            }
            throw new Error(`Classes do not match and no diagnostic defined`);
          }

          const unify = (left: Type, right: Type): boolean => {

            left = find(left);
            right = find(right);

            //console.log(`unify ${describeType(left)} ~ ${describeType(right)}`);

            const swap = () => { [right, left] = [left, right]; }

            if (left.kind !== TypeKind.Var && right.kind === TypeKind.Var) {
              swap();
            }

            if (left.kind === TypeKind.Var) {

              // Perform an occurs check, verifying whether left occurs
              // somewhere inside the structure of right. If so, unification
              // makes no sense.
              if (right.hasTypeVar(left)) {
                // TODO print a diagnostic
                return false;
              }

              // We are ready to join the types, so the first thing we do is  
              // propagating the type classes that 'left' requires to 'right'.
              // If 'right' is another type variable, we're lucky. We just copy
              // the missing type classes from 'left' to 'right'. Otherwise,
              const propagateClasses = (classes: Iterable<ClassDeclaration>, type: Type) => {
                if (type.kind === TypeKind.Var) {
                  for (const constraint of classes) {
                    type.context.add(constraint);
                  }
                } else if (type.kind === TypeKind.Con) {
                  for (const constraint of classes) {
                    propagateClassTCon(constraint, type);
                  }
                } else {
                  //assert(false);
                  //this.diagnostics.add(new );
                }
              }

              const propagateClassTCon = (clazz: ClassDeclaration, type: TCon) => {
                const s = this.findInstanceContext(type, clazz);
                let i = 0;
                for (const classes of s) {
                  propagateClasses(classes, type.argTypes[i++]);
                }
              }

              propagateClasses(left.context, right);

              // We are all clear; set the actual type of left to right.
              solution.set(left, right);

              // These types will be join, and we'd like to track that
              // into a special chain.
              TypeBase.join(left, right);

              if (left.node !== undefined) {
                right.node = left.node;
              }

              return true;
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

            if (left.kind === TypeKind.Nil && right.kind === TypeKind.Nil) {
              return true;
            }

            if (left.kind === TypeKind.Field && right.kind === TypeKind.Field) {
              if (left.name === right.name) {
                let success = true;
                path.push(left.name);
                if (!unifyField(left.type, right.type)) {
                  success = false;
                }
                path.pop();
                if (!unify(left.restType, right.restType)) {
                  success = false;
                }
                return success;
              }
              let success = true;
              const newRestType = new TVar(this.nextTypeVarId++);
              if (!unify(left.restType, new TField(right.name, right.type, newRestType))) {
                success = false;
              }
              if (!unify(right.restType, new TField(left.name, left.type, newRestType))) {
                success = false;
              }
              return success;
            }

            if (left.kind === TypeKind.Nil && right.kind === TypeKind.Field) {
              swap();
            }

            if (left.kind === TypeKind.Field && right.kind === TypeKind.Nil) {
              let success = true;
              path.push(left.name);
              if (!unifyField(left.type, new TAbsent(right.node))) {
                success = false;
              }
              path.pop();
              if (!unify(left.restType, right)) {
                success = false;
              }
              return success
            }

            if (left.kind === TypeKind.Nominal && right.kind === TypeKind.Nominal) {
              if (left.decl === right.decl) {
                return true;
              }
              // fall through to error reporting
            }

            if (left.kind === TypeKind.App && right.kind === TypeKind.App) {
              return unify(left.left, right.left)
                  && unify(left.right, right.right);
            }

            this.diagnostics.add(
              new TypeMismatchDiagnostic(
                left.substitute(solution),
                right.substitute(solution),
                [...constraint.getNodes()],
                path,
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

function getVariadicMember(node: StructPattern) {
  for (const member of node.members) { 
    if (member.kind === SyntaxKind.VariadicStructPatternElement) {
      return member;
    }
  }
  return null;
}

type HasTypeEnv
  = ClassDeclaration
  | InstanceDeclaration
  | LetDeclaration
  | ModuleDeclaration
  | SourceFile

function hasTypeEnv(node: Syntax): node is HasTypeEnv {
  return node.kind === SyntaxKind.ClassDeclaration
      || node.kind === SyntaxKind.InstanceDeclaration
      || node.kind === SyntaxKind.LetDeclaration
      || node.kind === SyntaxKind.ModuleDeclaration
      || node.kind === SyntaxKind.SourceFile
}

