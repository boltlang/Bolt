import {TransformManager} from ".";
import {SourceFile} from "../program";
import {isBoltSourceFile, createBoltSourceFile, BoltSourceElement, SyntaxKind, BoltModule, isBoltModule} from "../ast";
import {setOrigNodeRange} from "../util";

export class EliminateModulesTransform {

  constructor(private transformers: TransformManager) {

  }

  public isApplicable(sourceFile: SourceFile) {
    return isBoltSourceFile(sourceFile);
  }

  public transform(sourceFile: SourceFile): SourceFile {

    let needsUpdate = false;
    const elements: BoltSourceElement[] = [];

    for (const element of sourceFile.elements) {
      if (element.kind === SyntaxKind.BoltModule) {
        this.extractModuleElements(element, elements);
        needsUpdate = true;
      } else {
        elements.push(element);
      }
    }

    if (!needsUpdate) {
      return sourceFile;
    }

    const newSourceFile = createBoltSourceFile(elements);
    setOrigNodeRange(newSourceFile, sourceFile, sourceFile);
    return newSourceFile;
  }

  public extractModuleElements(node: BoltModule, out: BoltSourceElement[]) {
    for (const element of node.elements) {
      switch (element.kind) {
        case SyntaxKind.BoltModule:
          this.extractModuleElements(node, out);
          break;
        case SyntaxKind.BoltRecordDeclaration:
          // TODO
          break;
      }
    }
  }

}

export default EliminateModulesTransform;

