
import * as path from "path"
import * as fs from "fs"

export class TextFile {

  private cachedText: string | null = null;

  constructor(public origPath: string) {

  }

  public get fullPath() {
    return path.resolve(this.origPath)
  }

  public getText(): string {
    if (this.cachedText !== null) {
      return this.cachedText;
    }
    const text = fs.readFileSync(this.fullPath, 'utf8');
    this.cachedText = text;
    return text
  }

}

export class TextPos {

  constructor(
    public offset: number,
    public line: number,
    public column: number
  ) {

  }

  public clone() {
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

  public clone() {
    return new TextSpan(this.file, this.start.clone(), this.end.clone());
  }

}

