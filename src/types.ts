
import { FastStringMap } from "./util";

enum TypeKind {
  OpaqueType,
  AnyType,
  NeverType,
  FunctionType,
  RecordType,
  VariantType,
  TupleType,
}

export type Type
  = OpaqueType
  | AnyType
  | NeverType
  | FunctionType
  | RecordType
  | VariantType
  | TupleType

abstract class TypeBase {
  abstract kind: TypeKind;
}

export class OpaqueType extends TypeBase {
  kind: TypeKind.OpaqueType = TypeKind.OpaqueType;
}

export function isOpaqueType(value: any): value is OpaqueType {
  return value.kind === TypeKind.OpaqueType;
}

export class AnyType extends TypeBase {
  kind: TypeKind.AnyType = TypeKind.AnyType;
}

export function createAnyType(): AnyType {
  return new AnyType();
}

export function isAnyType(value: any): value is AnyType {
  return value.kind === TypeKind.AnyType;
}

export class NeverType extends TypeBase {
  kind: TypeKind.NeverType = TypeKind.NeverType;
}

export function isNeverType(value: any): value is NeverType {
  return value instanceof NeverType;
}

export class FunctionType extends TypeBase {

  kind: TypeKind.FunctionType = TypeKind.FunctionType;

  constructor(
    public paramTypes: Type[],
    public returnType: Type,
  ) {
    super();
  }

  public getParameterCount(): number {
    return this.paramTypes.length;
  }

  public getParamTypeAtIndex(index: number) {
    if (index < 0 || index >= this.paramTypes.length) {
      throw new Error(`Could not get the parameter type at index ${index} because the index  was out of bounds.`);
    }
    return this.paramTypes[index];
  }

}

export function isFunctionType(value: any): value is FunctionType {
  return value instanceof FunctionType;
}

export class VariantType extends TypeBase {

  kind: TypeKind.VariantType = TypeKind.VariantType;

  constructor(public elementTypes: Type[]) {
    super();
  }

  public getOwnElementTypes(): IterableIterator<Type> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export function isVariantType(value: any): value is VariantType {
  return value instanceof VariantType;
}

export class RecordType {

  kind: TypeKind.RecordType = TypeKind.RecordType;

  private fieldTypes = new FastStringMap<string, Type>();

  constructor(
    iterable: IterableIterator<[string, Type]>,
  ) {
    for (const [name, type] of iterable) {
      this.fieldTypes.set(name, type);
    }
  }

  public hasField(name: string) {
    return name in this.fieldTypes;
  }

  public getTypeOfField(name: string) {
    return this.fieldTypes.get(name);
  }

}

export function isRecordType(value: any): value is RecordType {
  return value.kind === TypeKind.RecordType;
}

export class TupleType extends TypeBase {

  kind: TypeKind.TupleType = TypeKind.TupleType;

  constructor(public elementTypes: Type[]) {
    super();
  }

}

export function intersectTypes(a: Type, b: Type): Type {
  if (isNeverType(a) || isNeverType(b)) {
    return new NeverType();
  }
  if (isAnyType(b)) {
    return a
  }
  if (isAnyType(a)) {
    return b;
  }
  if (isFunctionType(a) && isFunctionType(b)) {
    if (a.paramTypes.length !== b.paramTypes.length) {
      return new NeverType();
    }
    const returnType = intersectTypes(a.returnType, b.returnType);
    const paramTypes = a.paramTypes
      .map((_, i) => intersectTypes(a.paramTypes[i], b.paramTypes[i]));
    return new FunctionType(paramTypes, returnType)
  }
  return new NeverType();
}


export function isTypeAssignable(a: Type, b: Type): boolean {
  if (isNeverType(a)) {
    return false;
  }
  if (isAnyType(b)) {
    return true;
  }
  if (isOpaqueType(a) && isOpaqueType(b)) {
    return a === b;
  }
  if (a.kind !== b.kind) {
    return false;
  }
  if (isFunctionType(a) && isFunctionType(b)) {
    if (a.paramTypes.length !== b.paramTypes.length) {
      return false;
    }
    const paramCount = a.getParameterCount();
    for (let i = 0; i < paramCount; i++) {
      if (!isTypeAssignable(a.getParamTypeAtIndex(i), b.getParamTypeAtIndex(i))) {
        return false;
      }
    }
    return true;
  }
  throw new Error(`Should not get here.`);
}

