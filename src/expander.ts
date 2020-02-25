
import { 
  TokenStream,
  SyntaxKind,
  Syntax, 
  SourceFile, 
  Decl,
  Statement
} from "./ast"

import { Parser, ParseError } from "./parser"

type Transformer = (tokens: TokenStream) => Syntax;

export class Expander {

  transformers = new Map<string, Transformer>();

  constructor(public parser: Parser) {
    this.transformers.set('fn', parser.parseFuncDecl.bind(parser))
    this.transformers.set('let', parser.parseVarDecl.bind(parser))
    this.transformers.set('return', parser.parseRetStmt.bind(parser))
  }

  getFullyExpanded(node: Syntax): Syntax {

    if (node.kind === SyntaxKind.SourceFile) {

      const expanded: (Decl | Statement)[] = [];
      for (const element of node.elements) {
        if (element.kind === SyntaxKind.Sentence) {
          const newElement = this.getFullyExpanded(element)
          expanded.push(newElement as Decl | Statement)
        }
      }
      return new SourceFile(expanded, null, node);

    } else if (node.kind === SyntaxKind.Sentence) {

      while (true) {

        console.log('expanding sententce')

        const tokens: TokenStream = node.toStream()

        const t0 = tokens.peek();
        if (t0.kind !== SyntaxKind.Identifier) {
          throw new ParseError(t0, [SyntaxKind.Identifier]);
        }

        if (!this.transformers.has(t0.text)) {
          throw new Error(`the macro '${t0.text}' does not seem to exist`)
        }

        node = this.transformers.get(t0.text)!(tokens)

        if (node.kind !== SyntaxKind.Sentence) {
          break;
        }

      }

      return node

    } else {

      throw new Error(`unrecognised node of kind ${node.kind}`)

    }

  }

}

