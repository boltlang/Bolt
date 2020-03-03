
import { TextFile } from "./ast"

export class Package {

  constructor(
    public name: string | null,
    public files: TextFile[],
  ) {

  }

}

