
import test from "ava"
import { Punctuator, Scanner } from "./scanner";
import { Token, SyntaxKind } from "./cst";
import {TextFile} from "./text";

function scan(text: string): Token[] {
  const file = new TextFile("#<test-data>", text);
  const scanner = new Scanner(file);
  const tokens = new Punctuator(scanner);
  const result = [];
  for (;;) {
    const t0 = tokens.get();
    if (t0.kind === SyntaxKind.EndOfFile) {
      break;
    }
    result.push(t0);
  }
  return result;
}

test('a punctuator correctly punctuates a top-level block', t => {
  const tokens = scan(`
foo.
  a
  b
  c
`)
  t.assert(tokens.length === 10);
  //t.assert(tokens[0].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[0].kind === SyntaxKind.Identifier);
  t.assert(tokens[1].kind === SyntaxKind.BlockStart);
  //t.assert(tokens[3].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[2].kind === SyntaxKind.Identifier);
  t.assert(tokens[3].kind === SyntaxKind.LineFoldEnd);
  //t.assert(tokens[6].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[4].kind === SyntaxKind.Identifier);
  t.assert(tokens[5].kind === SyntaxKind.LineFoldEnd);
  //t.assert(tokens[9].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[6].kind === SyntaxKind.Identifier);
  t.assert(tokens[7].kind === SyntaxKind.LineFoldEnd);
  t.assert(tokens[8].kind === SyntaxKind.BlockEnd);
  t.assert(tokens[9].kind === SyntaxKind.LineFoldEnd);
});

test('a punctuator correctly punctuates nested blocks', t => {
  const tokens = scan(`
a.
  b.
    c
    d
    e
`)
  t.assert(tokens.length === 14);
  //t.assert(tokens[0].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[0].kind === SyntaxKind.Identifier);
  t.assert(tokens[1].kind === SyntaxKind.BlockStart);
  //t.assert(tokens[3].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[2].kind === SyntaxKind.Identifier);
  t.assert(tokens[3].kind === SyntaxKind.BlockStart);
  //t.assert(tokens[6].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[4].kind === SyntaxKind.Identifier);
  t.assert(tokens[5].kind === SyntaxKind.LineFoldEnd);
  //t.assert(tokens[9].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[6].kind === SyntaxKind.Identifier);
  t.assert(tokens[7].kind === SyntaxKind.LineFoldEnd);
  //t.assert(tokens[12].kind === SyntaxKind.LineFoldStart);
  t.assert(tokens[8].kind === SyntaxKind.Identifier);
  t.assert(tokens[9].kind === SyntaxKind.LineFoldEnd);
  t.assert(tokens[10].kind === SyntaxKind.BlockEnd);
  t.assert(tokens[11].kind === SyntaxKind.LineFoldEnd);
  t.assert(tokens[12].kind === SyntaxKind.BlockEnd);
  t.assert(tokens[13].kind === SyntaxKind.LineFoldEnd);
})

