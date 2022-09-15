#!/usr/bin/env node

import "source-map-support/register"

import * as commonmark from "commonmark"
import { sync as globSync } from "glob"
import fs from "fs";
import path from "path";
import yargs from "yargs";
import { Checker } from "../checker";
import { TextFile } from "../cst";
import { ConsoleDiagnostics } from "../diagnostics";
import { parseSourceFile } from "..";
import { Analyser } from "../analysis";

const projectDir = path.resolve(__dirname, '..', '..');

interface Test {
  code: string;
}

yargs
  .command('run', 'Execute the current working tree and optionally save to disk', yargs => {}, async (args) => {
    for await (const test of loadTests()) {
      console.log('--------');
      console.log(test.code);
      const diagnostics = new ConsoleDiagnostics();
      const file = new TextFile("#<anonymous>", test.code);
      let sourceFile = parseSourceFile(file, diagnostics);
      if (sourceFile !== null) {
        sourceFile.setParents();
        const analyser = new Analyser();
        analyser.addSourceFile(sourceFile);
        const checker = new Checker(analyser, diagnostics);
        checker.check(sourceFile);
      }
    }
  })
  .argv;

async function* loadTests(): AsyncIterable<Test> {
  for (const filename of globSync(path.join(projectDir, 'src', 'test', '**', '*.md'))) {
    const text = await fs.promises.readFile(filename, 'utf-8');
    const reader = new commonmark.Parser();
    const root = reader.parse(text);
    let child = root.firstChild;
    while (child !== null) {
      if (child.type === 'code_block') {
        yield { code: child.literal! };
      }
      child = child.next;
    }
  }
}

