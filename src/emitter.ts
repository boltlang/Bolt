
import { Syntax, SyntaxKind, kindToString } from "./ast"

export class Emitter {

  emit(node: Syntax) {

    debug(node);

    switch (node.kind) {

      case SyntaxKind.JSReferenceExpression:
        return node.name;

      case SyntaxKind.JSSourceFile:
        let out = ''
        for (const element of node.elements) {
          out += this.emit(element);
        }
        return out;

      default:
        throw new Error(`Could not emit source code for ${kindToString(node.kind)}`)

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

