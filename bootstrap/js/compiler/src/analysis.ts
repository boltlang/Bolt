import { DirectedHashGraph, strongconnect } from "yagl";
import { assert } from "./util";
import { Syntax, LetDeclaration, SourceFile, SyntaxKind } from "./cst";
import type { Scope } from "./scope"

export class Analyser {

  private referenceGraph = new DirectedHashGraph<LetDeclaration>();

  public addSourceFile(node: SourceFile): void {

    const visit = (node: Syntax, source: Syntax | null) => {

      const addReference = (scope: Scope, name: string) => {
        const target = scope.lookup(name);
        if (source === null || target === null || target.kind === SyntaxKind.Param) {
          return;
        }
        assert(source.kind === SyntaxKind.LetDeclaration);
        assert(target.kind === SyntaxKind.LetDeclaration);
        this.referenceGraph.addEdge(source, target);
      }

      switch (node.kind) {

        case SyntaxKind.ConstantExpression:
          break;

        case SyntaxKind.MatchExpression:
        {
          for (const arm of node.arms) {
            visit(arm.expression, source);
          }
          break;
        }

        case SyntaxKind.InstanceDeclaration:
        case SyntaxKind.ClassDeclaration:
        case SyntaxKind.SourceFile:
        case SyntaxKind.ModuleDeclaration:
        {
          for (const element of node.elements) {
            visit(element, source);
          }
          break;
        }

        case SyntaxKind.ReferenceExpression:
        {
          if (node.name.kind === SyntaxKind.Identifier) {
            assert(node.modulePath.length === 0);
            addReference(node.getScope(), node.name.text);
          }
          break;
        }

        case SyntaxKind.MemberExpression:
        {
          visit(node.expression, source);
          break;
        }

        case SyntaxKind.TupleExpression:
        {
          for (const element of node.elements) {
            visit(element, source);
          }
          break;
        }

        case SyntaxKind.StructExpression:
        {
          for (const member of node.members) {
            switch (member.kind) {
              case SyntaxKind.PunnedStructExpressionField:
              {
                addReference(node.getScope(), member.name.text);
                break;
              }
              case SyntaxKind.StructExpressionField:
              {
                visit(member.expression, source);
                break;
              };
            }
          }
          break;
        }

        case SyntaxKind.NestedExpression:
        {
          visit(node.expression, source);
          break;
        }

        case SyntaxKind.InfixExpression:
        {
          visit(node.left, source);
          visit(node.right, source);
          break;
        }

        case SyntaxKind.CallExpression:
        {
          visit(node.func, source);
          for (const arg of node.args) {
            visit(arg, source);
          }
          break;
        }

        case SyntaxKind.IfStatement:
        {
          for (const cs of node.cases) {
            if (cs.test !== null) {
              visit(cs.test, source);
            }
            for (const element of cs.elements) {
              visit(element, source);
            }
          }
          break;
        }

        case SyntaxKind.ExpressionStatement:
        {
          visit(node.expression, source);
          break;
        }

        case SyntaxKind.ReturnStatement:
        {
          if (node.expression !== null) {
            visit(node.expression, source);
          }
          break;
        }

        case SyntaxKind.LetDeclaration:
        {
          this.referenceGraph.addVertex(node);
          if (node.body !== null) {
            switch (node.body.kind) {
              case SyntaxKind.ExprBody:
              {
                visit(node.body.expression, node);
                break;
              }
              case SyntaxKind.BlockBody:
              {
                for (const element of node.body.elements) {
                  visit(element, node);
                }
                break;
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

    visit(node, null);

  }

  public isReferencedInParentScope(node: LetDeclaration): boolean {
    const maxDepth = node.getScope().depth;
    for (const other of this.referenceGraph.getSourceVertices(node)) {
      if (other.getScope().depth < maxDepth) {
        return true;
      }
    }
    return false;
  }

  /**
   * Gets a sorted list of collections where each collection contains
   * let-declarations that reference each other in some way or another.
   *
   * The declarations are sorted in such a way that declarations that reference
   * nothing come before declarations that reference another declaration. When
   * a let-declaration is not recusive, it will simply show up as a collection
   * with only one element.
   */
  public getSortedDeclarations(): Iterable<LetDeclaration[]> {
    return strongconnect(this.referenceGraph);
  }

}
