import { Constraint, ConstraintKind } from "./constraints";
import { Diagnostics, FieldNotFoundDiagnostic, TypeclassNotFoundDiagnostic, TypeclassNotImplementedDiagnostic, TypeMismatchDiagnostic } from "./diagnostics";
import { TAbsent, TField, TVar, TVSub, Type, TypeBase, TypeKind } from "./types";
import { assert } from "./util";

export class ConstraintSolver {

  private path: string[] = [];
  private constraint: Constraint | null = null;
  private maxTypeErrorCount = 5;

  public solution = new TVSub;

  public constructor(
    public diagnostics: Diagnostics,
    private nextTypeVarId: number,
  ) {

  }

  private find(type: Type): Type {
    while (type.kind === TypeKind.Var && this.solution.has(type)) {
      type = this.solution.get(type)!;
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

    left = this.find(left);
    right = this.find(right);

    // console.log(`unify ${describeType(left)} @ ${left.node && left.node.constructor && left.node.constructor.name} ~ ${describeType(right)} @ ${right.node && right.node.constructor && right.node.constructor.name}`);

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
      //const propagateClasses = (classes: Iterable<ClassDeclaration>, type: Type) => {
      //  if (type.kind === TypeKind.Var) {
      //    for (const constraint of classes) {
      //      type.context.add(constraint);
      //    }
      //  } else if (type.kind === TypeKind.Con) {
      //    for (const constraint of classes) {
      //      propagateClassTCon(constraint, type);
      //    }
      //  } else {
      //    //assert(false);
      //    //this.diagnostics.add(new );
      //  }
      //}

      //const propagateClassTCon = (clazz: ClassDeclaration, type: TCon) => {
      //  const s = this.findInstanceContext(type, clazz);
      //  let i = 0;
      //  for (const classes of s) {
      //    propagateClasses(classes, type.argTypes[i++]);
      //  }
      //}

      //propagateClasses(left.context, right);

      // We are all clear; set the actual type of left to right.
      this.solution.set(left, right);

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

    if (left.kind === TypeKind.Tuple && right.kind === TypeKind.Tuple) {
      if (left.elementTypes.length === right.elementTypes.length) {
        let success = false;
        const count = left.elementTypes.length;
        for (let i = 0; i < count; i++) {
          if (!this.unify(left.elementTypes[i], right.elementTypes[i], enableDiagnostics)) {
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
          if (!this.unify(left.argTypes[i], right.argTypes[i], enableDiagnostics)) {
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
      const newRestType = new TVar(this.nextTypeVarId++);
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

    if (left.kind === TypeKind.Nominal && right.kind === TypeKind.Nominal) {
      if (left.decl === right.decl) {
        return true;
      }
      // fall through to error reporting
    }

    if (left.kind === TypeKind.App && right.kind === TypeKind.App) {
      return this.unify(left.left, right.left, enableDiagnostics)
          && this.unify(left.right, right.right, enableDiagnostics);
    }

    if (enableDiagnostics) {
      this.diagnostics.add(
        new TypeMismatchDiagnostic(
          left.substitute(this.solution),
          right.substitute(this.solution),
          [...this.constraint!.getNodes()],
          this.path,
        )
      );
    }
    return false;
  }

  public solve(constraint: Constraint): void {

    let queue = [ constraint ];
    let next = [];
    let isNext = false;

    let errorCount = 0;

    for (;;) {

      if (queue.length === 0) {
        if (next.length === 0) {
          break;
        }
        isNext = true;
        queue = next;
        next = [];
      }

      const constraint = queue.shift()!;

      sw: switch (constraint.kind) {

        case ConstraintKind.Many:
        {
          for (const element of constraint.elements) {
            queue.push(element);
          }
          break;
        }

//         case ConstraintKind.Class:
//         {
//           if (constraint.type.kind === TypeKind.Var) {
//             if (isNext) {
//               // TODO
//             } else {
//               next.push(constraint);
//             }
//           } else {
//             const classDecl = this.lookupClass(constraint.className);
//             if (classDecl === null) {
//               this.diagnostics.add(new TypeclassNotFoundDiagnostic(constraint.className, constraint.node));
//               break;
//             }
//             for (const instance of classDecl.getInstances()) {
//               if (this.unify(instance.inferredType, constraint.type, false)) {
//                 break sw;
//               }
//             }
//             this.diagnostics.add(new TypeclassNotImplementedDiagnostic(constraint.className, constraint.type, constraint.node));
//           }
//           break;
//         }

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

}
