import { LetDeclaration, Pattern, SourceFile, Syntax, SyntaxKind } from "./cst";
import { MultiMap } from "./util";

export type NodeWithScope
  = SourceFile
  | LetDeclaration

export function isNodeWithScope(node: Syntax): node is NodeWithScope {
  return node.kind === SyntaxKind.SourceFile
      || node.kind === SyntaxKind.LetDeclaration;
}

export const enum Symkind {
  Var = 1,
  Type = 2,
  Module = 4,
  Typeclass = 8,
  Any = Var | Type | Module
}

export class Scope {

  private mapping = new MultiMap<string, [Symkind, Syntax]>();

  public constructor(
    public node: NodeWithScope,
  ) {
    this.scan(node);
  }

  public get depth(): number {
    let out = 0;
    let curr = this.getParent();
    while (curr !== null) {
      out++;
      curr = curr.getParent();
    }
    return out;
  }

  private getParent(): Scope | null {
    let curr = this.node.parent;
    while (curr !== null) {
      if (isNodeWithScope(curr)) {
        return curr.getScope();
      }
      curr = curr.parent;
    }
    return null;
  }

  private add(name: string, node: Syntax, kind: Symkind): void {
    this.mapping.add(name, [kind, node]);
  }

  private scan(node: Syntax): void {
    switch (node.kind) {
      case SyntaxKind.ClassDeclaration:
      {
        this.add(node.name.text, node, Symkind.Typeclass);
      }
      case SyntaxKind.InstanceDeclaration:
      case SyntaxKind.SourceFile:
      {
        for (const element of node.elements) {
          this.scan(element);
        }
        break;
      }
      case SyntaxKind.ModuleDeclaration:
      {
        this.add(node.name.text, node, Symkind.Module);
        for (const element of node.elements) {
          this.scan(element);
        }
        break;
      }
      case SyntaxKind.ExpressionStatement:
      case SyntaxKind.ReturnStatement:
      case SyntaxKind.IfStatement:
        break;
      case SyntaxKind.TypeDeclaration:
      {
        this.add(node.name.text, node, Symkind.Type);
        break;
      }
      case SyntaxKind.EnumDeclaration:
      {
        this.add(node.name.text, node, Symkind.Type);
        if (node.members !== null) {
          for (const member of node.members) {
            this.add(member.name.text, member, Symkind.Var);
          }
        }
      }
      case SyntaxKind.StructDeclaration:
      {
        this.add(node.name.text, node, Symkind.Type);
        this.add(node.name.text, node, Symkind.Var);
        break;
      }
      case SyntaxKind.LetDeclaration:
      {
        for (const param of node.params) {
          this.scanPattern(param.pattern, param);
        }
        if (node === this.node) {
          if (node.body !== null && node.body.kind === SyntaxKind.BlockBody) {
            for (const element of node.body.elements) {
              this.scan(element);
            }
          }
        } else {
          if (node.pattern.kind === SyntaxKind.WrappedOperator) {
            this.add(node.pattern.operator.text, node, Symkind.Var);
          } else {
            this.scanPattern(node.pattern, node);
          }
        }
        break;
      }
      default:
        throw new Error(`Unexpected ${node.constructor.name}`);
    }
  }

  private scanPattern(node: Pattern, decl: Syntax): void {
    switch (node.kind) {
      case SyntaxKind.LiteralPattern:
        break;
      case SyntaxKind.NamedPattern:
      {
        this.add(node.name.text, decl, Symkind.Var);
        break;
      }
      case SyntaxKind.NestedPattern:
        this.scanPattern(node.pattern, decl);
        break;
      case SyntaxKind.NamedTuplePattern:
      {
        for (const element of node.elements) {
          this.scanPattern(element, decl);
        }
        break;
      }
      case SyntaxKind.StructPattern:
      {
        for (const member of node.members) {
          switch (member.kind) {
            case SyntaxKind.StructPatternField:
            {
              this.scanPattern(member.pattern, decl);
              break;
            }
            case SyntaxKind.PunnedStructPatternField:
            {
              this.add(member.name.text, decl, Symkind.Var);
              break;
            }
          }
        }
        break;
      }
      default:
        console.log(node);
        throw new Error(`Unexpected ${node}`);
    }
  }

  public lookup(name: string, expectedKind: Symkind = Symkind.Any): Syntax | null {
    let curr: Scope | null = this;
    do {
      for (const [kind, decl] of curr.mapping.get(name)) {
        if (kind & expectedKind) {
          return decl;
        }
      }
      curr = curr.getParent();
    } while (curr !== null);
    return null;
  }

}

