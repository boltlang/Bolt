
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
  QualName
} from "./ast"

function describeKind(kind: SyntaxKind): string {
  switch (kind) {
    case SyntaxKind.Identifier:
      return "an identifier"
    case SyntaxKind.Operator:
      return "an operator"
    case SyntaxKind.Literal:
      return "a constant literal"
    case SyntaxKind.Semi:
      return "';'"
    case SyntaxKind.Colon:
      return "':'"
    case SyntaxKind.Dot:
      return "'.'"
    case SyntaxKind.Comma:
      return "','"
    case SyntaxKind.Braced:
      return "'{' .. '}'"
    case SyntaxKind.Bracketed:
      return "'[' .. ']'"
    case SyntaxKind.Parenthesized:
      return "'(' .. ')'"
    default:
      return "a token"
  }
}

function enumerate(elements: string[]) {
  if (elements.length === 1) {
    return elements[0]
  } else {
    return elements.slice(0, elements.length-2).join(',') + ' or ' + elements[elements.length-1]
  }
}


export class ParseError extends Error {
  constructor(public actual: Token, public expected: SyntaxKind[]) {
    super(`${actual.span.file.path}:${actual.span.start.line}:${actual.span.start.column}: got ${describeKind(actual.kind)} but expected ${enumerate(expected.map(e => describeKind(e)))}`)
  }
}

export class Parser {


  constructor() {

  }

  parseQualName(tokens: TokenStream) {

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

  parseTypeDecl(tokens: TokenStream): TypeDecl {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.Identifier) {
      const name = this.parseQualName(tokens)
      return new TypeRef(name, [], null, name.origNode)
    } else {
      throw new ParseError(t0, [SyntaxKind.Identifier]);
    }
  }

  parseExpr(tokens: TokenStream): Expr {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.Literal) {
      return new ConstExpr(t0.value, null, t0);
    
    } else if (t0.kind === SyntaxKind.Identifier) {
      const name = this.parseQualName(tokens);
      return new RefExpr(name, null, name.origNode);
    } else {
      throw new ParseError(t0, [SyntaxKind.Literal, SyntaxKind.Identifier]);
    }
  }

  parseParam(tokens: TokenStream) {

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
    } else if (t0.kind === SyntaxKind.EqSign) {
      tokens.get();
      defaultValue = this.parseExpr(tokens);
    }

    return new Param(pattern, typeDecl, defaultValue)

  }

  parseVarDecl(tokens: TokenStream): VarDecl {

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

  parseStmts(tokens: TokenStream, origNode: Syntax | null): Stmt[] {
    // TODO
    return []
  }

  protected assertEmpty(tokens: TokenStream) {
    const t0 = tokens.peek(1);
    if (t0.kind !== SyntaxKind.EOS) {
      throw new ParseError(t0, [SyntaxKind.EOS]);
    }
  }

  parseFuncDecl(tokens: TokenStream, origNode: Syntax | null) {

    // Assuming the first identifier is the 'fn' keyword
    tokens.get();

    let name: QualName;
    let returnType = null;
    let body = null;
    let params: Param[] = [];

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
        const innerTokens = t0.toStream();
        const param = this.parseParam(innerTokens)
        this.assertEmpty(innerTokens);
        return param
      } else {
        throw new ParseError(t0, [SyntaxKind.Identifier, SyntaxKind.Parenthesized])
      }
    }

    // Parse parameters

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
        const innerTokens = t2.toStream();
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
      returnType = this.parseTypeDecl(tokens, t2);
    }

    // Parse function body

    const t3 = tokens.peek();
    if (t3.kind === SyntaxKind.Braced) {
      body = this.parseStmts(tokens, t3);
    }

    return new FuncDecl(name, params, returnType, body, null, origNode)

  }

  parseDecl(tokens: TokenStream, origNode: Syntax | null) {
    const t0 = tokens.peek(1);   
    if (t0.kind === SyntaxKind.Identifier && t0.text === 'fn') {
      this.parseFuncDecl(tokens, origNode)
    }
  }

}

