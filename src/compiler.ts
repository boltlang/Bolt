
import acorn from "acorn"

import { 
  TypeChecker,
  Scope
} from "./checker"

import {
  Syntax,
  SyntaxKind,
  SourceFile,
  Stmt,
  Expr,
  Decl,
  isExpr,
} from "./ast"

export interface CompilerOptions {
  target: string;
}

function pushAll<T>(arr: T[], els: T[]) {
  for (const el of els) {
    arr.push(el)
  }
}

export class Compiler {

  readonly target: string;

  constructor(public checker: TypeChecker, options: CompilerOptions) {
    this.target = options.target
  }

  compile(files: SourceFile[]) {
    return files.map(s => {
      const body: (Decl | Stmt | Expr)[] = [];
      for (const element of s.elements) {
        this.compileDecl(element, body);
      }
      return {
        type: 'Program',
        body,
        loc: {
          source: s.getFile().path
        }
      }
    });
  }

  protected compileExpr(node: Syntax, preamble: Syntax[]): Expr {
  
    switch (node.kind) {

      case SyntaxKind.CallExpr:
        const compiledOperator = this.compileExpr(node.operator, preamble);
        const compiledArgs = node.args.map(a => this.compileExpr(a, preamble))
        return {
          type: 'CallExpression',
          callee: compiledOperator,
          arguments: compiledArgs,
        };

      case SyntaxKind.RefExpr:
        return {
          type: 'Identifier',
          name: node.name.name.text,
        }

      case SyntaxKind.ConstExpr:
        return {
          type: 'Literal',
          value: node.value,
        }

      default:
        throw new Error(`Could not compile expression node ${SyntaxKind[node.kind]}`)
    }

  }

  protected compileDecl(node: Syntax, preamble: Syntax[]): Expr | undefined {

    console.log(`compiling ${SyntaxKind[node.kind]}`)

    if (isExpr(node)) {
      const compiled = this.compileExpr(node, preamble);
      preamble.push({
        type: 'ExpressionStatement',
        expression: compiled
      })
      return;
    }

    switch (node.kind) {

      case SyntaxKind.FuncDecl:
        const params = [];
        if (node.target === this.target) {
          console.log(node.body)
          preamble.push({
            type: 'FunctionDeclaration',
            id: { type: 'Identifier',  name: node.name.name.text },
            params: node.params.map(p => ({ type: 'Identifier', name: p.bindings.name.text })),
            body: {
              type: 'BlockStatement',
              body: node.body
            }
          })
        } else {
          throw new Error(`Cannot yet compile Bolt functions`)
        }
        break;

      default:
        throw new Error(`Could not compile node ${SyntaxKind[node.kind]}`);

    }

  }

}

