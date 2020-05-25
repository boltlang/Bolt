
import * as path from "path"
import * as fs from "fs-extra"
import { now } from "microtime"
import { EventEmitter } from "events"

import { Program } from "./program"
import { TypeChecker } from "./checker"
import { Evaluator } from "./evaluator"
import { emit } from "./emitter"
import { Syntax, BoltSourceFile, SourceFile } from "./ast"
import { upsearchSync, FastStringMap, getFileStem, getLanguage } from "./util"
import { Package } from "./package"
import { verbose, memoize } from "./util"
import { Container } from "./di"
import ExpandBoltTransform from "./transforms/expand"
import CompileBoltToJSTransform from "./transforms/boltToJS"
import ConstFoldTransform from "./transforms/constFold"
import EliminateModulesTransform from "./transforms/eliminateModules"
import { TransformManager } from "./transforms/index"
import {DiagnosticPrinter} from "./diagnostics"

const targetExtensions: MapLike<string> = {
  'JS': '.mjs',
  'Bolt': '.bolt',
  'C': '.c',
};

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

export class Frontend {

  public evaluator: Evaluator;
  public checker: TypeChecker;
  public diagnostics: DiagnosticPrinter;
  public timing: Timing;
  
  private container = new Container();

  constructor() {
    this.diagnostics = new DiagnosticPrinter();
    this.checker = new TypeChecker(this.diagnostics);
    this.evaluator = new Evaluator(this.checker);
    this.timing = new Timing();
    this.container.bindSelf(this.evaluator);
    this.container.bindSelf(this.checker);
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

  public typeCheck(program: Program) {
    for (const sourceFile of program.getAllSourceFiles()) {
      this.checker.registerSourceFile(sourceFile as BoltSourceFile);
    }
    for (const sourceFile of program.getAllSourceFiles()) {
      this.checker.checkSourceFile(sourceFile as BoltSourceFile);
    }
  }

  public compile(program: Program, target: string) {

    switch (target) {

      case "JS":
        const transforms = new TransformManager(this.container);
        transforms.register(ExpandBoltTransform);
        transforms.register(CompileBoltToJSTransform);
        transforms.register(ConstFoldTransform);
        transforms.apply(program);
        break;

      default:
        throw new Error(`"${target}" is an invalid compile target.`);

    }

    for (const sourceFile of program.getAllSourceFiles()) {
      fs.mkdirp('.bolt-work');
      fs.writeFileSync(this.mapToTargetFile(sourceFile), emit(sourceFile), 'utf8');
    }

  }

  private mapToTargetFile(node: SourceFile) {
    return path.join('.bolt-work', getFileStem(node.span!.file.fullPath) + getDefaultExtension(getLanguage(node)));
  }

  public eval(program: Program) {
    for (const sourceFile of program.getAllSourceFiles()) {
      this.evaluator.eval(sourceFile)
    }
  }

}

function getDefaultExtension(target: string) {
  if (targetExtensions[target] === undefined) {
    throw new Error(`Could not derive an appropriate extension for target "${target}".`)
  }
  return targetExtensions[target];
}

