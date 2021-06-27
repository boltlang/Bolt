
import * as fs from "fs"
import chalk from "chalk"
import {countDigits} from "./util";

const BOLT_DIAG_NUM_EXTRA_LINES = 2;

export class TextFile {

  constructor(
    public origPath: string,
    public cachedText: string | null = null
  ) {

  }

  public getText(): string {
    if (this.cachedText === null) {
      return this.cachedText = fs.readFileSync(this.origPath, 'utf8');
    }
    return this.cachedText;
  }

}

export interface TextPosition {
  offset: number;
  line: number;
  column: number;
}

export type TextRange = [TextPosition, TextPosition];

export type TextSpan = [number, number];

function printPosition(file: TextFile, position: TextPosition) {
  return chalk.bold.yellow(`${file.origPath}:${position.line}:${position.column}`)
}

export interface PrintExcerptOptions {
  indentation?: string;
  highlightColor?: 'red' | 'blue' | 'yellow' | 'magenta' | 'green'
  highlightRange?: TextRange | null;
}

export function formatExcerpt(
  content: string,
  span: TextRange, {
  indentation = '  ',
  highlightRange,
  highlightColor = 'red'
}: PrintExcerptOptions = {}) {

  if (highlightRange === undefined) {
    highlightRange = span;
  }

  let out = '';

  const startLine = Math.max(0, span[0].line-1-BOLT_DIAG_NUM_EXTRA_LINES)
  const lines = content.split('\n')
  const endLine = Math.min(lines.length, (span[1] !== undefined ? span[1].line : startLine)+BOLT_DIAG_NUM_EXTRA_LINES)
  const gutterWidth = Math.max(2, countDigits(endLine+1))

  for (let i = startLine; i < endLine; i++) {

    const line = lines[i];

    let j = firstIndexOfNonEmpty(line);

    out +=  indentation + '  '+chalk.bgWhite.black(' '.repeat(gutterWidth-countDigits(i+1))+(i+1).toString())+' '+line+'\n'

    if (highlightRange) {

      const gutter = indentation + '  '+chalk.bgWhite.black(' '.repeat(gutterWidth))+' '

      let mark: number;
      let skip: number;

      if (i === highlightRange[0].line-1 && i === highlightRange[1].line-1) {
        skip = highlightRange[0].column-1;
        mark = highlightRange[1].column-highlightRange[0].column;
      } else if (i === highlightRange[0].line-1) {
        skip = highlightRange[0].column-1;
        mark = line.length-highlightRange[0].column+1;
      } else if (i === highlightRange[1].line-1) {
        skip = 0;
        mark = highlightRange[1].column-1;
      } else if (i > highlightRange[0].line-1 && i < highlightRange[1].line-1) {
        skip = 0;
        mark = line.length;
      } else {
        continue;
      }

      if (j <= skip) {
        j = 0;
      }

      out += gutter+' '.repeat(j+skip)+chalk[highlightColor]('~'.repeat(mark-j)) + '\n'
    }

  }
  return out;
}

function firstIndexOfNonEmpty(str: string) {
  let j = 0;
  for (; j < str.length; j++) {
    const ch = str[j];
    if (ch !== ' ' && ch !== '\t') {
      break;
    }
  }
  return j
}

