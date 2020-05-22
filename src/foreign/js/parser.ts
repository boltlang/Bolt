
import  { Stream, assertToken, setOrigNodeRange, ParseError } from "../../util"

import {
  SyntaxKind,
  JSToken,
  JSStatement,
  JSSourceElement,
  JSExpressionStatement,
  createJSExpressionStatement,
  JSExpression,
  JSReferenceExpression,
  createJSReferenceExpression,
  JSIdentifier,
  JSMemberExpression,
  createJSMemberExpression,
  createJSCallExpression
} from "../../ast"

export type JSTokenStream = Stream<JSToken>;

export class JSParser {

  public parseJSReferenceExpression(tokens: JSTokenStream): JSReferenceExpression {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.JSIdentifier);
    const result = createJSReferenceExpression((t0 as JSIdentifier).text);
    setOrigNodeRange(result, t0, t0);
    return result;
  }

  private parsePrimitiveJSExpression(tokens: JSTokenStream): JSExpression {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.JSIdentifier) {
      return this.parseJSReferenceExpression(tokens);
    } else {
      throw new ParseError(t0, [SyntaxKind.JSIdentifier]);
    }
  }

  public parseJSExpression(tokens: JSTokenStream): JSExpression {
    const firstToken = tokens.peek();
    let result = this.parsePrimitiveJSExpression(tokens);
    while (true) {
      const t1 = tokens.peek();
      if (t1.kind === SyntaxKind.JSCloseBrace || t1.kind === SyntaxKind.JSCloseParen || t1.kind === SyntaxKind.JSCloseBracket || t1.kind === SyntaxKind.JSSemi) {
        break;
      }
      if (t1.kind === SyntaxKind.JSDot) {
        tokens.get();
        const t2 = tokens.get();
        assertToken(t2, SyntaxKind.JSIdentifier);
        const oldResult = result;
        result = createJSMemberExpression(oldResult, t2 as JSIdentifier);
        setOrigNodeRange(result, oldResult, t2);
      } else if (t1.kind === SyntaxKind.JSOpenBracket) {
        tokens.get();
        // TODO
      } else if (t1.kind === SyntaxKind.JSOpenParen) {
        tokens.get();
        let lastToken;
        let args: JSExpression[] = [];
        while (true) {
          const t2 = tokens.peek();
          if (t2.kind === SyntaxKind.JSCloseParen) {
            lastToken = t2;
            break;
          }
          args.push(this.parseJSExpression(tokens));
          const t3 = tokens.get();
          if (t3.kind === SyntaxKind.JSCloseParen) {
            lastToken = t3;
            break;
          } else {
            assertToken(t3, SyntaxKind.JSComma);
          }
        }
        const oldResult = result;
        result = createJSCallExpression(oldResult, args);
        setOrigNodeRange(result, firstToken, lastToken);
      } else {
        throw new ParseError(t1, [SyntaxKind.JSDot, SyntaxKind.JSOpenBracket]);
      }
    }
    return result;
  }

  public parseJSExpressionStatement(tokens: JSTokenStream): JSExpressionStatement {
    const expr = this.parseJSExpression(tokens);
    const result = createJSExpressionStatement(expr);
    setOrigNodeRange(result, expr, expr);
    return result;
  }

  public parseJSStatement(tokens: JSTokenStream): JSStatement {
    return this.parseJSExpressionStatement(tokens);
  }

  public parseJSSourceElementList(tokens: JSTokenStream): JSSourceElement[] {
    const elements: JSSourceElement[] = [];
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile) {
        break;
      }
      if (t0.kind === SyntaxKind.JSSemi) {
        tokens.get();
        continue;
      }
      const statement = this.parseJSStatement(tokens)
      elements.push(statement);
    }
    return elements;
  }

}

