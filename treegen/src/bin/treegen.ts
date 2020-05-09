#!/usr/bin/env node

import * as path from "path"
import * as fs from "fs"

import { parse, SyntaxError } from "../parser"
import { Declaration } from "../ast"

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
  console.error(jsonify(decls));
  const generated = generateAST(decls);
  fs.writeFileSync(getFileStem(filename).toLowerCase(), generated, 'utf8');
}

interface FastStringMap<T> { [key: string]: T }

function generateAST(decls: Declaration[]) {

  const declByName: FastStringMap<Declaration> = Object.create(null);
  for (const decl of decls) {
    declByName[decl.name] = decl;
  }

  return ''

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

