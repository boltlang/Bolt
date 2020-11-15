
import { TextFile, TextPos } from "../text"

import { JSScanner } from "./js/scanner"
import { JSParser } from "./js/parser"

export function parseForeignLanguage(langName: string, text: string, file: TextFile, offset: TextPos) {
  switch (langName) {
    case "JS":
      const scanner = new JSScanner(file, text, offset);
      const parser = new JSParser();
      return parser.parseJSSourceElementList(scanner)
    default:
      throw new Error(`Did not know how to parse a foreign language named ${langName}.`);
  }
}

