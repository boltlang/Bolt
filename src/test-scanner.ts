
import test from "ava"
import { Punctuator, Scanner } from "./scanner";
import { Token, TokenType } from "./token";

function scan(text: string): Token[] {
  const scanner = new Scanner(text);
  const tokens = new Punctuator(scanner);
  const result = [];
  for (;;) {
    const t0 = tokens.get();
    if (t0.type === TokenType.EndOfFile) {
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
  //t.assert(tokens[0].type === TokenType.LineFoldStart);
  t.assert(tokens[0].type === TokenType.Identifier);
  t.assert(tokens[1].type === TokenType.BlockStart);
  //t.assert(tokens[3].type === TokenType.LineFoldStart);
  t.assert(tokens[2].type === TokenType.Identifier);
  t.assert(tokens[3].type === TokenType.LineFoldEnd);
  //t.assert(tokens[6].type === TokenType.LineFoldStart);
  t.assert(tokens[4].type === TokenType.Identifier);
  t.assert(tokens[5].type === TokenType.LineFoldEnd);
  //t.assert(tokens[9].type === TokenType.LineFoldStart);
  t.assert(tokens[6].type === TokenType.Identifier);
  t.assert(tokens[7].type === TokenType.LineFoldEnd);
  t.assert(tokens[8].type === TokenType.BlockEnd);
  t.assert(tokens[9].type === TokenType.LineFoldEnd);
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
  //t.assert(tokens[0].type === TokenType.LineFoldStart);
  t.assert(tokens[0].type === TokenType.Identifier);
  t.assert(tokens[1].type === TokenType.BlockStart);
  //t.assert(tokens[3].type === TokenType.LineFoldStart);
  t.assert(tokens[2].type === TokenType.Identifier);
  t.assert(tokens[3].type === TokenType.BlockStart);
  //t.assert(tokens[6].type === TokenType.LineFoldStart);
  t.assert(tokens[4].type === TokenType.Identifier);
  t.assert(tokens[5].type === TokenType.LineFoldEnd);
  //t.assert(tokens[9].type === TokenType.LineFoldStart);
  t.assert(tokens[6].type === TokenType.Identifier);
  t.assert(tokens[7].type === TokenType.LineFoldEnd);
  //t.assert(tokens[12].type === TokenType.LineFoldStart);
  t.assert(tokens[8].type === TokenType.Identifier);
  t.assert(tokens[9].type === TokenType.LineFoldEnd);
  t.assert(tokens[10].type === TokenType.BlockEnd);
  t.assert(tokens[11].type === TokenType.LineFoldEnd);
  t.assert(tokens[12].type === TokenType.BlockEnd);
  t.assert(tokens[13].type === TokenType.LineFoldEnd);
})

