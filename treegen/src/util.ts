
import * as path from "path"

export function getFileStem(filepath: string): string {
  return path.basename(filepath).split('.')[0];
}

export interface MapLike<T> { [key: string]: T }

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
