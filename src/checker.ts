import { Expression, Pattern, Syntax, SyntaxKind, TypeExpression } from "./cst";
import {BindingNotFoundDiagnostic, Diagnostic, Diagnostics, ParamCountMismatchDiagnostic, UnificationFailedDiagnostic} from "./diagnostics";
import {CustomMap, CustomSet} from "./util";

export enum TypeKind {
  TypeVar,
  ConType,
  ArrowType,
  BuiltinType,
  AnyType,
}

export type Type
  = ArrowType
  | TypeVar
  | BuiltinType
  | AnyType

const enum TypeFlags {

  /**
   * This is the default value for any new type being constructed.
   */
  None = 0,

  /**
   * A type that has been marked with this constant is assumed to have a
   * diagnostic associated with it and preferably does not generate new
   * diagnostics.
   */
  Fail = 1,

  /**
   * An opaque type never loses track of its associated node, not even when it
   * is a type variable that has itself substituted with something else.
   */
  Opaque = 4,
}

const enum BuiltinTypeID {
  Int,
  String,
  Bool,
}

const BUILTIN_TYPES: Array<[BuiltinTypeID, string]> = [
  [BuiltinTypeID.Int, 'Int'],
  [BuiltinTypeID.String, 'String'],
  [BuiltinTypeID.Bool, 'Bool'],
]

class TypeHandle {

  public constructor(
    public type: Type,
    public node: Syntax,
  ) {

  }

}

abstract class TypeBase {

  public abstract readonly kind: TypeKind;

  public constructor(
    public node: Syntax | null = null,
    public flags: TypeFlags = TypeFlags.None,
  ) {

  }

  public abstract shallowClone(): Type;

  public abstract substitute(mapping: TypeVarMap): Type;

  public abstract hasFreeVariable(typeVar: TypeVar): boolean;

}

export class TypeVar extends TypeBase {

  public readonly kind = TypeKind.TypeVar;

  private fullSubstitution: Type | null = null;
  private directSubstitution: Type | null = null;

  public constructor(
    public readonly id: number,
    public displayName: string,
    node: Syntax | null = null,
    flags: TypeFlags = TypeFlags.None,
  ) {
    super(node, flags);
  }

  public getFullySubstitutedType(): Type {
    let newType = this.fullSubstitution;
    while (newType !== null) {
      if (newType.kind !== TypeKind.TypeVar) {
        break;
      }
      newType = newType.fullSubstitution;
    }
    this.fullSubstitution = newType;
    return newType === null ? this : newType;
  }

  public hasSubstitution(): boolean {
    return this.directSubstitution !== null;
  }

  public setSubstitution(targetType: Type): void {
    if (this.directSubstitution !== null) {
      throw new Error(`A type variable can only be solved once.`);
    }
    this.directSubstitution = targetType;
    this.fullSubstitution = targetType;
  }

  public getSolved(): Type {
    return this.getFullySubstitutedType();
  }

  public substitute(mapping: TypeVarMap): Type {
    if (mapping.has(this)) {
      return mapping.get(this)!;
    }
    return this;
  }

  public hasFreeVariable(typeVar: TypeVar): boolean {
    return typeVar.id === this.id;
  }

  public format(): string {
    return this.displayName;
  }

  public shallowClone(): TypeVar {
    return new TypeVar(
      this.id,
      this.displayName,
      this.node,
      this.flags
    );
  }

}

export class BuiltinType extends TypeBase {

  public readonly kind = TypeKind.BuiltinType;

  public constructor(
    public id: BuiltinTypeID,
    node: Syntax | null = null,
    flags: TypeFlags = TypeFlags.None,
  ) {
    super(node, flags);
  }

  public substitute(mapping: TypeVarMap): BuiltinType {
    return this;
  }

  public hasFreeVariable(typeVar: TypeVar): boolean {
    return false;
  }

  public getSolved(): Type {
    return this;
  }

  public format(): string {
    switch (this.id) {
      case BuiltinTypeID.Int: return 'Int';
      case BuiltinTypeID.String: return 'String';
      case BuiltinTypeID.Bool: return 'Bool';
    }
  }

  public shallowClone(): BuiltinType {
    return new BuiltinType(
      this.id,
      this.node,
      this.flags,
    );
  }

}

export class ArrowType extends TypeBase {

  public readonly kind = TypeKind.ArrowType;

  public constructor(
    public paramTypes: Type[],
    public returnType: Type,
    node: Syntax | null = null,
    flags: TypeFlags = TypeFlags.None,
  ) {
    super(node, flags);
  }

  public substitute(mapping: TypeVarMap): ArrowType {
    return new ArrowType(
      this.paramTypes.map(t => t.substitute(mapping)),
      this.returnType.substitute(mapping),
      this.node,
      this.flags,
    );
  }

  public hasFreeVariable(typeVar: TypeVar): boolean {
    return this.paramTypes.some(t => t.hasFreeVariable(typeVar))
        || this.returnType.hasFreeVariable(typeVar);
  }

  public getSolved(): ArrowType {
    return new ArrowType(
      this.paramTypes.map(t => t.getSolved()),
      this.returnType.getSolved(),
      this.node,
      this.flags,
    );
  }

  public format(): string {
    return `${this.paramTypes.map(t => t.format()).join(' -> ')} -> ${this.returnType.format()}`;
  }

  public shallowClone(): ArrowType {
    return new ArrowType(
      this.paramTypes,
      this.returnType,
      this.node,
      this.flags,
    );
  }

}

export class AnyType extends TypeBase {

  public readonly kind = TypeKind.AnyType;

  public constructor(
    node: Syntax | null = null,
    flags: TypeFlags = TypeFlags.None,
  ) {
    super(node, flags);
  }

  public substitute(): AnyType {
    return this;
  }

  public hasFreeVariable(): boolean {
    return false;
  }

  public format(): string {
    return 'any';
  }

  public getSolved(): Type {
    return this;
  }

  public shallowClone(): AnyType {
    return new AnyType(
      this.node,
      this.flags,
    );
  }

}

class ConstraintBase {

}

export type Constraint
  = EqualityConstraint

class EqualityConstraint extends ConstraintBase {

  public constructor(
    public left: Type,
    public right: Type,
  ) {
    super();
  }

  public substitute(mapping: TypeVarMap): EqualityConstraint {
    return new EqualityConstraint(
      this.left.substitute(mapping),
      this.right.substitute(mapping),
    );
  }

}

class SchemeBase {

}

class TypeVarSet extends CustomSet<TypeVar, 'id'> {
  public constructor() {
    super('id');
  }
}

class TypeVarMap extends CustomMap<TypeVar, Type, 'id'> {
  public constructor() {
    super('id');
  }
}

export class ForallScheme extends SchemeBase {

  public constructor(
    public typeVars: TypeVarSet,
    public constraints: Constraint[],
    public type: Type,
  ) {
    super();
  }

}

export type Scheme
  = ForallScheme

export class TypingContext {

  public constraints: Constraint[] = [];

  private nextTypeVarId = 1;
  private typeVarPrefixCounts = Object.create(null);

  public constructor(
    public diagnostics: Diagnostics,
  ) {

  }

  public createTypeVar(
    displayName = 'a',
    node: Syntax | null = null,
    flags: TypeFlags = TypeFlags.None,
  ): TypeVar {
    if (!(displayName in this.typeVarPrefixCounts)) {
      this.typeVarPrefixCounts[displayName] = 2;
      return new TypeVar(this.nextTypeVarId++, displayName + '1', node, flags);
    }
    const count = this.typeVarPrefixCounts[displayName];
    this.typeVarPrefixCounts[displayName] = count + 1;
    return new TypeVar(this.nextTypeVarId++, displayName + count.toString(), node, flags);
  }

}

export class TypeEnv {

  private schemes: Record<string, Scheme> = Object.create(null);

  public constructor(
    public ctx: TypingContext,
    public parentEnv: TypeEnv | null = null,
  ) {

  }

  public addDefault(): void {
    for (const [id, name] of BUILTIN_TYPES) {
      this.addType(name, new BuiltinType(id));
    }
    const intType = new BuiltinType(BuiltinTypeID.Int)
    const boolType = new BuiltinType(BuiltinTypeID.Bool)
    this.addType('*', new ArrowType([ intType, intType ], intType));
    this.addType('/', new ArrowType([ intType, intType ], intType));
    this.addType('+', new ArrowType([ intType, intType ], intType));
    this.addType('-', new ArrowType([ intType, intType ], intType));
    this.addType('==', new ArrowType([ intType, intType ], boolType));
  }

  public addType(name: string, type: Type): void {
    this.schemes[name] = new ForallScheme(new TypeVarSet(), [], type);
  }

  public add(name: string, scheme: Scheme): void {
    this.schemes[name] = scheme;
  }

  public getScheme(name: string): Scheme | null {
    let currEnv: TypeEnv | null = this;
    do {
      if (name in currEnv.schemes) {
        return currEnv.schemes[name];
      }
      currEnv = currEnv.parentEnv;
    } while (currEnv !== null);
    return null;
  }

  public instantiate(scheme: Scheme): Type {
    const mapping = new TypeVarMap();
    for (const typeVar of scheme.typeVars) {
      // TODO Generate a type variable that has the same prefix
      const newTypeVar = this.ctx.createTypeVar(
        undefined,
        typeVar.node,
        typeVar.flags,
      );
      mapping.set(typeVar, newTypeVar);
    }
    for (const constraint of scheme.constraints) {
      this.ctx.constraints.push(constraint.substitute(mapping));
    }
    return scheme.type.substitute(mapping);
  }

  public lookup(name: string): Type | null {
    const scheme = this.getScheme(name);
    if (scheme === null) {
      return null;
    }
    return this.instantiate(scheme);
  }

}

export class TypeChecker {

  public constructor(
    public diagnostics: Diagnostics,
    private ctx: TypingContext,
  ) {

  }

  public forwardDeclare(node: Syntax, typeEnv: TypeEnv): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.forwardDeclare(element, typeEnv)
        }
        break;
      }

      case SyntaxKind.RecordDeclaration:
      {
        // TODO
        break;
      }

      case SyntaxKind.FunctionDefinition:
      {
        // Create a new type environment that represents the new scope of the
        // function.
        const innerEnv = new TypeEnv(typeEnv.ctx, typeEnv);

        // This set will contain the implicitly polymorphic parameters.
        const typeVars = new TypeVarSet();

        // Constraints need to be re-generated each time the type scheme is
        // instantiated. Therefore, we will need to store a new list of
        // constraints on the scheme itself. For now this list is empty.
        const constraints: Constraint[] = [];

        const paramTypes = [];
        if (node.typeSig !== null) {
          for (const [typeExpr, rarrowSign] of node.typeSig.paramTypes) {
            paramTypes.push(this.inferTypeExpr(typeExpr, typeEnv));
          }
        }
        for (let i = 0; i < node.params.length; i++) {
          const param = node.params[i];
          let paramType: Type = paramTypes[i] ;
          if (param.typeExpr !== null) {
            if (paramType === undefined) {
              paramTypes.push(paramType);
              paramType = this.inferTypeExpr(param.typeExpr, innerEnv);
            } else {
              constraints.push(new EqualityConstraint(paramType, this.inferTypeExpr(param.typeExpr, innerEnv)));
            }
          }
          if (paramType === undefined) {
            const typeVar = this.ctx.createTypeVar(undefined, param);
            typeVars.add(typeVar);
            paramType = typeVar;
            paramTypes.push(paramType);
          }
          this.inferBindings(param.pattern, paramType, innerEnv, constraints)
        }

        let returnType
        if (node.typeSig !== null) {
          returnType = this.inferTypeExpr(node.typeSig.returnType, innerEnv)
        } else {
          returnType = this.ctx.createTypeVar(undefined, node);
        }

        const type = new ArrowType(paramTypes, returnType, node);
        const scheme = new ForallScheme(typeVars, constraints, type);

        // Save the type and scoped type environment on the node so that
        // methods like this.infer() can make use of it.
        node.scheme = scheme;
        node.typeEnv = innerEnv;

        // Finally store whatever we were able to infer in the typing
        // environment.
        typeEnv.add(node.name.text, scheme);

        // Tail-recursive call where we iterate through the function's body,
        // looking for nested functions.
        if (node.body !== null) {
          this.forwardDeclare(node.body, innerEnv);
        }

        break;
      }

    }

  }

  public inferTypeExpr(node: TypeExpression, typeEnv: TypeEnv): Type {

    switch (node.kind) {

      case SyntaxKind.TypeReferenceExpression:
      {
        // TODO Take into account type arguments.
        // TODO Support fully qualified paths
        let type = typeEnv.lookup(node.name.name.text);
        if (type === null) {
          this.diagnostics.add(new BindingNotFoundDiagnostic(node.name.name));
          return new AnyType(node);
        }
        type = type.shallowClone()
        type.node = node;
        return type;
      }

      default:
        throw new Error(`Could not infer type constraints for TypeExpression: unhandled kind ${SyntaxKind[node.kind]}.`)

    }

  }

  public inferBindings(
    node: Pattern,
    valueType: Type,
    typeEnv: TypeEnv,
    constraints: Constraint[]
  ): void {

    switch (node.kind) {

      case SyntaxKind.BindPattern:
      {
        typeEnv.addType(node.name.text, valueType);
        break;
      }

      case SyntaxKind.ConstantExpression:
      {
        const type = this.inferExpr(node, typeEnv, constraints);
        constraints.push(new EqualityConstraint(type, valueType));
        break;
      }

      default:
        throw new Error(`Could not infer type constraints for Pattern: unhandled kind ${SyntaxKind[node.kind]}.`)

    }

  }

  public infer(
    node: Syntax,
    typeEnv: TypeEnv,
    constraints: Constraint[]
  ): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.infer(element, typeEnv, constraints);
        }
        break;
      }

      case SyntaxKind.CallExpression:
      case SyntaxKind.ConstantExpression:
      case SyntaxKind.MatchExpression:
      {
        this.inferExpr(node, typeEnv, constraints);
        break;
      }

      case SyntaxKind.RecordDeclaration:
      {
        // TODO
        break;
      }

      case SyntaxKind.InlineDefinitionBody:
      {
        this.inferExpr(node.expression, typeEnv, constraints);
        break;
      }

      case SyntaxKind.FunctionDefinition:
      {
        // Fetch the type and type environment that were calculated during a
        // traversal with this.forwardDeclare().
        const innerEnv = node.typeEnv!;
        const scheme = node.scheme!;
        const type = scheme.type as ArrowType;

        for (const param of node.params) {

          // Most of the parts have already been handled in
          // this.forwardDeclare() except for the default value.
          if (param.defaultValue !== null) {
            const defaultValueType = this.inferExpr(param.defaultValue[1], innerEnv, scheme.constraints);
            scheme.constraints.push(new EqualityConstraint(param.type!, defaultValueType));
          }

        }

        if (node.body !== null) {

          switch (node.body.kind) {

            case SyntaxKind.InlineDefinitionBody:
            {
              // Generate constraints but do not add them to the current constraint
              // list. Instead, add them to the scheme that represents this
              // definition.
              const resultType = this.inferExpr(node.body.expression, innerEnv, scheme.constraints);
              constraints.push(new EqualityConstraint(type.returnType, resultType));
              break;
            }

            case SyntaxKind.BlockDefinitionBody:
              // TODO
              break;

          }

        }

        break;
      }

      case SyntaxKind.VariableDefinition:
      {
        if (node.body !== null) {
          if (node.body.kind === SyntaxKind.InlineDefinitionBody) {
            const type = this.inferExpr(node.body.expression, typeEnv, constraints);
            this.inferBindings(node.pattern, type, typeEnv, constraints);
          } else if (node.body.kind === SyntaxKind.BlockDefinitionBody) {
            // TODO
          } else {
            // @ts-ignore TypeScript is too strict here
             throw new Error(`Unexpected node of kind ${SyntaxKind[node.body.kind]}.`);
          }
        }
        break;
      }

      default:
        throw new Error(`Could not infer type constraints for node: unhandled kind ${SyntaxKind[node.kind]}.`)

    }

  }

  public inferExpr(
    node: Expression,
    typeEnv: TypeEnv,
    constraints: Constraint[]
  ): Type {

    switch (node.kind) {

      case SyntaxKind.ConstantExpression:
      {
        switch (node.value.kind) {
          case SyntaxKind.DecimalInteger:
            return new BuiltinType(BuiltinTypeID.Int, node);
        }
      }

      case SyntaxKind.CallExpression:
      {
        const operatorType = this.inferExpr(node.operator, typeEnv, constraints);
        const argTypes = node.args.map(t => this.inferExpr(t, typeEnv, constraints));
        const returnType = this.ctx.createTypeVar(undefined, node);
        constraints.push(new EqualityConstraint(
          operatorType,
          new ArrowType(argTypes, returnType, node),
        ));
        return returnType;
      }

      case SyntaxKind.NestedExpression:
      {
        return this.inferExpr(node.expression, typeEnv, constraints);
      }

      case SyntaxKind.MatchExpression:
      {
        const valueType = this.inferExpr(node.expression, typeEnv, constraints)
        const resultType = this.ctx.createTypeVar(undefined, node);
        for (const arm of node.arms) {
          const innerEnv = new TypeEnv(typeEnv.ctx, typeEnv);
          this.inferBindings(arm.pattern, valueType, innerEnv, constraints);
          const armType = this.inferExpr(arm.expression, innerEnv, constraints)
          constraints.push(new EqualityConstraint(resultType, armType));
        }
        return resultType;
      }

      case SyntaxKind.BinaryExpression:
      {
        let operatorType = typeEnv.lookup(node.operator.getText());
        if (operatorType === null) {
          this.diagnostics.add(new BindingNotFoundDiagnostic(node.operator));
          return new AnyType(node);
        }
        operatorType = operatorType.shallowClone();
        operatorType.node = node;
        const argTypes = [
          this.inferExpr(node.lhs, typeEnv, constraints),
          this.inferExpr(node.rhs, typeEnv, constraints),
        ];
        const returnType = this.ctx.createTypeVar(undefined, node);
        constraints.push(new EqualityConstraint(
          new ArrowType(argTypes, returnType, node),
          operatorType
        ));
        return returnType;
      }

      case SyntaxKind.ReferenceExpression:
      {
        let type = typeEnv.lookup(node.name.text);
        if (type === null) {
          this.diagnostics.add(new BindingNotFoundDiagnostic(node.name));
          return new AnyType(node);
        }
        type = type.shallowClone();
        type.node = node;
        return type;
      }

      default:
        // @ts-ignore TypeScript is too strict here
        throw new Error(`Could not infer type constraints for Expression: unhandled kind ${SyntaxKind[node.kind]}.`)

    }

  }

  public solve(): void {
    for (const constraint of this.ctx.constraints) {
      if (constraint instanceof EqualityConstraint) {
        this.unifyEquality(constraint.left, constraint.right);
      }
    }
  }

  private failUnification(diagnostic: Diagnostic, deps: Type[]) {
    if (deps.some(t => t.flags & TypeFlags.Fail)) {
      return;
    }
    this.diagnostics.add(diagnostic);
    for (const type of deps) {
      type.flags |= TypeFlags.Fail;
    }
  }

  private unifyEquality(a: Type, b: Type): void {

    if (a.flags & TypeFlags.Fail || b.flags & TypeFlags.Fail) {
      return;
    }

    if (a === b) {
      return;
    }

    if (a.kind === TypeKind.TypeVar) {
      if (!a.hasSubstitution()) {
        if (b.hasFreeVariable(a)) {
          this.failUnification(new OccursCheckDiagnostic(a, b), [ a, b ]);
          return;
        }
        a.setSubstitution(b);
        // FIXME
        if (a.node !== null && (b.flags & TypeFlags.Opaque) === 0) {
          b.node = a.node;
        }
        return;
      }
      a = a.getFullySubstitutedType();
    }

    if (b.kind === TypeKind.TypeVar) {
      this.unifyEquality(b, a);
      return;
    }

    if (a.kind === TypeKind.AnyType || b.kind === TypeKind.AnyType) {
      return;
    }

    if (a.kind === TypeKind.BuiltinType && b.kind === TypeKind.BuiltinType) {
      if (a.id !== b.id) {
        this.failUnification(new UnificationFailedDiagnostic(a, b), [ a, b ]);
      }
      return;
    }

    if (a.kind === TypeKind.ArrowType && b.kind === TypeKind.ArrowType) {

      if (a.paramTypes.length !== b.paramTypes.length) {
        this.failUnification(new ParamCountMismatchDiagnostic(a, b), [ a, b ]);
      }

      for (let i = 0; i < Math.min(a.paramTypes.length, b.paramTypes.length); i++) {
        this.unifyEquality(a.paramTypes[i], b.paramTypes[i]);
      }

      this.unifyEquality(a.returnType, b.returnType);

      // if (a.flags & TypeFlags.Loose && (b.flags & TypeFlags.Loose) === 0) {
      //   a.node = b.node;
      // }
      // if ((a.flags & TypeFlags.Loose) === 0 && b.flags & TypeFlags.Loose) {
      //   b.node = a.node;
      // }
      return;
    }

    this.failUnification(new UnificationFailedDiagnostic(a, b), [ a, b ]);
  }

}

