
// FIXME Actually, the syntax expander could make use of algebraic effects to
// easily specify how the next expansion should happen. Just a thought.

import {
  BoltSyntax,
  createBoltRecordPattern,
  createBoltIdentifier,
  createBoltReferenceTypeNode,
  createBoltConstantExpression,
  createBoltTuplePattern,
  createBoltQualName,
  createBoltTypePattern,
  createBoltBindPattern,
  BoltPattern,
} from "./ast"

import { BoltTokenStream } from "./util"
import { TypeChecker } from "./checker"
import { Parser, ParseError } from "./parser"
import { Evaluator, TRUE, FALSE } from "./evaluator"

interface Transformer {
  pattern: BoltPattern;
  transform: (node: BoltTokenStream) => BoltSyntax;
}

function createSimpleBoltReferenceTypeNode(text: string) {
  const ids = text.split('.').map(name => createBoltIdentifier(name))
  return createBoltReferenceTypeNode(createBoltQualName(ids[ids.length-1], ids.slice(0, -1)), [])
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
const PATTERN_SYNTAX: Pattern = 
  createBoltRecordPattern(
    createSimpleBoltReferenceTypeNode('Bolt.AST.Sentence'),
    [{
      name: createBoltIdentifier('elements'),
      pattern: createBoltTuplePattern([
        createBoltRecordPattern(
          createSimpleBoltReferenceTypeNode('Bolt.AST.Identifier'),
          [{
            name: createBoltIdentifier('text'), 
            pattern: createBoltConstantExpression('syntax')
          }]
        ),
        createBoltRecordPattern(
          createSimpleBoltReferenceTypeNode('Bolt.AST.Braced'),
          [{
            name: createBoltIdentifier('elements'),
            pattern: createBoltTuplePattern([
              createBoltTypePattern(createSimpleBoltReferenceTypeNode('Bolt.AST.Pattern'), createBoltBindPattern(createBoltIdentifier('pattern'))),
              createBoltTypePattern(createSimpleBoltReferenceTypeNode('Bolt.AST.RArrow'), createBoltBindPattern(createBoltIdentifier('_'))),
              createBoltTypePattern(createSimpleBoltReferenceTypeNode('Bolt.AST.Expr'), createBoltBindPattern(createBoltIdentifier('expression')))
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

