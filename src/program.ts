
import { SourceFile } from "./ast"
import { FastStringMap } from "./util";

export class Program {

  private transformed = new FastStringMap<string, SourceFile>();

  constructor(
    sourceFiles: SourceFile[]
  ) {
    for (const sourceFile of sourceFiles) {
      this.transformed.set(sourceFile.span!.file.fullPath, sourceFile);
    }
  }

  public getAllSourceFiles() {
    return this.transformed.values();
  }

  public updateSourceFile(oldSourceFile: SourceFile, newSourceFile: SourceFile): void {
    if (!this.transformed.has(oldSourceFile.span!.file.fullPath)) {
      throw new Error(`Could not update ${oldSourceFile.span!.file.origPath} because it was not found in this program.`);
    }
    this.transformed.delete(oldSourceFile.span!.file.fullPath);
    this.transformed.set(newSourceFile.span!.file.fullPath, newSourceFile);
  }

}

