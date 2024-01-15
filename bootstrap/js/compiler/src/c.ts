
import type stream from "stream"
import { IndentWriter } from "./util";

export const enum CNodeKind {

  // Types
  BuiltinType,

  // Statements
  ExprStmt,
  RetStmt,

  // Expressions
  CallExpr,
  RefExpr,
  ConstExpr,

  // Declarations
  TypeDecl,
  VarDecl,
  FuncDecl,

  // Directives
  IncDir,

  // Other nodes
  Program,

}

export const enum CBuiltinTypeKind {
  Char,
  Short,
  Int,
  Long,
  LongLong,
  UnsignedChar,
  UnsignedShort,
  UnsignedInt,
  UnsignedLong,
  UnsignedLongLong,
}

abstract class CNodeBase {

  public abstract readonly kind: CNodeKind;

  public emit(file: stream.Writable): void {
    const emitter = new CEmitter(file);
    emitter.emit(this as any);
  }

}

export class CBuiltinType extends CNodeBase {

  public readonly kind = CNodeKind.BuiltinType;

  public constructor(
    public typeKind: CBuiltinTypeKind,
  ) {
    super();
  }

}

export type CType
  = CBuiltinType

export class CRefExpr extends CNodeBase {

  public readonly kind = CNodeKind.RefExpr;

  public constructor(
    public name: string
  ) {
    super();
  }

}

export class CCallExpr extends CNodeBase {

  public readonly kind = CNodeKind.CallExpr;

  public constructor(
    public operator: CExpr,
    public args: CExpr[],
  ) {
    super();
  }

}

export class CConstExpr extends CNodeBase {

  public readonly kind = CNodeKind.ConstExpr;

  public constructor(
    public value: bigint | string | boolean,
  ) {
    super();
  }

}

export type CExpr
  = CRefExpr
  | CCallExpr
  | CConstExpr
  ;

export class CRetStmt extends CNodeBase {

  public readonly kind = CNodeKind.RetStmt;

  public constructor(
    public value: CExpr | null,
  ) {
    super();
  }

}

export class CExprStmt extends CNodeBase {

  public readonly kind = CNodeKind.ExprStmt;

  public constructor(
    public expr: CExpr,
  ) {
    super();
  }

}

export type CStmt
  = CExprStmt
  | CRetStmt;

export class CTypeDecl extends CNodeBase {

  public readonly kind = CNodeKind.TypeDecl;

  public constructor(
    public name: string,
    public type: CType,
  ) {
    super();
  }

}

export class CFuncDecl extends CNodeBase {

  public readonly kind = CNodeKind.FuncDecl;

  public constructor(
    public returnType: CType,
    public name: string,
    public params: Array<[CType, string]>,
    public body: CStmt[] | null,
  ) {
    super();
  }

}

export class CVarDecl extends CNodeBase {

  public readonly kind = CNodeKind.VarDecl;

  public constructor(
    public isExtern: boolean,
    public type: CType,
    public name: string,
  ) {
    super();
  }

}

export type CDecl
  = CTypeDecl
  | CVarDecl
  | CFuncDecl

export class CIncDir extends CNodeBase {

  public readonly kind = CNodeKind.IncDir;

  public constructor(
    public filePath: string,
    public isSystem = false,
  ) {
    super();
  }

}

export type CDir
  = CIncDir;

export class CProgram extends CNodeBase {

  public readonly kind = CNodeKind.Program;

  public constructor(
    public elements: (CDecl | CDir)[],
  ) {
    super();
  }

}

export type CNode
  = CDecl
  | CDir
  | CStmt
  | CExpr
  | CType
  | CProgram

export class CEmitter {

  private writer: IndentWriter;

  public constructor(
    public stream: stream.Writable,
  ) {
    this.writer = new IndentWriter(stream);
  }

  public emit(node: CNode): void {

    switch (node.kind) {

      case CNodeKind.Program:
      {
        for (const element of node.elements) {
          this.emit(element);
        }
        break;
      }

      case CNodeKind.IncDir:
      {
        this.writer.write('#include ');
        this.writer.write(node.isSystem ? '<' : '"');
        this.writer.write(node.filePath);
        this.writer.write(node.isSystem ? '>' : '"');
        this.writer.write('\n\n');
        break;
      }

      case CNodeKind.BuiltinType:
      {
        switch (node.typeKind) {
          case CBuiltinTypeKind.Char:
            this.writer.write('char');
            break;
          case CBuiltinTypeKind.Short:
            this.writer.write('short');
            break;
          case CBuiltinTypeKind.Int:
            this.writer.write('int');
            break;
          case CBuiltinTypeKind.Long:
            this.writer.write('long');
            break;
          case CBuiltinTypeKind.LongLong:
            this.writer.write('long long');
            break;
          case CBuiltinTypeKind.UnsignedChar:
            this.writer.write('unsigned char');
            break;
          case CBuiltinTypeKind.UnsignedShort:
            this.writer.write('unsigned short');
            break;
          case CBuiltinTypeKind.UnsignedInt:
            this.writer.write('unsigned int');
            break;
          case CBuiltinTypeKind.UnsignedLong:
            this.writer.write('unsigned long');
            break;
          case CBuiltinTypeKind.UnsignedLongLong:
            this.writer.write('unsigned long long');
            break;
        }
        break;
      }

      case CNodeKind.FuncDecl:
      {
        this.emit(node.returnType);
        this.writer.write(' ' + node.name + '(');
        let count = 0;
        for (const [type, name] of node.params) {
          this.emit(type);
          this.writer.write(' ' + name);
          if (count++ > 0) {
            this.writer.write(', ');
          }
        }
        this.writer.write(') {\n');
        this.writer.indent();
        if (node.body !== null) {
          for (const element of node.body) {
            this.emit(element);
          }
        }
        this.writer.dedent();
        this.writer.write('}\n\n');
        break;
      }

      case CNodeKind.ExprStmt:
        this.emit(node.expr);
        this.writer.write(';\n');
        break;

      case CNodeKind.RetStmt:
      {
        this.writer.write('return');
        if (node.value !== null) {
          this.writer.write(' ');
          this.emit(node.value);
        }
        this.writer.write(';\n');
        break;
      }

      case CNodeKind.RefExpr:
        this.writer.write(node.name);
        break;

      case CNodeKind.CallExpr:
      {
        this.emit(node.operator);
        this.writer.write('(');
        let count = 0;
        for (const arg of node.args) {
          this.emit(arg);
          if (count++ > 0) {
            this.writer.write(', ');
          }
        }
        this.writer.write(')');
        break;
      }

      case CNodeKind.ConstExpr:
      {
        if (typeof(node.value) === 'string') {
          this.writer.write('"');
          for (const ch of node.value) {
            switch (ch) {
              case '\b': this.writer.write('\\b'); break;
              case '\f': this.writer.write('\\f'); break;
              case '\n': this.writer.write('\\n'); break;
              case '\r': this.writer.write('\\r'); break;
              case '\t': this.writer.write('\\t'); break;
              case '\v': this.writer.write('\\v'); break;
              case '\0': this.writer.write('\\0'); break;
              case '\'': this.writer.write('\\\''); break;
              case '"': this.writer.write('\\"'); break;
              default: this.writer.write(ch); break;
            }
          }
          this.writer.write('"');
        } else if (typeof(node.value) === 'bigint') {
          this.writer.write(node.value.toString());
        } else {
          throw new Error(`Unexpected type of value in CConstExpr`);
        }
        break;
      }

      default:
        throw new Error(`Unexpected ${node.constructor.name}`);

    }

  }

}
