import { InspectOptions } from "util";
import { ClassDeclaration, EnumDeclaration, StructDeclaration, Syntax } from "./cst";
import { InspectFn, assert, assertNever, toStringTag } from "./util";

export enum TypeKind {
  Arrow,
  UniVar,
  RigidVar,
  Con,
  Tuple,
  TupleIndex,
  App,
  Nominal,
  Field,
  Nil,
  Absent,
  Present,
}

export abstract class TypeBase {

  public abstract readonly kind: TypeKind;

  public parent: Type = this as unknown as Type;

  public next: Type = this as any;

  public abstract node: Syntax | null;

  public static join(a: Type, b: Type): void {
    const keep = a.next;
    a.next = b;
    b.next = keep;
  }

  public abstract getTypeVars(): Iterable<TVar>;

  public abstract shallowClone(): Type;

  public abstract substitute(sub: TVSub): Type;

  public find(): Type {
    let curr = this as unknown as Type;
    while (curr.parent !== curr) {
      curr.parent = curr.parent.parent;
      curr = curr.parent;
    }
    return curr;
  }

  public set(newType: Type): void {
    this.find().parent = newType;
  }

  public hasTypeVar(tv: TUniVar): boolean {
    for (const other of this.getTypeVars()) {
      if (tv.id === other.id) {
        return true;
      }
    }
    return false;
  }

  public abstract [toStringTag](depth: number, options: InspectOptions, inspect: InspectFn): string;

}

export function isType(value: any): value is Type {
  return value !== undefined
      && value !== null
      && value instanceof TypeBase;
}

export class TRigidVar extends TypeBase {

  public readonly kind = TypeKind.RigidVar;

  public constructor(
    public id: number,
    public displayName: string,
    public node: Syntax | null = null
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    yield this;
  }

  public shallowClone(): TRigidVar {
    return new TRigidVar(
      this.id,
      this.displayName,
      this.node
    );
  }

  public substitute(sub: TVSub): Type {
    const other = sub.get(this);
    return other === undefined
      ? this : other.substitute(sub);
  }

  public [toStringTag]() {
    return this.displayName;
  }

}

export class TUniVar extends TypeBase {

  public readonly kind = TypeKind.UniVar;

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

  public shallowClone(): TUniVar {
    return new TUniVar(this.id, this.node);
  }

  public substitute(sub: TVSub): Type {
    const other = sub.get(this);
    return other === undefined
      ? this : other.substitute(sub);
  }

  public [toStringTag]() {
    return 'a' + this.id;
  }

}

export class TNil extends TypeBase {

  public readonly kind = TypeKind.Nil;

  public constructor(
    public node: Syntax | null = null
  ) {
    super();
  }

  public substitute(_sub: TVSub): Type {
    return this;
  }

  public shallowClone(): Type {
    return new TNil(this.node);
  }

  public *getTypeVars(): Iterable<TVar> {
    
  }

  public [toStringTag]() {
    return '∂Abs';
  }

}

export class TAbsent extends TypeBase {

  public readonly kind = TypeKind.Absent;

  public constructor(
    public node: Syntax | null = null,
  ) {
    super();
  }

  public substitute(_sub: TVSub): Type {
    return this;
  }

  public shallowClone(): Type {
    return new TAbsent(this.node);
  }

  public *getTypeVars(): Iterable<TVar> {
    
  }

  public [toStringTag]() {
    return 'Abs';
  }

}

export class TPresent extends TypeBase {

  public readonly kind = TypeKind.Present;

  public constructor(
    public type: Type,
    public node: Syntax | null = null,
  ) {
    super();
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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    return 'Pre ' + inspect(this.type, options);
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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    return inspect(this.paramType, options) + ' -> ' + inspect(this.returnType, options);
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
    super();
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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    return this.displayName + ' ' + this.argTypes.map(t => inspect(t, options)).join(' ');
  }

}

export class TTupleIndex extends TypeBase {

  public readonly kind = TypeKind.TupleIndex;

  public constructor(
    public tupleType: Type,
    public index: number,
    public node: Syntax | null = null,
  ) {
    super();
  }

  public getTypeVars(): Iterable<TVar> {
    return this.tupleType.getTypeVars();
  }

  public substitute(sub: TVSub): Type {
    const newTupleType = this.tupleType.substitute(sub);
    if (newTupleType === this.tupleType) {
      return this;
    }
    return new TTupleIndex(newTupleType, this.index);
  }

  public shallowClone(): TTupleIndex {
    return new TTupleIndex(
      this.tupleType,
      this.index,
    );
  }

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn): string {
    return inspect(this.tupleType, options) + '.' + this.index;
  }

}

export class TTuple extends TypeBase {

  public readonly kind = TypeKind.Tuple;

  public constructor(
    public elementTypes: Type[],
    public node: Syntax | null = null,
  ) {
    super();
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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    return this.elementTypes.map(t => inspect(t, options)).join(' × ');
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
    super();
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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    let out = '{ ' + this.name + ': ' + inspect(this.type, options);
    let type = this.restType;
    while (type.kind === TypeKind.Field) {
      out += '; ' + type.name + ': ' + inspect(type.type, options);
      type = type.restType;
    }
    if (type.kind !== TypeKind.Nil) {
      out += '; ' + inspect(type, options);
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
    super();
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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    return inspect(this.left, options) + ' ' + inspect(this.right, options);
  }

}

export class TNominal extends TypeBase {

  public readonly kind = TypeKind.Nominal;

  public constructor(
    public decl: StructDeclaration | EnumDeclaration,
    public node: Syntax | null = null,
  ) {
    super();
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

  public [toStringTag]() {
    return this.decl.name.text;
  }

}

export type Type
  = TCon
  | TArrow
  | TRigidVar
  | TUniVar
  | TTuple
  | TApp
  | TNominal
  | TField
  | TNil
  | TPresent
  | TAbsent
  | TTupleIndex

export type TVar
  = TUniVar
  | TRigidVar


export function typesEqual(a: Type, b: Type): boolean {
  if (a.kind !== b.kind) {
    return false;
  }
  switch (a.kind) {
    case TypeKind.Con:
      assert(b.kind === TypeKind.Con);
      return a.id === b.id;
    case TypeKind.UniVar:
      assert(b.kind === TypeKind.UniVar);
      return a.id === b.id;
    case TypeKind.RigidVar:
      assert(b.kind === TypeKind.RigidVar);
      return a.id === b.id;
    case TypeKind.Nil:
    case TypeKind.Absent:
      return true;
    case TypeKind.Nominal:
      assert(b.kind === TypeKind.Nominal);
      return a.decl === b.decl;
    case TypeKind.App:
      assert(b.kind === TypeKind.App);
      return typesEqual(a.left, b.left) && typesEqual(a.right, b.right);
    case TypeKind.Field:
      assert(b.kind === TypeKind.Field);
      return a.name === b.name && typesEqual(a.type, b.type) && typesEqual(a.restType, b.restType);
    case TypeKind.Arrow:
      assert(b.kind === TypeKind.Arrow);
      return typesEqual(a.paramType, b.paramType) && typesEqual(a.returnType, b.returnType);
    case TypeKind.Tuple:
      assert(b.kind === TypeKind.Tuple);
      if (a.elementTypes.length !== b.elementTypes.length) {
        return false;
      }
      for (let i = 0; i < a.elementTypes.length; i++) {
        if (!typesEqual(a.elementTypes[i], b.elementTypes[i])) {
          return false;
        }
      }
      return true;
    case TypeKind.Present:
      assert(b.kind === TypeKind.Present);
      return typesEqual(a.type, b.type);
    case TypeKind.TupleIndex:
      assert(b.kind === TypeKind.TupleIndex);
      return a.index === b.index && typesEqual(a.tupleType, b.tupleType);
    default:
      assertNever(a);
  }
}

export class TVSet {

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

  public [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn) {
    let out = '{ ';
    let first = true;
    for (const tv of this) {
      if (first) first = false;
      else out += ', ';
      out += inspect(tv, options);
    }
    return out + ' }';
  }

}

export class TVSub {

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
