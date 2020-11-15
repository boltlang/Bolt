
import { BoltLiftedTypeExpression, SourceFile, Syntax, SyntaxKind } from "./ast";
import { FastStringMap } from "./util";

enum TypeKind {
  TypeVar,
  PrimType,
  ForallType,
  ArrowType,
}

type Type
  = TypeVar
  | PrimType
  | ForallType
  | ArrowType

interface TypeBase {
  kind: TypeKind;
}

interface TypeVar extends TypeBase {
  kind: TypeKind.TypeVar;
  varId: string;
}

interface PrimType extends TypeBase {
  kind: TypeKind.PrimType;
  primId: number;
  name: string;
}

class TypeVarSubstitution {

  private mapping = new FastStringMap<string, Type>()

  public add(source: TypeVar, target: Type) {
    this.mapping.set(source.varId, target);
  }

  public assign(other: TypeVarSubstitution): void {
    for (const [name, type] of other.mapping) {
      this.mapping.overwrite(name, type);
    }
  }

}

interface ForallType extends TypeBase {
  kind: TypeKind.ForallType;
  typeVars: TypeVarSubstitution;
  type: Type;
}

interface ArrowType extends TypeBase {
  kind: TypeKind.ArrowType;
  paramTypes: Type[];
  returnType: Type;
}

type Constraint = [Type, Type]

class TypeEnv {

  private mapping = new FastStringMap<string, Type>();

  public set(name: string, scheme: Type) {
    this.mapping.set(name, scheme)
  }

  public remove(name: string): void {
    this.mapping.delete(name)
  }

  public lookup(name: string): Type | null {
    if (!this.mapping.has(name)) {
      return null;
    }
    return this.mapping.get(name)
  }

  public has(typeVar: TypeVar): boolean {
    return this.mapping.has(typeVar.varId)
  }

}

export class TypeChecker {

  private nextVarId = 1;
  private nextPrimTypeId = 1;

  private intType = this.createPrimType('int');
  private stringType = this.createPrimType('String');

  public isIntType(type: Type) {
    return type === this.intType;
  }

  public isStringType(type: Type) {
    return type === this.stringType;
  }

  private builtinTypes = new FastStringMap([
    ['int', this.intType],
    ['String', this.stringType]
  ]);

  private createTypeVar(): TypeVar {
    return { kind: TypeKind.TypeVar, varId: (this.nextVarId++).toString() }
  }

  private createPrimType(name: string): PrimType {
    return { kind: TypeKind.PrimType, name, primId: this.nextPrimTypeId++ }
  }

  private createArrowType(paramTypes: Type[], returnType: Type): ArrowType {
    return { kind: TypeKind.ArrowType, paramTypes, returnType }
  }

  private createForallType(typeVars: FastStringMap<string, TypeVar>, type: Type): ForallType {
    return { kind: TypeKind.ForallType, typeVars, type }
  }

  public registerSourceFile(sourceFile: SourceFile): void {
    
  }

  private addFreeVariables(type: Type, freeVariables: Set<string>, excludedVariables: Set<string>) {

    switch (type.kind) {

      case TypeKind.PrimType:
        break;

      case TypeKind.TypeVar:
        if (!excludedVariables.has(type.varId)) {
          freeVariables.add(type.varId);
        }
        break;

      case TypeKind.ArrowType:
        for (const paramType of type.paramTypes) {
          this.addFreeVariables(paramType, freeVariables, excludedVariables);
        }
        this.addFreeVariables(type.returnType, freeVariables, excludedVariables);
        break;

      case TypeKind.ForallType:
        const newExcludedVariables = new Set(excludedVariables);
        for (const typeVar of type.typeVars.values()) {
          newExcludedVariables.add(typeVar.varId);
        }
        this.addFreeVariables(type.type, freeVariables, newExcludedVariables)
        break;

      default:
        throw new Error(`Could not determine the free variables in an unknown type`)

    }

  }

  private applySubstitution(type: Type, substitution: TypeVarSubstitution): Type {

    switch (type.kind) {

      case TypeKind.PrimType:
        return type;

      case TypeKind.TypeVar:
        if (substitution.has(type.varId)) {
          return substitution.get(type.varId)
        } else {
          return type;
        }

      case TypeKind.ArrowType:
        return this.createArrowType(
          type.paramTypes.map(t => this.applySubstitution(t, substitution)),
          this.applySubstitution(type.returnType, substitution)
        )

      case TypeKind.ForallType:
        const newSubstitution = new TypeVarSubstitution();
        for (const [name, mappedType] of substitution) {
          if (!type.typeVars.has(name)) {
            newSubstitution.set(name, mappedType);
          }
        }
        return this.createForallType(type.typeVars, this.applySubstitution(type.type, newSubstitution));

      default:
        throw new Error(`Could not substitute unrecognosed type`);

    }

  }

  private applySubstitutionToConstraints(constraints: Constraint[], substitution: TypeVarSubstitution): void {
    for (let i = 0; i < constraints.length; i++) {
      constraints[i][0] = this.applySubstitution(constraints[i][0], substitution)
      constraints[i][1] = this.applySubstitution(constraints[i][1], substitution)
    }
  }

  private inferNode(node: Syntax, env: TypeEnv) {

    switch (node.kind) {

      case SyntaxKind.BoltConstantExpression:
      {
        if (typeof(node.value) === 'bigint') {
          return this.intType;
        } else if (typeof(node.value === 'string')) {
          return this.stringType;
        }
      }

      case SyntaxKind.BoltReferenceExpression:
      {
        return this.createTypeVar()
      }

      case SyntaxKind.BoltCallExpression:
      {
        const operatorType = this.createTypeVar()
        const operandTypes = node.operands.map(this.createTypeVar.bind(this))
        return this.createArrowType(operandTypes, operatorType);
      }

    }

  }

  public checkNode(node: Syntax): void {

  }

  private solveConstraints(constraints: Constraint[]) {
    let substitution = new TypeVarSubstitution();
    while (true) {
      if (constraints.length === 0) {
        return substitution
      }
      const [a, b] = constraints.pop()!;
      const newSubstitution = this.unify(a, b);
      substitution.assign(newSubstitution);
      this.applySubstitutionToConstraints(constraints, newSubstitution);
    }
  }

  private areTypesEqual(a: Type, b: Type): boolean {
    if (a === b) { 
      return true;
    }
    if (a.kind !== b.kind) {
      return false;
    }
    if (a.kind === TypeKind.PrimType && b.kind === TypeKind.PrimType) {
      return a.primId === b.primId;
    }
    if (a.kind === TypeKind.ArrowType && b.kind === TypeKind.ArrowType) {
      if (a.paramTypes.length !== b.paramTypes.length
          || !this.areTypesEqual(a.returnType, b.returnType)) {
        return false;
      }
      for (let i = 0; i < a.paramTypes.length; i++) {
        if (!this.areTypesEqual(a.paramTypes[i], b.paramTypes[i])) {
          return false;
        }
      }
      return true;
    }
    if (a.kind === TypeKind.TypeVar && b.kind === TypeKind.TypeVar) {
      return a.varId === b.varId;
    }
    throw new Error(`Unexpected combination of types while checking equality`)
  }

  private unify(a: Type, b: Type): TypeVarSubstitution {
    if (this.areTypesEqual(a, b)) {
      return new TypeVarSubstitution();
    }
    if (a.kind === TypeKind.TypeVar) {
      const substitution = new TypeVarSubstitution();
      substitution.add(a, b);
      return substitution
    }
    if (b.kind === TypeKind.TypeVar) {
      const substitution = new TypeVarSubstitution();
      substitution.add(b, a);
      return substitution
    }
    throw new Error(`Types ${a} and ${b} could not be unified`)
  }

  public getTypeOfNode(node: Syntax, env: TypeEnv): unknown {
    return this.inferNode(node, env);
  }

  public isBuiltinType(name: string) {
    return this.builtinTypes.has(name);
  }

}
