import {
  Expression,
  Pattern,
  Syntax,
  SyntaxKind,
  TypeExpression
} from "./cst";
import { BindingNotFoudDiagnostic, Diagnostics, UnificationFailedDiagnostic } from "./diagnostics";
import { assert } from "./util";

export enum TypeKind {
  Arrow,
  Var,
  Con,
  Any,
  Tuple,
}

abstract class TypeBase {

  public abstract readonly kind: TypeKind;

  public abstract getTypeVars(): Iterable<TVar>;

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
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    yield this;
  }

  public substitute(sub: TVSub): Type {
    return sub.get(this) ?? this;
  }

}

class TArrow extends TypeBase {

  public readonly kind = TypeKind.Arrow;

  public constructor(
    public paramTypes: Type[],
    public returnType: Type,
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const paramType of this.paramTypes) {
      yield* paramType.getTypeVars();
    }
    yield* this.returnType.getTypeVars();
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
    return changed ? new TArrow(newParamTypes, newReturnType) : this;
  }

}

class TCon extends TypeBase {

  public readonly kind = TypeKind.Con;

  public constructor(
    public id: number,
    public argTypes: Type[],
    public displayName: string,
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const argType of this.argTypes) {
      yield* argType.getTypeVars();
    }
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
    return changed ? new TCon(this.id, newArgTypes, this.displayName) : this;
  }

}

class TAny extends TypeBase {

  public readonly kind = TypeKind.Any;

  public *getTypeVars(): Iterable<TVar> {
    
  }

  public substitute(sub: TVSub): Type {
    return this;
  }

}

class TTuple extends TypeBase {

  public readonly kind = TypeKind.Tuple;

  public constructor(
    public elementTypes: Type[],
  ) {
    super();
  }

  public *getTypeVars(): Iterable<TVar> {
    for (const elementType of this.elementTypes) {
      yield* elementType.getTypeVars();
    }
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
    return changed ? new TTuple(newElementTypes) : this;
  }

}

export type Type
  = TCon
  | TArrow
  | TVar
  | TAny
  | TTuple


class TVSet {

  private mapping = new Map<number, TVar>();

  public add(tv: TVar): void {
    this.mapping.set(tv.id, tv);
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
}

abstract class ConstraintBase {

  public abstract substitute(sub: TVSub): Constraint;

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

class ConstraintSet extends Array<Constraint> {
}

abstract class SchemeBase {
}

class Forall extends SchemeBase {

  public constructor(
    public tvs: TVar[],
    public constraints: Constraint[],
    public type: Type,
  ) {
    super();
  }

}

type Scheme
  = Forall

class TypeEnv extends Map<string, Scheme> {
}

export interface InferContext {
  typeVars: TVSet;
  env: TypeEnv;
  constraints: ConstraintSet;
}

export class Checker {

  private nextTypeVarId = 0;
  private nextConTypeId = 0;

  private stringType = new TCon(this.nextConTypeId++, [], 'String');
  private intType = new TCon(this.nextConTypeId++, [], 'Int');
  private boolType = new TCon(this.nextConTypeId++, [], 'Bool');

  private typeEnvs: TypeEnv[] = [];
  private typeVars: TVSet[] = [];
  private constraints: ConstraintSet[] = [];
  private returnTypes: Type[] = [];

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
    this.typeVars[this.typeVars.length-1].add(typeVar);
    return typeVar;
  }

  private addConstraint(constraint: Constraint): void {
    this.constraints[this.constraints.length-1].push(constraint);
  }

  private pushContext(context: InferContext) {
    if (context.typeVars !== null) {
      this.typeVars.push(context.typeVars);
    }
    if (context.env !== null) {
      this.typeEnvs.push(context.env);
    }
    if (context.constraints !== null) {
      this.constraints.push(context.constraints);
    }
  }

  private popContext(context: InferContext) {
    if (context.typeVars !== null) {
      this.typeVars.pop();
    }
    if (context.env !== null) {
      this.typeEnvs.pop();
    }
    if (context.constraints !== null) {
      this.constraints.pop();
    }
  }

  private lookup(name: string): Scheme | null {
    for (let i = this.typeEnvs.length-1; i >= 0; i--) {
      const scheme = this.typeEnvs[i].get(name);
      if (scheme !== undefined) {
        return scheme;
      }
    }
    return null;
  }

  private getReturnType(): Type {
    assert(this.returnTypes.length > 0);
    return this.returnTypes[this.returnTypes.length-1];
  }

  private instantiate(scheme: Scheme): Type {
    const sub = new TVSub();
    for (const tv of scheme.tvs) {
      sub.set(tv, this.createTypeVar());
    }
    for (const constraint of scheme.constraints) {
      this.addConstraint(constraint.substitute(sub));
    }
    return scheme.type.substitute(sub);
  }

  private addBinding(name: string, scheme: Scheme): void {
    const env = this.typeEnvs[this.typeEnvs.length-1];
    env.set(name, scheme);
  }

  private forwardDeclare(node: Syntax): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.forwardDeclare(element);
        }
        break;
      }

      case SyntaxKind.ExpressionStatement:
      case SyntaxKind.ReturnStatement:
      {
        // TODO This should be updated if block-scoped expressions are allowed.
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        const typeVars = new TVSet();
        const env = new TypeEnv();
        const constraints = new ConstraintSet();
        const context = { typeVars, env, constraints };
        node.context = context;

        this.pushContext(context);

        let type;
        if (node.typeAssert !== null) {
          type = this.inferTypeExpression(node.typeAssert.typeExpression);
        } else {
          type = this.createTypeVar();
        }
        node.type = type;

        if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
          for (const element of node.body.elements) {
            this.forwardDeclare(element);
          }
        }

        this.popContext(context);

        break;
      }

    }
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
        // Get the type that was stored on the node by forwardDeclare()
        const type = node.type!;
        const context = node.context!;

        this.pushContext(context);

        const paramTypes = [];
        const returnType = this.createTypeVar();
        for (const param of node.params) {
          const paramType = this.createTypeVar()
          this.inferBindings(param.pattern, paramType, [], []);
          paramTypes.push(paramType);
        }

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
              this.returnTypes.push(returnType);
              for (const element of node.body.elements) {
                this.infer(element);
              }
              this.returnTypes.pop();
              break;
            }
          }
        }

        this.addConstraint(new CEqual(type, new TArrow(paramTypes, returnType), node));

        this.popContext(context);

        this.inferBindings(node.pattern, type, context.typeVars, context.constraints);

        // FIXME these two may need to go below inferBindings
        //this.typeVars.pop();
        //this.constraints.pop();


        break;

      }

      default:
        throw new Error(`Unexpected ${node}`);

    }

  }

  public inferExpression(node: Expression): Type {

    switch (node.kind) {

      case SyntaxKind.ReferenceExpression:
      {
        assert(node.name.modulePath.length === 0);
        const scheme = this.lookup(node.name.name.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.name.text, node.name.name));
          return new TAny();
        }
        return this.instantiate(scheme);
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
        return ty;
      }

      case SyntaxKind.NamedTupleExpression:
      {
        const scheme = this.lookup(node.name.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          return new TAny();
        }
        const type = this.instantiate(scheme);
        assert(type.kind === TypeKind.Con);
        const argTypes = [];
        for (const element of node.elements) {
          argTypes.push(this.inferExpression(element));
        }
        return new TCon(type.id, argTypes, type.displayName);
      }

      case SyntaxKind.InfixExpression:
      {
        const scheme = this.lookup(node.operator.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.operator.text, node.operator));
          return new TAny();
        }
        const opType = this.instantiate(scheme);
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
        throw new Error(`Unexpected ${node}`);

    }

  }

  public inferTypeExpression(node: TypeExpression): Type {

    switch (node.kind) {

      case SyntaxKind.ReferenceTypeExpression:
      {
        const scheme = this.lookup(node.name.text);
        if (scheme === null) {
          this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          return new TAny();
        }
        return this.instantiate(scheme);
      }

      default:
        throw new Error(`Unrecognised ${node}`);

    }

  }

  public inferBindings(pattern: Pattern, type: Type, tvs: TVar[], constraints: Constraint[]): void {

    switch (pattern.kind) {

      case SyntaxKind.BindPattern:
      {
        this.addBinding(pattern.name.text, new Forall(tvs, constraints, type));
        break;
      }

    }

  }

  public check(node: Syntax): void {
    const constraints = new ConstraintSet();
    const env = new TypeEnv();
    env.set('String', new Forall([], [], this.stringType));
    env.set('Int', new Forall([], [], this.intType));
    env.set('True', new Forall([], [], this.boolType));
    env.set('False', new Forall([], [], this.boolType));
    env.set('+', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.set('-', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.set('*', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    env.set('/', new Forall([], [], new TArrow([ this.intType, this.intType ], this.intType)));
    this.typeVars.push(new TVSet);
    this.constraints.push(constraints);
    this.typeEnvs.push(env);
    this.forwardDeclare(node);
    this.infer(node);
    this.solve(new CMany(constraints));
    this.typeVars.pop();
    this.constraints.pop();
    this.typeEnvs.pop();
  }

  private solve(constraint: Constraint): TVSub {

    const queue = [ constraint ];
    const solution = new TVSub();

    while (queue.length > 0) {

      const constraint = queue.pop()!;

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
          if (!this.unify(constraint.left, constraint.right, solution)) {
            this.diagnostics.add(
              new UnificationFailedDiagnostic(
                constraint.left.substitute(solution),
                constraint.right.substitute(solution),
                constraint.node
              )
            );
          }
          break;
        }
      }

    }

    return solution;

  }

  private unify(left: Type, right: Type, solution: TVSub): boolean {

    if (left.kind === TypeKind.Var && solution.has(left)) {
      left = solution.get(left)!;
    }
    if (right.kind === TypeKind.Var && solution.has(right)) {
      right = solution.get(right)!;
    }

    if (left.kind === TypeKind.Var) {
      if (right.hasTypeVar(left)) {
        // TODO occurs check diagnostic
      }
      solution.set(left, right);
      return true;
    }

    if (right.kind === TypeKind.Var) {
      return this.unify(right, left, solution);
    }

    if (left.kind === TypeKind.Arrow && right.kind === TypeKind.Arrow) {
      if (left.paramTypes.length !== right.paramTypes.length) {
        this.diagnostics.add(new ArityMismatchDiagnostic(left, right));
        return false;
      }
      let success = true;
      const count = left.paramTypes.length;
      for (let i = 0; i < count; i++) {
        if (!this.unify(left.paramTypes[i], right.paramTypes[i], solution)) {
          success = false;
        }
      }
      if (!this.unify(left.returnType, right.returnType, solution)) {
        success = false;
      }
      return success;
    }

    if (left.kind === TypeKind.Con && right.kind === TypeKind.Con) {
      if (left.id !== right.id) {
        return false;
      }
      assert(left.argTypes.length === right.argTypes.length);
      const count = left.argTypes.length;
      for (let i = 0; i < count; i++) {
        if (!this.unify(left.argTypes[i], right.argTypes[i], solution)) {
          return false;
        }
      }
      return true;
    }

    return false;
  }

}

