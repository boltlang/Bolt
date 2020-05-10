
import * as path from "path"

export class TextFile {

  constructor(public origPath: string) {

  }

  get fullPath() {
    return path.resolve(this.origPath)
  }

}

export class TextPos {

  constructor(
    public offset: number,
    public line: number,
    public column: number
  ) {

  }

  clone() {
    return new TextPos(this.offset, this.line, this.column)
  }

}

export class TextSpan {

  constructor(
    public file: TextFile,
    public start: TextPos,
    public end: TextPos
  ) {

  }

  clone() {
    return new TextSpan(this.file, this.start.clone(), this.end.clone());
  }

}

