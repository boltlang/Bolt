import path from "path";
import fs from "fs"
import { parseSourceFile } from ".";
import { SourceFile, TextFile } from "./cst";
import { ConsoleDiagnostics, Diagnostics } from "./diagnostics";
import { Checker } from "./checker";
import { Analyser } from "./analysis";
import { Newable, Pass } from "./types";
import BoltToC from "./passes/BoltToC";
import BoltToJS from "./passes/BoltToJS";

type AnyPass = Pass<any, any>;

export enum TargetType {
  C,
  JS,
  WebAssembly,
  LLVM,
}

interface TargetSpec {
  type: TargetType;
}

export class PassManager {

  private registeredPasses: AnyPass[] = [];

  public add(pass: Newable<AnyPass>) {
    this.registeredPasses.push(new pass());
  }

  public apply<In>(input: In): unknown {
    for (const pass of this.registeredPasses) {
      input = pass.apply(input);
    }
    return input;
  }

}

export class Program {

  private sourceFilesByPath = new Map<string, SourceFile>();

  private analyser = new Analyser();

  public constructor(
    public fileNames: string[],
    public diagnostics: Diagnostics = new ConsoleDiagnostics(),
  ) {
    for (const fileName of fileNames) {
      const realPath = path.resolve(fileName);
      const text = fs.readFileSync(realPath, 'utf-8');
      const file = new TextFile(fileName, text);
      const sourceFile = parseSourceFile(file, diagnostics);
      if (sourceFile !== null) {
        this.sourceFilesByPath.set(realPath, sourceFile);
        this.analyser.addSourceFile(sourceFile);
      }
    }
  }

  public getSourceFiles(): Iterable<SourceFile> {
    return this.sourceFilesByPath.values();
  }

  public check(): void {
    const checker = new Checker(this.analyser, this.diagnostics);
    for (const sourceFile of this.getSourceFiles()) {
      checker.check(sourceFile);
    }
  }

  public emit(target: TargetSpec): void {
    let suffix;
    const passes = new PassManager();
    switch (target.type) {
      case TargetType.C:
        suffix = '.c';
        passes.add(BoltToC);
        break;
      case TargetType.JS:
        suffix = '.js'
        passes.add(BoltToJS);
        break;
    }
    for (const [sourceFilePath, sourceFile] of this.sourceFilesByPath) {
      const code = passes.apply(sourceFile) as any;
      const targetFilePath = stripExtension(sourceFilePath) + suffix;
      const file = fs.createWriteStream(targetFilePath, 'utf-8'); 
      code.emit(file);
    }
  }

}

function stripExtension(filepath: string): string {
  const basename = path.basename(filepath);
  const i = basename.lastIndexOf('.');
  if (i === -1) {
    return filepath;
  }
  return path.join(path.dirname(filepath), basename.substring(0, i));
}
