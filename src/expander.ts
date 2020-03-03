
// FIXME Actually, the syntax expander could make use of algebraic effects to
// easily specify how the next expansion should happen. Just a thought.

import { 
  TokenStream,
  SyntaxKind,
  Syntax, 
  SourceFile, 
  Decl,
  RecordPatt,
  Identifier,
  TypeRef,
  Patt,
  ConstExpr,
  QualName,
  TuplePatt,
  BindPatt,
  TypePatt,
  MatchExpr,
  Stmt,
  Module,
} from "./ast"

import { TypeChecker } from "./checker"
import { Parser, ParseError } from "./parser"
import { Evaluator, TRUE, FALSE } from "./evaluator"

interface Transformer {
  pattern: Patt;
  transform: (node: TokenStream) => Syntax;
}

function createTypeRef(text: string) {
  const ids = text.split('.').map(name => new Identifier(name))
  return new TypeRef(new QualName(ids[ids.length-1], ids.slice(0, -1)), [])
}

/// This is actually a hand-parsed version of the following:
///
/// Bolt.AST.Braced {
///   elements = [
///     Bolt.AST.Identifier { text = "name" },
///     Bolt.AST.Braced {
///       elements = [
///         pattern: Bolt.AST.Pattern,
///         _: RArrow,
///         expression: Bolt.AST.Expr
///       ]
///     }
///   ],
/// }
const PATTERN_SYNTAX: Patt = 
  new RecordPatt(
    createTypeRef('Bolt.AST.Sentence'),
    [{
      name: new Identifier('elements'),
      pattern: new TuplePatt([
        new RecordPatt(
          createTypeRef('Bolt.AST.Identifier'),
          [{
            name: new Identifier('text'), 
            pattern: new ConstExpr('syntax')
          }]
        ),
        new RecordPatt(
          createTypeRef('Bolt.AST.Braced'),
          [{
            name: new Identifier('elements'),
            pattern: new TuplePatt([
              new TypePatt(createTypeRef('Bolt.AST.Pattern'), new BindPatt(new Identifier('pattern'))),
              new TypePatt(createTypeRef('Bolt.AST.RArrow'), new BindPatt(new Identifier('_'))),
              new TypePatt(createTypeRef('Bolt.AST.Expr'), new BindPatt(new Identifier('expression')))
            ])
          }]
        )
      ])
    }]
  )

export class Expander {

  protected transformers: Transformer[] = []

  constructor(public parser: Parser, public evaluator: Evaluator, public checker: TypeChecker) {
    // this.transformers.push({
    //   pattern: PATTERN_SYNTAX,
    //   transform: this.parser.parseSyntax.bind(this.parser)
    // })
  }

  getFullyExpanded(node: Syntax): Syntax {

    if (node.kind === SyntaxKind.SourceFile) {

      const expanded: (Decl | Stmt)[] = [];

      let didExpand = false;

      for (const element of node.elements) {
        let newElement = this.getFullyExpanded(element);
        if (newElement !== element) {
          didExpand = true;
        }
        expanded.push(newElement as Decl | Stmt)
      }

      if (!didExpand) {
        return node;
      }

      return new SourceFile(expanded, null, node);

    } else if (node.kind == SyntaxKind.Module) {

      const expanded = [];

      let didExpand = false;

      for (const element of node.elements) {
        let newElement = this.getFullyExpanded(element);
        if (newElement !== element) {
          didExpand = true;
        }
        expanded.push(newElement as Decl | Stmt)
      }

      if (!didExpand) {
        return node;
      }

      return new Module(node.isPublic, node.name, expanded, null, node);


    } else if (node.kind === SyntaxKind.Sentence) {

      let newElement;

      const tokens = node.toTokenStream();

      try {

        newElement = this.parser.parseSourceElement(tokens)

      } catch (e) {

        // Regular errors should be propagated.

        if (!(e instanceof ParseError)) {
          throw e;
        }

        // The following applies a user-defined transformer to the token tree.

        while (true) {
          let didExpand = false;
          const expanded: Syntax[] = [];
          const tokens = node.toTokenStream();
          for (const transformer of this.transformers) {
            if (this.evaluator.eval(new MatchExpr(new ConstExpr(this.evaluator.createValue(node)), [
              [transformer.pattern, new ConstExpr(TRUE)],
              [new ConstExpr(TRUE), new ConstExpr(FALSE)]
            ]))) {
              expanded.push(transformer.transform(tokens))
              didExpand = true;
              // break; // FIXME
            }
          }
          if (!didExpand) {
            break;
          }
        }

        // If no transformer matched, then throw the original parse error.

        if (!newElement) {
          throw e;
        }

      }

      // Perform a full expansion on the transformed element.

      return this.getFullyExpanded(newElement)

    } else {

      this.checker.check(node);

      return node;

    }

  }

}

