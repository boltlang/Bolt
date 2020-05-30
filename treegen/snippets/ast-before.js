
export class NodeVisitor {
  visit(node) {
    for (const child of node.preorder()) {
      const key = `visit${kindToString(child.kind)}`;
      if (this[key] !== undefined) {
        this[key](child);
      }
    }
  }
}

let nextNodeId = 1;

class SyntaxBase {

  constructor(span) {
    this.id = nextNodeId++;
    this.errors = [];
    this.span = span;
  }

  *getChildNodes() {
    for (const key of Object.keys(this)) {
      if (key === 'span' || key === 'parentNode' || key === 'type') {
        continue
      }
      const value = this[key];
      if (Array.isArray(value)) {
        for (const element of value) {
          if (isSyntax(element)) {
            yield element;
          }
        }
      } else {
        if (isSyntax(value)) {
          yield value;
        }
      }
    }
  }

  visit(visitors) {
    const stack = [this];
    while (stack.length > 0) {
      const node = stack.pop();
      const kindName = kindToString(node.kind);
      const kindNamesToVisit = [kindName, ...NODE_TYPES[kindName].parents];
      for (const visitor of visitors) {
        for (const kindName of kindNamesToVisit) {
          const key = `visit${kindName}`
          if (visitor[key] !== undefined) {
            visitor[key](node);
          }
        }
      }
      for (const childNode of node.getChildNodes()) {
        stack.push(childNode);
      }
    }
  }

  *preorder() {
    const stack = [this];
    while (stack.length > 0) {
      const node = stack.pop();
      yield node
      for (const childNode of node.getChildNodes()) {
        stack.push(childNode);
      }
    }
  }

  mayContainKind(kind) {
    // TODO
    return true;
  }

  getParentOfKind(kind) {
    let currNode = this.parentNode;
    while (currNode !== null) {
      if (currNode.kind === kind) {
        return currNode;
      }
      currNode = currNode.parentNode;
    }
    return null;
  }

  *findAllChildrenOfKind(kind) {
    for (const node of this.preorder()) {
      if (!node.mayContainKind(kind)) {
        break;
      }
      if (node.kind === kind) {
        yield node
      }
    }
  }

}

export function isSyntax(value) {
  return typeof value === 'object'
      && value !== null
      && value instanceof SyntaxBase;
}

export function setParents(node, parentNode = null) {
  node.parentNode = parentNode;
  for (const child of node.getChildNodes()) {
    setParents(child, node)
  }
}
