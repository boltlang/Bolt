#!/usr/bin/env node

import "source-map-support/register"
import "reflect-metadata"

import fs from "fs"
import util from "util"
import path from "path"
import { Command } from "commander"

import { PassManager, Program, TargetType } from "../program"
import { TypeclassDictPassing } from "../passes/TypeclassDictPass"
import BoltToC from "../passes/BoltToC"
import BoltToJS from "../passes/BoltToJS"
import { stripExtension } from "../util"
import { sync as which } from "which"
import { spawnSync } from "child_process"
import { ConsoleDiagnostics, DiagnosticKind, DiagnosticStore, TypeMismatchDiagnostic } from "../diagnostics"
import { Syntax, SyntaxKind, TextFile, isExpression, visitEachChild } from "../cst"
import { Analyser, Checker, parseSourceFile } from ".."
import { typesEqual } from "../types"

function debug(value: any) {
  console.error(util.inspect(value, { colors: true, depth: Infinity }));
}

// The positions of all program arguments which are not flags will be parsed
// into this structure.
const commandIndices = [];

for (let i = 2; i < process.argv.length; i++) {
  const arg = process.argv[i];
  if (!arg.startsWith('-')) {
    commandIndices.push(i);
  }
}

// Iterate in reverse over the command indices, such that bolt-self-test-run
// gets precedence over bolt-self-test
for (let i = commandIndices.length-1; i >= 0; i--) {

  const argvIndex = commandIndices[i];

  // Construct the binary name from the parts of which we stored the locations in `commandIndices`.
  // Build from the first command up until the command at index `i`
  const binaryName = 'bolt-' + commandIndices.slice(0, i+1).map(index => process.argv[index]).join('-');

  const binaryPath = which(binaryName, { nothrow: true });

  // Reconstruct the args list without the commands in `binaryName`
  const argv = [];
  for (let i = 2; i < argvIndex; i++) {
    const arg = process.argv[i];
    if (arg.startsWith('-')) {
      argv.push(arg);
    }
  }
  for (let i = argvIndex+1; i < process.argv.length; i++) {
    argv.push(process.argv[i]);
  }

  // Only execute and return if the command was actually found. Otherwise, try
  // the other possible commands or execute the default program if this was the
  // last iteration.
  if (binaryPath) {
    const exitCode = spawnSync(binaryPath, argv, { stdio: 'inherit' }).status;
    process.exit(exitCode || 0);
  }

}

const program = new Command();

program
  .name('bolt')
  .description('The official Bolt language compiler')
  .version('0.0.1')
  .option('-C, --work-dir', 'Act as if run from this directory', '.');

program.command('build', { isDefault: true })
  .description('Build a set of Bolt sources')
  .argument('<file>', 'Path to the Bolt program to compile')
  .option('-C, --work-dir', 'Act as if run from this directory', '.')
  .option('--no-typecheck', 'Skip type-checking')
  .option('--no-emit', 'Do not output compiled files')
  .option('-t, --target <target-id>', 'What to compile to', 'c')
  .action((fileName, opts) => {

    const cwd = opts.workDir;
    const filePath = path.resolve(cwd, fileName);
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

    const program = new Program([ filePath ]);
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

program.command('verify', { hidden: true })
  .description('Run verification tests')
  .argument('<file>', 'File with verification source')
  .action((fileName, _opts) => {

    const diagnostics = new DiagnosticStore();
    const realPath = path.resolve(fileName);
    const text = fs.readFileSync(realPath, 'utf-8');
    const file = new TextFile(fileName, text);

    const sourceFile = parseSourceFile(file, diagnostics);
    if (!sourceFile) {
      process.exit(1);
    }

    const analyser = new Analyser();
    analyser.addSourceFile(sourceFile);
    const checker = new Checker(analyser, diagnostics);
    checker.check(sourceFile);

    const realDiagnostics = new ConsoleDiagnostics();

    let annotationTotalCount = 0;
    let annotationErrorCount = 0;
    let diagnosticTotalCount = diagnostics.size;
    let diagnosticErrorCount = 0;

    const visit = (node: Syntax) => {
      if (isExpression(node)) {
        for (const annotation of node.annotations) {
          if (annotation.kind === SyntaxKind.TypeAnnotation) {
            const actual = checker.getTypeOfNode(node);
            const expected = checker.getTypeOfNode(annotation.typeExpr);
            if (!typesEqual(actual, expected)) {
              realDiagnostics.add(new TypeMismatchDiagnostic(actual, expected, [ node ], []));
              annotationErrorCount++;
            }
            annotationTotalCount++;
          }
        }
      }
      visitEachChild(node, visit);
    }
    visit(sourceFile);

    const uncaughtDiagnostics = new Set(diagnostics);

    // TODO check comments that did not match any diagnostic
    for (const [line, comment] of file.comments) {
      if (comment[0].kind === SyntaxKind.At && comment[1].kind === SyntaxKind.Identifier && comment[1].text === 'expect_diagnostic' && comment[2].kind === SyntaxKind.StringLiteral) {
        for (const diagnostic of uncaughtDiagnostics) {
          if (diagnostic.position && diagnostic.position.line === line+1 && DiagnosticKind[diagnostic.kind] === comment[2].contents) {
            uncaughtDiagnostics.delete(diagnostic);
          }
        }
      }
    }

    for (const diagnostic of uncaughtDiagnostics) {
      realDiagnostics.add(diagnostic);
    }

    console.log(`${annotationTotalCount} type annotation(s) verified, ${annotationErrorCount} error(s).`);
    console.log(`${diagnosticTotalCount} diagnostic(s) generated, ${uncaughtDiagnostics.size} unexpected.`);
    if (realDiagnostics.hasError) {
      process.exit(1);
    }
  });

program.parse();

