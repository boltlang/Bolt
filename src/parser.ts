
import * as acorn from "acorn"

import {
  Syntax,
  Token, 
  FuncDecl,
  Identifier, 
  SyntaxKind, 
  TokenStream,
  RetStmt,
  VarDecl,
  Stmt,
  Patt,
  Expr,
  BindPatt,
  Param,
  RefExpr,
  TypeRef,
  TypeDecl,
  ConstExpr,
  QualName,
  CallExpr,
  ImportDecl,
  SourceElement,
  Module,
  RecordDecl,
} from "./ast"

import { stringType, intType } from "./checker"

import { PrimValue } from "./evaluator"

function describeKind(kind: SyntaxKind): string {
  switch (kind) {
    case SyntaxKind.Identifier:
      return "an identifier"
    case SyntaxKind.Operator:
      return "an operator"
    case SyntaxKind.StringLiteral:
      return "a string"
    case SyntaxKind.IntegerLiteral:
      return "an integer"
    case SyntaxKind.FnKeyword:
      return "'fn'"
    case SyntaxKind.ForeignKeyword:
      return "'foreign'"
    case SyntaxKind.PubKeyword:
      return "'pub'"
    case SyntaxKind.LetKeyword:
      return "'let'"
    case SyntaxKind.Semi:
      return "';'"
    case SyntaxKind.Colon:
      return "':'"
    case SyntaxKind.Dot:
      return "'.'"
    case SyntaxKind.Comma:
      return "','"
    case SyntaxKind.ModKeyword:
      return "'mod'"
    case SyntaxKind.StructKeyword:
      return "'struct'"
    case SyntaxKind.EnumKeyword:
      return "'enum'"
    case SyntaxKind.Braced:
      return "'{' .. '}'"
    case SyntaxKind.Bracketed:
      return "'[' .. ']'"
    case SyntaxKind.Parenthesized:
      return "'(' .. ')'"
    case SyntaxKind.EOS:
      return "'}', ')', ']' or end-of-file"
    default:
      throw new Error(`failed to describe ${SyntaxKind[kind]}`)
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

export class Parser {

  parseQualName(tokens: TokenStream): QualName {

    const path: Identifier[] = [];

    while (true) {
      const t0 = tokens.peek(2);
      if (t0.kind !== SyntaxKind.Dot) {
        break;
      }
      path.push(tokens.get() as Identifier)
      tokens.get();
    }

    const name = tokens.get();
    if (name.kind !== SyntaxKind.Identifier) {
      throw new ParseError(name, [SyntaxKind.Identifier]);
    }

    const startNode = path.length > 0 ? path[0] : name;
    const endNode = name;
    return new QualName(name, path, null, [startNode, endNode]);
  }

  parsePattern(tokens: TokenStream): Patt {
    const t0 = tokens.peek(1);
    if (t0.kind === SyntaxKind.Identifier) {
      tokens.get();
      return new BindPatt(t0, null, t0)
    } else {
      throw new ParseError(t0, [SyntaxKind.Identifier])
    }
  }

  parseImportDecl(tokens: TokenStream): ImportDecl {

    // Assuming first keyword is 'import'
    tokens.get();

    const t0 = tokens.get();
    if (t0.kind !== SyntaxKind.StringLiteral) {
      throw new ParseError(t0, [SyntaxKind.StringLiteral])
    }

    return new ImportDecl(t0.value, null, t0);

  }

  parseTypeDecl(tokens: TokenStream): TypeDecl {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.Identifier) {
      const name = this.parseQualName(tokens)
      return new TypeRef(name, [], null, name.origNode)
    } else {
      throw new ParseError(t0, [SyntaxKind.Identifier]);
    }
  }

  parsePrimExpr(tokens: TokenStream): Expr {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.StringLiteral) {
      tokens.get();
      return new ConstExpr(new PrimValue(stringType, t0.value), null, t0);
    } else if (t0.kind === SyntaxKind.IntegerLiteral) {
      tokens.get();
      return new ConstExpr(new PrimValue(intType, t0.value), null, t0);
    } else if (t0.kind === SyntaxKind.Identifier) {
      const name = this.parseQualName(tokens);
      return new RefExpr(name, null, name.origNode);
    } else {
      throw new ParseError(t0, [SyntaxKind.StringLiteral, SyntaxKind.Identifier]);
    }
  }

  parseSyntax(tokens: TokenStream): Syntax {

    // Assuming first token is 'syntax'
    tokens.get();

    throw new Error('not implemented')

  }

  parseExpr(tokens: TokenStream): Expr {
    return this.parsePrimExpr(tokens)
  }

  parseParam(tokens: TokenStream): Param {

    let defaultValue = null;
    let typeDecl = null;

    const pattern = this.parsePattern(tokens)

    const t0 = tokens.peek(1);
    if (t0.kind === SyntaxKind.Colon) {
      tokens.get();
      typeDecl = this.parseTypeDecl(tokens);
      const t1 = tokens.peek(1);
      if (t1.kind === SyntaxKind.EqSign) {
        tokens.get();
        defaultValue = this.parseExpr(tokens);
      }
    }

    if (t0.kind === SyntaxKind.EqSign) {
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
    if (t0.kind === SyntaxKind.Identifier && t0.text === 'mut') {
      tokens.get();
      isMutable = true;
    }

    const bindings = this.parsePattern(tokens)

    const t1 = tokens.peek();
    if (t1.kind === SyntaxKind.Colon) {
      tokens.get();
      typeDecl = this.parseTypeDecl(tokens);
    }

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.EqSign) {
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
    if (t1.kind !== SyntaxKind.EOS) { 
      expr = this.parseExpr(tokens)
    }

    return new RetStmt(expr, null, [t0, expr.getEndNode()]);
  }

  parseStmt(tokens: TokenStream): Stmt {

  }

  parseRecordDecl(tokens: TokenStream): RecordDecl {

    let isPublic = false;

    let kw = tokens.get();
    if (kw.kind !== SyntaxKind.Identifier) {
      throw new ParseError(kw, [SyntaxKind.PubKeyword, SyntaxKind.StructKeyword]);
    }
    if (kw.text === 'pub') {
      isPublic = true;
      kw = tokens.get();
    }

    if (kw.kind !== SyntaxKind.Identifier || kw.text !== 'struct') {
      throw new ParseError(kw, [SyntaxKind.StructKeyword])
    }

    const name = this.parseQualName(tokens);

    const t2 = tokens.get();

    if (t2.kind !== SyntaxKind.Braced) {
      throw new ParseError(kw, [SyntaxKind.Braced])
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
    if (kw.kind !== SyntaxKind.Identifier) {
      throw new ParseError(kw, [SyntaxKind.PubKeyword, SyntaxKind.ModKeyword]);
    }
    if (kw.text === 'pub') {
      isPublic = true;
      kw = tokens.get();
    }

    if (kw.kind !== SyntaxKind.Identifier || kw.text !== 'mod') {
      throw new ParseError(kw, [SyntaxKind.ModKeyword])
    }

    const name = this.parseQualName(tokens);

    const t1 = tokens.get();
    if (t1.kind !== SyntaxKind.Braced) {
      throw new ParseError(t1, [SyntaxKind.Braced])
    }

    return new Module(isPublic, name, t1.toSentences());

  }

  protected assertEmpty(tokens: TokenStream) {
    const t0 = tokens.peek(1);
    if (t0.kind !== SyntaxKind.EOS) {
      throw new ParseError(t0, [SyntaxKind.EOS]);
    }
  }

  parseFuncDecl(tokens: TokenStream, origNode: Syntax | null): FuncDecl {

    let target = "Bolt";
    let isPublic = false;

    const k0 = tokens.peek();
    if (k0.kind !== SyntaxKind.Identifier) {
      throw new ParseError(k0, [SyntaxKind.PubKeyword, SyntaxKind.ForeignKeyword, SyntaxKind.FnKeyword])
    }
    if (k0.text === 'pub') {
      tokens.get();
      isPublic = true;
    }

    const k1 = tokens.peek();
    if (k1.kind !== SyntaxKind.Identifier) {
      throw new ParseError(k1, [SyntaxKind.ForeignKeyword, SyntaxKind.FnKeyword])
    }
    if (k1.text === 'foreign') {
      tokens.get();
      const l1 = tokens.get();
      if (l1.kind !== SyntaxKind.StringLiteral) {
        throw new ParseError(l1, [SyntaxKind.StringLiteral])
      }
      target = l1.value;
    }
    const k2 = tokens.get();
    if (k2.kind !== SyntaxKind.Identifier || k2.text !== 'fn') {
      throw new ParseError(k2, [SyntaxKind.FnKeyword])
    }

    let name: QualName;
    let returnType = null;
    let body = null;
    let params: Param[] = [];

    // Parse parameters

    const t0 = tokens.peek(1);
    const t1 = tokens.peek(2);

    const isParamLike = (token: Token) =>
        token.kind === SyntaxKind.Identifier || token.kind === SyntaxKind.Parenthesized;

    const parseParamLike = (tokens: TokenStream) => {
      const t0 = tokens.peek(1);
      if (t0.kind === SyntaxKind.Identifier) {
        tokens.get();
        return new Param(new BindPatt(t0, null, t0), null, null, null, t0)
      } else if (t0.kind === SyntaxKind.Parenthesized) {
        tokens.get();
        const innerTokens = t0.toTokenStream();
        const param = this.parseParam(innerTokens)
        this.assertEmpty(innerTokens);
        return param
      } else {
        throw new ParseError(t0, [SyntaxKind.Identifier, SyntaxKind.Parenthesized])
      }
    }

    if (t0.kind === SyntaxKind.Operator) {

      name = new QualName(t0, [], null, t0);
      tokens.get();
      params.push(parseParamLike(tokens))

    } else if (isParamLike(t0) && t1.kind == SyntaxKind.Operator) {

      params.push(parseParamLike(tokens));
      name = new QualName(t1, [], null, t1);
      while (true) {
        const t2 = tokens.peek();
        if (t2.kind !== SyntaxKind.Operator) {
          break;
        }
        if (t2.text !== t1.text) {
          throw new Error(`Operators have to match when defining or declaring an n-ary operator.`);
        }
        tokens.get();
        params.push(parseParamLike(tokens))
      }

    } else if (t0.kind === SyntaxKind.Identifier) {

      name = this.parseQualName(tokens)
      const t2 = tokens.get();
      if (t2.kind === SyntaxKind.Parenthesized) {
        const innerTokens = t2.toTokenStream();
        while (true) {
          const t3 = innerTokens.peek();
          if (t3.kind === SyntaxKind.EOS) {
            break;
          }
          params.push(this.parseParam(innerTokens))
          const t4 = innerTokens.get();
          if (t4.kind === SyntaxKind.Comma) {
            continue;
          } else if (t4.kind === SyntaxKind.EOS) {
            break;
          } else {
            throw new ParseError(t4, [SyntaxKind.Comma, SyntaxKind.EOS])
          }
        }
      }

    } else {

      throw new ParseError(t0, [SyntaxKind.Identifier, SyntaxKind.Operator, SyntaxKind.Parenthesized])

    }

    // Parse return type

    const t2 = tokens.peek();
    if (t2.kind === SyntaxKind.RArrow) {
      tokens.get();
      returnType = this.parseTypeDecl(tokens);
    }

    // Parse function body

    const t3 = tokens.peek();
    if (t3.kind === SyntaxKind.Braced) {
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
    if (t0.kind === SyntaxKind.Identifier) {
      let i = 1;
      let kw: Token = t0;
      if (t0.text === 'pub') {
        i++;
        kw = tokens.peek(i);
        if (kw.kind !== SyntaxKind.Identifier) {
          throw new ParseError(kw, [SyntaxKind.ForeignKeyword, SyntaxKind.ModKeyword, SyntaxKind.LetKeyword, SyntaxKind.FnKeyword, SyntaxKind.EnumKeyword, SyntaxKind.StructKeyword])
        }
      }
      if (t0.text === 'foreign') {
        i += 2;
        kw = tokens.peek(i);
        if (kw.kind !== SyntaxKind.Identifier) {
          throw new ParseError(kw, [SyntaxKind.ModKeyword, SyntaxKind.LetKeyword, SyntaxKind.FnKeyword, SyntaxKind.EnumKeyword, SyntaxKind.StructKeyword])
        }
      }
      switch (kw.text) {
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
          throw new ParseError(kw, [SyntaxKind.ModKeyword, SyntaxKind.LetKeyword, SyntaxKind.FnKeyword, SyntaxKind.EnumKeyword, SyntaxKind.StructKeyword])
      }
    } else {
      return this.parseStmt(tokens)
    }
  }

  parseCallExpr(tokens: TokenStream): CallExpr {

    const operator = this.parsePrimExpr(tokens)
    const args: Expr[] = []

    const t2 = tokens.get();
    if (t2.kind !== SyntaxKind.Parenthesized) {
      throw new ParseError(t2, [SyntaxKind.Parenthesized])
    }

    const innerTokens = t2.toTokenStream();

    while (true) {
      const t3 = innerTokens.peek();
      if (t3.kind === SyntaxKind.EOS) {
        break; 
      }
      args.push(this.parseExpr(innerTokens))
      const t4 = innerTokens.get();
      if (t4.kind === SyntaxKind.EOS) {
        break
      } else if (t4.kind !== SyntaxKind.Comma){
        throw new ParseError(t4, [SyntaxKind.Comma])
      }
    }

    return new CallExpr(operator, args, null)

  }

}

