
const exported = {};

const nodeProto = {
  preorder() {

  }
}

function isSyntax(value) {
  return typeof value === 'object'
      && value !== null
      && value.__NODE_TYPE !== undefined;
}

function* getChildNodes(node) {
  for (const key of Object.keys(node)) {
    if (key === 'span' || key === 'parentNode') {
      continue
    }
    const value = node[key];
    if (Array.isArray(value)) {
      for (const element of value) {
        if (isSyntax(element)) {
          yield element;
        }
      }
    } else if (isSyntax(value)) {
      if (isSyntax(value)) {
        yield value;
      }
    }
  }
}

function createNode(nodeType) {
  const obj = Object.create(nodeProto);
  Object.defineProperty(obj, '__NODE_TYPE', {
    enumerable: false,
    writable: false,
    configurable: true,
    value: nodeType,
  });
  Object.defineProperty(obj, 'kind', {
    enumerable: false,
    configurable: true,
    get() {
      return this.__NODE_TYPE.index;
    }
  });
  obj.span = null;
  return obj;
}

for (const nodeName of Object.keys(NODE_TYPES)) {

  exported[`create${nodeName}`] = function (...args) {
    const nodeType = NODE_TYPES[nodeName];
    const node = createNode(nodeType);
    let i = 0;
    const iter = nodeType.fields[Symbol.iterator]();
    for (; i < args.length; i++) {
      const { done, value } = iter.next();
      if (done) {
        break;
      }
      const [fieldName, fieldType] = value;
      node[fieldName] = args[i];
    }
    while (true) {
      const { done, value } = iter.next();
      if (done) {
        break;
      }
      const [fieldName, fieldType] = value;
      throw new Error(`No argument provided for field '${fieldName}'`);
    }
    if (i < args.length) {
      node.span = args[i++];
    }
    if (i < args.length) {
      throw new Error(`Too many arguments provided to function create${nodeName}`);
    }
    return node;
  }

}

exported.setParents = function setParents(node, parentNode = null) {
  node.parentNode = parentNode;
  for (const child of getChildNodes(node)) {
    setParents(child, node)
  }
}

if (typeof module !== 'undefined') {
  module.exports = exports;
}

