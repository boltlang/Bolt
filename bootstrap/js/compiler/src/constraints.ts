
import { InspectOptions } from "util";
import { Syntax } from "./cst"
import { TVSub, TVar, Type } from "./types";
import { first, InspectFn, last, toStringTag } from "./util";

export const enum ConstraintKind {
  Equal,
  // Class,
  Many,
  Empty,
}

abstract class ConstraintBase {

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

  public abstract freeTypeVars(): Iterable<TVar>;

  public abstract substitute(sub: TVSub): Constraint;

}

export class CEqual extends ConstraintBase {

  public readonly kind = ConstraintKind.Equal;

  public constructor(
    public left: Type,
    public right: Type,
    public node: Syntax | null,
  ) {
    super();
  }

  public substitute(sub: TVSub): CEqual {
    return new CEqual(
      this.left.substitute(sub),
      this.right.substitute(sub),
      this.node,
    );
  }

  public *freeTypeVars(): Iterable<TVar> {
    yield* this.left.getTypeVars();
    yield* this.right.getTypeVars();
  }

  public [toStringTag](_currentDepth: number, options: InspectOptions, inspect: InspectFn): string {
    return inspect(this.left, options) + ' ~ ' + inspect(this.right, options);
  }

}

export class CMany extends ConstraintBase {

  public readonly kind = ConstraintKind.Many;

  public constructor(
    public elements: Constraint[]
  ) {
    super();
  }

  public substitute(sub: TVSub): CMany {
    const newElements = [];
    for (const element of this.elements) {
      newElements.push(element.substitute(sub));
    }
    return new CMany(newElements);
  }

  public *freeTypeVars(): Iterable<TVar> {
    for (const element of this.elements) {
      yield* element.freeTypeVars();
    }
  }

  public [toStringTag](_depth: number, opts: InspectOptions, inspect: InspectFn): string {
    if (this.elements.length === 0) {
      return '[]';
    }
    let out = '[\n';
    out += this.elements.map(constraint => '  ' + inspect(constraint, opts)).join('\n');
    out += '\n]';
    return out;
  }

}

// export class CClass extends ConstraintBase {

//   public readonly kind = ConstraintKind.Class;

//   public constructor(
//     public className: string,
//     public type: Type,
//     public node: Syntax,
//   ) {
//     super();
//   }

//   public substitute(sub: TVSub): Constraint {
//     return new CClass(
//       this.className,
//       this.type.substitute(sub),
//       this.node,
//     );
//   }

//   public *freeTypeVars(): Iterable<TVar> {
//     yield* this.type.getTypeVars();
//   }

//   public [toStringTag](_depth: number, opts: InspectOptions, inspect: InspectFn): string {
//     return this.className + ' => ' + inspect(this.type, opts);
//   }

// }

export class CEmpty extends ConstraintBase {

  public readonly kind = ConstraintKind.Empty;

  public substitute(_sub: TVSub): CEmpty {
    return this;
  }

  public *freeTypeVars(): Iterable<TVar> {
    
  }

  public [toStringTag]() {
    return 'ε';
  }

}

export type Constraint
  = CEqual
  // | CClass
  | CMany
  | CEmpty

export class ConstraintSet extends Array<Constraint> {

}
