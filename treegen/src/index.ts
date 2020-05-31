
import * as fs from "fs"
import * as path from "path"

const PACKAGE_ROOT = path.resolve(__dirname, '..');

const CUSTOM_TYPES = ['Package', 'BoltValue', 'JSValue'];

import { Syntax, Declaration, NodeDeclaration, TypeDeclaration, EnumDeclaration, TypeNode, NodeField } from "./ast"
import { MapLike, FileWriter } from "./util"

export function generateAST(decls: Declaration[]) {

  let jsFile = new FileWriter();
  let dtsFile = new FileWriter();
  let i;

  // Sort declarations by category

  const nodeDecls: NodeDeclaration[] = decls.filter(decl => decl.type === 'NodeDeclaration') as NodeDeclaration[];
  const typeDecls: TypeDeclaration[] = decls.filter(decl => decl.type === 'TypeDeclaration') as TypeDeclaration[];
  const enumDecls: EnumDeclaration[] = decls.filter(decl => decl.type === 'EnumDeclaration') as EnumDeclaration[];

  const declByName: MapLike<Declaration> = Object.create(null);
  i = 0;
  for (const decl of decls) {
    decl.index = i++;
    declByName[decl.name] = decl;
  }

  // Generate a mapping from parent node to child node
  // This makes it easy to generate union types for the intermediate nodes.

  const childrenOf: MapLike<string[]> = Object.create(null);
  for (const nodeDecl of nodeDecls) {
    for (const parentName of nodeDecl.parents) {
      if (childrenOf[parentName] === undefined) {
        childrenOf[parentName] = [];
      }
      childrenOf[parentName].push(nodeDecl.name);
    }
  }

  // After we're done mappping parents to children, we can use isLeafNode() 
  // to store the nodes we will be iterating most frequently on.

  const finalNodes: NodeDeclaration[] = nodeDecls.filter(decl => isFinalNode(decl.name));

  // Write a JavaScript file that contains all AST definitions.

  jsFile.write(fs.readFileSync(path.join(PACKAGE_ROOT, 'snippets', 'ast-before.js'), 'utf8'));

  for (const decl of finalNodes) {

    jsFile.write(`class ${decl.name} extends SyntaxBase {\n\n`);
    jsFile.indent();

    jsFile.write(`kind = ${decl.index};\n\n`);
    jsFile.write(`static parents = `);
    jsFile.write(JSON.stringify([...getParentChain(decl.name), 'Syntax'], undefined, 2));
    jsFile.write(';\n\n')
    //jsFile.write(`static fields = new Map([\n`);
    //jsFile.indent();
    //jsFile.write(JSON.stringify([...getAllFields(decl)].map(field => [field.name, jsonify(field.typeNode)]), undefined, 2)),
    //jsFile.dedent();

    jsFile.write(`constructor(\n`);
    jsFile.indent();
    for (const field of getAllFields(decl)) {
      jsFile.write(`${field.name},\n`);
    }
    jsFile.write(`span = null,\n`)
    jsFile.dedent();
    jsFile.write(`) {\n`);
    jsFile.indent();
    jsFile.write(`super(span);\n`);
    for (const field of getAllFields(decl)) {
      jsFile.write(`this.${field.name} = ${field.name};\n`);
    }
    jsFile.write(`this.span = span\n`)
    jsFile.dedent();
    jsFile.write('}\n\n')

    jsFile.dedent();
    jsFile.write('}\n');
  }

  jsFile.write(`const NODE_CLASSES = {\n`)
  jsFile.indent();
  for (const node of finalNodes) {
    jsFile.write(node.name + ',\n');
  }
  jsFile.dedent();
  jsFile.write('}\n\n')
  //jsFile.dedent();
  //jsFile.write('};\n\n');

  jsFile.write(`export function kindToString (kind) {\n  switch (kind) {\n`);
  jsFile.indent(2);
  for (const leafNode of finalNodes) {
    jsFile.write(`case ${leafNode.index}: return '${leafNode.name}';\n`);
  }
  jsFile.dedent(2);
  jsFile.write(`  }\n}\n\n`);



  jsFile.write(`export const SyntaxKind = {\n`)
  for (const leafNode of finalNodes) {
    jsFile.write(`  ${leafNode.name}: ${leafNode.index},\n`);
  }
  jsFile.write('}\n\n')

  for (const decl of nodeDecls) {
    jsFile.write(`export function is${decl.name}(value) {\n`);
    jsFile.indent();
    jsFile.write(`if (!isSyntax(value)) {\n  return false;\n}\n`);
    if (isFinalNode(decl.name)) {
      jsFile.write(`  return value.kind === ${decl.index};\n`);
    } else {
      jsFile.write('return ' + [...getFinalNodes(decl.name)].map(d => `value.kind === ${getDeclarationNamed(d).index}`).join(' || ') + '\n');
    }
    jsFile.dedent();
    jsFile.write(`}\n`);
  }

  for (const node of finalNodes) {
    jsFile.write(`export function create${node.name}(...args) {\n`)
    jsFile.indent()
    jsFile.write(`return new ${node.name}(...args);\n`)
    jsFile.dedent();
    jsFile.write('}\n\n')
  }

  jsFile.write(fs.readFileSync(path.join(PACKAGE_ROOT, 'snippets', 'ast-after.js'), 'utf8'));

  // Write corresponding TypeScript declarations

  dtsFile.write(fs.readFileSync(path.join(PACKAGE_ROOT, 'snippets', 'ast-before.d.ts'), 'utf8'));

  dtsFile.write('export class NodeVisitor {\n');
  dtsFile.write('  public visit(node: Syntax): void;\n');
  for (const decl of finalNodes) {
    dtsFile.write(`  protected visit${decl.name}?(node: ${decl.name}): void;\n`);
  }
  dtsFile.write('}\n\n');

  dtsFile.write(`\nexport const enum SyntaxKind {\n`);
  for (const decl of finalNodes) {
    dtsFile.write(`  ${decl.name} = ${decl.index},\n`);
  }
  dtsFile.write(`}\n\n`);

  for (const decl of decls) {
    if (decl.type === 'NodeDeclaration') {
      if (isFinalNode(decl.name)) {
        dtsFile.write(`export interface ${decl.name} extends SyntaxBase {\n`)
        dtsFile.indent()
        dtsFile.write(`kind: SyntaxKind.${decl.name};\n`);
        for (const field of getAllFields(decl)) {
          dtsFile.write(`${field.name}: ${emitTypeScriptType(field.typeNode)};\n`);
        }
        dtsFile.write(`parentNode: ${decl.name}Parent;\n`);
        dtsFile.write(`getChildNodes(): IterableIterator<${decl.name}Child>\n`)
        dtsFile.dedent();
        dtsFile.write(`}\n\n`);
        {
          dtsFile.write(`export type ${decl.name}Parent\n`)
          let first = true;
          for (const parentName of uniq(getNodesReferencingNode(decl.name))) {
            dtsFile.write((first ? '=' : '|') + ' ' + parentName + '\n');
            first = false;
          }
          dtsFile.write((first ? '=' : '|') + ' never\n\n');
        }
        {
          dtsFile.write(`export type ${decl.name}AnyParent\n`)
          let first = true;
          for (const parentDecl of uniq(getNodesTransitivelyReferencingNode(decl))) {
            dtsFile.write((first ? '=' : '|') + ' ' + parentDecl + '\n');
            first = false;
          }
          dtsFile.write((first ? '=' : '|') + ' never\n\n');
        }
        {
          dtsFile.write(`export type ${decl.name}Child\n`)
          let first = true;
          for (const childDecl of uniq(getFinalNodes(decl.name))) {
            dtsFile.write((first ? '=' : '|') + ' ' + childDecl + '\n');
            first = false;
          }
          dtsFile.write((first ? '=' : '|') + ' never\n\n');
        }
      } else {
        dtsFile.write(`export type ${decl.name}\n`);
        let first = true;
        dtsFile.indent();
        for (const childDecl of uniq(getFinalNodes(decl.name))) {
          dtsFile.write((first ? '=' : '|') + ' ' + childDecl + '\n');
          first = false;
        }
        dtsFile.dedent();
        dtsFile.write('\n\n');
      }
    } else if (decl.type === 'EnumDeclaration') {
      dtsFile.write(`export const enum ${decl.name} {\n`);
      dtsFile.indent();
      for (const field of decl.fields) {
        dtsFile.write(`${field.name} = ${field.value},`);
      }
      dtsFile.dedent();
      dtsFile.write('}\n\n');
    }
  }

  //dtsFile.write('export type ResolveSyntaxKind<K extends SyntaxKind>\n');
  //{
    //let i = 0;
    //for (const decl of leafNodes) {
      //dtsFile.write(i === 0 ? '  =' : '  |');
      //dtsFile.write(`  K extends SyntaxKind.${decl.name} ? ${decl.name}`);
      //dtsFile.write(' :');
      //dtsFile.write('\n');
      //i++;
    //}
    //dtsFile.write('  never\n\n');
  //}

  dtsFile.write(`export type Syntax\n`);
  let first = true;
  dtsFile.indent();
  for (const decl of finalNodes) {
    dtsFile.write((first ? '=' : '|') + ' ' + decl.name + '\n');
    first = false;
  }
  dtsFile.dedent();
  dtsFile.write('\n\n');

  dtsFile.write('export function kindToString(kind: SyntaxKind): string;\n\n');

  for (const decl of finalNodes) {
    dtsFile.write(`export function create${decl.name}(`);
    for (const field of getAllFields(decl)) {
      dtsFile.write(`${field.name}: ${emitTypeScriptType(field.typeNode)}, `);
    }
    dtsFile.write(`span?: TextSpan | null): ${decl.name};\n`);
  }

  dtsFile.write('\n');

  for (const decl of nodeDecls) {
    dtsFile.write(`export function is${decl.name}(value: any): value is ${decl.name};\n`);
  }

  dtsFile.write(fs.readFileSync(path.join(PACKAGE_ROOT, 'snippets', 'ast-after.d.ts'), 'utf8'));

  return {
    jsFile: jsFile.currentText,
    dtsFile: dtsFile.currentText,
  };

  // Below are some useful functions

  function hasDeclarationNamed(name: string): boolean {
    return name in declByName;
  }

  function* getNodesTransitivelyReferencingNode(node: NodeDeclaration): IterableIterator<string> {
    const visited = new Set();
    const stack = [ node.name ];
    while (stack.length > 0) {
      const nodeName = stack.pop()!;
      visited.add(nodeName);
      for (const parentName of getNodesReferencingNode(nodeName)) {
        if (!visited.has(parentName)) {
          yield parentName;
          stack.push(parentName);
        }
      }
    }
  }

  function containsDeclarationInInheritanceChain(rootName: string, name: string): boolean {
    const decl = getDeclarationNamed(name);
    if (decl.type === 'NodeDeclaration') {
      for (const childName of getNodesDirectlyInheritingFrom(rootName)) {
        if (childName === name) {
          return true;
        }
        if (containsDeclarationInInheritanceChain(childName, name)) {
          return true;
        }
      }
      return false;
    } else {
      throw new Error(`Checking membership of other types of declarations is not supported.`);
    }
  }

  function* getNodesReferencingNode(name: string): IterableIterator<string> {
    const decl = getDeclarationNamed(name) as NodeDeclaration;
    for (const parentNode of finalNodes) {
      inner: for (const field of getAllFields(parentNode)) {
        if (typeReferencesDeclarationNamed(field.typeNode, name)) {
          yield parentNode.name;
          break inner;
        }
        for (const upperNodeName of getNodesDirectlyInheritingFrom(name)) {
          if (containsDeclarationInInheritanceChain(upperNodeName, name)) {
            yield parentNode.name; 
            break inner;
          }
        }
      }
    }
  }

  function typeReferencesDeclarationNamed(type: TypeNode, name: string): boolean {
    for (const declName of getAllDeclarationsInType(type)) {
      const decl = getDeclarationNamed(declName);
      if (decl.type === 'NodeDeclaration') {
        return containsDeclarationInInheritanceChain(decl.name, name)
      } else if (decl.type === 'TypeDeclaration') {
        return typeReferencesDeclarationNamed(decl.typeNode, name);
      }
    }
    return false;
  }
  
  function* getAllDeclarationsInType(typeNode: TypeNode): IterableIterator<string> {
    if (typeNode.type === 'ReferenceTypeNode') {
      if (typeNode.typeArgs === null) {
        if (hasDeclarationNamed(typeNode.name)) {
          yield typeNode.name;
        }
      } else {
        for (const arg of typeNode.typeArgs) {
          yield* getAllDeclarationsInType(arg);
        }
      }
    } else if (typeNode.type === 'UnionTypeNode') {
      for (const element of typeNode.elements) {
        yield* getAllDeclarationsInType(element); 
      }
    } else {
      throw new Error(`Could not infer declarations inside ${typeNode}.`)
    }
  }

  function isDeclarationInType(typeNode: TypeNode, declName: string): boolean {
    if (typeNode.type === 'ReferenceTypeNode') {
      if (typeNode.typeArgs === null) {
        return typeNode.name === declName;
      }
      return typeNode.typeArgs.some(arg => isDeclarationInType(arg, declName));
    } else if (typeNode.type === 'UnionTypeNode') {
      return typeNode.elements.some(el => isDeclarationInType(el, declName));
    }
    throw new Error(`Could not infer whether declaration ${declName} occurs in the given type node.`)
  }

  function emitTypeScriptType(typeNode: TypeNode): string {
    if (typeNode.type === 'ReferenceTypeNode') {
      if (hasDeclarationNamed(typeNode.name)) {
        return typeNode.name;
      } else if (typeNode.name === 'Option') {
        return `${emitTypeScriptType(typeNode.typeArgs[0])} | null`;
      } else if (typeNode.name === 'Vec') {
        return `${emitTypeScriptType(typeNode.typeArgs[0])}[]`;
      } else if (CUSTOM_TYPES.indexOf(typeNode.name) !== -1) {
        return typeNode.name;
      } else if (typeNode.name === 'String') {
        return `string`;
      } else if (typeNode.name === 'Int') {
        return `bigint`;
      } else if (typeNode.name === 'usize') {
        return `number`;
      } else if (typeNode.name === 'bool') {
        return `boolean`;
      } else {
        throw new Error(`Could not emit TypeScript type for reference type node named ${typeNode.name}`);
      }
    } else if (typeNode.type === 'UnionTypeNode') {
      return '(' + typeNode.elements.map(emitTypeScriptType).join(' | ') + ')';
    }
    throw new Error(`Could not emit TypeScript type for type node ${typeNode}`);
  }

  function getNodesDirectlyInheritingFrom(declName: string) {
    const children = childrenOf[declName];
    if (children === undefined) {
      return []
    }
    return children;
  }

  function *getParentChain(nodeName: string): IterableIterator<string> {
    const stack = [ nodeName ];
    while (stack.length > 0) {
      const nodeDecl = getDeclarationNamed(stack.pop()!) as NodeDeclaration;
      for (const parentName of nodeDecl.parents) {
        yield parentName;
        stack.push(parentName);
      }
    }
  }

  function* getFinalNodes(declName: string): IterableIterator<string> {
    const stack = [ declName ];
    while (stack.length > 0) {
      const nodeName = stack.pop()!;
      for (const childName of getNodesDirectlyInheritingFrom(nodeName)) {
        //const childDecl = getDeclarationNamed(childName)
        //if (childDecl.type !== 'NodeDeclaration') {
        //  throw new Error(`Node ${declName} has a child named '${childDecl.name}' that is not a node.`);
        //}
        if (isFinalNode(childName)) {
          yield childName;
        } else {
          stack.push(childName);
        }
      }
    }
  }

  function* getAllFields(nodeDecl: NodeDeclaration): IterableIterator<NodeField> {
    yield* nodeDecl.fields;
    if (isFinalNode(nodeDecl.name)) {
      for (const parentName of nodeDecl.parents) {
        const parentDecl = getDeclarationNamed(parentName);
        if (parentDecl.type !== 'NodeDeclaration') {
          throw new Error(`Parent declaration '${parentName}' of '${nodeDecl.name}' must be a node declaration.`);
        }
        yield* parentDecl.fields;
      }
    } else {
      for (const nodeName of getFinalNodes(nodeDecl.name)) {
        yield* getAllFields(getDeclarationNamed(nodeName) as NodeDeclaration);
      }
    }
  }

  function getDeclarationNamed(name: string): Declaration {
    const decl = declByName[name];
    if (decl === undefined) {
      throw new Error(`Declaration '${name}' was not found in any of the definition files.`);
    }
    return decl;
  }

  function hasChildren(name: string): boolean {
    return childrenOf[name] !== undefined && childrenOf[name].length !== 0;
  }

  function isFinalNode(name: string): boolean {
    const decl = getDeclarationNamed(name);
    if (decl.type !== 'NodeDeclaration') {
      return false;
    }
    return !hasChildren(name);
  }

}

function pushAll<T>(arr: T[], els: T[]): void {
  for (const el of els) {
    arr.push(el);
  }
}

function isNode(value: any): value is Syntax {
  return typeof value === 'object' && value !== null && value.__IS_NODE;
}

function jsonify(value: any) {

  function visitNode(node: any) {

    const obj: any = {};

    for (const key of Object.keys(node)) {
      if (key !== 'type' && key !== 'span' && key !== '__IS_NODE') {
        const value = node[key];
        if (Array.isArray(value)) {
          obj[key] = value.map(visit);
        } else {
          obj[key] = visit(value);
        }
      }
    }

    return obj;
  }

  function visit(value: any) {
    if (isNode(value)) {
      return visitNode(value);
    } else {
      return value;
    }
  }

  return visit(value);
}

function stripSuffix(str: string, suffix: string): string {
  if (!str.endsWith(suffix)) {
    return str;
  }
  return str.substring(0, str.length-suffix.length);
}

function getFileStem(filepath: string) {
  return path.basename(filepath).split('.')[0];
}

function uniq<T>(elements: Iterable<T>): T[] {
  const out: T[] = [];
  const visited = new Set<T>();
  for (const element of elements) {
    if (visited.has(element)) {
      continue;
    }
    visited.add(element);
    out.push(element);
  }
  return out;
}
