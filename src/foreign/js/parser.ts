
import  { Stream } from "../../util"
import { assertToken, setOrigNodeRange, ParseError } from "../../common"

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
  createJSMemberExpression,
  createJSCallExpression,
  JSDeclaration,
  JSString,
  createJSImportDeclaration,
  createJSImportStarBinding,
  createJSImportAsBinding,
  createJSReturnStatement,
  JSReturnStatement,
  createJSTryCatchStatement,
  JSPattern,
  createJSLiteralExpression,
} from "../../ast"

export type JSTokenStream = Stream<JSToken>;

const T0_EXPRESSION = [
  SyntaxKind.JSIdentifier,
  SyntaxKind.JSString,
  SyntaxKind.JSAddOp,
  SyntaxKind.JSSubOp,
  SyntaxKind.JSNotOp,
  SyntaxKind.JSBNotOp,
];

const T0_STATEMENT = [
  ...T0_EXPRESSION,
  SyntaxKind.JSReturnKeyword,
  SyntaxKind.JSTryKeyword,
  SyntaxKind.JSForKeyword,
];

const T0_DECLARATION = [
  SyntaxKind.JSConstKeyword,
  SyntaxKind.JSLetKeyword,
  SyntaxKind.JSFunctionKeyword,
  SyntaxKind.JSImportKeyword,
  SyntaxKind.JSExportKeyword,
];

export class JSParser {

  public parseJSPattern(tokens: JSTokenStream): JSPattern {
    // TODO
    tokens.get();
  }

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
    } else if (t0.kind === SyntaxKind.JSInteger) {
      tokens.get();
      const result = createJSLiteralExpression(t0.value);
      setOrigNodeRange(result, t0, t0);
      return result;
    } else {
      throw new ParseError(t0, [SyntaxKind.JSIdentifier]);
    }
  }

  public parseJSExpression(tokens: JSTokenStream): JSExpression {
    const firstToken = tokens.peek();
    let result = this.parsePrimitiveJSExpression(tokens);
    while (true) {
      const t1 = tokens.peek();
      if (t1.kind === SyntaxKind.JSCloseBrace
       || t1.kind === SyntaxKind.JSCloseParen
       || t1.kind === SyntaxKind.JSCloseBracket
       || t1.kind === SyntaxKind.JSComma
       || t1.kind === SyntaxKind.JSSemi) {
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

  public parseJSReturnStatement(tokens: JSTokenStream): JSReturnStatement {
    let value = null;
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.JSReturnKeyword);
    const t1 = tokens.peek();
    if (T0_EXPRESSION.indexOf(t1.kind) !== -1) {
      value = this.parseJSExpression(tokens);
    }
    const result = createJSReturnStatement(value)
    setOrigNodeRange(result, t0, value !== null ? value : t0);
    return result;
  }

  public parseJSTryCatchStatement(tokens: JSTokenStream): JSTryCatchStatement {

    let catchBlock = null;
    let finallyBlock = null;

    let lastToken: JSToken;

    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.JSTryKeyword);
    const t1 = tokens.get();
    assertToken(t1, SyntaxKind.JSOpenBrace);
    const tryBlock = this.parseJSSourceElementList(tokens);
    const t3 = tokens.get();
    assertToken(t3, SyntaxKind.JSCloseBrace);

    let t4 = tokens.peek();
    if (t4.kind === SyntaxKind.JSCatchKeyword) {
      tokens.get();
      const t5 = tokens.get();
      let bindings = null;
      if (t5.kind === SyntaxKind.JSOpenParen) {
        bindings = this.parseJSPattern(tokens);
        const t6 = tokens.get();
        assertToken(t6, SyntaxKind.JSCloseParen);
      }
      const t7 = tokens.get();
      assertToken(t7, SyntaxKind.JSOpenBrace);
      const elements = this.parseJSSourceElementList(tokens);
      const t8 = tokens.get();
      assertToken(t8, SyntaxKind.JSCloseBrace);
      lastToken = t8
      t4 = tokens.peek();
    }

    if (t4.kind === SyntaxKind.JSFinallyKeyword) {
      tokens.get();
      const t7 = tokens.get();
      assertToken(t7, SyntaxKind.JSOpenBrace);
      finallyBlock = this.parseJSSourceElementList(tokens);
      const t8 = tokens.get();
      assertToken(t8, SyntaxKind.JSCloseBrace);
      lastToken = t8
    }

    const result = createJSTryCatchStatement(tryBlock, catchBlock, finallyBlock)
    setOrigNodeRange(result, t0, lastToken!);
    return result;
  }

  public parseJSStatement(tokens: JSTokenStream): JSStatement {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.JSReturnKeyword) {
      return this.parseJSReturnStatement(tokens);
    } else if (t0.kind === SyntaxKind.JSTryKeyword) {
      return this.parseJSTryCatchStatement(tokens);
    } else if (T0_EXPRESSION.indexOf(t0.kind) !== -1) {
      return this.parseJSExpressionStatement(tokens);
    } else {
      throw new ParseError(t0, T0_STATEMENT);
    }
  }

  public parseImportDeclaration(tokens: JSTokenStream): JSImportDeclaration {
    const t0 = tokens.get();
    assertToken(t0, SyntaxKind.JSImportKeyword);
    const t1 = tokens.peek();
    let bindings = [];
    let filename;
    if (t1.kind === SyntaxKind.JSString) {
      tokens.get();
      filename = t1 as JSString;
    } else {
      while (true) {
        const t1 = tokens.get();
        if (t1.kind === SyntaxKind.JSFromKeyword) {
          break;
        }
        if (t1.kind === SyntaxKind.JSMulOp) {
          const t2 = tokens.get();
          assertToken(t2, SyntaxKind.JSAsKeyword);
          const t3 = tokens.get();
          assertToken(t3, SyntaxKind.JSIdentifier);
          const binding = createJSImportStarBinding(t3 as JSIdentifier);
          setOrigNodeRange(binding, t1, t1);
          bindings.push(binding);
        } else if (t1.kind === SyntaxKind.JSOpenBrace) {
          // TODO
        } else if (t1.kind === SyntaxKind.JSIdentifier) {
          const binding = createJSImportAsBinding(t1, null)
          setOrigNodeRange(binding, t1, t1);
          bindings.push(binding);
        } else {
          throw new ParseError(t1, [SyntaxKind.JSMulOp, SyntaxKind.JSIdentifier, SyntaxKind.JSOpenBrace]);
        }
      }
      const t2 = tokens.get();
      assertToken(t2, SyntaxKind.JSString);
      filename = t2 as JSString;
    }
    const result = createJSImportDeclaration(bindings, filename)
    setOrigNodeRange(result, t0, filename);
    return result;
  }

  public parseExportDeclaration(tokens: JSTokenStream): JSExportDeclaration {

  }

  public parseJSDeclaration(tokens: JSTokenStream): JSDeclaration {
    const t0 = tokens.peek();
    if (t0.kind === SyntaxKind.JSImportKeyword) {
      return this.parseImportDeclaration(tokens);
    } else if (t0.kind === SyntaxKind.JSExportKeyword) {
      return this.parseExportDeclaration(tokens);
    } else {
      throw new ParseError(t0, T0_DECLARATION);
    }
  }

  public parseJSSourceElement(tokens: JSTokenStream): JSSourceElement {
    const t0 = tokens.peek();
    if (T0_DECLARATION.indexOf(t0.kind) !== -1) {
      return this.parseJSDeclaration(tokens);
    } else {
      return this.parseJSStatement(tokens);
    }
  }

  public parseJSSourceElementList(tokens: JSTokenStream): JSSourceElement[] {
    const elements: JSSourceElement[] = [];
    while (true) {
      const t0 = tokens.peek();
      if (t0.kind === SyntaxKind.EndOfFile || t0.kind === SyntaxKind.JSCloseBrace) {
        break;
      }
      if (t0.kind === SyntaxKind.JSSemi) {
        tokens.get();
        continue;
      }
      const element = this.parseJSSourceElement(tokens)
      elements.push(element);
    }
    return elements;
  }

}

