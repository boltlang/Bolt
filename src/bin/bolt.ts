#!/usr/bin/env node

import "source-map-support/register"

import util from "util"
import path from "path"
import fs from "fs"
import yargs from "yargs"

import { Diagnostics, UnexpectedCharDiagnostic, UnexpectedTokenDiagnostic } from "../diagnostics"
import { Punctuator, ScanError, Scanner } from "../scanner"
import { ParseError, Parser } from "../parser"
import { Checker } from "../checker"
import { TextFile } from "../cst"

function debug(value: any) {
  console.error(util.inspect(value, { colors: true, depth: Infinity }));
}

yargs
  .string('work-dir')
  .describe('work-dir', 'Act as if run from this directory')
  .default('work-dir', '.')
  .alias('work-dir', 'C')
  .command(
    ['$0 <file>', 'exec <file>'],
    'Execute a Bolt script',
    yargs => yargs
      .string('file')
      .demandOption('file')
      .describe('file', 'Path to the script to execute')
    , args => {

      const cwd = args.C;
      const filename = path.resolve(cwd, args.file);

      const diagnostics = new Diagnostics();
      const text = fs.readFileSync(filename, 'utf8')
      const file = new TextFile(filename, text);
      const scanner = new Scanner(text, 0, diagnostics, file);
      const punctuated = new Punctuator(scanner);
      const parser = new Parser(file, punctuated);
      let sourceFile;
      try {
        sourceFile = parser.parseSourceFile();
      } catch (error) {
        if (error instanceof ParseError) {
          diagnostics.add(new UnexpectedTokenDiagnostic(error.file, error.actual, error.expected));
          return;
        }
        if (error instanceof ScanError) {
          diagnostics.add(new UnexpectedCharDiagnostic(error.file, error.position, error.actual));
          return;
        }
        throw error;
      }
      sourceFile.setParents();
      //debug(sourceFile.toJSON());
      const checker = new Checker(diagnostics);
      checker.check(sourceFile);

    }
  )
  .help()
  .version()
  .argv
