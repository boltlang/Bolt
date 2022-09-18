#!/usr/bin/env node

import "source-map-support/register"

import util from "util"
import path from "path"
import fs from "fs"
import yargs from "yargs"

import { ConsoleDiagnostics, Diagnostics, UnexpectedCharDiagnostic, UnexpectedTokenDiagnostic } from "../diagnostics"
import { Punctuator, ScanError, Scanner } from "../scanner"
import { ParseError, Parser } from "../parser"
import { Checker } from "../checker"
import { SourceFile, TextFile } from "../cst"
import { parseSourceFile } from ".."
import { Analyser } from "../analysis"
import { Program, TargetType } from "../program"

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

      const program = new Program([ filename ]);
      if (program.diagnostics.hasError) {
        process.exit(1);
      }
      program.check();
      if (program.diagnostics.hasError) {
        process.exit(1);
      }
      program.emit({ type: TargetType.JS });
      if (program.diagnostics.hasError) {
        process.exit(1);
      }
    }
  )
  .help()
  .version()
  .argv
