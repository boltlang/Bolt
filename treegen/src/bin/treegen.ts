#!/usr/bin/env node

import * as path from "path"
import * as fs from "fs"

import { parse, SyntaxError } from "../parser"
import { Declaration, NodeDeclaration, TypeDeclaration, EnumDeclaration, TypeNode, NodeField } from "../ast"
import { FileWriter } from "../util"

for (const filename of process.argv.slice(2)) {
  const contents = fs.readFileSync(filename, 'utf8');
  let decls: Declaration[];
  try {
    decls = parse(contents);
  } catch (e) {
    if (e instanceof SyntaxError) {
      console.error(`${filename}:${e.location.start.line}:${e.location.start.column}: ${e.message}`);
      process.exit(1);
    } else {
      throw e;
    }
  }
  const generated = generateAST(decls, getFileStem(filename));
  fs.writeFileSync('src/ast-generated.js', generated, 'utf8');
}

interface FastStringMap<T> { [key: string]: T }

function generateAST(decls: Declaration[], langName: string) {

  let jsFile = new FileWriter();

  const nodeDecls: NodeDeclaration[] = decls.filter(decl => decl.type === 'NodeDeclaration') as NodeDeclaration[];
  const typeDecls: TypeDeclaration[] = decls.filter(decl => decl.type === 'TypeDeclaration') as TypeDeclaration[];
  const enumDecls: EnumDeclaration[] = decls.filter(decl => decl.type === 'EnumDeclaration') as EnumDeclaration[];

  const declByName: FastStringMap<Declaration> = Object.create(null);
  for (const decl of decls) {
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

  // Write a JavaScript file that contains all AST definitions.

  jsFile.write(`\nconst NODE_TYPES = [\n`);
  jsFile.indent();
  for (const decl of decls) {
    if (decl.type === 'NodeDeclaration' && isLeafNode(decl.name)) {
      jsFile.write(`'${decl.name}': new Map([\n`);
      jsFile.indent();
      for (const field of getAllFields(decl)) {
        jsFile.write(`[${field.name}, ${emitTypeNode(field.typeNode)}],\n`);
      }
      jsFile.dedent();
      jsFile.write(']),\n');
    }
  }
  jsFile.dedent();
  jsFile.write('];\n\n');

  function emitTypeNode(typeNode: TypeNode): string {
    console.error(typeNode);
    if (typeNode.type === 'ReferenceTypeNode') {
      if (hasDeclarationNamed(typeNode.name)) {
        return typeNode.name;
      } else if (typeNode.name === 'Bool') {
        return 'boolean';
      } else if (typeNode.name === 'String') {
        return 'string';
      } else if (typeNode.name === 'usize') {
        return 'bigint';
      } else if (typeNode.name === 'Vec') {
        return `${emitTypeNode(typeNode.typeArgs[0])}[]`;
      } else if (typeNode.name === 'Option') {
        return `${emitTypeNode(typeNode.typeArgs[0])} | null`;
      }
    }
    throw new Error(`Could not emit TypeScript type for type node ${typeNode.type}.`);
  }

  function hasDeclarationNamed(name: string): boolean {
    return name in declByName;
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

  return jsFile.currentText;

}

function pushAll<T>(arr: T[], els: T[]): void {
  for (const el of els) {
    arr.push(el);
  }
}

function jsonify(value: any) {

  function visitNode(node: any) {

    const obj: any = {};

    for (const key of Object.keys(node)) {
      if (key !== 'type' && key !== 'span') {
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
    if (value.__IS_NODE) {
      return visitNode(value);
    } else {
      return value;
    }
  }

  return visit(value);
}

function getFileStem(filepath: string) {
  return path.basename(filepath).split('.')[0];
}

