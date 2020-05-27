
import * as path from "path"
import * as fs from "fs-extra"
import { now } from "microtime"
import { EventEmitter } from "events"

import { Program } from "./program"
import { emitNode } from "./emitter"
import { Syntax, BoltSourceFile, SourceFile, NodeVisitor, createBoltConditionalCase } from "./ast"
import { getFileStem, MapLike } from "./util"
import { verbose, memoize } from "./util"
import { Container, Newable } from "./ioc"
import ExpandBoltTransform from "./transforms/expand"
import CompileBoltToJSTransform from "./transforms/boltToJS"
import ConstFoldTransform from "./transforms/constFold"
import { TransformManager } from "./transforms/index"
import {DiagnosticPrinter} from "./diagnostics"
import { TypeChecker } from "./types"
import { checkServerIdentity } from "tls"
import { CheckInvalidFilePaths, CheckTypeAssignments, CheckReferences } from "./checks"
import { SymbolResolver, BoltSymbolResolutionStrategy } from "./resolver"
import { Evaluator } from "./evaluator"
import { getNodeLanguage } from "./common"

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

  private runningTasks: MapLike<TimingInfo> = Object.create(null);

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

  //public resolver = new SymbolResolver();
  //public evaluator = new Evaluator(this.resolver);
  public diagnostics: DiagnosticPrinter;
  public timing: Timing;

  constructor() {
    this.diagnostics = new DiagnosticPrinter();
    this.timing = new Timing();
  }

  public check(program: Program) {

    const resolver = new SymbolResolver(program, new BoltSymbolResolutionStrategy);
    const checker = new TypeChecker(resolver);

    const container = new Container();
    container.bindSelf(program);
    container.bindSelf(resolver);
    container.bindSelf(checker);
    container.bindSelf(this.diagnostics);

    const checks: Newable<NodeVisitor>[] = [
       CheckInvalidFilePaths,
       CheckReferences,
       CheckTypeAssignments,
    ];
    
    const checkers = checks.map(check => container.createInstance(check));

    for (const sourceFile of program.getAllSourceFiles()) {
      resolver.registerSourceFile(sourceFile as BoltSourceFile);
    }
    for (const sourceFile of program.getAllSourceFiles()) {
      sourceFile.visit(checkers)
    }
  }

  public compile(program: Program, target: string) {

    const container = new Container();
    const resolver = new SymbolResolver(program, new BoltSymbolResolutionStrategy);
    for (const sourceFile of program.getAllSourceFiles()) {
      resolver.registerSourceFile(sourceFile as BoltSourceFile);
    }
    const transforms = new TransformManager(container);
    container.bindSelf(transforms);
    container.bindSelf(program);
    container.bindSelf(resolver);

    switch (target) {

      case "JS":
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
      fs.writeFileSync(this.mapToTargetFile(sourceFile), emitNode(sourceFile), 'utf8');
    }

  }

  private mapToTargetFile(node: SourceFile) {
    return path.join('.bolt-work', getFileStem(node.span!.file.fullPath) + getDefaultExtension(getNodeLanguage(node)));
  }

  public eval(program: Program) {
    const resolver = new SymbolResolver(program, new BoltSymbolResolutionStrategy);
    const checker = new TypeChecker(resolver);
    const evaluator = new Evaluator(checker)
    for (const sourceFile of program.getAllSourceFiles()) {
      evaluator.eval(sourceFile)
    }
  }

}

function getDefaultExtension(target: string) {
  if (targetExtensions[target] === undefined) {
    throw new Error(`Could not derive an appropriate extension for target "${target}".`)
  }
  return targetExtensions[target];
}

