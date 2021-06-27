#!/usr/bin/env node

import "source-map-support/register"

import * as fs from "fs"
import * as util from "util"
import yargs from "yargs"
import { Parser } from "./parser";
import { Scanner } from "./scanner";
import {ConsoleDiagnostics} from "./diagnostics";
import {TextFile} from "./text";

yargs
  .command('check [file..]', 'Type-check a project/file and run its tests', yargs => {}, args => {
    for (const fileName of toArray(args.file as string | string[])) {
      const text = fs.readFileSync(fileName, 'utf8');
      const file = new TextFile(fileName, text);
      const scanner = new Scanner(text);
      const diagnostics = new ConsoleDiagnostics();
      const parser = new Parser(diagnostics);
      const sourceFile = parser.parseSourceFile(scanner, { enableDiagnostics: true, file, });
      if (typeof(sourceFile) === 'number') {
        return;
      }
      console.log(util.inspect(sourceFile, { colors: true, depth: Infinity }));
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

