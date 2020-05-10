
const exported = {};

const nodeProto = {
  preorder() {

  }
}

function createNode(nodeType) {
  const obj = Object.create(nodeProto);
  Object.defineProperty(obj, '__IS_NODE', {
    enumerable: false,
    writable: false,
    configurable: true,
    value: true,
  });
  Object.defineProperty(obj, '__NODE_TYPE', {
    enumerable: false,
    writable: false,
    configurable: true,
    value: nodeType,
  });
  Object.defineProperty(obj, 'kind', {
    enumerable: false,
    writable: false,
    configurable: true,
    getter() {
      return this.__NODE_TYPE.index;
    }
  });
  obj.span = null;
  obj.origNodes = null;
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
      node.origNodes = i < args.length ? args[i++] : null;
    }
    if (i < args.length) {
      throw new Error(`Too many arguments provided to function create${nodeName}`);
    }
    return node;
  }
}

if (typeof module !== 'undefined') {
  module.exports = exports;
}

