
// FIXME Actually, the syntax expander could make use of algebraic effects to
// easily specify how the next expansion should happen. Just a thought.

import {
  SyntaxKind,
  setParents,
  kindToString,
  BoltSyntax,
  BoltSentence,
  createBoltEOS,
  createBoltRecordPattern,
  createBoltExpressionPattern,
  createBoltIdentifier,
  createBoltReferenceTypeNode,
  createBoltConstantExpression,
  createBoltTuplePattern,
  createBoltQualName,
  createBoltTypePattern,
  createBoltBindPattern,
  createBoltMatchExpression,
  createBoltMatchArm,
  createBoltModule,
  createBoltSourceFile,
  BoltPattern,
  BoltSourceElement,
  BoltReferenceTypeNode,
  createBoltRecordDeclaration,
  createBoltRecordDeclarationField,
  isBoltSourceElement,
  createBoltExpressionStatement,
  isBoltExpression,
} from "./ast"

import { TextSpan } from "./text"
import { TypeChecker } from "./checker"
import { Parser, ParseError } from "./parser"
import { Evaluator, TRUE, FALSE } from "./evaluator"
import { StreamWrapper, setOrigNodeRange, BoltTokenStream, createTokenStream } from "./util"

interface Transformer {
  pattern: BoltPattern;
  transform: (node: BoltTokenStream) => BoltSyntax;
}

function createSimpleBoltReferenceTypeNode(text: string): BoltReferenceTypeNode {
  const ids = text.split('.').map(name => createBoltIdentifier(name))
  return createBoltReferenceTypeNode(createBoltQualName(ids.slice(0, -1), ids[ids.length-1]), [])
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
//const PATTERN_SYNTAX: BoltPattern = 
//  createBoltRecordPattern(
//    createSimpleBoltReferenceTypeNode('Bolt.AST.Sentence'),
//    [
//      createBoltRecordDeclarationField(
//        createBoltIdentifier('elements'),
//        createBoltTuplePattern([
//          createBoltRecordPattern(
//            createSimpleBoltReferenceTypeNode('Bolt.AST.Identifier'),
//            [{
//              name: createBoltIdentifier('text'), 
//              pattern: createBoltConstantExpression('syntax')
//            }]
//          ),
//          createBoltRecordPattern(
//            createSimpleBoltReferenceTypeNode('Bolt.AST.Braced'),
//            [{
//              name: createBoltIdentifier('elements'),
//              pattern: createBoltTuplePattern([
//                createBoltTypePattern(createSimpleBoltReferenceTypeNode('Bolt.AST.Pattern'), createBoltBindPattern(createBoltIdentifier('pattern'))),
//                createBoltTypePattern(createSimpleBoltReferenceTypeNode('Bolt.AST.RArrow'), createBoltBindPattern(createBoltIdentifier('_'))),
//                createBoltTypePattern(createSimpleBoltReferenceTypeNode('Bolt.AST.Expr'), createBoltBindPattern(createBoltIdentifier('expression')))
//              ])
//            }]
//          )
//        ])
//    )]
//  )

export class Expander {

  protected transformers: Transformer[] = []

  constructor(public parser: Parser, public evaluator: Evaluator, public checker: TypeChecker) {
    // this.transformers.push({
    //   pattern: PATTERN_SYNTAX,
    //   transform: this.parser.parseSyntax.bind(this.parser)
    // })
  }

  getFullyExpanded(node: BoltSyntax): BoltSyntax {

    if (node.kind === SyntaxKind.BoltSourceFile) {

      const expanded: BoltSourceElement[] = [];

      let didExpand = false;

      for (const element of node.elements) {

        let newElement = this.getFullyExpanded(element);

        // Automatically lift top-level expressions into so that they are valid.

        if (isBoltExpression(newElement)) {
          newElement = createBoltExpressionStatement(newElement);
        }

        // From this point, newElement really should be a BoltSourceElement

        if (!isBoltSourceElement(newElement)) {
          throw new Error(`Expanded element ${kindToString(newElement.kind)} is not valid in a top-level context.`);
        }

        if (newElement !== element) {
          didExpand = true;
        }

        expanded.push(newElement);
      }

      if (!didExpand) {
        return node;
      }

      const newSourceFile = createBoltSourceFile(expanded);
      setOrigNodeRange(newSourceFile, node, node);
      setParents(newSourceFile);
      return newSourceFile;

    } else if (node.kind == SyntaxKind.BoltModule) {

      const expanded: BoltSourceElement[] = [];

      let didExpand = false;

      for (const element of node.elements) {
        let newElement = this.getFullyExpanded(element);
        if (!isBoltSourceElement(newElement)) {
          throw new Error(`Expanded element is invalid in a module context.`);
        }
        if (newElement !== element) {
          didExpand = true;
        }
        expanded.push(newElement);
      }

      if (!didExpand) {
        return node;
      }

      const newModule = createBoltModule(0, node.name, expanded);
      setOrigNodeRange(newModule, node, node);
      setParents(newModule);
      return newModule;

    } else if (node.kind === SyntaxKind.BoltSentence) {

      let newElement;

      const tokens = createTokenStream(node);

      try {

        newElement = this.parser.parseSourceElement(tokens)
        setOrigNodeRange(newElement, node, node);

      } catch (e) {

        // Regular errors should be propagated.

        if (!(e instanceof ParseError)) {
          throw e;
        }

        // The following applies a user-defined transformer to the token tree.

        while (true) {
          let didExpand = false;
          const expanded: BoltSyntax[] = [];
          const tokens = createTokenStream(node);
          for (const transformer of this.transformers) {
            if (this.evaluator.eval(createBoltMatchExpression(createBoltConstantExpression(this.evaluator.createValue(node)), [
              createBoltMatchArm(
                transformer.pattern,
                createBoltConstantExpression(TRUE)
              ),
              createBoltMatchArm(
                createBoltExpressionPattern(createBoltConstantExpression(TRUE)),
                createBoltConstantExpression(FALSE)
              )
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

