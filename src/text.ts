
import * as path from "path"
import * as fs from "fs"
import { serializeTag, serialize, deserializable } from "./util";

@deserializable()
export class TextFile {

  private cachedText: string | null = null;

  constructor(public origPath: string) {

  }

  public get fullPath() {
    return path.resolve(this.origPath)
  }

  [serializeTag]() {
    return [ this.origPath ];
  }

  public getText(encoding: BufferEncoding = 'utf8'): string {
    if (this.cachedText !== null) {
      return this.cachedText;
    }
    const text = fs.readFileSync(this.fullPath, encoding);
    this.cachedText = text;
    return text
  }

}

@deserializable()
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

  [serializeTag]() {
    return [
      this.offset,
      this.line,
      this.column,
    ]
  }

  public advance(str: string) {
    for (const ch of str) {
      if (ch === '\n') {
        this.line++;
        this.column = 1;
      } else {
        this.column++;
      }
      this.offset++;
    }
  }

}

@deserializable()
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

  [serializeTag]() {
    return [
      this.file,
      this.start,
      this.end,
    ]
  }

}

