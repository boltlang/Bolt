
import { SourceFile } from "../program"
import { TransformManager } from "../transformers";

export class ConstFoldTransform {

  constructor(public transformers: TransformManager) {

  }

  public isApplicable(node: SourceFile): boolean {
    return true;
  }

  public transform(node: SourceFile): SourceFile {
    return node;
  }

}

export default ConstFoldTransform;

