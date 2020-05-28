
import { FastStringMap, assert, isPlainObject, some, prettyPrintTag } from "./util";
import { SyntaxKind, Syntax, isBoltTypeExpression, BoltExpression, BoltFunctionDeclaration, BoltFunctionBodyElement, kindToString, SourceFile, isBoltExpression, isBoltMacroCall, BoltTypeExpression, BoltCallExpression, BoltSyntax, BoltMemberExpression, BoltDeclaration, isBoltDeclaration, isBoltTypeDeclaration, BoltTypeDeclaration, BoltReturnStatement, BoltIdentifier, BoltRecordDeclaration, isBoltRecordDeclaration, isBoltDeclarationLike } from "./ast";
import { getSymbolPathFromNode, ScopeType, SymbolResolver, SymbolInfo, SymbolPath } from "./resolver";
import { Value, Record } from "./evaluator";
import { SourceMap } from "module";
import { timingSafeEqual } from "crypto";
import { isRightAssoc, getReturnStatementsInFunctionBody, BoltFunctionBody, getModulePathToNode } from "./common";
import { relativeTimeThreshold } from "moment";
import { E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL, E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL, E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER, E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER, E_TYPES_NOT_ASSIGNABLE, E_TYPES_MISSING_MEMBER, E_NODE_DOES_NOT_CONTAIN_MEMBER, E_RECORD_MISSING_MEMBER, E_MUST_RETURN_A_VALUE, E_MAY_NOT_RETURN_BECAUSE_TYPE_RESOLVES_TO_VOID, E_MAY_NOT_RETURN_A_VALUE, E_MUST_RETURN_BECAUSE_TYPE_DOES_NOT_RESOLVE_TO_VOID } from "./diagnostics";
import { emitNode } from "./emitter";
import { BOLT_MAX_FIELDS_TO_PRINT } from "./constants";

// TODO For function bodies, we can do something special.
//      Sort the return types and find the largest types, eliminating types that fall under other types.
//      Next, add the resulting types as type hints to `fnReturnType`.

enum TypeKind {
  OpaqueType,
  AnyType,
  NeverType,
  FunctionType,
  RecordType,
  PlainRecordFieldType,
  VariantType,
  UnionType,
  TupleType,
}

export type Type
  = OpaqueType
  | AnyType
  | NeverType
  | FunctionType
  | RecordType
  | VariantType
  | TupleType
  | UnionType
  | PlainRecordFieldType

abstract class TypeBase {

  public abstract kind: TypeKind;

  /**
   * Holds the node that created this type, if any.
   */
  public node?: Syntax

  constructor(public sym?: SymbolInfo) {
    
  }

  public [prettyPrintTag](): string {
    return prettyPrintType(this as Type);
  }

}

export function isType(value: any) {
  return typeof(value) === 'object'
      && value !== null
      && value.__IS_TYPE !== null;
}

export class OpaqueType extends TypeBase {

  public kind: TypeKind.OpaqueType = TypeKind.OpaqueType;

}

export class AnyType extends TypeBase {
  public kind: TypeKind.AnyType = TypeKind.AnyType;
}

export class NeverType extends TypeBase {
  public kind: TypeKind.NeverType = TypeKind.NeverType;
}

export class FunctionType extends TypeBase {

  public kind: TypeKind.FunctionType = TypeKind.FunctionType;

  constructor(
    public paramTypes: Type[],
    public returnType: Type,
  ) {
    super();
  }

  public getParameterCount(): number {
    return this.paramTypes.length;
  }

  public getTypeAtParameterIndex(index: number) {
    if (index < 0 || index >= this.paramTypes.length) {
      throw new Error(`Could not get the parameter type at index ${index} because the index  was out of bounds.`);
    }
    return this.paramTypes[index];
  }

}

export class VariantType extends TypeBase {

  public kind: TypeKind.VariantType = TypeKind.VariantType;

  constructor(public elementTypes: Type[]) {
    super();
  }

  public getOwnElementTypes(): IterableIterator<Type> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export class UnionType extends TypeBase {

  private elements: Type[] = [];

  public kind: TypeKind.UnionType = TypeKind.UnionType;

  constructor(elements: Iterable<Type> = []) {
    super();
    this.elements = [...elements];
  }

  public addElement(element: Type): void {
    this.elements.push(element);
  }

  public getElementTypes(): IterableIterator<Type> {
    return this.elements[Symbol.iterator]();
  }

}

export type RecordFieldType
 = PlainRecordFieldType

class PlainRecordFieldType extends TypeBase {

  public kind: TypeKind.PlainRecordFieldType = TypeKind.PlainRecordFieldType;

  constructor(public type: Type) {
    super();
  }

}

export class RecordType extends TypeBase {

  public kind: TypeKind.RecordType = TypeKind.RecordType;

  private fieldTypes = new FastStringMap<string, RecordFieldType>();

  constructor(
    iterable?: Iterable<[string, RecordFieldType]>,
  ) {
    super();
    if (iterable !== undefined) {
      for (const [name, type] of iterable) {
        this.fieldTypes.set(name, type);
      }
    }
  }
  
  public getFieldNames() {
    return this.fieldTypes.keys();
  }

  public addField(name: string, type: RecordFieldType): void {
    this.fieldTypes.set(name, type);
  }

  public getFields() {
    return this.fieldTypes[Symbol.iterator]();
  }

  public hasField(name: string) {
    return name in this.fieldTypes;
  }

  public getFieldType(name: string) {
    return this.fieldTypes.get(name);
  }

  public clear(): void {
    this.fieldTypes.clear();
  }

}

export class TupleType extends TypeBase {

  kind: TypeKind.TupleType = TypeKind.TupleType;

  constructor(public elementTypes: Type[] = []) {
    super();
  }

}

export enum ErrorType {
  AssignmentError,
  NotARecord,
  TypeMismatch,
  TooFewArguments,
  TooManyArguments,
  MayNotReturnValue,
  MustReturnValue,
}

interface NotARecordError {
  type: ErrorType.NotARecord;
  node: Syntax;
  candidate: Syntax;
}

interface AssignmentError {
  type: ErrorType.AssignmentError;
  left: Syntax;
  right: Syntax;
}

interface TypeMismatchError {
  type: ErrorType.TypeMismatch;
  left: Type;
  right: Type;
}

interface TooManyArgumentsError {
  type: ErrorType.TooManyArguments;
  caller: Syntax;
  callee: Syntax;
  index: number;
}

interface TooFewArgumentsError {
  type: ErrorType.TooFewArguments;
  caller: Syntax;
  callee: Syntax;
  index: number;
}

interface MustReturnValueError {
  type: ErrorType.MustReturnValue;
}

interface MayNotReturnValueError {
  type: ErrorType.MayNotReturnValue;
}

export type CompileError
  = AssignmentError
  | TypeMismatchError
  | TooManyArgumentsError
  | TooFewArgumentsError
  | NotARecordError
  | MustReturnValueError
  | MayNotReturnValueError

export interface FunctionSignature {
  paramTypes: Type[];
}

function* getAllPossibleElementTypes(type: Type): IterableIterator<Type> {
  switch (type.kind) {
    case TypeKind.UnionType:
    {
      for (const elementType of type.getElementTypes()) {
        yield* getAllPossibleElementTypes(elementType);
      }
      break;
    }
    default:
      yield type;
  }
}

export function prettyPrintType(type: Type): string {
  let out = ''
  let hasElementType = false;
  for (const elementType of getAllPossibleElementTypes(type)) {
    hasElementType = true;
    if (elementType.sym !== undefined) {
      out += elementType.sym.name;
    } else {
      switch (elementType.kind) {
        case TypeKind.AnyType:
        {
          out += 'any';
          break;
        }
        case TypeKind.RecordType:
        {
          out += '{'
          let i = 0;
          for (const [fieldName, fieldType] of elementType.getFields()) {
            out += fieldName + ': ' + prettyPrintType(fieldType);
            i++;
            if (i >= BOLT_MAX_FIELDS_TO_PRINT) {
              break
            }
          }
          out += '}'
          break;
        }
        default:
          throw new Error(`Could not pretty-print type ${TypeKind[elementType.kind]}`)
      }
    }
  }
  if (!hasElementType) {
    out += '()'
  }
  return out;
}

export class TypeChecker {

  private opaqueTypes = new FastStringMap<number, OpaqueType>();

  private anyType = new AnyType();

  private syntaxType = new UnionType(); // FIXME

  constructor(private resolver: SymbolResolver) {

  }

  public getTypeOfValue(value: Value): Type {
    if (typeof(value) === 'string') {
      const sym = this.resolver.resolveGlobalSymbol('String', ScopeType.Type);
      assert(sym !== null);
      return new OpaqueType(sym!);
    } else if (typeof(value) === 'bigint') {
      const sym = this.resolver.resolveGlobalSymbol('int', ScopeType.Type);
      assert(sym !== null);
      return new OpaqueType(sym!);
    } else if (typeof(value) === 'number') {
      const sym = this.resolver.resolveGlobalSymbol('f64', ScopeType.Type);
      assert(sym !== null);
      return new OpaqueType(sym!);
    } else if (value instanceof Record) {
      const recordType = new RecordType()   
      for (const [fieldName, fieldValue] of value.getFields()) {
         recordType.addField(name, new PlainRecordFieldType(this.getTypeOfValue(fieldValue)));
      }
      return recordType;
    } else {
      throw new Error(`Could not determine type of given value.`);
    }
  }

  private checkTypeMatches(a: Type, b: Type) {
    switch (b.kind) {
      case TypeKind.FunctionType:
        if (a.kind === TypeKind.AnyType) {
          return true;
        }
        if (a.kind === TypeKind.FunctionType) {
          if (b.getParameterCount() > a.getParameterCount()) {
            a.node?.errors.push({
              message: E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
              severity: 'error',
              args: {
                expected: a.getParameterCount(),
                actual: a.getParameterCount(),
              },
              nested: [{
                message: E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER,
                severity: 'error',
                node: b.getTypeAtParameterIndex(a.getParameterCount()).node!
              }]
            })
          }
          if (b.getParameterCount() < a.getParameterCount()) {
            let nested = [];
            for (let i = b.getParameterCount(); i < a.getParameterCount(); i++) {
              nested.push({
                message: E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER,
                severity: 'error',
                node: (a.node as BoltCallExpression).operands[i]
              });
            }
            a.node?.errors.push({
              message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
              severity: 'error',
              args: {
                expected: a.getParameterCount(),
                actual: b.getParameterCount(), 
              },
              nested,
            });
          }
          const paramCount = a.getParameterCount();
          for (let i = 0; i < paramCount; i++) {
            const paramA = a.getTypeAtParameterIndex(i);
            const paramB = b.getTypeAtParameterIndex(i);
            if (this.isTypeAssignableTo(paramA, paramB)) {
              a.node?.errors.push({
                message: E_TYPES_NOT_ASSIGNABLE,
                severity: 'error',
                args: {
                  left: a,
                  right: b,
                },
                node: a.node,
              })
            }
          }
        }
    }
  }

  public registerSourceFile(sourceFile: SourceFile): void {
    for (const node of sourceFile.preorder()) {
      if (isBoltMacroCall(node)) {
        continue;  // FIXME only continue when we're not in an expression context
      }
      if (isBoltExpression(node)) {
        node.type = this.createInitialTypeForExpression(node);
      }
    }
    for (const callExpr of sourceFile.findAllChildrenOfKind(SyntaxKind.BoltCallExpression)) {
      const callTypeSig = new FunctionType(callExpr.operands.map(op => op.type!), this.anyType);
      for (const callableType of this.findTypesInExpression(callExpr.operator)) {
        this.checkTypeMatches(callableType, callTypeSig);
      }
    }
  }

  private createInitialTypeForExpression(node: Syntax): Type {

    if (node.type !== undefined) {
      return node.type;
    }

    let resultType;

    switch (node.kind) {

      case SyntaxKind.BoltMatchExpression:
      {
        const unionType = new UnionType();
        for (const matchArm of node.arms) {
          unionType.addElement(this.createInitialTypeForExpression(matchArm.body));
        }
        resultType = unionType;
        break;
      }

      case SyntaxKind.BoltRecordDeclaration:
      {
        const recordSym = this.resolver.getSymbolForNode(node, ScopeType.Type);
        assert(recordSym !== null);
        if (this.opaqueTypes.has(recordSym!.id)) {
          resultType = this.opaqueTypes.get(recordSym!.id);
        } else {
          const opaqueType = new OpaqueType(name, node);
          this.opaqueTypes.set(recordSym!.id, opaqueType);
          resultType = opaqueType;
        }
        break;
      }

      case SyntaxKind.BoltFunctionExpression:
      {
        const paramTypes = node.params.map(param => {
          if (param.typeExpr === null) {
            return this.anyType;
          }
          return this.createInitialTypeForTypeExpression(param.typeExpr);
        });
        let returnType = node.returnType === null
          ? this.anyType
          : this.createInitialTypeForTypeExpression(node.returnType);
        resultType = new FunctionType(paramTypes, returnType);
        break;
      }

      case SyntaxKind.BoltQuoteExpression:
      {
        resultType = this.syntaxType;
        break
      }

      case SyntaxKind.BoltMemberExpression:
      case SyntaxKind.BoltReferenceExpression:
      case SyntaxKind.BoltCallExpression:
      case SyntaxKind.BoltBlockExpression:
      {
        resultType = this.anyType;
        break;
      }

      case SyntaxKind.BoltConstantExpression:
      {
        resultType = this.getTypeOfValue(node.value);
        break;
      }

      default:
        throw new Error(`Could not create a type for node ${kindToString(node.kind)}.`);

    }

    node.type = resultType;

    return resultType;

  }

  private createInitialTypeForTypeExpression(node: BoltTypeExpression): Type {
    switch (node.kind) {
      case SyntaxKind.BoltLiftedTypeExpression:
        return this.createInitialTypeForExpression(node.expression);
      default:
        throw new Error(`Could not create a type for node ${kindToString(node.kind)}.`);
    }
  }

  public isVoidType(type: Type): boolean {
    return this.isTypeAssignableTo(new TupleType, type);
  }

  private *getTypesForMember(origNode: Syntax, fieldName: string, type: Type): IterableIterator<Type> {
    switch (type.kind) {
      case TypeKind.UnionType:
      {
        const typesMissingMember = [];
        for (const elementType of getAllPossibleElementTypes(type)) {
          let foundType = false;
          for (const recordType of this.getTypesForMemberNoUnionType(origNode, fieldName, elementType, false)) {
            yield recordType;
            foundType = true;
          }
          if (!foundType) {
            origNode.errors.push({
              message: E_TYPES_MISSING_MEMBER,
              severity: 'error',
            })
          }
        }
      }
      default:
        return this.getTypesForMemberNoUnionType(origNode, fieldName, type, true);
    }
  }

  private *getTypesForMemberNoUnionType(origNode: Syntax, fieldName: string, type: Type, hardError: boolean): IterableIterator<Type> {
    switch (type.kind) {
      case TypeKind.AnyType:
        break;
      case TypeKind.FunctionType:
        if (hardError) {
          origNode.errors.push({
            message: E_TYPES_MISSING_MEMBER,
            severity: 'error',
            args: {
              name: fieldName,
            },
            nested: [{
              message: E_NODE_DOES_NOT_CONTAIN_MEMBER,
              severity: 'error',
              node: type.node!,
            }]
          });
        }
        break;
      case TypeKind.RecordType:
      {
        if (type.hasField(fieldName)) {
          const fieldType = type.getFieldType(fieldName);
          assert(fieldType.kind === TypeKind.PlainRecordFieldType);
          yield (fieldType as PlainRecordFieldType).type;
        } else {
          if (hardError) {
            origNode.errors.push({
              message: E_TYPES_MISSING_MEMBER,
              severity: 'error',
              args: {
                name: fieldName
              }
            })
          }
        }
        break;
      }
      default:
        throw new Error(`I do not know how to find record member types for ${TypeKind[type.kind]}`)
    }
  }

  private isTypeAlwaysCallable(type: Type) {
    return type.kind === TypeKind.FunctionType;
  }

  private *getAllNodesForType(type: Type): IterableIterator<Syntax> {
    if (type.node !== undefined) {
      yield type.node;
    }
    switch (type.kind) {
      case TypeKind.UnionType:
        for (const elementType of type.getElementTypes()) {
          yield* this.getAllNodesForType(type);
        }
        break;
      default:
    }
  }

  private *findTypesInTypeExpression(node: BoltTypeExpression): IterableIterator<Type> {
    switch (node.kind) {
      case SyntaxKind.BoltTypeOfExpression:
      {
        yield* this.findTypesInExpression(node.expression);
        break;
      }
      case SyntaxKind.BoltReferenceTypeExpression:
      {
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Variable);
        assert(scope !== null);
        const symbolPath = getSymbolPathFromNode(node.name);
        const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
        if (resolvedSym !== null) {
          for (const decl of resolvedSym.declarations) {
            assert(isBoltTypeDeclaration(decl));
            this.findTypesInTypeDeclaration(decl as BoltTypeDeclaration);
          }
        }
        break;
      } 
      default:
        throw new Error(`Unexpected node type ${kindToString(node.kind)}`);
    }
  }

  private *findTypesInExpression(node: BoltExpression): IterableIterator<Type> {

      switch (node.kind) {

        case SyntaxKind.BoltMemberExpression:
        {
          for (const element of node.path) {
            for (const memberType of this.getTypesForMember(element, element.text, node.expression.type!)) {
              yield memberType;
            }
          }
          break;
        }

        case SyntaxKind.BoltMatchExpression:
        {
          const unionType = new UnionType();
          for (const matchArm of node.arms) {
            unionType.addElement(this.createInitialTypeForExpression(matchArm.body));
          }
          yield unionType;
          break;
        }

        case SyntaxKind.BoltQuoteExpression:
        {
          break;
        }

        case SyntaxKind.BoltCallExpression:
        {
          const nodeSignature = new FunctionType(node.operands.map(op => op.type!), this.anyType);
          for (const callableType of this.findTypesInExpression(node.operator)) {
            yield callableType;
          }
          break;
        }

        case SyntaxKind.BoltReferenceExpression:
        {
          const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Variable);
          assert(scope !== null);
          const symbolPath = getSymbolPathFromNode(node.name);
          const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
          if (resolvedSym !== null) {
            for (const decl of resolvedSym.declarations) {
              assert(isBoltDeclaration(decl));
              yield* this.findTypesInDeclaration(decl as BoltDeclaration);
            }
          }
          break;
        }

        default:
          throw new Error(`Unexpected node type ${kindToString(node.kind)}`);

      }
    }

  private *findTypesInTypeDeclaration(node: BoltTypeDeclaration): IterableIterator<Type> {
    switch (node.kind) {
      case SyntaxKind.BoltTypeAliasDeclaration:
      {
        yield* this.findTypesInTypeExpression(node.typeExpr);
        break;
      }
      default:
        throw new Error(`Unexpected node type ${kindToString(node.kind)}`);
    }
  }

    private *findTypesInDeclaration(node: BoltDeclaration) {
      switch (node.kind) {
        case SyntaxKind.BoltVariableDeclaration:
          if (node.typeExpr !== null) {
            yield* this.findTypesInTypeExpression(node.typeExpr);
          }
          if (node.value !== null) {
            yield* this.findTypesInExpression(node.value);
          }
          break;
        case SyntaxKind.BoltFunctionDeclaration:
        {
          yield this.getFunctionReturnType(node);
          break;
        }
        default:
          throw new Error(`Could not find callable expressions in declaration ${kindToString(node.kind)}`)
      }
    }

    private getTypeOfExpression(node: BoltExpression): Type {
      return new UnionType(this.findTypesInExpression(node));
    }

    private getFunctionReturnType(node: BoltFunctionDeclaration): Type {
      let returnType: Type = this.anyType;
      if (node.returnType !== null) {
        returnType = new UnionType(this.findTypesInTypeExpression(node.returnType));
      }
      for (const returnStmt of this.getAllReturnStatementsInFunctionBody(node.body)) {
        if (returnStmt.value === null) {
          if (!this.isVoidType(returnType)) {
            returnStmt.errors.push({
              message: E_MAY_NOT_RETURN_A_VALUE,
              severity: 'error',
              nested: [{
                message: E_MAY_NOT_RETURN_BECAUSE_TYPE_RESOLVES_TO_VOID,
                severity: 'error',
                node: node.returnType !== null ? node.returnType : node,
              }]
            });
          }
        } else {
          const stmtReturnType = this.getTypeOfExpression(returnStmt.value);
          if (!this.isTypeAssignableTo(returnType, stmtReturnType)) {
            if (this.isVoidType(stmtReturnType)) {
              returnStmt.value.errors.push({
                message: E_MUST_RETURN_A_VALUE,
                severity: 'error',
                nested: [{
                  message: E_MUST_RETURN_BECAUSE_TYPE_DOES_NOT_RESOLVE_TO_VOID,
                  severity: 'error',
                  node: node.returnType !== null ? node.returnType : node,
                }]
              })
            } else {
              returnStmt.value.errors.push({
                message: E_TYPES_NOT_ASSIGNABLE,
                severity: 'error',
                args: {
                  left: returnType,
                  right: stmtReturnType,
                }
              })
            }
          }

        }
      }
      return returnType;
    }

    private *getAllReturnStatementsInFunctionBody(body: BoltFunctionBody): IterableIterator<BoltReturnStatement> {
      for (const element of body) {
        switch (element.kind) {
          case SyntaxKind.BoltReturnStatement:
          {
            yield element;
            break;
          }
          case SyntaxKind.BoltConditionalStatement:
          {
            for (const caseNode of element.cases) {
              yield* this.getAllReturnStatementsInFunctionBody(caseNode.body);
            }
            break;
          }
          case SyntaxKind.BoltExpressionStatement:
            break;
          default:
            throw new Error(`I did not know how to find return statements in ${kindToString(node.kind)}`);
        }
      }
    }

    private isTypeAssignableTo(left: Type, right: Type): boolean {
      if (left.kind === TypeKind.NeverType || right.kind === TypeKind.NeverType) {
        return false;
      }
      if (left.kind === TypeKind.AnyType || right.kind === TypeKind.AnyType) {
        return true;
      }
      if (left.kind === TypeKind.OpaqueType && right.kind === TypeKind.OpaqueType) {
        return left === right;
      }
      if (left.kind === TypeKind.RecordType && right.kind === TypeKind.RecordType) {
        for (const fieldName of left.getFieldNames()) {
          if (!right.hasField(fieldName)) {
            return false;
          }
        }
        for (const fieldName of right.getFieldNames()) {
          if (!left.hasField(fieldName)) {
            return false;
          }
          if (!this.isTypeAssignableTo(left.getFieldType(fieldName), right.getFieldType(fieldName))) {
            return false;
          }
        }
        return true;
      }
      if (left.kind === TypeKind.FunctionType && right.kind === TypeKind.FunctionType) {
        if (left.getParameterCount() !== right.getParameterCount()) {
          return false;
        }
        for (let i = 0; i < left.getParameterCount(); i++) {
          if (!this.isTypeAssignableTo(left.getTypeAtParameterIndex(i), right.getTypeAtParameterIndex(i))) {
            return false;
          }
        }
        if (!this.isTypeAssignableTo(left.returnType, right.returnType)) {
          return false;
        }
        return true;
      }
      return false;
    }

}
