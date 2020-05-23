
// FIXME Actually, the syntax expander could make use of algebraic effects to
// easily specify how the next expansion should happen. Just a thought.

import {
  SyntaxKind,
  BoltSyntax,
  BoltPattern,
  isBoltSourceFile,
  BoltMacroCall,
} from "../ast"

import { TextSpan } from "../text"
import { TypeChecker, Scope } from "../checker"
import { ParseError } from "../util"
import { BoltTokenStream, Parser, isModifierKeyword } from "../parser"
import { Evaluator, TRUE, FALSE } from "../evaluator"
import { setOrigNodeRange, createTokenStream } from "../util"
import { Transformer, TransformManager } from "./index"
import { inject } from "../di"
import {SourceFile} from "../program"

interface SyntaxTransformer {
  pattern: BoltPattern;
  transform: (node: BoltTokenStream) => BoltSyntax;
}

export class ExpandBoltTransform implements Transformer {

  private parser = new Parser();
  private syntaxTransformers: SyntaxTransformer[] = []

  private toExpand: BoltMacroCall[] = [];

  constructor(
    private transforms: TransformManager,
    @inject private evaluator: Evaluator,
    @inject private checker: TypeChecker
  ) {

  }

  //private getRequiredMacros(node: BoltSentence): boolean {
  //  const scope = this.checker.getScope(node);
  //  const tokens = createTokenStream(node);
  //  if (node.parentNode === null || node.parentNode.kind === SyntaxKind.BoltModule) {
  //    return this.canParseSourceElements(tokens, scope)
  //  } else if (node.parentNode.kind === SyntaxKind.BoltFunctionDeclaration) {
  //    return this.canParseFunctionBodyElements(tokens, scope);
  //  } else if (node.parentNode.kind === SyntaxKind.BoltRecordDeclaration) {
  //    return this.canParseRecordMembers(tokens, scope);
  //  } else if (isBoltExpression(node.parentNode)) {
  //    return this.canParseExpression(node, scope);
  //  } else {
  //    throw new Error(`Could not auto-detect the context in which the node is declared.`);
  //  }
  //}

  public isApplicable(node: SourceFile): boolean {
    return isBoltSourceFile(node)
  }

  public transform(node: BoltSyntax) {
    return this.expand(node);
  }

  private expand(node: BoltSyntax) {

    for (const identNode of node.findAllChildrenOfKind(SyntaxKind.BoltIdentifier)) {
      if (identNode.text.endsWith('1')) {
        this.toExpand.push(node.parentNode!);
      }
    }

    // FIXME
    return node;

  }

  //private getFullyExpanded(node: BoltSyntax): BoltSyntax {

  //  if (node.kind === SyntaxKind.BoltSourceFile) {

  //    const expanded: BoltSourceElement[] = [];

  //    let didExpand = false;

  //    for (const element of node.elements) {

  //      let newElement = this.getFullyExpanded(element);

  //      // Automatically lift top-level expressions into so that they are valid.

  //      if (isBoltExpression(newElement)) {
  //        newElement = createBoltExpressionStatement(newElement);
  //      }

  //      // From this point, newElement really should be a BoltSourceElement

  //      if (!isBoltSourceElement(newElement)) {
  //        throw new Error(`Expanded element ${kindToString(newElement.kind)} is not valid in a top-level context.`);
  //      }

  //      if (newElement !== element) {
  //        didExpand = true;
  //      }

  //      expanded.push(newElement);
  //    }

  //    if (!didExpand) {
  //      return node;
  //    }

  //    const newSourceFile = createBoltSourceFile(expanded);
  //    setOrigNodeRange(newSourceFile, node, node);
  //    setParents(newSourceFile);
  //    return newSourceFile;

  //  } else if (node.kind == SyntaxKind.BoltModule) {

  //    const expanded: BoltSourceElement[] = [];

  //    let didExpand = false;

  //    for (const element of node.elements) {
  //      let newElement = this.getFullyExpanded(element);
  //      if (!isBoltSourceElement(newElement)) {
  //        throw new Error(`Expanded element is invalid in a module context.`);
  //      }
  //      if (newElement !== element) {
  //        didExpand = true;
  //      }
  //      expanded.push(newElement);
  //    }

  //    if (!didExpand) {
  //      return node;
  //    }

  //    const newModule = createBoltModule(0, node.name, expanded);
  //    setOrigNodeRange(newModule, node, node);
  //    setParents(newModule);
  //    return newModule;

  //  } else if (node.kind === SyntaxKind.BoltSentence) {

  //    let newElement;

  //    const tokens = createTokenStream(node);

  //    try {

  //      newElement = this.parser.parseSourceElement(tokens)
  //      setOrigNodeRange(newElement, node, node);

  //    } catch (e) {

  //      // Regular errors should be propagated.

  //      if (!(e instanceof ParseError)) {
  //        throw e;
  //      }

  //      // The following applies a user-defined transformer to the token tree.

  //      while (true) {
  //        let didExpand = false;
  //        const expanded: BoltSyntax[] = [];
  //        const tokens = createTokenStream(node);
  //        for (const transformer of this.syntaxTransformers) {
  //          if (this.evaluator.eval(createBoltMatchExpression(createBoltConstantExpression(this.evaluator.createValue(node)), [
  //            createBoltMatchArm(
  //              transformer.pattern,
  //              createBoltConstantExpression(TRUE)
  //            ),
  //            createBoltMatchArm(
  //              createBoltExpressionPattern(createBoltConstantExpression(TRUE)),
  //              createBoltConstantExpression(FALSE)
  //            )
  //          ]))) {
  //            expanded.push(transformer.transform(tokens))
  //            didExpand = true;
  //            // break; // FIXME
  //          }
  //        }
  //        if (!didExpand) {
  //          break;
  //        }
  //      }

  //      // If no transformer matched, then throw the original parse error.

  //      if (!newElement) {
  //        throw e;
  //      }

  //    }

  //    // Perform a full expansion on the transformed element.

  //    return this.getFullyExpanded(newElement)

  //  } else {

  //    return node;

  //  }

  //}

}

export default ExpandBoltTransform;

