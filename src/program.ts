import path from "path";
import fs from "fs"
import { parseSourceFile } from ".";
import { SourceFile, TextFile } from "./cst";
import { ConsoleDiagnostics, Diagnostics } from "./diagnostics";
import { Checker } from "./checker";
import { Analyser } from "./analysis";
import { generateCode } from "./codegen";
import { CEmitter } from "./cast";

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

  public emit(): void {
    for (const [filePath, sourceFile] of this.sourceFilesByPath) {
      const file = fs.createWriteStream(stripExtension(filePath) + '.c', 'utf-8'); 
      const emitter = new CEmitter(file);
      emitter.emit(generateCode(sourceFile));
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
