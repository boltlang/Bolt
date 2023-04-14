import { Syntax } from "../cst";
import { JSNode, JSProgram } from "../js";
import { Pass } from "../types";

export class BoltToJS implements Pass<Syntax, JSNode> {

  public apply(input: Syntax): JSNode {
    return new JSProgram([]);
  }

}

export default BoltToJS;

