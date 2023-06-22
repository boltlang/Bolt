
// TODO support rigid vs free variables
//      https://www.reddit.com/r/haskell/comments/d4v83/comment/c0xmc3r/

import {
  ClassDeclaration,
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
} from "./diagnostics";
import { assert, assertNever, isEmpty, MultiMap, toStringTag, InspectFn, implementationLimitation } from "./util";
import { Analyser } from "./analysis";
import { InspectOptions } from "util";
import { ConstraintSolver } from "./solver";
import { TypeKind, TApp, TArrow, TCon, TField, TNil, TNominal, TPresent, TTuple, TVar, TVSet, TVSub, Type } from "./types";
import { CClass, CEmpty, CEqual, CMany, Constraint, ConstraintKind, ConstraintSet } from "./constraints";

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

// TODO actually use these
const kindOfTypes = new KType();
const kindOfRows = new KRow();

export type Kind
  = KType
  | KArrow
  | KVar
  | KRow


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

  public static fromArrays(typeVars: TVar[], constraints: Constraint[], type: Type): Forall {
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

export interface InferContext {
  typeVars: TVSet;
  env: TypeEnv;
  constraints: ConstraintSet;
  returnType: Type | null;
}

function isSignatureDeclarationLike(node: LetDeclaration): boolean {
  return false; // May be foreignKeyword !== null later
}

function isVariableDeclarationLike(node: LetDeclaration): boolean {
  return node.pattern.kind !== SyntaxKind.NamedPattern || !node.body;
}

function isFunctionDeclarationLike(node: LetDeclaration): boolean {
  return !isSignatureDeclarationLike(node) && !isVariableDeclarationLike(node);
  // return (node.pattern.kind === SyntaxKind.NamedPattern || node.pattern.kind === SyntaxKind.NestedPattern && node.pattern.pattern.kind === SyntaxKind.NamedPattern)
  //     && (node.params.length > 0 || (node.body !== null && node.body.kind === SyntaxKind.BlockBody));
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

  private getTypeEnv(): TypeEnv {
    return this.getContext().env;
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
        case ConstraintKind.Class:
        case ConstraintKind.Equal:
          const newConstraint = constraint.substitute(sub);
          newConstraint.node = node;
          newConstraint.prevInstantiation = constraint;
          return newConstraint;
        default:
          assertNever(constraint);
      }
    }
    this.addConstraint(transform(scheme.constraint));
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

      case SyntaxKind.ForallTypeExpression:
      {
        // TODO we currently automatically introduce type variables but maybe we should use the Forall?
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

          node.activeCycle = true;
          node.visited = true;

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
                for (const element of node.body.elements) {
                  this.infer(element);
                }
                break;
              }
            }
          }

          this.contexts.pop();
          node.activeCycle = false;

        } else {

          const ctx = this.getContext();
          const constraints = new ConstraintSet;
          const innerCtx: InferContext = {
            ...ctx,
            constraints,
          };
          this.pushContext(innerCtx);
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
              constraints.push(
                new CEqual(
                  type,
                  bodyType,
                  node.body
                )
              );
            }
          }
          if (type === undefined) {
            type = this.createTypeVar();
          }
          this.popContext(innerCtx);
          this.inferBindings(node.pattern, type, undefined, constraints, true);
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
          const armPatternType = this.createTypeVar();
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
        if (target !== null && target.kind === SyntaxKind.LetDeclaration) {
          if (target.activeCycle) {
            return target.inferredType!;
          }
          if (!target.visited) {
            this.infer(target);
          }
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
          for (const constraint of node.constraints) {
            implementationLimitation(constraint.types.length === 1);
            this.addConstraint(new CClass(constraint.name.text, this.inferTypeExpression(constraint.types[0]), constraint.name));
          }
          return this.inferTypeExpression(node.typeExpr, introduceTypeVars);
        }

        case SyntaxKind.ForallTypeExpression:
        {
          const ctx = this.getContext();
          // FIXME this is an ugly hack that doesn't even work. Either disallow Forall in this method or create a new TForall
          for (const varExpr of node.varTypeExps) {
            const tv = this.createTypeVar();
            ctx.env.add(varExpr.name.text, Forall.mono(tv), Symkind.Type);
            ctx.typeVars.add(tv);
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

    node.inferredType = type;

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
        this.inferBindings(pattern.pattern, type, typeVars, constraints);
        break;

      // case SyntaxKind.NamedTuplePattern:
      // {
      //   const scheme = this.lookup(pattern.name, Symkind.Type);
      //   if (scheme === null) {
      //     return this.createTypeVar();
      //   }
      //   let tupleType = new TTuple(pattern.elements.map(p =>
      //     this.inferBindings(p, this.createTypeVar(), typeVars, constraints));
      //   // FIXME not tested
      //   this.addConstraint(new CEqual(tupleType, type, pattern));
      //   return TApp.build(
      //     new TNominal(scheme.type.node as StructDeclaration | EnumDeclaration, pattern),
      //     tupleType
      //   );
      // }

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
        this.inferBindings(pattern.left, type, typeVars, constraints),
        this.inferBindings(pattern.right, type, typeVars, constraints);
        break;
      }

      case SyntaxKind.StructPattern:
      {
        const variadicMember = getVariadicMember(pattern);
        let structType: Type;
        if (variadicMember === null) {
          structType = new TNil(pattern);
        } else {
          structType = this.createTypeVar();
          if (variadicMember.pattern !== null) {
            this.inferBindings(variadicMember.pattern, structType, typeVars, constraints);
          }
        }
        for (const member of pattern.members) {
          switch (member.kind) {
            case SyntaxKind.StructPatternField:
            {
              const fieldType = this.createTypeVar();
              this.inferBindings(member.pattern, fieldType, typeVars, constraints);
              structType = new TField(member.name.text, new TPresent(fieldType), structType, pattern);
              break;
            }
            case SyntaxKind.PunnedStructPatternField:
            {
              const fieldType = this.createTypeVar();
              this.addBinding(member.name.text, Forall.mono(fieldType), Symkind.Var);
              structType = new TField(member.name.text, new TPresent(fieldType), structType, pattern);
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
            TField.sort(structType),
            pattern,
          )
        );
        break;
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
                this.diagnostics.add(new TypeclassNotFoundDiagnostic(constraint.name.text, constraint.name));
              }
            }
          }
          this.classDecls.set(node.name.text, node);
        }
        const env = node.typeEnv = new TypeEnv(parentEnv);
        for (const tv of node.types) {
          assert(tv.kind === SyntaxKind.VarTypeExpression);
          env.add(tv.name.text, Forall.mono(this.createTypeVar(tv)), Symkind.Type);
        }
        for (const element of node.elements) {
          this.initialize(element, env);
        }
        break;
      }

      case SyntaxKind.InstanceDeclaration:
      {
        if (!this.classDecls.has(node.name.text)) {
          this.diagnostics.add(new TypeclassNotFoundDiagnostic(node.name.text, node.name));
        }
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
          env.add(name.text, Forall.mono(kindArg), Symkind.Type);
          kindArgs.push(kindArg);
        }
        const type = TApp.build(new TNominal(node, node), kindArgs);
        parentEnv.add(node.name.text, new Forall(typeVars, new CMany(constraints), type), Symkind.Type);
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
            parentEnv.add(member.name.text, new Forall(typeVars, new CMany(constraints), ctorType), Symkind.Var);
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
          env.add(varExpr.text, Forall.mono(typeVar), Symkind.Type);
        }
        const type = this.inferTypeExpression(node.typeExpression);
        this.popContext(context);
        const scheme = new Forall(typeVars, new CMany(constraints), TApp.build(type, kindArgs));
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
          env.add(varExpr.text, Forall.mono(kindArg), Symkind.Type);
          kindArgs.push(kindArg);
        }
        let type: Type = new TNil(node);
        if (node.fields !== null) {
          for (const field of node.fields) {
            type = new TField(field.name.text, new TPresent(this.inferTypeExpression(field.typeExpr)), type, node);
          }
        }
        this.popContext(context);
        parentEnv.add(node.name.text, new Forall(typeVars, new CMany(constraints), TField.sort(type)), Symkind.Type);
        //parentEnv.add(node.name.text, new Forall(typeVars, constraints, new TArrow(type, TApp.build(type, kindArgs))), Symkind.Var);
        break;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

  public check(sourceFile: SourceFile): void {

    const kenv = new KindEnv(this.globalKindEnv);
    this.forwardDeclareKind(sourceFile, kenv);
    this.inferKind(sourceFile, kenv);

    const typeVars = new TVSet();
    const constraints = new ConstraintSet();
    const env = new TypeEnv(this.globalTypeEnv);
    const context: InferContext = { typeVars, constraints, env, returnType: null };

    this.pushContext(context);

    this.initialize(sourceFile, env);

    this.pushContext({
      typeVars,
      constraints,
      env: sourceFile.typeEnv!,
      returnType: null
    });

    const sccs = [...this.analyser.getSortedDeclarations()];

    for (const nodes of sccs) {

      const typeVars = new TVSet();
      const constraints = new ConstraintSet();

      for (const node of nodes) {

        if (!isFunctionDeclarationLike(node)) {
          continue;
        }

        const env = node.typeEnv!;
        const innerCtx: InferContext = {
          typeVars,
          constraints,
          env,
          returnType: null,
        };
        node.context = innerCtx;

        this.contexts.push(innerCtx);

        const returnType = this.createTypeVar();
        innerCtx.returnType = returnType;

        const paramTypes = node.params.map(param => {
          const paramType = this.createTypeVar();
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
            typeVars: innerCtx.typeVars,
            constraints: innerCtx.constraints,
            env: scopeDecl.typeEnv!,
            returnType: null,
          };
          this.contexts.push(outer)
          this.inferBindings(node.pattern, type, typeVars, constraints);
          this.contexts.pop();
        }
      }

    }

    this.infer(sourceFile);

    this.contexts.pop();
    this.popContext(context);

    const solver = new ConstraintSolver(this.diagnostics, this.nextTypeVarId);

    solver.solve(new CMany(constraints));

    this.solution = solver.solution;

  }

  private lookupClass(name: string): ClassDeclaration | null {
    return this.classDecls.get(name) ?? null;
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

type HasTypeEnv
  = ClassDeclaration
  | InstanceDeclaration
  | LetDeclaration
  | ModuleDeclaration
  | SourceFile

function shouldChangeTypeEnvDuringVisit(node: Syntax): node is HasTypeEnv {
  return node.kind === SyntaxKind.ClassDeclaration
      || node.kind === SyntaxKind.InstanceDeclaration
      || node.kind === SyntaxKind.ModuleDeclaration
      || node.kind === SyntaxKind.SourceFile
}

