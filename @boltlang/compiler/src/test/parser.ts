
import test from "ava";
import {BoltBindPattern, BoltOperator, SyntaxKind} from "../ast";
import { createTokenStream } from "../common";
import {Parser} from "../parser";

test('the Bolt parser correctly parses an operator definition', t => {

  const parser = new Parser();

  const tokens1 = createTokenStream(`fn +x;`)
  const funcDecl1 = parser.parseFunctionDeclaration(tokens1);
  t.assert(funcDecl1.kind === SyntaxKind.BoltFunctionDeclaration);
  t.assert(funcDecl1.name.kind === SyntaxKind.BoltOperator);
  const funcDecl1Name = funcDecl1.name as BoltOperator;
  t.assert(funcDecl1Name.text === '+');
  t.assert(funcDecl1.params.length === 1);
  t.assert(funcDecl1.params[0].kind === SyntaxKind.BoltParameter);
  t.assert(funcDecl1.params[0].bindings.kind === SyntaxKind.BoltBindPattern);
  const funcDecl1BindPatt = funcDecl1.params[0].bindings as BoltBindPattern;
  t.assert(funcDecl1BindPatt.name.text === 'x');
  t.assert(tokens1.get().kind === SyntaxKind.BoltSemi);

  const tokens2 = createTokenStream(`fn a + b;`);
  const funcDecl2 = parser.parseFunctionDeclaration(tokens2);
  t.assert(funcDecl2.kind === SyntaxKind.BoltFunctionDeclaration);
  t.assert(funcDecl2.name.kind === SyntaxKind.BoltOperator);
  const funcDecl2Name = funcDecl2.name as BoltOperator;
  t.assert(funcDecl2Name.text === '+');
  t.assert(funcDecl2.params.length === 2);
  t.assert(funcDecl2.params[0].kind === SyntaxKind.BoltParameter);
  t.assert(funcDecl2.params[0].bindings.kind === SyntaxKind.BoltBindPattern);
  const funcDecl2BindPatt0 = funcDecl2.params[0].bindings as BoltBindPattern;
  t.assert(funcDecl2BindPatt0.name.text === 'a');
  t.assert(funcDecl2.params[1].kind === SyntaxKind.BoltParameter);
  t.assert(funcDecl2.params[1].bindings.kind === SyntaxKind.BoltBindPattern);
  const funcDecl2BindPatt1 = funcDecl2.params[1].bindings as BoltBindPattern;
  t.assert(funcDecl2BindPatt1.name.text === 'b');
  t.assert(tokens2.get().kind === SyntaxKind.BoltSemi);

  const tokens3 = createTokenStream(`fn (a: Int) + (b: Int);`);
  const funcDecl3 = parser.parseFunctionDeclaration(tokens3);
  t.assert(funcDecl3.kind === SyntaxKind.BoltFunctionDeclaration);
  // TODO complete these assertions

  const tokens4 = createTokenStream(`fn a + b -> Int;`);
  const funcDecl4 = parser.parseFunctionDeclaration(tokens4);
  t.assert(funcDecl4.kind === SyntaxKind.BoltFunctionDeclaration);
  // TODO complete these assertions

})

