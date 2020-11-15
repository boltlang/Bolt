
import * as path from "path"
import { getPackage } from "./common"
import { Package } from "./package"
import { SourceFile, Syntax } from "./ast"
import { FastStringMap, assert, isInsideDirectory, stripExtensions } from "./util";

export class Program {

  private packagesByName = new FastStringMap<string, Package>();

  private sourceFilesByFilePath = new FastStringMap<string, SourceFile>();

  constructor(
    private pkgs: Package[]
  ) {
    for (const pkg of pkgs) {
      if (pkg.name !== null) {
        this.packagesByName.set(pkg.name, pkg);
      }
      for (const sourceFile of pkg.getAllSourceFiles()) {
        this.sourceFilesByFilePath.set(stripExtensions(sourceFile.span!.file.fullPath), sourceFile);
      }
    }
  }

  public getAllSourceFiles() {
    return this.sourceFilesByFilePath.values();
  }

  public getSourceFile(filepath: string): SourceFile | null {
    assert(path.isAbsolute(filepath));
    if (!this.sourceFilesByFilePath.has(filepath)) {
      return null;
    }
    return this.sourceFilesByFilePath.get(filepath);
  }

  public getAllPackages(): IterableIterator<Package> {
    return this.pkgs[Symbol.iterator]();
  }

  public *getAllGloballyDeclaredSourceFiles(): IterableIterator<SourceFile> {
    for (const pkg of this.getAllPackages()) {
      if (pkg.isAutoImported) {
        const mainLibrarySourceFile = pkg.getMainLibrarySourceFile();
        if (mainLibrarySourceFile !== null) {
          yield mainLibrarySourceFile;
        }
      }
    }
  }

  public getPackageNamed(name: string): Package {
    return this.packagesByName.get(name);
  }

  public resolveToSourceFile(importPath: string, fromNode: Syntax): SourceFile | null {
    let resolvedFilePath: string;
    if (importPath.startsWith('.')) {
      const pkg = getPackage(fromNode);
      resolvedFilePath = path.join(pkg.rootDir, importPath.substring(2));
      assert(isInsideDirectory(resolvedFilePath, pkg.rootDir));
    } else {
      const elements = importPath.split('/');
      const pkg = this.getPackageNamed(elements[0]);
      let filename: string;
      if (elements.length === 1) {
        filename = 'lib';
      } else {
        assert(elements.length > 0);
        assert(!elements.slice(1).some(element => element.startsWith('.')));
        filename = elements.slice(1).join(path.sep);
      }
      resolvedFilePath = path.join(pkg.rootDir, filename)
      assert(isInsideDirectory(resolvedFilePath, pkg.rootDir));
    }
    return this.getSourceFile(resolvedFilePath);
  }

  public updateSourceFile(oldSourceFile: SourceFile, newSourceFile: SourceFile): void {
    if (!this.sourceFilesByFilePath.has(oldSourceFile.span!.file.fullPath)) {
      throw new Error(`Could not update ${oldSourceFile.span!.file.origPath} because it was not found in this program.`);
    }
    this.sourceFilesByFilePath.delete(oldSourceFile.span!.file.fullPath);
    this.sourceFilesByFilePath.set(newSourceFile.span!.file.fullPath, newSourceFile);
  }

}

