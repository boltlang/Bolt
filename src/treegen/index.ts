
import * as fs from "fs"
import * as path from "path"

const PACKAGE_ROOT = path.resolve(__dirname, '..', '..');

const CUSTOM_TYPES = ['Package'];

import { Syntax, Declaration, NodeDeclaration, TypeDeclaration, EnumDeclaration, TypeNode, NodeField } from "./ast"
import { MapLike } from "../util"
import { FileWriter } from "./util"

export function generateAST(decls: Declaration[]) {

  let jsFile = new FileWriter();
  let dtsFile = new FileWriter();
  let i;

  // Sort declarations by category

  const nodeDecls: NodeDeclaration[] = decls.filter(decl => decl.type === 'NodeDeclaration') as NodeDeclaration[];
  const typeDecls: TypeDeclaration[] = decls.filter(decl => decl.type === 'TypeDeclaration') as TypeDeclaration[];
  const enumDecls: EnumDeclaration[] = decls.filter(decl => decl.type === 'EnumDeclaration') as EnumDeclaration[];
  const langNames: string[] = decls.filter(decl => decl.type === 'LanguageDeclaration').map(decl => decl.name);

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

  const leafNodes: NodeDeclaration[] = nodeDecls.filter(decl => isLeafNode(decl.name));

  // Write a JavaScript file that contains all AST definitions.

  jsFile.write(`\nconst NODE_TYPES = {\n`);
  jsFile.indent();
  for (const decl of leafNodes) {
    if (decl.type === 'NodeDeclaration' && isLeafNode(decl.name)) {
      jsFile.write(`'${decl.name}': {\n`);
      jsFile.indent();
      jsFile.write(`index: ${decl.index},\n`);
      jsFile.write(`fields: new Map([\n`);
      jsFile.indent();
      for (const field of getAllFields(decl)) {
        jsFile.write(`['${field.name}', ${JSON.stringify(jsonify(field.typeNode))}],\n`);
      }
      jsFile.dedent();
      jsFile.write(']),\n');
      jsFile.dedent();
      jsFile.write('},\n');
    }
  }
  jsFile.dedent();
  jsFile.write('};\n\n');

  jsFile.write(fs.readFileSync(path.join(PACKAGE_ROOT, 'src', 'treegen', 'ast-template.js'), 'utf8'));

  jsFile.write(`exported.kindToString = function (kind) {\n  switch (kind) {\n`);
  jsFile.indent(2);
  for (const leafNode of leafNodes) {
    jsFile.write(`case ${leafNode.index}: return '${leafNode.name}';\n`);
  }
  jsFile.dedent(2);
  jsFile.write(`  }\n}\n\n`);

  for (const decl of nodeDecls) {
    jsFile.write(`exported.is${decl.name} = function (value) {\n`);
    jsFile.indent();
    jsFile.write(`if (!isSyntax(value)) {\n  return false;\n}\n`);
    if (isLeafNode(decl.name)) {
      jsFile.write(`  return value.kind === ${decl.index};\n`);
    } else {
      jsFile.write('return ' + getAllChildren(decl).map(d => `value.kind === ${d.index}`).join(' || ') + '\n');
    }
    jsFile.dedent();
    jsFile.write(`}\n`);
  }

  jsFile.write(`\nif (typeof module !== 'undefined') {\n  module.exports = exported;\n}\n\n`)

  // Write corresponding TypeScript declarations

  dtsFile.write(fs.readFileSync(path.join(PACKAGE_ROOT, 'src', 'treegen', 'ast.dts.template'), 'utf8'));

  dtsFile.write(`\nexport const enum SyntaxKind {\n`);
  for (const decl of leafNodes) {
    dtsFile.write(`  ${decl.name} = ${decl.index},\n`);
  }
  dtsFile.write(`}\n\n`);

  for (const decl of decls) {
    if (decl.type === 'NodeDeclaration') {
      if (isLeafNode(decl.name)) {
        dtsFile.write(`export interface ${decl.name} extends SyntaxBase<SyntaxKind.${decl.name}> {\n`)
        dtsFile.indent()
        dtsFile.write(`kind: SyntaxKind.${decl.name};\n`);
        for (const field of getAllFields(decl)) {
          dtsFile.write(`${field.name}: ${emitTypeScriptType(field.typeNode)};\n`);
        }
        dtsFile.dedent();
        dtsFile.write(`}\n\n`);
      } else {
        dtsFile.write(`export type ${decl.name}\n`);
        let first = true;
        dtsFile.indent();
        for (const childDecl of getAllChildren(decl)) {
          dtsFile.write((first ? '=' : '|') + ' ' + childDecl.name + '\n');
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

  for (const langName of langNames) {
    dtsFile.write(`export type ${langName}Syntax\n`);
    let first = true;
    dtsFile.indent();
    for (const decl of leafNodes) {
      if (decl.name.startsWith(langName)) {
        dtsFile.write((first ? '=' : '|') + ' ' + decl.name + '\n');
        first = false;
      }
    }
    dtsFile.dedent();
    dtsFile.write('\n\n');
  }

  dtsFile.write(`export type Syntax\n`);
  let first = true;
  dtsFile.indent();
  for (const decl of leafNodes) {
    dtsFile.write((first ? '=' : '|') + ' ' + decl.name + '\n');
    first = false;
  }
  dtsFile.dedent();
  dtsFile.write('\n\n');

  dtsFile.write('export function kindToString(kind: SyntaxKind): string;\n\n');

  for (const decl of leafNodes) {
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

  return {
    jsFile: jsFile.currentText,
    dtsFile: dtsFile.currentText,
  };

  // Below are some useful functions

  function hasDeclarationNamed(name: string): boolean {
    return name in declByName;
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
      return typeNode.elements.map(emitTypeScriptType).join(' | ');
    }
    throw new Error(`Could not emit TypeScript type for type node ${typeNode}`);
  }

  function getAllChildren(nodeDecl: NodeDeclaration): NodeDeclaration[] {
    const out: NodeDeclaration[] = [];
    const childNames = childrenOf[nodeDecl.name];
    if (childNames === undefined || childNames.length === 0) {
      out.push(nodeDecl);

    } else {
      for (const childName of childNames) {
        const childDecl = getDeclarationNamed(childName)
        if (childDecl.type !== 'NodeDeclaration') {
          throw new Error(`Node ${nodeDecl.name} has a child named '${childDecl.name}' that is not a node.`);
        }
        pushAll(out, getAllChildren(childDecl));
      }
    }
    return out;
  }

  function getAllFields(nodeDecl: NodeDeclaration) {
    let out: NodeField[] = [];
    pushAll(out, nodeDecl.fields);
    for (const parentName of nodeDecl.parents) {
      const parentDecl = getDeclarationNamed(parentName);
      if (parentDecl.type !== 'NodeDeclaration') {
        throw new Error(`Parent declaration '${parentName}' of '${nodeDecl.name}' must be a node declaration.`);
      }
      pushAll(out, getAllFields(parentDecl));
    }
    return out;
  }

  function getDeclarationNamed(name: string): Declaration {
    const decl = declByName[name];
    if (decl === undefined) {
      throw new Error(`Declaration '${name}' was not found in any of the definition files.`);
    }
    return decl;
  }

  function isLeafNode(name: string): boolean {
    const decl = getDeclarationNamed(name);
    if (decl.type !== 'NodeDeclaration') {
      return false;
    }
    return childrenOf[name] === undefined || childrenOf[name].length === 0;
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

