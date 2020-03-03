
import * as path from "path"
import * as fs from "fs"

import { FastStringMap } from "./util"
import { Parser } from "./parser"
import { TypeChecker } from "./checker"
import { Evaluator } from "./evaluator"
import { Expander } from "./expander"
import { Scanner } from "./scanner"
import { Compiler } from "./compiler"
import { TextFile, SourceFile } from "./ast"

export class Program {

  parser: Parser
  evaluator: Evaluator;
  checker: TypeChecker;
  expander: Expander;

  sourceFiles = new Map<TextFile, SourceFile>();

  constructor(public files: TextFile[]) {
    this.checker = new TypeChecker();
    this.parser = new Parser();
    this.evaluator = new Evaluator(this.checker);
    this.expander = new Expander(this.parser, this.evaluator, this.checker);
    for (const file of files) {
      const contents = fs.readFileSync(file.fullPath, 'utf8');
      const scanner = new Scanner(file, contents)
      this.sourceFiles.set(file, scanner.scan());
    }
  }

  compile(file: TextFile) {
    const original = this.sourceFiles.get(file);
    if (original === undefined) {
      throw new Error(`File ${file.path} does not seem to be part of this Program.`)
    }
    const expanded = this.expander.getFullyExpanded(original) as SourceFile;
    const compiler = new Compiler(this.checker, { target: "JS" })
    const compiled = compiler.compile(expanded)
    return compiled
  }

  eval(file: TextFile) {
    const original = this.sourceFiles.get(file);
    if (original === undefined) {
      throw new Error(`File ${file.path} does not seem to be part of this Program.`)
    }
    const expanded = this.expander.getFullyExpanded(original) as SourceFile;
    return this.evaluator.eval(expanded)
  }

}

