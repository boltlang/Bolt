
import { Syntax, SyntaxKind, isJSNode } from "./ast"

export class Emitter {

  emit(node: Syntax) {

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

/**
 * A wrapper around `Emitter` for quick emission of AST nodes with sane defaults.
 */
export function emit(node: Syntax) {
  const emitter = new Emitter();
  return emitter.emit(node);
}

