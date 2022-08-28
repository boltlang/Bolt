#!/usr/bin/env node

import "source-map-support/register"

import path from "path"
import fs from "fs"
import yargs from "yargs"

import { Diagnostics } from "../diagnostics"
import { Punctuator, Scanner } from "../scanner"
import { Parser } from "../parser"

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
      const scanner = new Scanner(text, 0, diagnostics);
      const punctuated = new Punctuator(scanner);
      const parser = new Parser(punctuated);
      const sourceFile = parser.parseSourceFile();

      console.log(sourceFile);

    }
  )
  .help()
  .version()
  .argv
