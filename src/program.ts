
import * as path from "path"
import * as fs from "fs-extra"
import { now } from "microtime"
import { EventEmitter } from "events"

import { Parser } from "./parser"
import { TypeChecker } from "./checker"
import { Evaluator } from "./evaluator"
import { Expander } from "./expander"
import { Scanner } from "./scanner"
import { Compiler } from "./compiler"
import { emit } from "./emitter"
import { TextFile } from "./text"
import { BoltSourceFile, Syntax, JSSourceFile } from "./ast"
import { upsearchSync, FastStringMap, getFileStem, getLanguage } from "./util"
import { Package } from "./package"
import { verbose, memoize } from "./util"

const targetExtensions: FastStringMap<string> = {
  'JS': '.mjs',
  'Bolt': '.bolt',
  'C': '.c',
};

export interface TransformationContext {
  
}

interface TimingInfo {
  timestamp: number;
  refCount: number;
}

class Timing extends EventEmitter {

  private runningTasks: FastStringMap<TimingInfo> = Object.create(null);

  public start(name: string) {
    if (this.runningTasks[name] !== undefined) {
      this.runningTasks[name].refCount++;
      return;
    }
    this.runningTasks[name] = { timestamp: now(), refCount: 1 };
    this.emit(`start ${name}`);
  }

  public end(name: string) {
    if (this.runningTasks[name] === undefined) {
      throw new Error(`Task '${name}' was never started.`);
    }
    const info = this.runningTasks[name];
    info.refCount--;
    if (info.refCount === 0) {
      const usecs = now() - info.timestamp;
      verbose(`Task '${name}' completed after ${usecs} microseconds.`);
      this.emit(`end ${name}`);
    }
  }

}

export class Program {

  public parser: Parser
  public evaluator: Evaluator;
  public checker: TypeChecker;
  public expander: Expander;
  public timing: Timing;

  constructor(public files: string[]) {
    this.checker = new TypeChecker();
    this.parser = new Parser();
    this.evaluator = new Evaluator(this.checker);
    this.expander = new Expander(this.parser, this.evaluator, this.checker);
    this.timing = new Timing();
  }

  @memoize
  public getTextFile(filename: string): TextFile {
    return new TextFile(filename);
  }

  @memoize
  public getSourceFile(file: TextFile): BoltSourceFile {
    this.timing.start('read');
    const contents = fs.readFileSync(file.origPath, 'utf8');
    this.timing.end('read');
    const scanner = new Scanner(file, contents)
    this.timing.start('scan');
    const sourceFile = scanner.scan();
    this.timing.end('scan');
    return sourceFile;
  }

  @memoize
  public getFullyExpandedSourceFile(file: TextFile): BoltSourceFile {
    const sourceFile = this.getSourceFile(file);
    this.timing.start('expand');
    const expanded = this.expander.getFullyExpanded(sourceFile) as BoltSourceFile;
    this.timing.end('expand');
    return expanded;
  }

  @memoize
  public getPackage(filepath: string) {
    const file = this.getTextFile(filepath)
    const projectFile = upsearchSync('Boltfile', path.dirname(file.fullPath));
    if (projectFile === null) {
      return null;
    }
    const projectDir = path.resolve(path.dirname(projectFile));
    return new Package(projectDir);
  }

  public compile(target: string) {
    const compiler = new Compiler(this, this.checker, { target })
    const expanded = this.files.map(filename => this.getFullyExpandedSourceFile(this.getTextFile(filename)));
    const compiled = compiler.compile(expanded) as JSSourceFile[];
    for (const rootNode of compiled) {
      //const filepath = rootNode.span!.file.fullPath;
      //const pkg = this.getPackage(filepath);
      //if (pkg !== null) {
      //
      //}
      fs.mkdirp('.bolt-work');
      fs.writeFileSync(this.mapToTargetFile(rootNode), emit(rootNode), 'utf8');
    }
  }

  private mapToTargetFile(node: Syntax) {
    return path.join('.bolt-work', getFileStem(node.span!.file.fullPath) + getDefaultExtension(getLanguage(node)));
  }

  public eval() {
    for (const filename of this.files) {
      const file = this.getTextFile(filename);
      const expanded = this.getFullyExpandedSourceFile(file);
      this.evaluator.eval(expanded)
    }
  }

}

function getDefaultExtension(target: string) {
  if (targetExtensions[target] === undefined) {
    throw new Error(`Could not derive an appropriate extension for target "${target}".`)
  }
  return targetExtensions[target];
}

