
import {
  ClassDeclaration,
  Expression,
  ExprOperator,
  Identifier,
  IdentifierAlt,
  LetDeclaration,
  Pattern,
  ReferenceExpression,
  ReferenceTypeExpression,
  SourceFile,
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
  KindMismatchDiagnostic,
  ModuleNotFoundDiagnostic,
  TypeclassNotFoundDiagnostic,
  TypeclassDeclaredTwiceDiagnostic,
  FieldNotFoundDiagnostic,
  TypeMismatchDiagnostic,
} from "./diagnostics";
import { assert, assertNever, isEmpty, MultiMap, toStringTag, InspectFn } from "./util";
import { Analyser } from "./analysis";
import { InspectOptions } from "util";
import { TypeKind, TApp, TArrow, TCon, TField, TNil, TPresent, TRegularVar, TVSet, TVSub, Type, TypeBase, TAbsent, TRigidVar, TVar, buildTupleTypeWithLoc, buildTupleType, isTVar } from "./types";
import { CEmpty, CEqual, CMany, Constraint, ConstraintKind, ConstraintSet } from "./constraints";

// export class Qual {

//   public constructor(
//     public preds: Pred[],
//     public type: Type,
//   ) {

//   }

//   public substitute(sub: TVSub): Qual {
//     return new Qual(
//       this.preds.map(pred => pred.substitute(sub)),
//       this.type.substitute(sub),
//     );
//   }

//   public *getTypeVars() {
//     for (const pred of this.preds) {
//       yield* pred.type.getTypeVars();
//     }
//     yield* this.type.getTypeVars();
//   }

// }

// class IsInPred {

//   public constructor(
//     public id: string,
//     public type: Type,
//   ) {

//   }

//   public substitute(sub: TVSub): Pred {
//     return new IsInPred(this.id, this.type.substitute(sub));

//   }

// }

// type Pred = IsInPred;

export const enum KindType {
  Type,
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

  public readonly type = KindType.Type;

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

// TODO actually use these
const kindOfTypes = new KType();
//const kindOfTypes = new KCon('*');
//const kindOfRows = new KCon('r');
//const kindOfConstraints = new KCon();

export type Kind
  = KType
  | KArrow
  | KVar

abstract class SchemeBase {
}

class Forall extends SchemeBase {

  public constructor(
    public typeVars: TVSet,
    public constraint: Constraint,
    public type: Type,
  ) {
    super();
  }

  public *freeTypeVars(): Iterable<TVar> {
    for (const tv of this.constraint.freeTypeVars()) {
      if (!this.typeVars.has(tv)) {
        yield tv;
      }
    }
    for (const tv of this.type.getTypeVars()) {
      if (!this.typeVars.has(tv)) {
        yield tv;
      }
    }
  }

  protected [toStringTag](_depth: number, options: InspectOptions, inspect: InspectFn): string {
     let out = 'forall';
     if (this.typeVars.size > 0) {
       out += ' ' + [...this.typeVars].map(tv => inspect(tv, options)).join(' ');
     }
     out += '. ' + inspect(this.type, options);
     return out;
  }

  public static mono(type: Type): Forall {
    return new Forall(new TVSet, new CEmpty, type);
  }

  public static fromArrays(typeVars: TRegularVar[], constraints: Constraint[], type: Type): Forall {
    return new Forall(new TVSet(typeVars), new CMany(constraints), type);
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
  const isMonoVar = scheme.type.kind === TypeKind.RegularVar && scheme.typeVars.size === 0;
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

  public hasTypeVar(seek: TVar): boolean {
    for (const [_name, [_kind, scheme]] of this.mapping) {
      for (const tv of scheme.freeTypeVars()) {
        if (tv.id === seek.id) {
          return true;
        }
      }
    }
    return false;
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

class PolyContext {

  public constructor(
    public typeVars = new TVSet(),
    public constraints: ConstraintSet = [],
  ) {

  }

}

export interface TCInfo {
  inferredType?: Type;
  inferredKind?: Kind;
  poly?: PolyContext;
  kindEnv?: KindEnv;
  typeEnv?: TypeEnv;
  returnType?: Type | null;
}

function isSignatureDeclarationLike(node: LetDeclaration): boolean {
  return false; // May be foreignKeyword !== null later
}

function isVariableDeclarationLike(node: LetDeclaration): boolean {
  return node.pattern.kind !== SyntaxKind.NamedPattern || !node.body;
}

function isFunctionDeclarationLike(node: LetDeclaration): boolean {
  return node.parent!.kind === SyntaxKind.ClassDeclaration
      || (!isSignatureDeclarationLike(node) && !isVariableDeclarationLike(node));
  // return (node.pattern.kind === SyntaxKind.NamedPattern || node.pattern.kind === SyntaxKind.NestedPattern && node.pattern.pattern.kind === SyntaxKind.NamedPattern)
  //     && (node.params.length > 0 || (node.body !== null && node.body.kind === SyntaxKind.BlockBody));
}

function hasTypeVar(typeVars: TVSet, type: Type): boolean {
  for (const tv of type.getTypeVars()) {
    if (typeVars.has(tv)) {
      return true;
    }
  }
  return false;
}

export class Checker {

  private nextTypeVarId = 0;
  private nextKindVarId = 0;
  private nextConTypeId = 0;

  private stringType = this.createTCon('String');
  private intType = this.createTCon('Int');
  private boolType = this.createTCon('Bool');
  private unitType = buildTupleType([]);

  private classDecls = new Map<string, ClassDeclaration>();
  private globalKindEnv = new KindEnv();
  private globalTypeEnv = new TypeEnv();

  private typeSolution = new TVSub();
  private kindSolution = new KVSub();

  private typeEnvStack: TypeEnv[] = [];
  private polyContextStack: PolyContext[] = [];
  private returnTypeStack: (Type | null)[] = [];

  public constructor(
    private analyser: Analyser,
    private diagnostics: Diagnostics
  ) {

    this.globalKindEnv.set('Int', kindOfTypes);
    this.globalKindEnv.set('String', kindOfTypes);
    this.globalKindEnv.set('Bool', kindOfTypes);

    const a = new TRegularVar(this.nextTypeVarId++);
    const b = new TRegularVar(this.nextTypeVarId++);

    this.globalTypeEnv.add('$', Forall.fromArrays([ a, b ], [], new TArrow(new TArrow(new TArrow(a, b), a), b)), Symkind.Var);
    this.globalTypeEnv.add('String', Forall.fromArrays([], [], this.stringType), Symkind.Type);
    this.globalTypeEnv.add('Int', Forall.fromArrays([], [], this.intType), Symkind.Type);
    this.globalTypeEnv.add('Bool', Forall.fromArrays([], [], this.boolType), Symkind.Type);
    this.globalTypeEnv.add('True', Forall.fromArrays([], [], this.boolType), Symkind.Var);
    this.globalTypeEnv.add('False', Forall.fromArrays([], [], this.boolType), Symkind.Var);
    this.globalTypeEnv.add('+', Forall.fromArrays([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('-', Forall.fromArrays([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('*', Forall.fromArrays([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('/', Forall.fromArrays([], [], TArrow.build([ this.intType, this.intType ], this.intType)), Symkind.Var);
    this.globalTypeEnv.add('==', Forall.fromArrays([ a ], [], TArrow.build([ a, a ], this.boolType)), Symkind.Var);
    this.globalTypeEnv.add('not', Forall.fromArrays([], [], new TArrow(this.boolType, this.boolType)), Symkind.Var);

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

  private createTCon(name: string, node: Syntax | null = null): TCon {
    return new TCon(this.nextConTypeId++, name, node);
  }

  private getInfo(node: Syntax): TCInfo {
    return node as unknown as TCInfo;
  }

  private getPolyContext(): PolyContext {
    return this.polyContextStack[this.polyContextStack.length-1];
  }

  private pushInfo(info: TCInfo): void {
    if (info.poly !== undefined) {
      this.polyContextStack.push(info.poly);
    }
    if (info.returnType !== undefined) {
      this.returnTypeStack.push(info.returnType);
    }
    if (info.typeEnv !== undefined) {
      this.typeEnvStack.push(info.typeEnv);
    }
  }

  private popInfo(info: TCInfo): void {
    if (info.poly !== undefined) {
      this.polyContextStack.pop();
    }
    if (info.returnType !== undefined) {
      this.returnTypeStack.pop();
    }
    if (info.typeEnv !== undefined) {
      this.typeEnvStack.pop();
    }
  }

  public getReturnType(): Type {
    const ty = this.returnTypeStack[this.returnTypeStack.length-1];
    assert(ty !== null);
    return ty;
  }

  private getTypeEnv(): TypeEnv {
    return this.typeEnvStack[this.typeEnvStack.length-1];
  }

  private createTRegularVar(node: Syntax | null = null): TRegularVar {
    const typeVar = new TRegularVar(this.nextTypeVarId++, node);
    this.getPolyContext().typeVars.add(typeVar);
    return typeVar;
  }

  private createRigidVar(displayName: string, node: Syntax | null = null): TRigidVar {
    const tv = new TRigidVar(this.nextTypeVarId++, displayName, node);
    this.getPolyContext().typeVars.add(tv);
    return tv;
  }

  private addConstraint(constraint: Constraint): void {

    switch (constraint.kind) {

      case ConstraintKind.Empty:
        break;

      case ConstraintKind.Many:
        for  (const element of constraint.elements) {
          this.addConstraint(element);
        }
        break;

      case ConstraintKind.Equal:
      {
        const global = 0;

        let maxLevelLeft = global;
        for (let i = this.polyContextStack.length; i-- > 0;) {
          const ctx = this.polyContextStack[i];
          if (hasTypeVar(ctx.typeVars, constraint.left)) {
            maxLevelLeft = i;
            break;
          }
        }

        let maxLevelRight = global;
        for (let i = this.polyContextStack.length; i-- > 0;) {
          const ctx = this.polyContextStack[i];
          if (hasTypeVar(ctx.typeVars, constraint.right)) {
            maxLevelRight = i;
            break;
          }
        }

        const upperLevel = Math.max(maxLevelLeft, maxLevelRight);
        let lowerLevel = upperLevel;
        for (let i = 0; i < this.polyContextStack.length; i++) {
          const ctx = this.polyContextStack[i];
          if (hasTypeVar(ctx.typeVars, constraint.left) || hasTypeVar(ctx.typeVars, constraint.right)) {
            lowerLevel = i;
            break;
          }
        }

        if (upperLevel == lowerLevel || maxLevelLeft == global || maxLevelRight == global) {
          this.solve(constraint);
        } else {
          this.polyContextStack[upperLevel].constraints.push(constraint);
        }

        break;
      }

    }

  }

  private generalize(type: Type, constraints: Constraint[], env: TypeEnv): Scheme {
    const tvs = new TVSet();
    for (const tv of type.getTypeVars()) {
      if  (!env.hasTypeVar(tv)) {
        tvs.add(tv);
      }
    }
    for (const constraint of constraints) {
      for (const tv of constraint.freeTypeVars()) {
        if (!env.hasTypeVar(tv)) {
          tvs.add(tv);
        }
      }
    }
    return new Forall(tvs, new CMany(constraints), type);
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
        const currDownInfo = this.getInfo(currDown);
        const found = currDownInfo.kindEnv!.get(name.text);
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

  private lookup(node: NodeWithReference, expectedKind: Symkind, enableDiagnostics = true): Scheme | null {

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
              if (enableDiagnostics) {
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

        const currDownInfo = this.getInfo(currDown);

        const found = currDownInfo.typeEnv!.get(name.text, expectedKind);
        if (found !== null) {
          return found;
        }

        if (enableDiagnostics) {
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

      for (let i = this.typeEnvStack.length-1; i >= 0; i--) {
        const curr = this.typeEnvStack[i];
        const found = curr.get(name.text, expectedKind);
        if (found !== null) {
          return found;
        }
      }

      if (enableDiagnostics) {
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

  private createSubstitution(scheme: Scheme): TVSub {
    const sub = new TVSub();
    const tvs = [...scheme.typeVars]
    for (const tv of tvs) {
      sub.set(tv, this.createTRegularVar());
    }
    return sub;
  }

  private simplifyType(type: Type): Type {
    type = type.find();
    switch (type.kind) {
      case TypeKind.RegularVar:
      case TypeKind.RigidVar:
      case TypeKind.Nil:
      case TypeKind.Absent:
      case TypeKind.Con:
        return type;
      case TypeKind.App:
      {
        const left = type.left.find();
        const right = type.right.find();
        if (left === type.left && right === type.right) {
          return type;
        }
        return new TApp(left, right, type.node);
      }
      case TypeKind.Arrow:
      {
        const paramType = type.paramType.find();
        const returnType = type.returnType.find();
        if (paramType === type.paramType && returnType === type.returnType) {
          return type;
        }
        return new TArrow(paramType, returnType, type.node);
      }
      case TypeKind.Field:
      {
        const newType = type.type.find();
        const newRestType = type.restType.find();
        if (newType === type.type && newRestType === type.restType) {
          return type;
        }
        return new TField(type.name, newType, newRestType, type.node);
      }
      case TypeKind.Present:
      {
        const newType = type.type.find();
        if (newType === type.type) {
          return type;
        }
        return new TPresent(newType, type.node);
      }
    }
  }

  private instantiate(scheme: Scheme, node: Syntax | null, sub = this.createSubstitution(scheme)): Type {
    const transform = (constraint: Constraint): Constraint => {
      switch (constraint.kind) {
        case ConstraintKind.Many:
          const newConstraints: Constraint[] = [];
          for (const element of constraint.elements) {
            newConstraints.push(transform(element));
          }
          return new CMany(newConstraints);
        case ConstraintKind.Empty:
          return constraint;
        case ConstraintKind.Equal:
          constraint.left = this.simplifyType(constraint.left)
          constraint.right = this.simplifyType(constraint.right)
          const newConstraint = constraint.substitute(sub);
          newConstraint.node = node;
          newConstraint.prevInstantiation = constraint;
          return newConstraint;
        default:
          assertNever(constraint);
      }
    }
    this.addConstraint(transform(scheme.constraint));
    return this.simplifyType(scheme.type).substitute(sub);
  }

  private addBinding(name: string, scheme: Scheme, kind: Symkind): void {
    this.getTypeEnv().add(name, scheme, kind);
  }

  private unifyKindMany(first: Kind, rest: Kind[], node: TypeExpression): boolean {
    return rest.every(kind => this.unifyKind(kind, first, node));
  }

  private inferKindFromTypeExpression(node: TypeExpression, env: KindEnv): Kind {

    // Store the resluting kind in this variable whenever we didn't encounter
    // any errors and wish to proceed with type inference on this node.
    let kind: Kind | undefined;

    // Fetch the type checking information for this node because we're going to use it anyways.
    const info = this.getInfo(node);

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

      case SyntaxKind.ForallTypeExpression:
      {
        // TODO we currently automatically introduce type variables but maybe we should use the ForallTypeExpression?
        kind = this.inferKindFromTypeExpression(node.typeExpr, env);
        break;
      }

      case SyntaxKind.TypeExpressionWithConstraints:
      {
        // TODO check if we need to kind node.constraints
        kind = this.inferKindFromTypeExpression(node.typeExpr, env);
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
    info.inferredKind = kind;

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
        const info = this.getInfo(node);
        const innerEnv = info.kindEnv = new KindEnv(env);
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
        let kind: Kind = kindOfTypes;
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
        const info = this.getInfo(node);
        const innerEnv = info.kindEnv!;
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
              this.unifyKind(this.inferKindFromTypeExpression(typeExpr, env), kindOfTypes, typeExpr);
            }
          }
        }
        for (const typeExpr of node.types) {
          this.unifyKind(this.inferKindFromTypeExpression(typeExpr, env), kindOfTypes, typeExpr);
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
        let kind: Kind = kindOfTypes;
        for (let i = node.varExps.length-1; i >= 0; i--) {
          const varExpr = node.varExps[i];
          const paramKind = this.createKindVar();
          innerEnv.set(varExpr.text, paramKind);
          kind = new KArrow(paramKind, kind);
        }
        this.unifyKind(declKind, kind, node);
        if (node.fields !== null) {
          for (const field of node.fields) {
            this.unifyKind(this.inferKindFromTypeExpression(field.typeExpr, innerEnv), kindOfTypes, field.typeExpr);
          }
        }
        break;
      }

      case SyntaxKind.EnumDeclaration:
      {
        const declKind = env.lookup(node.name.text)!;
        const innerEnv = new KindEnv(env);
        let kind: Kind = kindOfTypes;
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
                  this.unifyKind(this.inferKindFromTypeExpression(element, innerEnv), kindOfTypes, element);
                }
                break;
              }
              case SyntaxKind.EnumDeclarationStructElement:
              {
                for (const field of member.fields) {
                  this.unifyKind(this.inferKindFromTypeExpression(field.typeExpr, innerEnv), kindOfTypes, field.typeExpr);
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
          this.unifyKind(this.inferKindFromTypeExpression(node.typeAssert.typeExpression, env), kindOfTypes, node.typeAssert.typeExpression);
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

    if (a.type === KindType.Type && b.type === KindType.Type) {
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
          type = this.unitType;
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
        const info = this.getInfo(node);

        if (isFunctionDeclarationLike(node)) {

          node.activeCycle = true;
          node.visited = true;

          this.pushInfo(info);

          const returnType = info.returnType!;

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
                for (const element of node.body.elements) {
                  this.infer(element);
                }
                break;
              }
            }
          }

          this.popInfo(info);

          node.activeCycle = false;

        } else {

          // const constraints = new ConstraintSet;
          // this.polyContextStack.push(new PolyContext(parentPoly.typeVars, constraints));

          let type;

          if (node.typeAssert !== null) {
            type = this.inferTypeExpression(node.typeAssert.typeExpression);
          }

          if (node.body !== null) {
            let bodyType;
            switch (node.body.kind) {
              case SyntaxKind.ExprBody:
              {
                bodyType = this.inferExpression(node.body.expression);
                break;
              }
              case SyntaxKind.BlockBody:
              {
                // TODO
                assert(false);
              }
            }
            if (type === undefined) {
              type = bodyType;
            } else {
              this.addConstraint(
                new CEqual(
                  type,
                  bodyType,
                  node.body
                )
              );
            }
          }

          if (type === undefined) {
            type = this.createTRegularVar();
          }

          // this.polyContextStack.pop();

          this.inferBindings(node.pattern, type, undefined, undefined, true);
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

    for (const annotation of node.annotations) {
      if (annotation.kind === SyntaxKind.TypeAnnotation) {
        this.inferTypeExpression(annotation.typeExpr, false, false);
      }
    }

    // We're going to use this eventually so might as well fetch it now
    const info = this.getInfo(node);

    let type: Type;

    switch (node.kind) {

      case SyntaxKind.NestedExpression:
        type = this.inferExpression(node.expression);
        break;

      case SyntaxKind.MatchExpression:
      {
        let exprType;

        if (node.expression !== null) {
          exprType = this.inferExpression(node.expression);
        } else {
          exprType = this.createTRegularVar();
        }

        type = this.createTRegularVar();

        for (const arm of node.arms) {

          const newEnv = new TypeEnv();
          this.typeEnvStack.push(newEnv);

          const armPatternType = this.createTRegularVar();

          this.inferBindings(arm.pattern, armPatternType);

          this.addConstraint(
            new CEqual(
              armPatternType,
              exprType,
              arm.pattern,
            )
          );

          this.addConstraint(
            new CEqual(
              type,
              this.inferExpression(arm.expression),
              arm.expression
            )
          );

          this.typeEnvStack.pop();
        }

        if (node.expression === null) {
          type = new TArrow(exprType, type);
        }

        break;
      }

      case SyntaxKind.TupleExpression:
        type = buildTupleTypeWithLoc(node.elements.map(el => [el, this.inferExpression(el)]), node);
        break;

      case SyntaxKind.ReferenceExpression:
      {
        const scope = node.getScope();
        const target = scope.lookup(node.name.text);
        if (target !== null && target.kind === SyntaxKind.LetDeclaration) {
          if (target.activeCycle) {
            return this.getInfo(target).inferredType!;
          }
          if (!target.visited) {
            this.infer(target);
          }
        }
        const scheme = this.lookup(node, Symkind.Var);
        if (scheme === null) {
          //this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
          type = this.createTRegularVar();
          break;
        }
        type = this.instantiate(scheme, node);
        type.node = node;
        break;
      }

      case SyntaxKind.MemberExpression:
      {
        type = this.inferExpression(node.expression);
        for (const [_dot, name] of node.path) {
          let label;
          switch (name.kind) {
            case SyntaxKind.Identifier:
              label = name.text;
              break;
            case SyntaxKind.Integer:
              label = Number(name.value);
              break;
            default:
              assertNever(name);
          }
          const newFieldType = this.createTRegularVar(name);
          const newRestType = this.createTRegularVar();
          this.addConstraint(
            new CEqual(
              type,
              new TField(label, new TPresent(newFieldType), newRestType, name),
              node,
            )
          );
          type = newFieldType;
        }
        break;
      }

      case SyntaxKind.CallExpression:
      {
        const opType = this.inferExpression(node.func);
        type = this.createTRegularVar(node);
        const paramTypes = [];
        for (const arg of node.args) {
          paramTypes.push(this.inferExpression(arg));
        }
        this.addConstraint(
          new CEqual(
            opType,
            TArrow.build(paramTypes, type),
            node
          )
        );
        break;
      }

      case SyntaxKind.ConstantExpression:
      {
        switch (node.token.kind) {
          case SyntaxKind.StringLiteral:
            type = this.getStringType();
            break;
          case SyntaxKind.Integer:
            type = this.getIntType();
            break;
        }
        type = type.shallowClone();
        type.node = node;
        break;
      }

      case SyntaxKind.StructExpression:
      {
        const fields = new Map<string, Type>();
        const restType = new TNil(node);
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
                fieldType = this.createTRegularVar();
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
        type = TField.build(fields, restType);
        break;
      }

      case SyntaxKind.InfixExpression:
      {
        const scheme = this.lookup(node.operator, Symkind.Var);
        if (scheme === null) {
          // this.diagnostics.add(new BindingNotFoudDiagnostic(node.operator.text, node.operator));
          return this.createTRegularVar();
        }
        const opType = this.instantiate(scheme, node.operator);
        const leftType = this.inferExpression(node.left);
        const rightType = this.inferExpression(node.right);
        type = this.createTRegularVar();
        this.addConstraint(
          new CEqual(
            new TArrow(leftType, new TArrow(rightType, type)),
            opType,
            node,
          ),
        );
        break;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

    info.inferredType = type;

    return type;

  }

  public inferTypeExpression(node: TypeExpression, introduceTypeVars = false, checkKind = true): Type {

    let type;

    const info = this.getInfo(node);

    if (checkKind && info.inferredKind === undefined) {

      type = this.createTRegularVar();

    } else {

      switch (node.kind) {

        case SyntaxKind.ReferenceTypeExpression:
        {
          const scheme = this.lookup(node, Symkind.Type);
          if (scheme === null) {
            // this.diagnostics.add(new BindingNotFoudDiagnostic(node.name.text, node.name));
            type = this.createTRegularVar();
            break;
          }
          type = this.instantiate(scheme, node.name);
          // It is not guaranteed that `type` is copied during instantiation,
          // so the following check ensures that we really are holding a copy
          // that we can mutate.
          if (type === scheme.type) {
            type = type.shallowClone();
          }
          // Mutate the type
          type.node = node;
          break;
        }

        case SyntaxKind.TupleTypeExpression:
        {
          type = buildTupleTypeWithLoc(node.elements.map(el => [el, this.inferTypeExpression(el, introduceTypeVars)]), node);
          break;
        }

        case SyntaxKind.NestedTypeExpression:
          type = this.inferTypeExpression(node.typeExpr, introduceTypeVars);
          break;

        case SyntaxKind.VarTypeExpression:
        {
          const scheme = this.lookup(node.name, Symkind.Type, !introduceTypeVars);
          if (scheme === null) {
            if (!introduceTypeVars) {
              this.diagnostics.add(new BindingNotFoundDiagnostic([], node.name.text, node.name));
            }
            type = this.createRigidVar(node.name.text, node);
            // TODO if !introduceTypeVars: re-emit a 'var not found' whenever the same var is encountered
            this.addBinding(node.name.text, Forall.mono(type), Symkind.Type);
          } else {
            assert(isEmpty(scheme.typeVars));
            assert(scheme.constraint.kind === ConstraintKind.Empty);
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

        case SyntaxKind.TypeExpressionWithConstraints:
        {
          // TODO
          // for (const constraint of node.constraints) {
          //   implementationLimitation(constraint.types.length === 1);
          //   this.addConstraint(new CClass(constraint.name.text, this.inferTypeExpression(constraint.types[0]), constraint.name));
          // }
          return this.inferTypeExpression(node.typeExpr, introduceTypeVars);
        }

        case SyntaxKind.ForallTypeExpression:
        {
          const env = this.getTypeEnv();
          const poly = this.getPolyContext();
          // FIXME this is an ugly hack that doesn't even work. Either disallow Forall in this method or create a new TForall
          for (const varExpr of node.varTypeExps) {
            const tv = this.createTRegularVar();
            env.add(varExpr.name.text, Forall.mono(tv), Symkind.Type);
            poly.typeVars.add(tv);
          }
          return this.inferTypeExpression(node.typeExpr, introduceTypeVars);
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

    info.inferredType = type;

    return type;

  }

  public inferBindings(pattern: Pattern, type: Type, typeVars = new TVSet, constraints: Constraint[] = [], generalize = false): void {

    switch (pattern.kind) {

      case SyntaxKind.NamedPattern:
      {
        let scheme;
        const env = this.getTypeEnv();
        if (generalize) {
          scheme = this.generalize(type, constraints, env);
        } else {
          scheme = new Forall(typeVars, new CMany(constraints), type);
        }
        this.addBinding(pattern.name.text, scheme, Symkind.Var);
        break;
      }

      case SyntaxKind.NestedPattern:
        this.inferBindings(pattern.pattern, type, typeVars, constraints, generalize);
        break;

      case SyntaxKind.NamedTuplePattern:
      {
        const scheme = this.lookup(pattern.name, Symkind.Var);
        if (scheme === null) {
          return;
        }
        const ctorType = this.instantiate(scheme, pattern);
        let elementTypes = [];
        for (const element of pattern.elements) {
          const tv = this.createTRegularVar();
          this.inferBindings(element, tv, typeVars, constraints, generalize);
          elementTypes.push(tv);
        }
        this.addConstraint(new CEqual(TArrow.build(elementTypes, type), ctorType, pattern));
        break;
      }

      case SyntaxKind.LiteralPattern:
      {
        let literalType;
        switch (pattern.token.kind) {
          case SyntaxKind.Integer:
            literalType = this.getIntType();
            break;
          case SyntaxKind.StringLiteral:
            literalType = this.getStringType();
            break;
        }
        literalType = literalType.shallowClone();
        literalType.node = pattern;
        this.addConstraint(
          new CEqual(
            literalType,
            type,
            pattern,
          )
        );
        break;
      }

      case SyntaxKind.DisjunctivePattern:
      {
        this.inferBindings(pattern.left, type, typeVars, constraints, generalize),
        this.inferBindings(pattern.right, type, typeVars, constraints, generalize);
        break;
      }

      case SyntaxKind.StructPattern:
      {
        const variadicMember = getVariadicMember(pattern);
        const fields = new Map<string, Type>();
        let restType: Type;
        if (variadicMember === null) {
          restType = new TNil(pattern);
        } else {
          restType = this.createTRegularVar();
          if (variadicMember.pattern !== null) {
            this.inferBindings(variadicMember.pattern, restType, typeVars, constraints, generalize);
          }
        }
        for (const member of pattern.members) {
          switch (member.kind) {
            case SyntaxKind.StructPatternField:
            {
              const fieldType = this.createTRegularVar();
              this.inferBindings(member.pattern, fieldType, typeVars, constraints, generalize);
              fields.set(member.name.text, fieldType);
              break;
            }
            case SyntaxKind.PunnedStructPatternField:
            {
              const fieldType = this.createTRegularVar();
              this.addBinding(member.name.text, Forall.mono(fieldType), Symkind.Var);
              fields.set(member.name.text, fieldType);
              break;
            }
            case SyntaxKind.VariadicStructPatternElement:
              break;
            default:
              assertNever(member);
          }
        }
        this.addConstraint(
          new CEqual(
            type,
            TField.build(fields, restType),
            pattern,
          )
        );
        break;
      }

      default:
        throw new Error(`Unexpected ${pattern.constructor.name}`);

    }

  }

  private initialize(node: Syntax): void {

    switch (node.kind) {

      case SyntaxKind.SourceFile:
      {
        const info = this.getInfo(node);
        const poly = info.poly = new PolyContext();
        const returnType = info.returnType = null;
        const env = info.typeEnv = new TypeEnv();

        this.polyContextStack.push(poly);
        this.typeEnvStack.push(env);
        this.returnTypeStack.push(returnType);

        for (const element of node.elements) {
          this.initialize(element);
        }

        this.polyContextStack.pop();
        this.typeEnvStack.pop();
        this.returnTypeStack.pop();

        break;
      }

      case SyntaxKind.ModuleDeclaration:
      {
        const info = this.getInfo(node);
        info.typeEnv = new TypeEnv();
        for (const element of node.elements) {
          this.initialize(element);
        }
        break;
      }

      case SyntaxKind.ClassDeclaration:
      {
        const info = this.getInfo(node);
        const env = info.typeEnv = new TypeEnv();
        for (const tv of node.types) {
          assert(tv.kind === SyntaxKind.VarTypeExpression);
          env.add(tv.name.text, Forall.mono(this.createTRegularVar(tv)), Symkind.Type);
        }
        for (const element of node.elements) {
          this.initialize(element);
        }
        break;
      }

      case SyntaxKind.InstanceDeclaration:
      {
        if (!this.classDecls.has(node.name.text)) {
          this.diagnostics.add(new TypeclassNotFoundDiagnostic(node.name.text, node.name));
        }
        const info = this.getInfo(node);
        info.typeEnv = new TypeEnv();
        for (const element of node.elements) {
          this.initialize(element);
        }
        break;
      }

      case SyntaxKind.LetDeclaration:
      {
        const info = this.getInfo(node);
        info.typeEnv = new TypeEnv();
        // The rest of the info properties are set in Checker.check()
        if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
          for (const element of node.body.elements) {
            this.initialize(element);
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
        const info = this.getInfo(node);
        const env = info.typeEnv = new TypeEnv();;
        const poly = info.poly = new PolyContext();
        const parentEnv = this.getTypeEnv();

        this.typeEnvStack.push(env);
        this.polyContextStack.push(poly);

        const typeArgs = [];
        for (const name of node.varExps) {
          const typeArg = this.createTRegularVar();
          env.add(name.text, Forall.mono(typeArg), Symkind.Type);
          typeArgs.push(typeArg);
        }

        const type = this.createTCon(node.name.text, node);
        const appliedType = TApp.build(type, typeArgs);
        parentEnv.add(node.name.text, new Forall(poly.typeVars, new CMany(poly.constraints), type), Symkind.Type);

        let elementTypes: Type[] = [];

        if (node.members !== null) {

          for (const member of node.members) {

            let ctorType, elementType;

            switch (member.kind) {

              case SyntaxKind.EnumDeclarationTupleElement:
              {
                const args: Array<[Syntax, Type]> = member.elements.map(el => [el, this.inferTypeExpression(el, false)]);
                elementType = buildTupleTypeWithLoc(args, member);
                ctorType = TArrow.build(args.map(a => a[1]), appliedType, member);
                break;
              }

              case SyntaxKind.EnumDeclarationStructElement:
              {
                const restType = new TNil(member);
                const fields = new Map<string, Type>();
                for (const field of member.fields) {
                  fields.set(field.name.text, this.inferTypeExpression(field.typeExpr, false));
                }
                elementType = TField.build(fields, restType);
                ctorType = new TArrow(elementType, appliedType, member);
                break;
              }

              default:
                throw new Error(`Unexpected ${member}`);

            }

            parentEnv.add(member.name.text, new Forall(poly.typeVars, new CMany(poly.constraints), ctorType), Symkind.Var);
            elementTypes.push(elementType);
          }

        }

        this.polyContextStack.pop();
        this.typeEnvStack.pop();

        break;
      }

      case SyntaxKind.TypeDeclaration:
      {
        const info = this.getInfo(node);
        const parentEnv = this.getTypeEnv();
        const env = info.typeEnv = new TypeEnv();;
        const poly = info.poly = new PolyContext();

        this.polyContextStack.push(poly);
        this.typeEnvStack.push(env);

        const typeArgs = [];

        for (const varExpr of node.varExps) {
          const typeVar = this.createTRegularVar();
          typeArgs.push(typeVar);
          env.add(varExpr.text, Forall.mono(typeVar), Symkind.Type);
        }

        const type = this.inferTypeExpression(node.typeExpression);

        this.polyContextStack.pop();
        this.typeEnvStack.pop();

        const scheme = new Forall(poly.typeVars, new CMany(poly.constraints), TApp.build(type, typeArgs));

        parentEnv.add(node.name.text, scheme, Symkind.Type); 

        break;
      }

      case SyntaxKind.StructDeclaration:
      {
        const info = this.getInfo(node);
        const parentEnv = this.getTypeEnv();
        const env = info.typeEnv = new TypeEnv();
        const poly = info.poly = new PolyContext();

        this.polyContextStack.push(poly);
        this.typeEnvStack.push(env);

        const typeArgs = [];
        for (const varExpr of node.varExps) {
          const typeArg = this.createTRegularVar();
          env.add(varExpr.text, Forall.mono(typeArg), Symkind.Type);
          typeArgs.push(typeArg);
        }

        const fields = new Map<string, Type>();
        const restType = new TNil(node);

        if (node.fields !== null) {
          for (const field of node.fields) {
            fields.set(field.name.text, this.inferTypeExpression(field.typeExpr));
          }
        }

        const type = this.createTCon(node.name.text, node.name);
        const recordType = TField.build(fields, restType);

        this.polyContextStack.pop();
        this.typeEnvStack.pop();

        parentEnv.add(node.name.text, new Forall(poly.typeVars, new CMany(poly.constraints), type), Symkind.Type);
        parentEnv.add(node.name.text, new Forall(poly.typeVars, new CMany(poly.constraints), new TArrow(recordType, type)), Symkind.Var);

        break;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  public check(sourceFile: SourceFile): void {

    // Kind inference
    const kindEnv = new KindEnv(this.globalKindEnv);
    this.forwardDeclareKind(sourceFile, kindEnv);
    this.inferKind(sourceFile, kindEnv);

    // Type inference

    this.typeEnvStack.push(this.globalTypeEnv);

    this.initialize(sourceFile);

    const sourceFileInfo = this.getInfo(sourceFile);
    this.pushInfo(sourceFileInfo);

    const sccs = [...this.analyser.getSortedDeclarations()];

    for (const nodes of sccs) {

      const poly = new PolyContext();

      this.polyContextStack.push(poly);

      for (const node of nodes) {

        if (!isFunctionDeclarationLike(node)) {
          continue;
        }

        const info = this.getInfo(node);
        info.poly = poly;
        const returnType = info.returnType = this.createTRegularVar();

        this.typeEnvStack.push(info.typeEnv!);
        this.returnTypeStack.push(info.returnType!);

        const paramTypes = node.params.map(param => {
          const paramType = this.createTRegularVar();
          this.inferBindings(param.pattern, paramType)
          return paramType;
        });

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

        info.inferredType = type;

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

        this.returnTypeStack.pop();
        this.typeEnvStack.pop();

        if (node.parent!.kind !== SyntaxKind.InstanceDeclaration) {
          this.inferBindings(node.pattern, type, poly.typeVars, poly.constraints);
        }

      }

      this.polyContextStack.pop();

    }

    this.infer(sourceFile);

    // Pop off whatever we pushed in during initialization
    this.popInfo(sourceFileInfo);
    this.typeEnvStack.pop();

    this.solve(new CMany(sourceFileInfo.poly!.constraints));

  }

  private path: (string | number)[] = [];
  private constraint: Constraint | null = null;
  private maxTypeErrorCount = 5;

  private find(type: Type): Type {
    while (type.kind === TypeKind.RegularVar && this.typeSolution.has(type)) {
      type = this.typeSolution.get(type)!;
    }
    return type;
  }

  private unifyField(left: Type, right: Type, enableDiagnostics: boolean): boolean {

    const swap = () => { [right, left] = [left, right]; }

    if (left.kind === TypeKind.Absent && right.kind === TypeKind.Absent) {
      return true;
    }

    if (right.kind === TypeKind.Absent) {
      swap();
    }

    if (left.kind === TypeKind.Absent) {
      assert(right.kind === TypeKind.Present);
      const fieldName = this.path[this.path.length-1];
      if (enableDiagnostics) {
        this.diagnostics.add(
          new FieldNotFoundDiagnostic(fieldName, left.node, right.type.node, this.constraint!.firstNode)
        );
      }
      return false;
    }

    assert(left.kind === TypeKind.Present && right.kind === TypeKind.Present);
    return this.unify(left.type, right.type, enableDiagnostics);
  }


  private unify(left: Type, right: Type, enableDiagnostics: boolean): boolean {

    //console.log(`unify ${describeType(left)} @ ${left.node && left.node.constructor && left.node.constructor.name} ~ ${describeType(right)} @ ${right.node && right.node.constructor && right.node.constructor.name}`);
    //console.log(`unify ${describeType(left)} ~ ${describeType(right)}`);

    left = this.simplifyType(left);
    right = this.simplifyType(right);

    const swap = () => { [right, left] = [left, right]; }

    if (left.kind === TypeKind.RigidVar && right.kind === TypeKind.RigidVar) {
      if (left.id === right.id) {
        return true;
      }
    }

    if (left.kind !== TypeKind.RegularVar && right.kind === TypeKind.RegularVar) {
      swap();
    }

    if (left.kind === TypeKind.RegularVar) {

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
        if (isTVar(type)) {
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
          propagateClasses(classes, type.types[i++]);
        }
      }

      propagateClasses(left.context, right);

      // We are all clear; set the actual type of left to right.
      left.set(right);

      // This is a very specific adjustment that is critical to the
      // well-functioning of the infer/unify algorithm. When addConstraint() is
      // called, it may decide to solve the constraint immediately during
      // inference. If this happens, a type variable might get assigned a concrete
      // type such as Int. We therefore never want the variable to be polymorphic
      // and be instantiated with a fresh variable, as that would allow Bool to
      // collide with Int.
      //
      // Should it get assigned another unification variable, that's OK too
      // because then that variable is what matters and it will become the new
      // (possibly polymorphic) variable.
      if (this.polyContextStack.length > 0) {
        this.polyContextStack[this.polyContextStack.length-1].typeVars.delete(left);
      }

      // These types will be join, and we'd like to track that
      // into a special chain.
      TypeBase.join(left, right);

      // if (left.node !== null) {
      //   right.node = left.node;
      // }

      return true;
    }

    if (left.kind === TypeKind.Arrow && right.kind === TypeKind.Arrow) {
      let success = true;
      if (!this.unify(left.paramType, right.paramType, enableDiagnostics)) {
        success = false;
      }
      if (!this.unify(left.returnType, right.returnType, enableDiagnostics)) {
        success = false;
      }
      if (success) {
        TypeBase.join(left, right);
      }
      return success;
    }

    if (left.kind === TypeKind.Con && right.kind === TypeKind.Con) {
      if (left.id === right.id) {
        TypeBase.join(left, right);
        return true;
      }
    }

    if (left.kind === TypeKind.Nil && right.kind === TypeKind.Nil) {
      return true;
    }

    if (left.kind === TypeKind.Field && right.kind === TypeKind.Field) {
      if (left.name === right.name) {
        let success = true;
        this.path.push(left.name);
        if (!this.unifyField(left.type, right.type, enableDiagnostics)) {
          success = false;
        }
        this.path.pop();
        if (!this.unify(left.restType, right.restType, enableDiagnostics)) {
          success = false;
        }
        return success;
      }
      let success = true;
      const newRestType = new TRegularVar(this.nextTypeVarId++);
      if (!this.unify(left.restType, new TField(right.name, right.type, newRestType), enableDiagnostics)) {
        success = false;
      }
      if (!this.unify(right.restType, new TField(left.name, left.type, newRestType), enableDiagnostics)) {
        success = false;
      }
      return success;
    }

    if (left.kind === TypeKind.Nil && right.kind === TypeKind.Field) {
      swap();
    }

    if (left.kind === TypeKind.Field && right.kind === TypeKind.Nil) {
      let success = true;
      this.path.push(left.name);
      if (!this.unifyField(left.type, new TAbsent(right.node), enableDiagnostics)) {
        success = false;
      }
      this.path.pop();
      if (!this.unify(left.restType, right, enableDiagnostics)) {
        success = false;
      }
      return success
    }

    if (left.kind === TypeKind.App && right.kind === TypeKind.App) {
      return this.unify(left.left, right.left, enableDiagnostics)
          && this.unify(left.right, right.right, enableDiagnostics);
    }

    if (enableDiagnostics) {
      this.diagnostics.add(
        new TypeMismatchDiagnostic(
          this.simplifyType(left),
          this.simplifyType(right),
          [...this.constraint!.getNodes()],
          this.path,
        )
      );
    }
    return false;
  }

  public solve(constraint: Constraint): void {

    let queue = [ constraint ];

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
          this.constraint = constraint;
          if (!this.unify(constraint.left, constraint.right, true)) {
            errorCount++;
            if (errorCount === this.maxTypeErrorCount) {
              return;
            }
          }
          break;
        }

      }

    }

  }

  private lookupClass(name: string): ClassDeclaration | null {
    return this.classDecls.get(name) ?? null;
  }

  public getTypeOfNode(node: Syntax): Type  {
    const info = this.getInfo(node);
    assert(info.inferredType !== undefined);
    return this.simplifyType(info.inferredType);
  }

  // private *findInstanceContext(type: TCon, clazz: ClassDeclaration): Iterable<ClassDeclaration[]> {
  //   for (const instance of clazz.getInstances()) {
  //     assert(instance.types.length === 1);
  //     const instTy0 = instance.types[0];
  //     if ((instTy0.kind === SyntaxKind.AppTypeExpression
  //         && instTy0.operator.kind === SyntaxKind.ReferenceTypeExpression
  //         && instTy0.operator.name.text === type.displayName)
  //        || (instTy0.kind === SyntaxKind.ReferenceTypeExpression
  //         && instTy0.name.text === type.displayName)) {
  //       if (instance.constraintClause === null) {
  //         return;
  //       }
  //       for (const argType of type.argTypes) {
  //         const classes = [];
  //         for (const constraint of instance.constraintClause.constraints) {
  //           assert(constraint.types.length === 1);
  //           const classDecl = this.lookupClass(constraint.name.text);
  //           if (classDecl === null) {
  //             this.diagnostics.add(new TypeclassNotFoundDiagnostic(constraint.name));
  //           } else {
  //             classes.push(classDecl);
  //           }
  //         }
  //         yield classes;
  //       }
  //     }
  //   }
  // }

}

function getVariadicMember(node: StructPattern) {1713
  for (const member of node.members) { 
    if (member.kind === SyntaxKind.VariadicStructPatternElement) {
      return member;
    }
  }
  return null;
}
