
import * as astring from "astring"

import { Syntax, SyntaxKind, isJSNode } from "./ast"

export class Emitter {

  emit(node: Syntax) {

    if (isJSNode(node)) {
      return astring.generate(node)
    }

    switch (node.kind) {

      case SyntaxKind.SourceFile:
        let out = ''
        for (const element of node.elements) {
          out += this.emit(element);
        }
        return out;

      default:
        throw new Error(`Could not emit source code for ${SyntaxKind[node.kind]}`)

    }

  }
}

