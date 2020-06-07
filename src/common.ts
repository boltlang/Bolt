import {
  BoltFunctionBodyElement,
  BoltReturnStatement,
  SyntaxKind,
  BoltExpression,
  BoltQualName,
  kindToString,
  Syntax,
  Token,
  isBoltPunctuated,
  SourceFile,
  BoltSourceFile,
  isSourceFile,
  BoltSyntax,
  BoltModifiers,
  ReturnStatement,
  FunctionBodyElement
} from "./ast";
import { BOLT_SUPPORTED_LANGUAGES } from "./constants"
import { FastStringMap, enumOr, escapeChar, assert, registerClass, Newable } from "./util";
import { TextSpan, TextPos, TextFile } from "./text";
import { Scanner } from "./scanner";
import { convertNodeToSymbolPath } from "./resolver";
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

export function createTokenStream(node: Syntax) {
  if (isBoltPunctuated(node)) {
    const origPos = node.span!.start;
    const startPos = new TextPos(origPos.offset+1, origPos.line, origPos.column+1);
    return new Scanner(node.span!.file, node.text, startPos);
  } else {
    throw new Error(`Could not convert ${kindToString(node.kind)} to a token stream.`);
  }
}

export const EOF = ''

export class ScanError extends Error {
  constructor(public file: TextFile, public position: TextPos, public char: string) {
    super(`${file.origPath}:${position.line}:${position.column}: unexpected char '${escapeChar(char)}'`)
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
  constructor(public actual: Syntax, public expected: SyntaxKind[]) {
    super(`${actual.span!.file.origPath}:${actual.span!.start.line}:${actual.span!.start.column}: expected ${enumOr(expected.map(e => describeKind(e)))} but got ${describeKind(actual.kind)}`);
  }
}

export interface OperatorInfo {
  kind: OperatorKind;
  arity: number;
  name: string;
  precedence: number;
}

export function assertToken(node: Token, kind: SyntaxKind) {
  if (node.kind !== kind) {
    throw new ParseError(node, [kind]);
  }
}

type OperatorTableList = [OperatorKind, number, string][][];

export class OperatorTable {

  private operatorsByName = new FastStringMap<string, OperatorInfo>();
  //private operatorsByPrecedence = FastStringMap<number, OperatorInfo>();

  constructor(definitions: OperatorTableList) {
    let i = 0;
    for (const group of definitions) {
      for (const [kind, arity, name] of group) {
        const info = { kind, arity, name, precedence: i }
        this.operatorsByName.set(name, info);
        //this.operatorsByPrecedence[i] = info;
      }
      i++;
    }
  }

  public lookup(name: string): OperatorInfo | null {
    if (!this.operatorsByName.has(name)) {
      return null;
    }
    return this.operatorsByName.get(name);
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

export function getFullyQualifiedPathToNode(node: Syntax) {
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
    default:
      throw new Error(`failed to describe ${kindToString(kind)}`)
  }
}

export function *getAllReturnStatementsInFunctionBody(body: FunctionBodyElement[]): IterableIterator<ReturnStatement> {
  for (const element of body) {
    switch (element.kind) {
      case SyntaxKind.BoltReturnStatement:
      case SyntaxKind.JSReturnStatement:
      {
        yield element;
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
