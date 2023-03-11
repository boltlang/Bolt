import path from "path";
import fs from "fs"
import { parseSourceFile } from ".";
import { SourceFile, TextFile } from "./cst";
import { ConsoleDiagnostics, Diagnostics } from "./diagnostics";
import { Checker } from "./checker";
import { Analyser } from "./analysis";
import { Newable, Pass } from "./types";

type AnyPass = Pass<any, any>;

export enum TargetType {
  Bolt,
  C,
  JS,
  WebAssembly,
  LLVM,
}

export class PassManager {

  private registeredPasses: AnyPass[] = [];

  public add(pass: Newable<AnyPass>) {
    this.registeredPasses.push(new pass());
  }

  public apply(input: any): any {
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

}
