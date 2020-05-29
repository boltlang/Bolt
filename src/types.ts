
import { FastStringMap, assert, isPlainObject, some, prettyPrintTag, map, flatMap, filter, memoize, comparator, createTransparentProxy, TransparentProxy, every, FastMultiMap, getKeyTag } from "./util";
import { SyntaxKind, Syntax, isBoltTypeExpression, BoltExpression, BoltFunctionDeclaration, BoltFunctionBodyElement, kindToString, SourceFile, isBoltExpression, BoltCallExpression, BoltIdentifier, isBoltDeclarationLike, isBoltPattern, isJSExpression, isBoltStatement, isJSStatement, isJSPattern, isJSParameter, isBoltParameter, isBoltMatchArm, isBoltRecordField, isBoltRecordFieldPattern, isEndOfFile, isSyntax, } from "./ast";
import { convertNodeToSymbolPath, ScopeType, SymbolResolver, SymbolInfo, SymbolPath } from "./resolver";
import { Value, Record } from "./evaluator";
import { getReturnStatementsInFunctionBody, getAllReturnStatementsInFunctionBody, getFullyQualifiedPathToNode, hasDiagnostic, hasTypeError } from "./common";
import { E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL, E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL, E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER, E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER, E_TYPE_MISMATCH, Diagnostic, E_ARGUMENT_TYPE_NOT_ASSIGNABLE } from "./diagnostics";
import { BOLT_MAX_FIELDS_TO_PRINT } from "./constants";
import { emitNode } from "./emitter";

// TODO For function bodies, we can do something special.
//      Sort the return types and find the largest types, eliminating types that fall under other types.
//      Next, add the resulting types as type hints to `fnReturnType`.

// This is a character that is used as a prefix in path names to distinguish 
// a global symbol from a symbol coming from a specific source file.
const GLOBAL_SCOPE_MARKER = '@'

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
  ReturnType,
}

export type Type
  = OpaqueType
  | ReturnType
  | AnyType
  | NeverType
  | FunctionType
  | RecordType
  | VariantType
  | TupleType
  | UnionType
  | PlainRecordFieldType

let nextTypeId = 1;

function areTypesLexicallyEquivalent(a: Type, b: Type): boolean {

  if (a.kind !== b.kind) {
    return false;
  }

  if (a.kind === TypeKind.NeverType && a.kind === TypeKind.NeverType) {
      return true;
  }
  if (a.kind === TypeKind.AnyType && a.kind === TypeKind.AnyType) {
    return true;
  }
  if (a.kind === TypeKind.OpaqueType && b.kind === TypeKind.OpaqueType) {
    return a.name === b.name;
  }
  
  if (a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType) {
    return a.source.id === b.source.id;
  }
  if (a.kind === TypeKind.RecordType && b.kind === TypeKind.RecordType) {
    return a.source.id === b.source.id;
  }

  throw new Error(`I did not expected to see the provided type combination.`)

}

function areTypesLexicallyLessThan(a: Type, b: Type): boolean {

  if (a.kind !== b.kind) {
    return a.kind < b.kind;
  }

  if (a.kind === TypeKind.NeverType && a.kind === TypeKind.NeverType) {
      return false;
  }
  if (a.kind === TypeKind.AnyType && a.kind === TypeKind.AnyType) {
    return false;
  }
  if (a.kind === TypeKind.OpaqueType && b.kind === TypeKind.OpaqueType) {
    return a.name < b.name;
  }

  //if (a.kind === TypeKind.UnionType && b.kind === TypeKind.UnionType) {
  //  a.elementTypes.sort(comparator(areTypesLexicallyLessThan));
  //  b.elementTypes.sort(comparator(areTypesLexicallyLessThan));
  //  let i = 0;
  //  let j = 0;
  //  while (true) {
  //    if (!areTypesLexicallyLessThan(a.elementTypes[i], b.elementTypes[j])) {
  //      return false;
  //    }
  //    j++;
  //  }
  //}

  if (a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType) {
    return a.source.id < b.source.id;
  }
  if (a.kind === TypeKind.RecordType && b.kind === TypeKind.RecordType) {
    return a.source.id < b.source.id;
  }

  throw new Error(`I did not expected to see the provided type combination.`)
}

abstract class TypeBase {

  public abstract kind: TypeKind;

  public id = nextTypeId++;
 
  public node?: Syntax;

  public nextType?: Type;

  public get solved(): Type {
    let type = this as Type;
    while (type.nextType !== undefined) {
      type = type.nextType;
    }
    return type;
  }

  public [getKeyTag](): string {
    return this.id.toString();
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

  constructor(public name: string, public source?: Syntax) {
    super();
  }

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
    public source: Syntax,
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

  public getVariantTypes(): IterableIterator<Type> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export class UnionType extends TypeBase {

  public elementTypes: Type[] = [];

  public kind: TypeKind.UnionType = TypeKind.UnionType;

  constructor(elements: Iterable<Type> = []) {
    super();
    this.elementTypes = [...elements];
  }

  public addElement(element: Type): void {
    this.elementTypes.push(element);
  }

  public getElementTypes(): IterableIterator<Type> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export type RecordFieldType
 = PlainRecordFieldType
 | AnyType
 | UnionType

class PlainRecordFieldType extends TypeBase {

  public kind: TypeKind.PlainRecordFieldType = TypeKind.PlainRecordFieldType;

  constructor(public name: string, public type: Type) {
    super();
  }

}

export class RecordType extends TypeBase {

  public kind: TypeKind.RecordType = TypeKind.RecordType;

  private memberTypes: RecordFieldType[] = [];
  private memberTypesByFieldName = new FastStringMap<string, RecordFieldType>();

  constructor(public source: Syntax | Type | number, iterable?: Iterable<RecordFieldType>) {
    super();
    if (iterable !== undefined) {
      for (const type of iterable) {
        this.addMemberType(type);
      }
    }
  }

  public getRequiredFieldNames(): IterableIterator<string> {
    return this.memberTypesByFieldName.keys();
  }

  public addMemberType(type: RecordFieldType): void {
    this.memberTypes.push(type);
    if (type instanceof PlainRecordFieldType) {
      this.memberTypesByFieldName.set(type.name, type);
    }
  }

  public getMemberTypes(): IterableIterator<RecordFieldType> {
    return this.memberTypes[Symbol.iterator]();
  }

  public isFieldRequired(name: string): boolean {
    return this.memberTypesByFieldName.has(name);
  }

  public clear(): void {
    this.memberTypes = [];
    this.memberTypesByFieldName.clear();
  }

}

export class TupleType extends TypeBase {

  kind: TypeKind.TupleType = TypeKind.TupleType;

  constructor(public elementTypes: Type[] = []) {
    super();
  }

}

export class ReturnType extends TypeBase {

  public kind: TypeKind.ReturnType = TypeKind.ReturnType;

  constructor(public fnType: Type, public argumentTypes: Type[]) {
    super();
  }

  public getArgumentTypes(): IterableIterator<Type> {
    return this.argumentTypes[Symbol.iterator]();
  }

}

function isTypePotentiallyCallable(type: Type): boolean {
  return type.kind === TypeKind.FunctionType
      || type.kind === TypeKind.AnyType
      || (type.kind === TypeKind.UnionType && type.elementTypes.some(isTypePotentiallyCallable))
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
    if (hasElementType) {
      out += ' | '
    }
    hasElementType = true;
    switch (elementType.kind) {
      case TypeKind.PlainRecordFieldType:
        out += '{ ' + elementType.name + ' } ';
        break;
      case TypeKind.ReturnType:
        out += 'return type of '
        out += prettyPrintType(elementType.fnType);
        out += '(';
        out += elementType.argumentTypes.map(prettyPrintType).join(', ')
        out += ')'
        break;
      case TypeKind.OpaqueType:
        out += elementType.name;
        break;
      case TypeKind.AnyType:
        out += 'any';
        break;
      case TypeKind.NeverType:
        out += 'never'
        break;
      case TypeKind.FunctionType:
      {
        switch (elementType.source.kind) {
          case SyntaxKind.BoltFunctionDeclaration:
            out += emitNode(elementType.source.name);
            break;
          default:
            throw new Error(`Unexpected source object on function type.`)
        }
        break;
      }
      case TypeKind.TupleType:
      {
        out += '('
        let i = 0;
        for (const tupleElementType of elementType.elementTypes) {
          out += prettyPrintType(tupleElementType);
          i++
          if (i >= BOLT_MAX_FIELDS_TO_PRINT) {
            out += ' (element types omitted)'
            break;
          }
        }
        out += ')'
        break;
      }
      case TypeKind.RecordType:
      {
        if (isSyntax(elementType.source)) {
          switch (elementType.source.kind) {
            case SyntaxKind.BoltRecordDeclaration:
              out += elementType.source.name.text;
              break;
            default:
              throw new Error(`I did not know how to print AST node for a record type`)
          }
        } else {
          throw new Error(`I did not know how to print the source of a record type.`)
        }
        break;
        //out += '{'
        //let i = 0;
        //for (const memberType of elementType.getMemberTypes()) {
        //  for (const memberTypeNoUnion of getAllPossibleElementTypes(memberType)) {
        //    switch (memberTypeNoUnion.kind) {
        //      case TypeKind.AnyType:
        //        out += ' ...';
        //        break;
        //      case TypeKind.PlainRecordFieldType:
        //        out += ' ' + memberTypeNoUnion.name + ': ' + prettyPrintType(memberTypeNoUnion.type);
        //        break;
        //      default:
        //        throw new Error(`I did not know how to pretty-print a record field type.`)
        //    }
        //    i++;
        //    if (i >= BOLT_MAX_FIELDS_TO_PRINT) {
        //      out += ' (field types omitted)'
        //      break
        //    }
        //  }
        //}
        //out += ' }'
        //break;
      }
      default:
        throw new Error(`Could not pretty-print type ${TypeKind[elementType.kind]}`)
    }
  }
  if (!hasElementType) {
    out += 'never'
  }
  return out;
}

let nextRecordTypeId = 1;

function introducesType(node: Syntax) {
  return isBoltExpression(node)
      || isBoltDeclarationLike(node)
      || isBoltParameter(node)
      || isBoltMatchArm(node)
      || isBoltRecordField(node)
      || isBoltRecordFieldPattern(node)
      || isBoltPattern(node)
      || isBoltTypeExpression(node)
      || isJSExpression(node)
      || isJSPattern(node)
      || isJSParameter(node)
}

export class TypeChecker {

  private opaqueTypeFallbacks = new FastStringMap<string, OpaqueType>();

  private dependencyGraph = new FastStringMap<string, Syntax[]>();

  constructor(private resolver: SymbolResolver) {

  }

  private getOpaqueType(path: string): Type {
    const elements = path.split('::');
    const symbolPath = new SymbolPath(elements.slice(0,-1), true, elements[elements.length-1]);
    const sym = this.resolver.resolveGlobalSymbol(symbolPath, ScopeType.Type);
    if (sym === null) {
      if (this.opaqueTypeFallbacks.has(path)) {
        return this.opaqueTypeFallbacks.get(path);
      }
      const opaqueType = new OpaqueType(GLOBAL_SCOPE_MARKER + path);
      this.opaqueTypeFallbacks.set(path, opaqueType);
      return opaqueType;
    }
    return new OpaqueType(GLOBAL_SCOPE_MARKER + path);
  }

  public createTypeForValue(value: Value): Type {
    if (typeof(value) === 'string') {
      return this.getOpaqueType('String')
    } else if (typeof(value) === 'bigint') {
      return this.getOpaqueType('int');
    } else if (typeof(value) === 'number') {
      return this.getOpaqueType('f64');
    } else if (value instanceof Record) {
      const memberTypes = [];
      for (const [fieldName, fieldValue] of value.getFields()) {
        const recordFieldType = new PlainRecordFieldType(name, 
          createTransparentProxy(this.createTypeForValue(fieldValue)));
         memberTypes.push(createTransparentProxy(recordFieldType));
      }
      const recordType = new RecordType(nextRecordTypeId++, memberTypes);
      return recordType;
    } else {
      throw new Error(`Could not determine type of given value.`);
    }
  }

  private *diagnoseTypeMismatch(a: Type, b: Type): IterableIterator<Diagnostic> {

  }

  public registerSourceFile(sourceFile: SourceFile): void {
    for (const node of sourceFile.preorder()) {
      if (introducesType(node)) {
        node.type = new AnyType;
      }
    }
  }

  private *getParentsThatMightNeedUpdate(node: Syntax): IterableIterator<Syntax> {
    while (true) {
      const parentNode = node.parentNode!;
      if (!introducesType(parentNode)) {
        break;
      }
      yield parentNode;
      node = parentNode;
    }
  }

  public solve(sourceFiles: IterableIterator<SourceFile>): void {

    let queued: Syntax[] = [];
    const nextQueue = new Set<Syntax>();

    for (const sourceFile of sourceFiles) {
      for (const node of sourceFile.preorder()) {
        if (introducesType(node)) {
          queued.push(node);
        }
      }
    }

    while (true) {

      if (queued.length === 0) {
        if (nextQueue.size > 0) {
          queued = [...nextQueue];
          nextQueue.clear();
          continue;
        } else {
          break;
        }
      }
      const node = queued.shift()!;

      const derivedType = this.deriveTypeUsingChildren(node);
      derivedType.node = node;
      const newType = this.simplifyType(derivedType);

      //if (newType !== derivedType) {
        //derivedType.solved.nextType = newType;
      //}

      const narrowedType = this.narrowTypeDownTo(node.type!.solved, newType);
      
      if (narrowedType === null) {
        if (!hasTypeError(node)) {
          node.errors.push({
            message: E_TYPE_MISMATCH,
            severity: 'error',
            args: { left: node.type!, right: newType },
            nested: [...this.diagnoseTypeMismatch(node.type!.solved, newType)],
          });
        }
      } else {
        narrowedType.node = node;
        if (node.type.solved !== narrowedType) {
          node.type.solved.nextType = narrowedType;
        }
        for (const dependantNode of this.getParentsThatMightNeedUpdate(node)) {
          nextQueue.add(dependantNode);
        }
        for (const dependantNode of this.getNodesRequiringUpdate(node)) {
          nextQueue.add(dependantNode);
        }
      }

    }
  }

  private markNodeAsRequiringUpdate(origNode: Syntax, nodeToUpdate: Syntax) {
    if (!this.dependencyGraph.has(origNode.id.toString())) {
      this.dependencyGraph.set(origNode.id.toString(), [ nodeToUpdate ]);
    } else {
      this.dependencyGraph.get(origNode.id.toString()).push(nodeToUpdate);
    }
  }

  private *getNodesRequiringUpdate(node: Syntax): IterableIterator<Syntax> {
    if (!this.dependencyGraph.has(node.id.toString())) {
      return;
    }
    const visited = new Set<Syntax>();
    const stack = [ ...this.dependencyGraph.get(node.id.toString()) ]
    while (stack.length > 0) {
      const node = stack.pop()!;
      if (visited.has(node)) {
        continue;
      }
      yield node;
      visited.add(node)
      if (this.dependencyGraph.has(node.id.toString())) {
        for (const dependantNode of this.dependencyGraph.get(node.id.toString())) {
          stack.push(dependantNode);
        }
      }
    }
  }

  public isVoidType(type: Type): boolean {
    return this.areTypesSemanticallyEquivalent(new TupleType, type);
  }

  /**
   * Narrows @param outter down to @param inner. If @param outer could not be narrowed, this function return null.
   * 
   * @param outer The type that will be narrowed.
   * @param inner The type that serves as the outer bound of @param outer.
   */
  private narrowTypeDownTo(outer: Type, inner: Type): Type | null {
    
    // Only types that can be assigned to one another can be narrowed.
    // In all other cases, we should indicate to the user that the operation is invalid.
    if (!this.areTypesSemanticallyEquivalent(inner, outer)) {
      return null;
    }

    // We will build a new union type that contains all type elements that are
    // shared between the inner type and the outer type.
    const elementTypes = [];

    for (const elementType of getAllPossibleElementTypes(inner)) {

      switch (elementType.kind) {

        case TypeKind.AnyType:
          // If we find an `any` type in the innermost type, then it does not make sense
          // to search for other types, as the any-type must be matches at all cost.
          return new AnyType;

        case TypeKind.NeverType:
          // Encountering `never` in the inner type has the same effect as asserting that
          // no type in `outer` is ever matched.
          return new NeverType;

        // The following types kinds are all nominally typed, so we can safely add them to the resulting
        // element types without risk of interference.
        case TypeKind.OpaqueType:
        case TypeKind.FunctionType:
        case TypeKind.RecordType:
          elementTypes.push(elementType);
          break;
   
        default:
          throw new Error(`I received an unexpected ${TypeKind[elementType.kind]}`)

      }

    }

    let hasAnyType = false;

    for (const elementType of getAllPossibleElementTypes(outer)) {

      switch (elementType.kind) {

        case TypeKind.NeverType:
          // A `never`-type in the outer type means we simply skip the type, because only it is 'never'
          // matched, but other types might.
          continue;

        case TypeKind.AnyType:
          // When encountering an `any`-type in the outer type, we can safely skip it because the
          // inner type most likely has more information about the structure of our new type.
          hasAnyType = true;
          break;

        // The following types kinds are all nominally typed, so we can safely add them to the resulting
        // element types without risk of interference.
        case TypeKind.FunctionType:
        case TypeKind.RecordType:
        case TypeKind.OpaqueType:
          elementTypes.push(elementType);
          break

        default:
          throw new Error(`I received an unexpected ${TypeKind[elementType.kind]}`)

      }

    }

    if (hasAnyType && elementTypes.length === 0) {
      return new AnyType;
    }

    this.mergeDuplicateTypes(elementTypes);

    // This is a small optimisation to prevent the complexity of processing a 
    // union type everwhere where it just contains one element.
    if (elementTypes.length === 1) {
      return elementTypes[0];
    }

    return new UnionType(elementTypes);
  }

  private deriveTypeUsingChildren(node: Syntax): Type {

    switch (node.kind) {

      //case SyntaxKind.JSReturnStatement:
      //{
      //  if (node.value === null) {
      //    return this.getOpaqueType('undefined');
      //  }
      //  this.markNodeAsRequiringUpdate(node.value, node);
      //  return node.value.type!.solved;
      //}

      //case SyntaxKind.JSExpressionStatement:
      //{
      //  if (node.expression === null) {
      //    return new TupleType;
      //  }
      //  this.markNodeAsRequiringUpdate(node.expression, node);
      //  return node.expression.type!.solved;
      //}

      case SyntaxKind.JSMemberExpression:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.JSLiteralExpression:
      {
        if (typeof(node.value) === 'string') {
          return this.getOpaqueType('String'); 
        } else if (typeof(node.value) === 'number') {
          return this.getOpaqueType('JSNum');
        } else {
          throw new Error(`I did not know how to derive a type for JavaScript value ${node.value}`)
        }
      }

      case SyntaxKind.JSReferenceExpression:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.JSCallExpression:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.BoltMatchExpression:
      {
        return new UnionType(node.arms.map(arm => {
          this.markNodeAsRequiringUpdate(arm, node);
          return arm.type!.solved;
        }));
      }

      case SyntaxKind.BoltMatchArm:
      {
        const resultType = new UnionType([node.pattern.type!.solved, node.body.type!.solved]);
        this.markNodeAsRequiringUpdate(node.pattern, node);
        this.markNodeAsRequiringUpdate(node.body, node);
        return resultType;
      }

      case SyntaxKind.BoltParameter:
      {
        const resultTypes = [ node.bindings.type!.solved ]
        this.markNodeAsRequiringUpdate(node.bindings, node);
        if (node.typeExpr !== null) {
          resultTypes.push(node.typeExpr.type!.solved);
          this.markNodeAsRequiringUpdate(node.typeExpr, node);
        }
        if (node.defaultValue !== null) {
          resultTypes.push(node.defaultValue.type!.solved)
          this.markNodeAsRequiringUpdate(node.defaultValue, node)
        }
        return new UnionType(resultTypes);
      }

      case SyntaxKind.BoltFunctionExpression:
      {
        const paramTypes = node.params.map(param => {
          this.markNodeAsRequiringUpdate(param, node);
          return param.type!.solved;
        });
        let returnType;
        if (node.returnType === null) {
          returnType = new AnyType;
        } else {
          returnType = node.returnType.type!.solved;
          this.markNodeAsRequiringUpdate(node.returnType, node);
        }
        return new FunctionType(node, paramTypes, returnType);
      }

      case SyntaxKind.BoltRecordDeclaration:
      {
        if (node.members === null) {
          const symbolPath = getFullyQualifiedPathToNode(node);
          return new OpaqueType(symbolPath.encode(), node);
        } else {
          let memberTypes: RecordFieldType[] = []
          for (const member of node.members) {
            //assert(member instanceof PlainRecordFieldType);
            memberTypes.push(member.type!.solved as RecordFieldType)
            this.markNodeAsRequiringUpdate(member, node);
          }
          return new RecordType(node, memberTypes)
        }
      }

      case SyntaxKind.BoltRecordField:
      {
        const resultType = new PlainRecordFieldType(node.name.text, node.typeExpr.type!.solved);
        this.markNodeAsRequiringUpdate(node.typeExpr, node)
        return resultType;
      }

      case SyntaxKind.BoltRecordPattern:
      {
        let memberTypes = []
        for (const member of node.fields) {
          //assert(member.type instanceof PlainRecordFieldType);
          memberTypes.push(member.type!.solved as RecordFieldType);
          this.markNodeAsRequiringUpdate(member, node);
        }
        return new RecordType(node.name.type, memberTypes);
      }

      case SyntaxKind.BoltRecordFieldPattern:
      {
        if (node.isRest) {
          // TODO
        } else {
          assert(node.name !== null);
          let nestedFieldType;
          if (node.pattern === null) {
            nestedFieldType = new AnyType;
          } else {
            nestedFieldType = node.pattern.type!.solved;
            this.markNodeAsRequiringUpdate(node.pattern, node);
          }
          return new PlainRecordFieldType(node.name!.text, nestedFieldType);
        }
      }
      
      case SyntaxKind.BoltTypeAliasDeclaration:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.BoltImplDeclaration:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.BoltTraitDeclaration:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.BoltVariableDeclaration:
      {
        let elementTypes = []
        if (node.value !== null) {
          elementTypes.push(node.value.type!.solved);
          this.markNodeAsRequiringUpdate(node.value, node);
        }
        if (node.typeExpr !== null) {
          elementTypes.push(node.typeExpr.type!.solved);
          this.markNodeAsRequiringUpdate(node.typeExpr, node);
        }
        return new UnionType(elementTypes);
      }

      case SyntaxKind.BoltExpressionPattern:
      {
        return this.deriveTypeUsingChildren(node.expression);
      }

      case SyntaxKind.BoltBindPattern:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.BoltConstantExpression:
      {
        return this.createTypeForValue(node.value);
      }

      case SyntaxKind.BoltFunctionTypeExpression:
      {
        const paramTypes = node.params.map(param => {
          this.markNodeAsRequiringUpdate(param, node);
          return param.type!.solved;
        })
        let returnType = null;
        if (node.returnType === null) {
          returnType = new AnyType;
        } else {
          returnType = node.returnType.type!.solved;
          this.markNodeAsRequiringUpdate(node.returnType!, node);
        }
        return new FunctionType(node, paramTypes, returnType);
      }

      case SyntaxKind.BoltMacroCall:
      {
        // TODO
        return new AnyType;
      }

      case SyntaxKind.BoltQuoteExpression:
      {
        return this.getOpaqueType('Lang::Bolt::Node');
      }

      //case SyntaxKind.BoltExpressionStatement:
      //{
      //  return this.deriveTypeUsingChildren(node.expression);
      //}

      //case SyntaxKind.BoltReturnStatement:
      //{
      //  if (node.value === null) {
      //    const tupleType = new TupleType();
      //    return tupleType
      //  }
      //  return node.value.type!.solved;
      //}

      //case SyntaxKind.BoltBlockExpression:
      //{
      //  let elementTypes = [];
      //  if (node.elements !== null) {
      //    for (const returnStmt of getReturnStatementsInFunctionBody(node.elements)) {
      //      if (returnStmt.value !== null) {
      //        elementTypes.push(returnStmt.value.type!.solved)
      //        this.markNodeAsRequiringUpdate(returnStmt.value, node);
      //      }
      //    }
      //  }
      //  return new UnionType(elementTypes);
      //}

      case SyntaxKind.BoltMemberExpression:
      {
        let unionType = new UnionType([]);
        assert(node.path.length === 1);
        const recordTypes = []; 
        for (const memberType of this.getTypesForMember(node.path[0], node.path[0].text, node.expression.type!.solved)) {
          unionType.addElement(memberType);
        }
        return unionType;
      }

      case SyntaxKind.BoltCallExpression:
      {
        let operandTypes = []
        for (const operand of node.operands) {
          operandTypes.push(operand.type!.solved);
          this.markNodeAsRequiringUpdate(operand, node);
        }
        this.markNodeAsRequiringUpdate(node.operator, node);
        return new ReturnType(node.operator.type!.solved, operandTypes);
      }

      case SyntaxKind.BoltReferenceExpression:
      {
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Variable);
        if (scope === null) {
          return new AnyType;
        }
        const symbolPath = convertNodeToSymbolPath(node.name);
        const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
        if (resolvedSym === null) {
          return new AnyType;
        }
        let elementTypes = [];
        for (const decl of resolvedSym.declarations) {
          elementTypes.push(decl.type!.solved);
          this.markNodeAsRequiringUpdate(decl, node)
        }
        return new UnionType(elementTypes);
      }

      case SyntaxKind.BoltFunctionDeclaration:
      {
        let returnTypes: Type[] = [];
        if (node.returnType !== null) {
          returnTypes.push(node.returnType.type!.solved);
          this.markNodeAsRequiringUpdate(node.returnType, node);
        }
        if (node.body !== null) {
          for (const returnStmt of getAllReturnStatementsInFunctionBody(node.body)) {
            if (returnStmt.value !== null) {
              returnTypes.push(returnStmt.value.type!.solved);
              this.markNodeAsRequiringUpdate(returnStmt.value, node);
            } else {
              returnTypes.push(new TupleType);
            }
          }
        }
        let paramTypes = [];
        for (const param of node.params) {
          paramTypes.push(param.type!.solved);
          this.markNodeAsRequiringUpdate(param, node);
        }
        return new FunctionType(node, paramTypes, new UnionType(returnTypes));
      }

      case SyntaxKind.BoltReferenceTypeExpression:
      {
        if (node.name.modulePath.length === 0) {
          switch ((node.name.name as BoltIdentifier).text) {
            case 'never':
              return new NeverType;
            case 'any':
              return new AnyType;
          }
        }
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Type);
        assert(scope !== null);
        const symbolPath = convertNodeToSymbolPath(node.name);
        const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
        if (resolvedSym === null) {
          return new AnyType;
        }
        let elementTypes = [];
        for (const decl of resolvedSym.declarations) {
          this.markNodeAsRequiringUpdate(decl, node);
          elementTypes.push(decl.type!.solved);
        }
        return new UnionType(elementTypes);
      } 

      default:
        throw new Error(`Unexpected node type ${kindToString(node.kind)}`);

    }
  }

    //if (returnStmt.value === null) {
    //  if (!this.isVoidType(returnType)) {
    //    returnStmt.errors.push({
    //      message: E_MAY_NOT_RETURN_A_VALUE,
    //      severity: 'error',
    //      nested: [{
    //        message: E_MAY_NOT_RETURN_BECAUSE_TYPE_RESOLVES_TO_VOID,
    //        severity: 'error',
    //        node: node.returnType !== null ? node.returnType : node,
    //      }]
    //    });
    //  }
    //} else {
    //  const stmtReturnType = this.getTypeOfExpression(returnStmt.value);
    //  if (!this.isTypeAssignableTo(returnType, stmtReturnType)) {
    //    if (this.isVoidType(stmtReturnType)) {
    //      returnStmt.value.errors.push({
    //        message: E_MUST_RETURN_A_VALUE,
    //        severity: 'error',
    //        nested: [{
    //          message: E_MUST_RETURN_BECAUSE_TYPE_DOES_NOT_RESOLVE_TO_VOID,
    //          severity: 'error',
    //          node: node.returnType !== null ? node.returnType : node,
    //        }]
    //      })
    //    } else {
    //      returnStmt.value.errors.push({
    //        message: E_TYPES_NOT_ASSIGNABLE,
    //        severity: 'error',
    //        args: {
    //          left: returnType,
    //          right: stmtReturnType,
    //        }
    //      })
    //    }
    //  }
    //}

  private areTypesSemanticallyEquivalent(a: Type, b: Type): boolean {

    // The next statements handle equivalence checking of the special types.
    // These checks should happen before other checks.

    if (a.kind === TypeKind.NeverType && b.kind === TypeKind.NeverType) {
      return true;
    }
    if (a.kind === TypeKind.NeverType) {
      return false;
    }
    if (a.kind === TypeKind.AnyType || b.kind === TypeKind.AnyType) {
      return true;
    }

    // Next up are checks for the semantic equivalence of union types. If a union type occurs on the left,
    // each and every type inside the union type must match the right-hand-side. In the other case,
    // the type is semantic equivalent is one of the element types matched the left-hand-side.

    if (a.kind === TypeKind.UnionType) {
      return a.elementTypes.every(el => this.areTypesSemanticallyEquivalent(el, b));
    }
    if (b.kind === TypeKind.UnionType) {
      return b.elementTypes.some(el => this.areTypesSemanticallyEquivalent(el, a));
    }

    // To check equivalence, we have no choice but to resolve the return types and see if the returned type matches
    // the other size of the equivalence. The equivalence is anticommutative, so we have to repeat the checks for
    // each side of the equivalence.

    if (a.kind === TypeKind.ReturnType) {
      const resolvedType = this.resolveReturnType(a);
      return this.areTypesSemanticallyEquivalent(resolvedType, b);
    }
    if (b.kind === TypeKind.ReturnType) {
      const resolvedType = this.resolveReturnType(b);
      return this.areTypesSemanticallyEquivalent(a, resolvedType);
    }

    // The following cases cover types that have nominal typing as their semantics.
    // Checking equivalence between them should be the same as checking whether they originated
    // from the same node in the AST.

    if (a.kind === TypeKind.OpaqueType && b.kind === TypeKind.OpaqueType) {
      return a.name === b.name;
    }
    if (a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType) {
      return a.source === b.source;
    }
    if (a.kind === TypeKind.RecordType && b.kind === TypeKind.RecordType) {
      return a.source === b.source;
    }

    // FIXME There are probably more cases that should be covered.
    throw new Error(`I did not know how to calculate the equivalence of ${TypeKind[a.kind]} and ${TypeKind[b.kind]}`)
  }

  private resolveReturnType(type: ReturnType): Type {

    let resultTypes = [];

    let hasAnyType = false;

    for (const elementType of getAllPossibleElementTypes(type.fnType)) {

      switch (elementType.kind) {

        case TypeKind.NeverType:
          // If we requested the return type of a union containing a 'never'-type, then
          // we are not allowed to let any function match.
          return new NeverType;
          
        case TypeKind.AnyType:
          // The return type of an 'any'-type (which includes all functions that have 'any' as return type)
          // is the 'any'-type. If we don't find a more specific match, this will be the type that is returned.
          hasAnyType = true;
          break;
          
        case TypeKind.FunctionType:

          if (elementType.paramTypes.length < type.argumentTypes.length) {

            if (!hasDiagnostic(type.node!, E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL)) {

              let nested: Diagnostic[] = [];
              for (let i = elementType.paramTypes.length; i < type.argumentTypes.length; i++) {
                nested.push({
                  message: E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER,
                  severity: 'error',
                  node: elementType.getTypeAtParameterIndex(type.argumentTypes.length).node!
                });
              }
              type.node!.errors.push({
                message: E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
                severity: 'error',
                args: {
                  expected: elementType.paramTypes.length,
                  actual: type.argumentTypes.length,
                },
                nested,
              });

            }
            
            // Skip this return type
            continue;

          } else if (elementType.paramTypes.length > type.argumentTypes.length) {

            if (!hasDiagnostic(type.node!, E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL)) {

              const nested: Diagnostic[] = [];
              for (let i = type.argumentTypes.length; i < elementType.paramTypes.length; i++) {
                nested.push({
                  message: E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER,
                  severity: 'info',
                  node: elementType.paramTypes[i].node!
                });
              }
              type.node!.errors.push({
                message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
                severity: 'error',
                args: {
                  expected: elementType.paramTypes.length,
                  actual: type.argumentTypes.length, 
                },
                nested,
              });

            }
          
            // Skip this return type
            continue;

          } else {

            let hasErrors = false;

            const paramCount = type.argumentTypes.length;
            for (let i = 0; i < paramCount; i++) {
              const argType = type.argumentTypes[i];
              const paramType = elementType.paramTypes[i];
              if (!this.areTypesSemanticallyEquivalent(argType, paramType)) {
                if (!hasDiagnostic(type.solved.node!, E_ARGUMENT_TYPE_NOT_ASSIGNABLE)) {
                  type.node!.errors.push({
                    message: E_ARGUMENT_TYPE_NOT_ASSIGNABLE,
                    severity: 'error',
                    node: argType.node,
                    args: { argType, paramType }
                  })
                }
                hasErrors = true;
              }
            }

            // Skip this return type if we had type errors
            if (hasErrors) {
              continue;
            }

          }

          // If the argument types and parameter types didn't fail to match,
          // the function type is eligable to be 'called' by the given ReturnType.
          resultTypes.push(elementType.returnType.solved);
          break;

        default:
          throw new Error(`Resolving the given type will not work.`)

      }

    }

    if (resultTypes.length === 0) {
      if (hasAnyType) {
        return new AnyType;
      } else {
        return new NeverType;
      }
    }

    // Small optimisation to make debugging easier.
    if (resultTypes.length === 1) {
      return resultTypes[0];
    }

    return new UnionType(resultTypes);
  }

  private mergeDuplicateTypes(resultTypes: Type[]): void {

    resultTypes.sort(comparator(areTypesLexicallyLessThan));

    for (let i = 0; i < resultTypes.length; i++) {
      const typeA = resultTypes[i];
      let j = i+1;
      for (; j < resultTypes.length; j++) {
        if (!areTypesLexicallyEquivalent(typeA, resultTypes[j])) {
          break;
        }
      }
      resultTypes.splice(i+1, j-i-1);
    }
    
  }

  private simplifyType(type: Type): Type {

    const resultTypes = [];

    let hasAnyType = false;

    // We will use a stack instead of a normal iteration because during the simplification
    // new union types might be created, of which the elements need to take part in the 
    // iteration.
    const stack = [ type ];

    while (stack.length > 0) {

      const elementType = stack.pop()!;
      
      switch (elementType.kind) {

        case TypeKind.UnionType:
          for (const elementType2 of getAllPossibleElementTypes(elementType)) {
            stack.push(elementType2);
          }
          break;

        case TypeKind.AnyType:
          // We just want one 'any'-type to be present in the resulting union type, so we keep
          // track of a special flag that indicates whether such a type has been detected.
          hasAnyType = true;
          break;

        case TypeKind.NeverType:
          // If any of the union type elements is a type that never matches, then that type has
          // precedence over all the other types.
          return new NeverType;

        case TypeKind.ReturnType:
          const resolvedType = this.resolveReturnType(elementType);
          stack.push(resolvedType);
          break;

        case TypeKind.FunctionType:
        case TypeKind.NeverType:
        case TypeKind.OpaqueType:
          resultTypes.push(elementType);
          break;

        default:
          throw new Error(`I did not know how to simpllify type ${TypeKind[elementType.kind]}`)

      }

    }

    if (resultTypes.length === 0) {
      if (hasAnyType) {
        return new AnyType;
      } else {
        return new NeverType;
      }
    }

    this.mergeDuplicateTypes(resultTypes);

    // Small optimisation to make debugging easier.
    if (resultTypes.length === 1) {
      return resultTypes[0]
    }

    return new UnionType(resultTypes);
  }

}
