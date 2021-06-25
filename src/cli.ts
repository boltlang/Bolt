#!/usr/bin/env node

import "source-map-support/register"

import * as fs from "fs"
import * as util from "util"
import yargs from "yargs"
import { Parser } from "./parser";
import { Scanner } from "./scanner";

yargs
  .command('check [file..]', 'Type-check a project/file and run its tests', yargs => {}, args => {
    for (const fileName of toArray(args.file as string | string[])) {
      const text = fs.readFileSync(fileName, 'utf8');
      const scanner = new Scanner(text);
      const parser = new Parser();
      const sourceFile = parser.parseSourceFile(scanner);
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

