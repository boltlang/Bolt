import { TypeExpression } from "../cst";
import {
  ExprBody,
  NamedPattern,
  LBrace,
  RBrace,
  LetKeyword,
  LetDeclaration,
  SourceFile,
  Syntax,
  SyntaxKind,
  Identifier,
  StructExpression,
  StructExpressionField,
  Equals,
  InstanceDeclaration,
  FunctionExpression,
  Backslash,
  canHaveInstanceDeclaration,
  vistEachChild
} from "../cst";
import { Pass } from "../program";
import { assert } from "../util";

function encode(typeExpr: TypeExpression): string {
  switch (typeExpr.kind) {
    case SyntaxKind.ReferenceTypeExpression:
      let out = '';
      if (typeExpr.modulePath.length > 0) {
        out += '_xm';
        for (const [name, _dot] of typeExpr.modulePath) {
          out += name + '_';
        }
      }
      return out + typeExpr.name.text;
    default:
      throw new Error(`Could not encode type.`)
  }
}

function lcfirst(text: string): string {
  return text[0].toLowerCase() + text.substring(1);
}

export class TypeclassDictPassing implements Pass<SourceFile, SourceFile> {

  private mangleInstance(node: InstanceDeclaration): string {
    return lcfirst(node.name.text) + '_' + node.types.map(encode).join('');
  }

  private visit(node: Syntax): Syntax {
    if (canHaveInstanceDeclaration(node)) {
      return vistEachChild(node, this.visit.bind(this));
    }
    if (node.kind === SyntaxKind.InstanceDeclaration) {
      const decl = new LetDeclaration(
        node.pubKeyword,
        new LetKeyword(),
        null,
        null,
        new NamedPattern(new Identifier(null, this.mangleInstance(node))),
        [],
        null, // TODO
        new ExprBody(
          new Equals(),
          new StructExpression(
            new LBrace(),
            node.elements.map(element => {
              assert(element.kind === SyntaxKind.LetDeclaration);
              assert(element.pattern.kind === SyntaxKind.NamedPattern);
              return new StructExpressionField(
                new Identifier(null, element.pattern.name.text),
                new Equals(),
                new FunctionExpression(new Backslash(), element.params, element.body!)
              );
            }),
            new RBrace(),
          )
        )
      );  
      return decl;
    }
    return node;
  }

  public apply(input: SourceFile): SourceFile {
    return this.visit(input) as SourceFile;
  }

}


