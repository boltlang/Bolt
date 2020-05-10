
import * as path from "path"
import * as fs from "fs"

export interface FastStringMap<T> {
  [key: string]: T
}

export interface Stream<T> {
  get(): T;
  peek(count?: number): T;
}

export class StreamWrapper<T> {

  offset = 0

  constructor(protected data: T[], protected createSentry: () => T) {

  }

  peek(count = 1) {
    const offset = this.offset + (count - 1);
    if (offset >= this.data.length) {
      return this.createSentry();
    }
    return this.data[offset];
  }

  get() {
    if (this.offset >= this.data.length) {
      return this.createSentry();
    }
    return this.data[this.offset++];
  }

}

export function upsearchSync(filename: string, startDir = '.') {
  let currDir = startDir;
  while (true) {
    const filePath = path.join(currDir, filename);
    if (fs.existsSync(filePath)) {
      return filePath
    }
    const  { root, dir } = path.parse(currDir);
    if (dir === root) {
      return null;
    }
    currDir = dir;
  }
}

function isWhiteSpace(ch: string) {
  return /[\r\t ]/.test(ch);
}

export interface FileWriterOptions {
  indentStr?: string;
  startIndent?: number;
  indentWidth?: number;
}

export class FileWriter {

  public currentText = '';

  private atBlankLine = true;
  private currentIndent: number;
  private indentStr: string;
  private indentWidth: number;

  constructor(opts: FileWriterOptions = {}) {
    this.indentStr = opts.indentStr ?? ' ';
    this.indentWidth = opts.indentWidth ?? 2;
    this.currentIndent = (opts.startIndent ?? 0) * this.indentWidth;
  }

  public indent(count = 1) {
    this.currentIndent += this.indentWidth * count;
  }

  public dedent(count = 1) {
    this.currentIndent -= this.indentWidth * count;
  }

  public write(str: string) {
    for (const ch of str) {
      if (ch === '\n') {
        this.atBlankLine = true;
      } else if (!(this.atBlankLine && isWhiteSpace(ch))) {
        if (this.atBlankLine) {
          this.currentText += this.indentStr.repeat(this.currentIndent)
        }
        this.atBlankLine = false;
      }
      this.currentText += ch;
    }
  }

}
