import { SourceFile, TextFile } from "./cst";
import { Diagnostics, UnexpectedCharDiagnostic, UnexpectedTokenDiagnostic } from "./diagnostics";
import { ParseError, Parser } from "./parser";
import { Punctuator, ScanError, Scanner } from "./scanner";

export function parseSourceFile(file: TextFile, diagnostics: Diagnostics): SourceFile | null {
  const scanner = new Scanner(file.text, 0, diagnostics, file);
  const punctuated = new Punctuator(scanner);
  const parser = new Parser(file, punctuated);
  let sourceFile;
  try {
    sourceFile = parser.parseSourceFile();
  } catch (error) {
    if (error instanceof ParseError) {
      diagnostics.add(new UnexpectedTokenDiagnostic(error.file, error.actual, error.expected));
      return null;
    }
    if (error instanceof ScanError) {
      diagnostics.add(new UnexpectedCharDiagnostic(error.file, error.position, error.actual));
      return null;
    }
    throw error;
  }
  sourceFile.setParents();
  return sourceFile;
}

