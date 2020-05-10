#!/usr/bin/env node

import * as path from "path"
import * as fs from "fs"

import { parse, SyntaxError } from "../treegen/parser"
import { Syntax, Declaration, NodeDeclaration, TypeDeclaration, EnumDeclaration, TypeNode, NodeField } from "../ast"
import { FileWriter } from "../util"
import minimist from "minimist"

const PACKAGE_ROOT = path.join(__dirname, '..', '..');

const argv = minimist(process.argv.slice(2));

const jsFilePath = argv['js-file'] ?? 'lib/ast.js';
const dtsFilePath = argv['dts-file'] ?? 'src/ast.d.ts';

for (const filename of argv._) {
  const contents = fs.readFileSync(filename, 'utf8');
  let decls: Declaration[];
  try {
    decls = parse(contents, { prefix: getFileStem(filename) });
  } catch (e) {
    if (e instanceof SyntaxError) {
      console.error(`${filename}:${e.location.start.line}:${e.location.start.column}: ${e.message}`);
      process.exit(1);
    } else {
      throw e;
    }
  }
  const { jsFile, dtsFile } = generateAST(decls);
  fs.writeFileSync(jsFilePath, jsFile, 'utf8');
  fs.writeFileSync(dtsFilePath, dtsFile, 'utf8');
}

interface FastStringMap<T> { [key: string]: T }

function generateAST(decls: Declaration[]) {

  let jsFile = new FileWriter();
  let dtsFile = new FileWriter();
  let i;

  // Sort declarations by category

  const nodeDecls: NodeDeclaration[] = decls.filter(decl => decl.type === 'NodeDeclaration') as NodeDeclaration[];
  const typeDecls: TypeDeclaration[] = decls.filter(decl => decl.type === 'TypeDeclaration') as TypeDeclaration[];
  const enumDecls: EnumDeclaration[] = decls.filter(decl => decl.type === 'EnumDeclaration') as EnumDeclaration[];

  const declByName: FastStringMap<Declaration> = Object.create(null);
  i = 0;
  for (const decl of decls) {
    decl.index = i++;
    declByName[decl.name] = decl;
  }

  // Generate a mapping from parent node to child node
  // This makes it easy to generate union types for the intermediate nodes.

  const childrenOf: FastStringMap<string[]> = Object.create(null);
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

  jsFile.write(`if (typeof module !== 'undefined') {\n  module.exports = exported;\n}\n\n`)

  // Write corresponding TypeScript declarations

  dtsFile.write(`\nexport const enum SyntaxKind {\n`);
  for (const decl of leafNodes) {
    dtsFile.write(`  ${decl.name} = ${decl.index}\n`);
  }
  dtsFile.write(`}\n\n`);

  for (const decl of leafNodes) {
    dtsFile.write(`export function create${decl.name}(`);
    for (const field of getAllFields(decl)) {
      dtsFile.write(`${field.name}: ${emitTypeScriptType(field.typeNode)}, `);
    }
    dtsFile.write(`span: TextSpan | null = null, origNodes: SyntaxRange | null = null);\n`);
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
    //throw new Error(`Could not emit TypeScript type for type node ${typeNode.type}`);
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
      throw new Error(`Declaration '${name}' is not a node declaration.`)
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

