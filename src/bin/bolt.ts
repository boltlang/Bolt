#!/usr/bin/env node

import "source-map-support/register"

import util from "util"
import path from "path"
import { Command } from "commander"

import { Program, TargetType } from "../program"

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
  .action((file, opts) => {

    const cwd = opts.workDir;
    const filename = path.resolve(cwd, file);

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

  });

program.parse();

