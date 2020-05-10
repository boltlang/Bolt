
import * as acorn from "acorn"

import {
  SyntaxKind,
  BoltToken, 
  BoltIdentifier,
  createBoltFuncDecl,
  createBoltIdentifier, 
  createBoltSyntaxKind, 
  createBoltTokenStream,
  createBoltRetStmt,
  createBoltVarDecl,
  createBoltStmt,
  createBoltPatt,
  createBoltExpr,
  createBoltBindPatt,
  createBoltParam,
  createBoltRefExpr,
  createBoltTypeRef,
  createBoltTypeDecl,
  createBoltConstExpr,
  createBoltQualName,
  createBoltCallExpr,
  createBoltImportDecl,
  createBoltSourceElement,
  createBoltModule,
  createBoltRecordDecl,
  createBoltNewTypeDecl,
  BoltQualName,
  BoltPattern,
  createBoltBindPattern,
  BoltImportDeclaration,
  BoltTypeNode,
  createBoltReferenceTypeNode,
  createJSReferenceExpression,
  createBoltReferenceExpression,
} from "./ast"

import { stringType, intType } from "./checker"

import { PrimValue } from "./evaluator"
import {BoltTokenStream} from "./util"

function describeKind(kind: SyntaxKind): string {
  switch (kind) {
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
    case SyntaxKind.BoltForeignKeyword:
      return "'foreign'"
    case SyntaxKind.BoltPubKeyword:
      return "'pub'"
    case SyntaxKind.BoltLetKeyword:
      return "'let'"
    case SyntaxKind.BoltSemi:
      return "';'"
    case SyntaxKind.BoltColon:
      return "':'"
    case SyntaxKind.BoltDot:
      return "'.'"
    case SyntaxKind.BoltRArrow:
      return "'->'"
    case SyntaxKind.BoltComma:
      return "','"
    case SyntaxKind.BoltModKeyword:
      return "'mod'"
    case SyntaxKind.BoltStructKeyword:
      return "'struct'"
    case SyntaxKind.BoltEnumKeyword:
      return "'enum'"
    case SyntaxKind.BoltBraced:
      return "'{' .. '}'"
    case SyntaxKind.BoltBracketed:
      return "'[' .. ']'"
    case SyntaxKind.BoltParenthesized:
      return "'(' .. ')'"
    case SyntaxKind.BoltEOS:
      return "'}', ')', ']' or end-of-file"
    default:
      throw new Error(`failed to describe ${kindToString(kind)}`)
  }
}

function enumerate(elements: string[]) {
  if (elements.length === 1) {
    return elements[0]
  } else {
    return elements.slice(0, elements.length-1).join(',') + ' or ' + elements[elements.length-1]
  }
}


export class ParseError extends Error {
  constructor(public actual: Token, public expected: SyntaxKind[]) {
    super(`${actual.span.file.path}:${actual.span.start.line}:${actual.span.start.column}: expected ${enumerate(expected.map(e => describeKind(e)))} but got ${describeKind(actual.kind)}`)
  }
}

enum OperatorKind {
  Prefix,
  InfixL,
  InfixR,
  Suffix,
}

interface OperatorInfo {
  kind: OperatorKind;
  arity: number;
  name: string;
  precedence: number;
}

export class Parser {

  operatorTable = [
    [
      [OperatorKind.InfixL, 2, '&&'],
      [OperatorKind.InfixL, 2, '||']
    ],
    [
      [OperatorKind.InfixL, 2, '<'],
      [OperatorKind.InfixL, 2, '>'],
      [OperatorKind.InfixL, 2, '<='],
      [OperatorKind.InfixL, 2, '>=']
    ],
    [
      [OperatorKind.InfixL, 2, '>>'],
      [OperatorKind.InfixL, 2, '<<']
    ],
    [
      [OperatorKind.InfixL, 2, '+'],
      [OperatorKind.InfixL, 2, '-'],
    ],
    [
      [OperatorKind.InfixL, 2, '/'],
      [OperatorKind.InfixL, 2, '*'],
      [OperatorKind.InfixL, 2, '%'],
    ],
    [
      [OperatorKind.Prefix, '!']
    ],
  ];


  parseQualName(tokens: BoltTokenStream): BoltQualName {

    const path: BoltIdentifier[] = [];

    while (true) {
      const t0 = tokens.peek(2);
      if (t0.kind !== SyntaxKind.BoltDot) {
        break;
      }
      path.push(tokens.get() as BoltIdentifier)
      tokens.get();
    }

    const name = tokens.get();
    if (name.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(name, [SyntaxKind.BoltIdentifier]);
    }

    const startNode = path.length > 0 ? path[0] : name;
    const endNode = name;
    return createBoltQualName(path, name, null, [startNode, endNode]);
  }

  parsePattern(tokens: BoltTokenStream): BoltPattern {
    const t0 = tokens.peek(1);
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      tokens.get();
      return createBoltBindPattern(t0.text, null, [t0, t0])
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier])
    }
  }

  parseImportDecl(tokens: BoltTokenStream): BoltImportDeclaration {

    // Assuming first keyword is 'import'
    tokens.get();

    const t0 = tokens.get();
    if (t0.kind !== SyntaxKind.BoltStringLiteral) {
      throw new ParseError(t0, [SyntaxKind.BoltStringLiteral])
    }

    return createBoltImportDecl(t0.value, null, t0);

  }

  parseTypeDecl(tokens: BoltTokenStream): BoltTypeNode {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      const name = this.parseQualName(tokens)
      return createBoltReferenceTypeNode(name, [], null, name.origNodes)
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltIdentifier]);
    }
  }

  parsePrimExpr(tokens: TokenStream): Expr {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltStringLiteral) {
      tokens.get();
      return new ConstExpr(new PrimValue(stringType, t0.value), null, t0);
    } else if (t0.kind === SyntaxKind.BoltIntegerLiteral) {
      tokens.get();
      return new ConstExpr(new PrimValue(intType, t0.value), null, t0);
    } else if (t0.kind === SyntaxKind.BoltIdentifier) {
      const name = this.parseQualName(tokens);
      return createBoltReferenceExpression(name, null, name.origNode);
    } else {
      throw new ParseError(t0, [SyntaxKind.BoltStringLiteral, SyntaxKind.BoltIdentifier]);
    }
  }

  parseSyntax(tokens: TokenStream): Syntax {

    // Assuming first token is 'syntax'
    tokens.get();

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced])
    }

    const innerTokens = t1.toTokenStream();

    const pattern = this.parsePattern(innerTokens)

    const t2 = innerTokens.get();
    if (t2.kind !== SyntaxKind.BoltRArrow) {
      throw new ParseError(t2, [SyntaxKind.BoltRArrow]);
    }

    const body = this.parseBody(innerTokens);

    return new Macro(pattern, body)

  }

  parseExpr(tokens: TokenStream): Expr {
    return this.parsePrimExpr(tokens)
  }

  parseParam(tokens: TokenStream): Param {

    let defaultValue = null;
    let typeDecl = null;

    const pattern = this.parsePattern(tokens)

    const t0 = tokens.peek(1);
    if (t0.kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeDecl = this.parseTypeDecl(tokens);
      const t1 = tokens.peek(1);
      if (t1.kind === SyntaxKind.BoltEqSign) {
        tokens.get();
        defaultValue = this.parseExpr(tokens);
      }
    }

    if (t0.kind === SyntaxKind.BoltEqSign) {
      tokens.get();
      defaultValue = this.parseExpr(tokens);
    }

    return new Param(pattern, typeDecl, defaultValue)

  }

  parseVarDecl(tokens: TokenStream): VarDecl {

    let isMutable = false;
    let typeDecl = null;
    let value = null;

    // Assuming first token is 'let'
    tokens.get();

    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.BoltIdentifier && t0.text === 'mut') {
      tokens.get();
      isMutable = true;
    }

    const bindings = this.parsePattern(tokens)

    const t1 = tokens.peek();
    if (t1.kind === SyntaxKind.BoltColon) {
      tokens.get();
      typeDecl = this.parseTypeDecl(tokens);
    }

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltEqSign) {
      tokens.get();
      value = this.parseExpr(tokens);
    }

    return new VarDecl(isMutable, bindings, typeDecl, value, null)

  }

  parseRetStmt(tokens: TokenStream): RetStmt {

    // Assuming first token is 'return'
    const t0 = tokens.get();

    let expr = null;

    const t1 = tokens.peek();
    if (t1.kind !== SyntaxKind.BoltEOS) { 
      expr = this.parseExpr(tokens)
    }

    return new RetStmt(expr, null, [t0, expr.getEndNode()]);
  }

  parseStmt(tokens: TokenStream): Stmt {
    this.parseCallExpr(tokens)
  }

  parseRecordDecl(tokens: TokenStream): RecordDecl {

    let isPublic = false;

    let kw = tokens.get();
    if (kw.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(kw, [SyntaxKind.BoltPubKeyword, SyntaxKind.BoltStructKeyword]);
    }
    if (kw.text === 'pub') {
      isPublic = true;
      kw = tokens.get();
    }

    if (kw.kind !== SyntaxKind.BoltIdentifier || kw.text !== 'struct') {
      throw new ParseError(kw, [SyntaxKind.BoltStructKeyword])
    }

    const name = this.parseQualName(tokens);

    const t2 = tokens.get();

    if (t2.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(kw, [SyntaxKind.BoltBraced])
    }

    let fields = [];
  
    return new RecordDecl(isPublic, name, fields);

  }

  parseStmts(tokens: TokenStream, origNode: Syntax | null): Stmt[] {
    // TODO
    return []
  }

  parseModDecl(tokens: TokenStream): Module {

    let isPublic = false;

    let kw = tokens.get();
    if (kw.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(kw, [SyntaxKind.BoltPubKeyword, SyntaxKind.BoltModKeyword]);
    }
    if (kw.text === 'pub') {
      isPublic = true;
      kw = tokens.get();
    }

    if (kw.kind !== SyntaxKind.BoltIdentifier || kw.text !== 'mod') {
      throw new ParseError(kw, [SyntaxKind.BoltModKeyword])
    }

    const name = this.parseQualName(tokens);

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.BoltBraced) {
      throw new ParseError(t1, [SyntaxKind.BoltBraced])
    }

    return new Module(isPublic, name, t1.toSentences());

  }

  protected assertEmpty(tokens: TokenStream) {
    const t0 = tokens.peek(1);
    if (t0.kind !== SyntaxKind.BoltEOS) {
      throw new ParseError(t0, [SyntaxKind.BoltEOS]);
    }
  }

  parseNewType(tokens: TokenSteam): NewTypeDecl {

    let isPublic = false;
    let t0 = tokens.get();
    if (t0.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(t0, [SyntaxKind.BoltPubKeyword, SyntaxKind.BoltNewTypeKeyword])
    }
    if (t0.text === 'pub') {
      isPublic = true;
      t0 = tokens.get();
      if (t0.kind !== SyntaxKind.BoltIdentifier) {
        throw new ParseError(t0, [SyntaxKind.BoltNewTypeKeyword])
      }
    }

    if (t0.text !== 'newtype') {
      throw new ParseError(t0, [SyntaxKind.BoltNewTypeKeyword])
    }

    const name = tokens.get();
    if (name.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(name, [SyntaxKind.BoltIdentifier])
    }

    return new NewTypeDecl(isPublic, name)

  }

  parseFuncDecl(tokens: TokenStream, origNode: Syntax | null): FuncDecl {

    let target = "Bolt";
    let isPublic = false;

    const k0 = tokens.peek();
    if (k0.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(k0, [SyntaxKind.BoltPubKeyword, SyntaxKind.BoltForeignKeyword, SyntaxKind.BoltFnKeyword])
    }
    if (k0.text === 'pub') {
      tokens.get();
      isPublic = true;
    }

    const k1 = tokens.peek();
    if (k1.kind !== SyntaxKind.BoltIdentifier) {
      throw new ParseError(k1, [SyntaxKind.BoltForeignKeyword, SyntaxKind.BoltFnKeyword])
    }
    if (k1.text === 'foreign') {
      tokens.get();
      const l1 = tokens.get();
      if (l1.kind !== SyntaxKind.BoltStringLiteral) {
        throw new ParseError(l1, [SyntaxKind.BoltStringLiteral])
      }
      target = l1.value;
    }
    const k2 = tokens.get();
    if (k2.kind !== SyntaxKind.BoltIdentifier || k2.text !== 'fn') {
      throw new ParseError(k2, [SyntaxKind.BoltFnKeyword])
    }

    let name: QualName;
    let returnType = null;
    let body = null;
    let params: Param[] = [];

    // Parse parameters

    const t0 = tokens.peek(1);
    const t1 = tokens.peek(2);

    const isParamLike = (token: Token) =>
        token.kind === SyntaxKind.BoltIdentifier || token.kind === SyntaxKind.BoltParenthesized;

    const parseParamLike = (tokens: TokenStream) => {
      const t0 = tokens.peek(1);
      if (t0.kind === SyntaxKind.BoltIdentifier) {
        tokens.get();
        return new Param(new BindPatt(t0, null, t0), null, null, null, t0)
      } else if (t0.kind === SyntaxKind.BoltParenthesized) {
        tokens.get();
        const innerTokens = t0.toTokenStream();
        const param = this.parseParam(innerTokens)
        this.assertEmpty(innerTokens);
        return param
      } else {
        throw new ParseError(t0, [SyntaxKind.BoltIdentifier, SyntaxKind.BoltParenthesized])
      }
    }

    if (t0.kind === SyntaxKind.BoltOperator) {

      name = new QualName(t0, [], null, t0);
      tokens.get();
      params.push(parseParamLike(tokens))

    } else if (isParamLike(t0) && t1.kind == SyntaxKind.BoltOperator) {

      params.push(parseParamLike(tokens));
      name = new QualName(t1, [], null, t1);
      while (true) {
        const t2 = tokens.peek();
        if (t2.kind !== SyntaxKind.BoltOperator) {
          break;
        }
        if (t2.text !== t1.text) {
          throw new Error(`Operators have to match when defining or declaring an n-ary operator.`);
        }
        tokens.get();
        params.push(parseParamLike(tokens))
      }

    } else if (t0.kind === SyntaxKind.BoltIdentifier) {

      name = this.parseQualName(tokens)
      const t2 = tokens.get();
      if (t2.kind === SyntaxKind.BoltParenthesized) {
        const innerTokens = t2.toTokenStream();
        while (true) {
          const t3 = innerTokens.peek();
          if (t3.kind === SyntaxKind.BoltEOS) {
            break;
          }
          params.push(this.parseParam(innerTokens))
          const t4 = innerTokens.get();
          if (t4.kind === SyntaxKind.BoltComma) {
            continue;
          } else if (t4.kind === SyntaxKind.BoltEOS) {
            break;
          } else {
            throw new ParseError(t4, [SyntaxKind.BoltComma, SyntaxKind.BoltEOS])
          }
        }
      }

    } else {

      throw new ParseError(t0, [SyntaxKind.BoltIdentifier, SyntaxKind.BoltOperator, SyntaxKind.BoltParenthesized])

    }

    // Parse return type

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.BoltRArrow) {
      tokens.get();
      returnType = this.parseTypeDecl(tokens);
    }

    // Parse function body

    const t3 = tokens.peek();
    if (t3.kind === SyntaxKind.BoltBraced) {
      tokens.get();
      switch (target) {
        case "Bolt":
          body = this.parseStmts(tokens, t3);
          break;
        case "JS":
          body = acorn.parse(t3.text).body;
          break;
        default:
          throw new Error(`Unrecognised language: ${target}`);
      }
    }

    return new FuncDecl(isPublic, target, name, params, returnType, body, null, origNode)

  }

  parseSourceElement(tokens: TokenStream): SourceElement {
    const t0 = tokens.peek(1);
    if (t0.kind === SyntaxKind.BoltIdentifier) {
      let i = 1;
      let kw: Token = t0;
      if (t0.text === 'pub') {
        i++;
        kw = tokens.peek(i);
        if (kw.kind !== SyntaxKind.BoltIdentifier) {
          throw new ParseError(kw, [SyntaxKind.BoltForeignKeyword, SyntaxKind.BoltModKeyword, 
            SyntaxKind.BoltLetKeyword, SyntaxKind.BoltFnKeyword, SyntaxKind.BoltEnumKeyword, SyntaxKind.BoltStructKeyword])
        }
      }
      if (t0.text === 'foreign') {
        i += 2;
        kw = tokens.peek(i);
        if (kw.kind !== SyntaxKind.BoltIdentifier) {
          throw new ParseError(kw, [SyntaxKind.BoltModKeyword, SyntaxKind.BoltLetKeyword, 
            SyntaxKind.BoltFnKeyword, SyntaxKind.BoltEnumKeyword, SyntaxKind.BoltStructKeyword])
        }
      }
      switch (kw.text) {
        case 'newtype':
          return this.parseNewType(tokens);
        case 'syntax':
          return this.parseSyntax(tokens);
        case 'mod':
          return this.parseModDecl(tokens);
        case 'fn':
          return this.parseFuncDecl(tokens, null);
        case 'let':
          return this.parseVarDecl(tokens);
        case 'struct':
          return this.parseRecordDecl(tokens);
        case 'enum':
          return this.parseVariantDecl(tokens);
        default:
          try { 
            return this.parseExpr(tokens)
          } catch (e) {
            if (e instanceof ParseError) {
              throw new ParseError(kw, [...e.expected, SyntaxKind.BoltModKeyword, SyntaxKind.BoltLetKeyword, 
                SyntaxKind.BoltFnKeyword, SyntaxKind.BoltEnumKeyword, SyntaxKind.BoltStructKeyword])
            } else {
              throw e;
            }
          }
      }
    } else {
      return this.parseStmt(tokens)
    }
  }

  getOperatorDesc(seekArity: number, seekName: string): OperatorInfo {
    for (let i = 0; i < this.operatorTable.length; ++i) {
      for (const [kind, arity, name] of this.operatorTable[i]) {
        if (arity == seekArity && name === seekName) {
          return {
            kind,
            name,
            arity,
            precedence: i
          }
        }
      }
    }
  }

  parseBinOp(tokens: TokenStream, lhs: Expr , minPrecedence: number) {
    let lookahead = tokens.peek(1);
    while (true) {
      if (lookahead.kind !== SyntaxKind.BoltOperator) {
        break;
      }
      const lookaheadDesc = this.getOperatorDesc(2, lookahead.text);
      if (lookaheadDesc === null || lookaheadDesc.precedence < minPrecedence) {
        break;
      }
      const op = lookahead;
      const opDesc = this.getOperatorDesc(2, op.text);
      tokens.get();
      let rhs = this.parsePrimExpr(tokens)
      lookahead = tokens.peek()
      while (lookaheadDesc.arity === 2 
          && ((lookaheadDesc.precedence > opDesc.precedence)
            || lookaheadDesc.kind === OperatorKind.InfixR && lookaheadDesc.precedence === opDesc.precedence)) {
          rhs = this.parseBinOp(tokens, rhs, lookaheadDesc.precedence)
      }
      lookahead = tokens.peek();
      lhs = new CallExpr(new RefExpr(new QualName(op, [])), [lhs, rhs]);
    }
    return lhs
  }

  parseCallExpr(tokens: TokenStream): CallExpr {

    const operator = this.parsePrimExpr(tokens)
    const args: Expr[] = []

    const t2 = tokens.get();
    if (t2.kind !== SyntaxKind.BoltParenthesized) {
      throw new ParseError(t2, [SyntaxKind.BoltParenthesized])
    }

    const innerTokens = t2.toTokenStream();

    while (true) {
      const t3 = innerTokens.peek();
      if (t3.kind === SyntaxKind.BoltEOS) {
        break; 
      }
      args.push(this.parseExpr(innerTokens))
      const t4 = innerTokens.get();
      if (t4.kind === SyntaxKind.BoltEOS) {
        break
      } else if (t4.kind !== SyntaxKind.BoltComma){
        throw new ParseError(t4, [SyntaxKind.BoltComma])
      }
    }

    return new CallExpr(operator, args, null)

  }

}

