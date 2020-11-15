
import { Syntax, SyntaxKind, kindToString } from "./ast"

export class Emitter {

  public emit(node: Syntax) {

    let out = '';

    switch (node.kind) {

      case SyntaxKind.BoltQualName:
        if (node.modulePath !== null) {
          if (node.isAbsolute) {
            out += '::'
          }
          for (const element of node.modulePath) {
            out += element.text + '::';
          }
        }
        out += this.emit(node.name);
        break;

      case SyntaxKind.BoltIdentifier:
      case SyntaxKind.BoltOperator:
        out += node.text;
        break;

      case SyntaxKind.BoltGtSign:
        out += '>';
        break;

      case SyntaxKind.BoltLtSign:
        out += '<';
        break;

      case SyntaxKind.BoltEqSign:
        out += '=';
        break;

      case SyntaxKind.BoltVBar:
        out += '|';
        break;

      case SyntaxKind.BoltExMark:
        out += '!';
        break;

      case SyntaxKind.JSExpressionStatement:
        out += this.emit(node.expression) + ';\n';
        break;

      case SyntaxKind.JSReferenceExpression:
        out += node.name;
        break;

      case SyntaxKind.JSConstantExpression:
        if (typeof node.value === 'string') {
          out += '"' + node.value + '"';
        } else if (typeof node.value === 'bigint') {
          out += node.value.toString();
        } else {
          throw new Error(`Could not emit the value of a specific JSConstantExpression.`);
        }
        break;

      case SyntaxKind.JSFunctionDeclaration:
        out += 'function ' + node.name.text + '(';
        //out += node.params.map(p => this.emit(p)).join(', ');
        out += ') {\n';
        out += '}\n\n'
        break;

      case SyntaxKind.JSCallExpression:
        out += this.emit(node.operator) + '(';
        out += node.operands.map(op => this.emit(op)).join(', ');
        out += ')'
        break;

      case SyntaxKind.JSSourceFile:
        for (const element of node.elements) {
          out += this.emit(element);
        }
        break;

      default:
        throw new Error(`Could not emit source code for ${kindToString(node.kind)}`)

    }

    return out;

  }

}

/**
 * A wrapper around `Emitter` for quick emission of AST nodes with sane defaults.
 */
export function emitNode(node: Syntax) {
  const emitter = new Emitter();
  return emitter.emit(node);
}

