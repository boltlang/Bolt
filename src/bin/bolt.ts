#!/usr/bin/env node

import "source-map-support/register"

import fs from "fs"
import util from "util"
import path from "path"
import { Command } from "commander"

import { PassManager, Program, TargetType } from "../program"
import { TypeclassDictPassing } from "../passes/TypeclassDictPass"
import BoltToC from "../passes/BoltToC"
import BoltToJS from "../passes/BoltToJS"
import { stripExtension } from "../util"

function debug(value: any) {
  console.error(util.inspect(value, { colors: true, depth: Infinity }));
}

const program = new Command();

program
  .name('bolt')
  .description('The official Bolt language compiler')
  .version('0.0.1')
  .option('-C, --work-dir', 'Act as if run from this directory', '.');

program
  .command('build', 'Build a set of Bolt sources')
  .argument('<file>', 'Path to the Bolt program to compile')
  .option('--no-typecheck', 'Skip type-checking')
  .option('--no-emit', 'Do not output compiled files')
  .option('-t, --target <target-id>', 'What to compile to', 'c')
  .action((file, opts) => {

    const cwd = opts.workDir;
    const filename = path.resolve(cwd, file);
    const shouldTypecheck = opts.typecheck;
    const shouldEmit = opts.emit;

    let targetType: TargetType;
    switch (opts.target) {
      case 'bolt':
        targetType = TargetType.Bolt;
        break;
      case 'js':
        targetType = TargetType.JS;
        break;
      case 'c':
        targetType = TargetType.C;
        break;
      default:
        console.error(`Invalid target '${opts.target}' provided.`);
        process.exit(1);
    }

    const program = new Program([ filename ]);
    if (program.diagnostics.hasError) {
      process.exit(1);
    }

    if (shouldTypecheck) {
      program.check();
      if (program.diagnostics.hasError) {
        process.exit(1);
      }
    }

    if (shouldEmit) {

      const passes = new PassManager();
      passes.add(TypeclassDictPassing);

      let suffix;
      switch (targetType) {
        case TargetType.Bolt:
          suffix = '.gen.bolt';
          break;
        case TargetType.C:
          suffix = '.c';
          passes.add(BoltToC);
          break;
        case TargetType.JS:
          suffix = '.js'
          passes.add(BoltToJS);
          break;
      }

      for (const sourceFile of program.getSourceFiles()) {
        const code = passes.apply(sourceFile);
        const targetFilePath = stripExtension(sourceFile.getFile().getFullPath()) + suffix;
        const file = fs.createWriteStream(targetFilePath, 'utf-8'); 
        code.emit(file);
      }

      if (program.diagnostics.hasError) {
        process.exit(1);
      }

    }

  });

program.parse();

