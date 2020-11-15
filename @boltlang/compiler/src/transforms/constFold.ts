
import { SourceFile } from "../ast"

export class ConstFoldTransform {

  public isApplicable(node: SourceFile): boolean {
    return true;
  }

  public transform(node: SourceFile): SourceFile {
    return node;
  }

}

export default ConstFoldTransform;

