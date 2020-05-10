#!/usr/bin/env node

import * as path from "path"
import * as fs from "fs"

import { parse, SyntaxError } from "../treegen/parser"
import { Declaration } from "../treegen/ast"
import { generateAST } from "../treegen/index"
import { getFileStem } from "../treegen/util"
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

