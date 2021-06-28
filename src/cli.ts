#!/usr/bin/env node

import "source-map-support/register"

import * as fs from "fs"
import * as util from "util"
import yargs from "yargs"
import { ParseError, Parser } from "./parser";
import { Punctuator, Scanner } from "./scanner";
import { ConsoleDiagnostics } from "./diagnostics";
import { TextFile } from "./text";
import { TypeChecker, TypeEnv, TypingContext } from "./checker";

const forceExceptions = process.env['BOLT_FORCE_EXCEPTIONS']

yargs
  .command('check [file..]', 'Type-check a project/file and run its tests', yargs => {}, args => {
    for (const fileName of toArray(args.file as string | string[])) {
      const text = fs.readFileSync(fileName, 'utf8');
      const file = new TextFile(fileName, text);
      const scanner = new Scanner(text);
      const tokens = new Punctuator(scanner);
      const diagnostics = new ConsoleDiagnostics();
      const parser = new Parser(file, diagnostics, tokens);
      let sourceFile;
      try {
        sourceFile = parser.parseSourceFile();
      } catch (e) {
        if (e instanceof ParseError) {
          diagnostics.add(e.getDiagnostic());
          if (forceExceptions) {
            throw e;
          }
          process.exit(1);
        }
        throw e;
      }
      sourceFile.setParents()
      // console.log(util.inspect(sourceFile, { colors: true, depth: Infinity }));
      const ctx = new TypingContext(diagnostics);
      const typeEnv = new TypeEnv(ctx);
      typeEnv.addDefault()
      const checker = new TypeChecker(diagnostics, ctx);
      checker.forwardDeclare(sourceFile, typeEnv);
      checker.infer(sourceFile, typeEnv, ctx.constraints);
      checker.solve();
    }
  })
  .demandCommand()
  .argv

function toArray<T>(value: T | T[]): T[] {
  if (Array.isArray(value)) {
    return value;
  }
  return value === undefined || value === null ? [] : [ value ]
}

