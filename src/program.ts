
import * as path from "path"
import * as fs from "fs-extra"
import * as crypto from "crypto"

import { Parser } from "./parser"
import { TypeChecker } from "./checker"
import { Evaluator } from "./evaluator"
import { Expander } from "./expander"
import { Scanner } from "./scanner"
import { Compiler } from "./compiler"
import { emit } from "./emitter"
import { TextFile, SourceFile, AnySourceFile } from "./ast"
import { upsearchSync, FastStringMap } from "./util"
import { Package } from "./package"

const targetExtensions: FastStringMap<string> = {
  'JS': '.mjs'
};

export class Program {

  public parser: Parser
  public evaluator: Evaluator;
  public checker: TypeChecker;
  public expander: Expander;

  private sourceFiles = new Map<string, SourceFile>();
  private packages: FastStringMap<Package> = Object.create(null);

  constructor(files: TextFile[]) {
    this.checker = new TypeChecker();
    this.parser = new Parser();
    this.evaluator = new Evaluator(this.checker);
    this.expander = new Expander(this.parser, this.evaluator, this.checker);
    for (const file of files) {
      const contents = fs.readFileSync(file.fullPath, 'utf8');
      const scanner = new Scanner(file, contents)
      this.sourceFiles.set(file.fullPath, scanner.scan());
    }
  }

  public getPackage(filepath: string) {
    filepath = path.resolve(filepath);
    const projectFile = upsearchSync('Boltfile', path.dirname(filepath));
    if (projectFile === null) {
      return null;
    }
    const projectDir = path.resolve(path.dirname(projectFile));
    if (this.packages[projectDir] !== undefined) {
      return this.packages[projectDir];
    }
    return this.packages[projectDir] = new Package(projectDir);
  }

  public compile(target: string) {
    const compiler = new Compiler(this, this.checker, { target })
    const expanded: SourceFile[] = [];
    for (const [filepath, sourceFile] of this.sourceFiles) {
      expanded.push(this.expander.getFullyExpanded(sourceFile) as SourceFile);
    }
    const compiled = compiler.compile(expanded) as AnySourceFile[];
    for (const rootNode of compiled) {
      const filepath = rootNode.span!.file.fullPath;
      const pkg = this.getPackage(filepath);
      if (pkg !== null) {
        
      }
      fs.writeFileSync(this.mapToTargetFile(rootNode), emit(node), 'utf8');
    }
  }

  private mapToTargetFile(node: AnySourceFile) {
    getFileStem(node.span.file.fullPath) + '.' + getDefaultExtension(getTargetLanguage(node.kind));
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

function getDefaultExtension(target: string) {
  if (targetExtensions[target] === undefined) {
    throw new Error(`Could not derive an appropriate extension for target "${target}".`)
  }
  return targetExtensions[target];
}

function getFileStem(filepath: string): string {
  const chunks = path.basename(filepath).split('.')
  return chunks[chunks.length-1];
}

