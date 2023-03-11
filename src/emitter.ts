import { Syntax, SyntaxKind } from "./cst";
import { IndentWriter, assertNever } from "./util";

export class Emitter {

  public constructor(
    public writer: IndentWriter,
  ) {

  }

  public emit(node: Syntax): void {

    switch (node.kind) {
    
      case SyntaxKind.ModuleDeclaration:
        this.writer.write(`mod ${node.name.text}`);
        if (node.elements === null) {
          this.writer.write('\n');
          break;
        }
        this.writer.write('.\n');
        this.writer.indent();
        for (const element of node.elements) {
          this.emit(element);
        }
        this.writer.dedent();
        break;

      case SyntaxKind.ReferenceExpression:
        for (const [name, _dot] of node.modulePath) {
          this.writer.write(name.text);
          this.writer.write('.');
        }
        this.writer.write(node.name.text);
        break;

      case SyntaxKind.CallExpression:
        this.emit(node.func);
        for (const arg of node.args) {
          this.writer.write(' ');
          this.emit(arg);
        }
        break;

      case SyntaxKind.ReferenceTypeExpression:
        for (const [name, _dot] of node.modulePath) {
          this.writer.write(name.text);
          this.writer.write('.');
        }
        this.writer.write(node.name.text);
        break;

      case SyntaxKind.StructExpressionField:
        this.writer.write(node.name.text);
        this.writer.write(' = ');
        this.emit(node.expression);
        break;

      case SyntaxKind.StructExpression:
        this.writer.write('{ ');
        for (const member of node.members) {
          this.emit(member);
          this.writer.write(', ');
        }
        this.writer.write(' }');
        break;

      case SyntaxKind.ConstantExpression:
        this.writer.write(node.token.text);
        break;

      case SyntaxKind.FunctionExpression:
        this.writer.write('\\');
        for (const param of node.params) {
          this.emit(param);
          this.writer.write(' ');
        }
        this.emit(node.body);
        break;

      case SyntaxKind.ArrowTypeExpression:
        for (const typeExpr of node.paramTypeExprs) {
          this.emit(typeExpr);
          this.writer.write(' -> ');
        }
        this.emit(node.returnTypeExpr);
        break;

      case SyntaxKind.VarTypeExpression:
        this.writer.write(node.name.text);
        break;

      case SyntaxKind.Param:
        this.emit(node.pattern);
        break;

      case SyntaxKind.NamedPattern:
        this.writer.write(node.name.text);
        break;

      case SyntaxKind.ExpressionStatement:
        this.emit(node.expression);
        this.writer.write('\n');
        break;

      case SyntaxKind.SourceFile:
        for (const element of node.elements) {
          this.emit(element);
        }
        break;

      case SyntaxKind.TypeAssert:
        this.writer.write(': ');
        this.emit(node.typeExpression);
        break;

      case SyntaxKind.ExprBody:
        this.writer.write(node.equals.text);
        this.writer.write(' ');
        this.emit(node.expression);
        break

      case SyntaxKind.BlockBody:
        this.writer.write('.\n');
        this.writer.indent();
        for (const element of node.elements) {
          this.emit(element);
        }
        this.writer.dedent();
        break;

      case SyntaxKind.LetDeclaration:
        if (node.pubKeyword) {
          this.writer.write('pub ');
        }
        this.writer.write('let ');
        if (node.mutKeyword) {
          this.writer.write(' mut ');
        }
        this.emit(node.pattern);
        this.writer.write(' ');
        for (const param of node.params) {
          this.emit(param);
          this.writer.write(' ');
        }
        if (node.typeAssert) {
          this.emit(node.typeAssert);
          this.writer.write(' ');
        }
        if (node.body) {
          this.emit(node.body);
        }
        this.writer.write('\n\n');
        break;

      case SyntaxKind.ClassConstraint:
        this.writer.write(node.name.text);
        for (const type of node.types) {
          this.writer.write(' ');
          this.emit(type);
        }
        break;

      case SyntaxKind.ClassDeclaration:
        if (node.pubKeyword) {
          this.writer.write('pub ');
        }
        this.writer.write(`class `);
        if (node.constraints) {
          for (const constraint of node.constraints.constraints) {
            this.emit(constraint);
            this.writer.write(`, `);
          }
          this.writer.write(' => ');
        }
        this.emit(node.constraint);
        if (node.elements !== null) {
          this.writer.write('.\n');
          this.writer.indent();
          for (const element of node.elements) {
            this.emit(element);
          }
          this.writer.dedent();
        }
        break;

      default:
        assertNever(node);


    }

  }

}

