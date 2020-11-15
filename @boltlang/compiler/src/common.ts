import {
  BoltFunctionBodyElement,
  BoltReturnStatement,
  SyntaxKind,
  BoltExpression,
  kindToString,
  Syntax,
  Token,
  isBoltPunctuated,
  BoltSourceFile,
  isSourceFile,
  BoltModifiers,
  FunctionBodyElement,
  BoltToken,
  BoltSymbol
} from "./ast";

import { BOLT_SUPPORTED_LANGUAGES } from "./constants"
import { enumOr, escapeChar, assert, registerClass, GeneratorStream, FastMultiMap } from "./util";
import { TextSpan, TextPos, TextFile } from "./text";
import { Scanner } from "./scanner";
import { convertNodeToSymbolPath, SymbolPath } from "./resolver";
import { TYPE_ERROR_MESSAGES } from "./diagnostics";
import { NODE_TYPES } from "./ast"

for (const key of Object.keys(NODE_TYPES)) {
  registerClass((NODE_TYPES as any)[key]);
}

export function getSourceFile(node: Syntax) {
  while (true) {
    if (isSourceFile(node)) {
      return node
    }
    assert(node.parentNode !== null);
    node = node.parentNode!;
  }
}

export function getPackage(node: Syntax) {
  const sourceFile = getSourceFile(node);
  assert(sourceFile.kind === SyntaxKind.BoltSourceFile);
  return (sourceFile as BoltSourceFile).pkg;
}

export function getNodeLanguage(node: Syntax): string {
  const kindStr = kindToString(node.kind);
  for (const prefix of BOLT_SUPPORTED_LANGUAGES) {
    if (kindStr.startsWith(prefix)) {
      return prefix;
    }
  }
  throw new Error(`Could not determine the language of ${kindStr}`);
}

export function createTokenStream(value: any) {
  if (value instanceof Scanner) {
    return new GeneratorStream<BoltToken>(() => value.scan());
  } else if (typeof(value) === 'string') {
    const scanner = new Scanner(new TextFile('#<anonymous>'), value);
    return new GeneratorStream<BoltToken>(() => scanner.scan());
  } else if (isBoltPunctuated(value)) {
    const origPos = value.span!.start;
    const startPos = new TextPos(origPos.offset+1, origPos.line, origPos.column+1);
    const scanner = new Scanner(value.span!.file, value.text, startPos);
    return new GeneratorStream<BoltToken>(() => scanner.scan());
  } else {
    throw new Error(`Could not convert ${kindToString(value.kind)} to a token stream.`);
  }
}

export const EOF = ''

export class ScanError extends Error {

  public errorText: string;

  constructor(public file: TextFile, public position: TextPos, public char: string) {
    super(`${file.origPath}:${position.line}:${position.column}: unexpected character '${escapeChar(char)}'`)
    this.errorText = `Unexpected character '${escapeChar(char)}'`
  }

}

export function cloneSpan(span: TextSpan | null) {
  if (span === null) {
    return null;
  }
  return span.clone();
}

export function setOrigNodeRange(node: Syntax, startNode: Syntax, endNode: Syntax): void {
  node.span = new TextSpan(startNode.span!.file, startNode.span!.start.clone(), endNode.span!.end.clone());
}

export type BoltFunctionBody = BoltFunctionBodyElement[];

export function getReturnStatementsInFunctionBody(body: BoltFunctionBody): BoltReturnStatement[] {

  const results: BoltReturnStatement[] = [];

  for (const element of body) {
    visit(element);
  }

  return results;

  function visit(node: BoltFunctionBodyElement) {
    switch (node.kind) {
      case SyntaxKind.BoltReturnStatement:
        results.push(node);
        break;
      case SyntaxKind.BoltExpressionStatement:
        visitExpression(node.expression);
        break;
    }
  }

  function visitExpression(node: BoltExpression) {
    switch (node.kind) {
      case SyntaxKind.BoltBlockExpression:
        for (const element of node.elements) {
          visit(element);
        }
        break;
      case SyntaxKind.BoltMatchExpression:
        for (const arm of node.arms) {
          visitExpression(arm.body);
        }
        break;
      case SyntaxKind.BoltCallExpression:
        visitExpression(node.operator);
        for (const operand of node.operands) {
          visitExpression(operand);
        }
        break;
    }
  }

}

export enum OperatorKind {
  Prefix,
  InfixL,
  InfixR,
  Suffix,
}

export function isRightAssoc(kind: OperatorKind) {
  return kind === OperatorKind.InfixR;
}

export class ParseError extends Error {

  public errorText: string;

  constructor(public actual: Syntax, public expected: SyntaxKind[]) {
    super(`${actual.span!.file.origPath}:${actual.span!.start.line}:${actual.span!.start.column}: expected ${enumOr(expected.map(e => describeKind(e)))} but got ${describeKind(actual.kind)}`);
    this.errorText = `Expected ${enumOr(expected.map(e => describeKind(e)))} but got ${describeKind(actual.kind)}`
  }

}

export class HardParseError extends ParseError {
  
}

export function getSymbolText(node: BoltSymbol): string {
  switch (node.kind) {
    case SyntaxKind.BoltIdentifier:
      return node.text;
    case SyntaxKind.BoltOperator:
      return node.text;
    case SyntaxKind.BoltGtSign:
      return '>';
    case SyntaxKind.BoltExMark:
      return '!';
    case SyntaxKind.BoltLtSign:
      return '<';
    case SyntaxKind.BoltVBar:
      return '|';
    default:
      throw new Error(`Could not convert the node ${kindToString(node.kind)} to the name of an operator`);
  }
}

export interface OperatorInfo {
  kind: OperatorKind;
  arity: number;
  name: string;
  precedence: number;
}

export function assertToken(node: Token, kind: SyntaxKind, isHardError = false) {
  if (node.kind !== kind) {
    if (isHardError) {
      throw new HardParseError(node, [kind]);
    } else {
      throw new ParseError(node, [kind]);
    }
  }
}

type OperatorTableList = [OperatorKind, number, string][][];

export class OperatorTable {

  private operatorsByName = new FastMultiMap<string, OperatorInfo>();

  constructor(definitions: OperatorTableList) {
    let i = 0;
    for (const group of definitions) {
      for (const [kind, arity, name] of group) {
        const info = { kind, arity, name, precedence: i }
        this.operatorsByName.add(name, info);
      }
      i++;
    }
  }

  public lookup(arity: number, name: string): OperatorInfo | null {
    if (!this.operatorsByName.has(name)) {
      return null;
    }
    for (const operatorInfo of this.operatorsByName.get(name)) {
      if (operatorInfo.arity === arity) {
        return operatorInfo;
      }
    }
    return null;
  }

}

export function getModulePathToNode(node: Syntax): string[] {
  let elements = [];
  while (true) {
    if (node.kind === SyntaxKind.BoltModule) {
      for (const element of node.name) {
        elements.unshift(element.text);
      }
    }
    if (node.parentNode === null) {
      break;
    }
    node = node.parentNode;
  }
  return elements;
}

export function isExported(node: Syntax) { 
  switch (node.kind) {
    case SyntaxKind.BoltVariableDeclaration:
    case SyntaxKind.BoltFunctionDeclaration:
    case SyntaxKind.BoltModule:
    case SyntaxKind.BoltRecordDeclaration:
    case SyntaxKind.BoltTypeAliasDeclaration:
    case SyntaxKind.BoltTraitDeclaration:
    case SyntaxKind.BoltImplDeclaration:
      return (node.modifiers & BoltModifiers.IsPublic) > 0;
    default:
      return false;
  }
}

export function hasTypeError(node: Syntax) {
  for (const message of TYPE_ERROR_MESSAGES) {
    if (hasDiagnostic(node, message)) {
      return true;
    }
  }
  return false;
}

export function hasDiagnostic(node: Syntax, message: string): boolean {
  return node.errors.some(d => d.message === message);
}

export function getFullyQualifiedPathToNode(node: Syntax): SymbolPath {
  const symbolPath = convertNodeToSymbolPath(node);
  while (true) {
    const parentNode = node.parentNode;
    if (parentNode === null) {
      break;
    }
    node = parentNode;
    if (node.kind === SyntaxKind.BoltModule) {
      for (const element of node.name) {
        symbolPath.modulePath.unshift(element.text);
      }
    }
  }
  return symbolPath;
}

export function describeKind(kind: SyntaxKind): string {
  switch (kind) {
    case SyntaxKind.BoltImportKeyword:
      return "'import'";
    case SyntaxKind.BoltExportKeyword:
      return "'export'";
    case SyntaxKind.BoltExMark:
      return "'!'";
    case SyntaxKind.JSIdentifier:
      return "a JavaScript identifier"
    case SyntaxKind.BoltIdentifier:
      return "an identifier"
    case SyntaxKind.BoltOperator:
      return "an operator"
    case SyntaxKind.BoltStringLiteral:
      return "a string"
    case SyntaxKind.BoltIntegerLiteral:
      return "an integer"
    case SyntaxKind.BoltFnKeyword:
      return "'fn'"
    case SyntaxKind.BoltWhereKeyword:
      return "'where'";
    case SyntaxKind.BoltQuoteKeyword:
      return "'quote'";
    case SyntaxKind.BoltModKeyword:
      return "'mod'";
    case SyntaxKind.BoltForeignKeyword:
      return "'foreign'"
    case SyntaxKind.BoltMatchKeyword:
      return "'match'";
    case SyntaxKind.BoltYieldKeyword:
      return "'yield'";
    case SyntaxKind.BoltReturnKeyword:
      return "'return'";
    case SyntaxKind.BoltPubKeyword:
      return "'pub'"
    case SyntaxKind.BoltLetKeyword:
      return "'let'"
    case SyntaxKind.BoltSemi:
      return "';'"
    case SyntaxKind.BoltColon:
      return "':'"
    case SyntaxKind.BoltColonColon:
      return "'::'";
    case SyntaxKind.BoltDot:
      return "'.'"
    case SyntaxKind.JSDot:
      return "'.'"
    case SyntaxKind.JSDotDotDot:
      return "'...'"
    case SyntaxKind.BoltRArrow:
      return "'->'"
    case SyntaxKind.BoltVBar:
      return "'|'";
    case SyntaxKind.BoltComma:
      return "','"
    case SyntaxKind.BoltModKeyword:
      return "'mod'"
    case SyntaxKind.BoltStructKeyword:
      return "'struct'"
    case SyntaxKind.BoltEnumKeyword:
      return "'enum'"
    case SyntaxKind.BoltTypeKeyword:
      return "'type'";
    case SyntaxKind.BoltBraced:
      return "'{' .. '}'"
    case SyntaxKind.BoltBracketed:
      return "'[' .. ']'"
    case SyntaxKind.BoltParenthesized:
      return "'(' .. ')'"
    case SyntaxKind.EndOfFile:
      return "'}', ')', ']' or end-of-file"
    case SyntaxKind.BoltLtSign:
      return "'<'";
    case SyntaxKind.BoltGtSign:
      return "'<'";
    case SyntaxKind.BoltEqSign:
      return "'='";
    case SyntaxKind.JSOpenBrace:
      return "'{'";
    case SyntaxKind.JSCloseBrace:
      return "'}'";
    case SyntaxKind.JSOpenBracket:
      return "'['";
    case SyntaxKind.JSCloseBracket:
      return "']'";
    case SyntaxKind.JSOpenParen:
      return "'('";
    case SyntaxKind.JSCloseParen:
      return "')'";
    case SyntaxKind.JSSemi:
      return "';'";
    case SyntaxKind.JSComma:
      return "','";
    case SyntaxKind.BoltTraitKeyword:
      return "'trait'";
    case SyntaxKind.BoltTraitKeyword:
      return "'impl'";
    case SyntaxKind.BoltImplKeyword:
      return "'impl'";
    case SyntaxKind.BoltForKeyword:
      return "'for'";
    case SyntaxKind.JSMulOp:
      return "'*'";
    case SyntaxKind.JSAddOp:
      return "'+'";
    case SyntaxKind.JSDivOp:
      return "'/'";
    case SyntaxKind.JSSubOp:
      return "'-'";
    case SyntaxKind.JSLtOp:
      return "'<'";
    case SyntaxKind.JSGtOp:
      return "'>'";
    case SyntaxKind.JSBOrOp:
      return "'|'";
    case SyntaxKind.JSBXorOp:
      return "'^'";
    case SyntaxKind.JSBAndOp:
      return "'&'";
    case SyntaxKind.JSBNotOp:
      return "'~'";
    case SyntaxKind.JSNotOp:
      return "'~'";
    case SyntaxKind.JSString:
      return "a JavaScript string"
    case SyntaxKind.JSReturnKeyword:
      return "'return'";
    case SyntaxKind.JSForKeyword:
      return "'for'";
    case SyntaxKind.JSTryKeyword:
      return "'try'";
    case SyntaxKind.BoltRArrowAlt:
      return "'=>'";
    case SyntaxKind.BoltBraced:
      return "'{ ... }'";
    case SyntaxKind.BoltIfKeyword:
      return "'if'";
    case SyntaxKind.BoltElseKeyword:
      return "'else'";
    case SyntaxKind.BoltTypeAliasDeclaration:
      return "a type alias";
    case SyntaxKind.BoltMacroCall:
      return "a macro call";
    default:
      throw new Error(`failed to describe ${kindToString(kind)}`)
  }
}

export function *getAllReturnStatementsInFunctionBody(body: FunctionBodyElement[]): IterableIterator<BoltReturnStatement> {
  for (const element of body) {
    switch (element.kind) {
      case SyntaxKind.BoltReturnStatement:
      {
        yield element;
        break;
      }
      case SyntaxKind.BoltCaseStatement:
      {
        for (const caseNode of element.cases) {
          yield* getAllReturnStatementsInFunctionBody(caseNode.body);
        }
        break;
      }
      case SyntaxKind.BoltConditionalStatement:
      {
        for (const caseNode of element.cases) {
          yield* getAllReturnStatementsInFunctionBody(caseNode.body);
        }
        break;
      }
      case SyntaxKind.JSTryCatchStatement:
      {
        yield* getAllReturnStatementsInFunctionBody(element.tryBlock)
        if (element.catchBlock !== null) {
          yield* getAllReturnStatementsInFunctionBody(element.catchBlock.elements)
        }
        if (element.finalBlock !== null) {
          yield* getAllReturnStatementsInFunctionBody(element.finalBlock)
        }
        break;
      }
      case SyntaxKind.JSExpressionStatement:
      case SyntaxKind.BoltExpressionStatement:
      case SyntaxKind.JSImportDeclaration:
        break;
      default:
        throw new Error(`I did not know how to find return statements in ${kindToString(element.kind)}`);
    }
  }
}
