
import { FastStringMap, assert, isPlainObject, some, prettyPrintTag, map, flatMap, filter, memoize, comparator, Ref, createRef, every, FastMultiMap, getKeyTag, pushAll } from "./util";
import { SyntaxKind, Syntax, isBoltTypeExpression, BoltExpression, BoltFunctionDeclaration, BoltFunctionBodyElement, kindToString, SourceFile, isBoltExpression, BoltCallExpression, BoltIdentifier, isBoltDeclarationLike, isBoltPattern, isJSExpression, isBoltStatement, isJSStatement, isJSPattern, isJSParameter, isBoltParameter, isBoltMatchArm, isBoltRecordFieldValue, isBoltRecordFieldPattern, isEndOfFile, isSyntax, isBoltFunctionDeclaration, isBoltTypeDeclaration, isBoltRecordDeclaration, BoltImplDeclaration, isBoltTraitDeclaration, isBoltImplDeclaration, BoltTypeExpression, BoltDeclaration, BoltTypeDeclaration, BoltReferenceExpression, BoltReferenceTypeExpression, BoltRecordDeclaration, } from "./ast";
import { convertNodeToSymbolPath, ScopeType, SymbolResolver, SymbolInfo, SymbolPath } from "./resolver";
import { Value, Record } from "./evaluator";
import { getAllReturnStatementsInFunctionBody, getFullyQualifiedPathToNode, hasDiagnostic } from "./common";
import { E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL, E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL, E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER, E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER, E_TYPE_MISMATCH, Diagnostic, E_ARGUMENT_TYPE_NOT_ASSIGNABLE, DiagnosticPrinter, E_THIS_NODE_CAUSED_INVALID_TYPE, E_NOT_CALLABLE, E_PARAMETER_DECLARED_HERE, E_BUILTIN_TYPE_MISSING } from "./diagnostics";
import { BOLT_MAX_FIELDS_TO_PRINT } from "./constants";
import { emitNode } from "./emitter";
import { type } from "os";
import { fn } from "moment";
import { intersects } from "semver";

// TODO For function bodies, we can do something special.
//      Sort the return types and find the largest types, eliminating types that fall under other types.
//      Next, add the resulting types as type hints to `fnReturnType`.

// This is a character that is used as a prefix in path names to distinguish 
// a global symbol from a symbol coming from a specific source file.
const GLOBAL_SCOPE_MARKER = '@'

enum TypeKind {
  PrimType,
  AnyType,
  NeverType,
  VoidType,
  FunctionType,
  RecordType,
  PlainRecordFieldType,
  VariantType,
  IntersectType,
  UnionType,
  TupleType,
  CallType,
  TraitType,
}

function toArray<T>(value: T[] | T | null | undefined): T[] {
  if (Array.isArray(value)) {
    return value;
  }
  if (value === null || value === undefined) {
    return [];
  }
  return [ value ];
}

export type Type
  = PrimType
  | VoidType
  | CallType
  | AnyType
  | NeverType
  | FunctionType
  | RecordType
  | VariantType
  | TupleType
  | IntersectType
  | UnionType
  | PlainRecordFieldType
  | TraitType

type TypeRef = Ref<Type>;

let nextTypeId = 1;

function areTypesEquivalent(a: Type, b: Type): boolean {

  if (a.kind === TypeKind.TupleType && b.kind === TypeKind.VoidType) {

  }

  // This is a hack that should be fixed
  if (a.kind !== b.kind) {
    return false;
  }

  if (a.kind === TypeKind.NeverType && a.kind === TypeKind.NeverType) {
      return true;
  }
  if (a.kind === TypeKind.AnyType && a.kind === TypeKind.AnyType) {
    return true;
  }
  if (a.kind === TypeKind.PrimType && b.kind === TypeKind.PrimType) {
    return a.name === b.name;
  }

  if (a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType) {
    return a.source.id === b.source.id;
  }
  if (a.kind === TypeKind.RecordType && b.kind === TypeKind.RecordType) {
    return a.source.id === b.source.id;
  }
  if (a.kind === TypeKind.TraitType && b.kind === TypeKind.TraitType) {
    return a.source.id === b.source.id;
  }

  throw new Error(`I did not expected to see the provided type combination.`)

}

function isTypeLessThan(a: Type, b: Type): boolean {

  if (a.kind !== b.kind) {
    return a.kind < b.kind;
  }

  // These types have only one unique inhabitant, so they are always equal,
  // and, by extension, never less than one another.
  if ((a.kind === TypeKind.NeverType && a.kind === TypeKind.NeverType)
    || (a.kind === TypeKind.AnyType && a.kind === TypeKind.AnyType)
    || (a.kind === TypeKind.VoidType && b.kind === TypeKind.VoidType)) {
      return false;
  }

  if (a.kind === TypeKind.PrimType && b.kind === TypeKind.PrimType) {
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

  if ((a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType)
    || (a.kind === TypeKind.RecordType && b.kind === TypeKind.RecordType)
    || (a.kind === TypeKind.TraitType && b.kind === TypeKind.TraitType)) {
    return a.id < b.id;
  }

  throw new Error(`I did not expected to see the provided type combination.`)
}

abstract class TypeBase {

  public abstract kind: TypeKind;

  public id = nextTypeId++;

  public failed = false;

  constructor(public sourceNodes: Syntax[] = []) {
    
  }

  public [getKeyTag](): string {
    return this.id.toString();
  }

  public markAsFailed() {
    this.failed = true;
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

export class PrimType extends TypeBase {

  public kind: TypeKind.PrimType = TypeKind.PrimType;

  constructor(public name: string, sources: Syntax[] = []) {
    super(sources);
  }

}

export class VoidType extends TypeBase {
  public kind: TypeKind.VoidType = TypeKind.VoidType;
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
    public id: number,
    public paramTypes: TypeRef[],
    public returnType: TypeRef,
    sources: Syntax[] = [],
  ) {
    super(sources);
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

  constructor(public elementTypes: TypeRef[], sources: Syntax[] = []) {
    super(sources);
  }

  public getVariantTypes(): IterableIterator<TypeRef> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export class IntersectType extends TypeBase {

  public kind: TypeKind.IntersectType = TypeKind.IntersectType;

  public elementTypes: TypeRef[] = [];

  constructor(
    elementTypes: Iterable<TypeRef>,
    sources: Syntax[] = []
  ) {
    super(sources);
    this.elementTypes = [...elementTypes];
  }

}

export class UnionType extends TypeBase {

  public kind: TypeKind.UnionType = TypeKind.UnionType;

  public elementTypes: TypeRef[] = [];

  constructor(
    elementTypes: Iterable<TypeRef> = [],
    sources: Syntax[] = []
  ) {
    super(sources);
    this.elementTypes = [...elementTypes];
  }

  public addElement(element: TypeRef): void {
    this.elementTypes.push(element);
  }

  public getElementTypes(): IterableIterator<TypeRef> {
    return this.elementTypes[Symbol.iterator]();
  }

}

export type RecordFieldType
 = PlainRecordFieldType
 | AnyType
 | UnionType

class PlainRecordFieldType extends TypeBase {

  public kind: TypeKind.PlainRecordFieldType = TypeKind.PlainRecordFieldType;

  constructor(public name: string, public type: TypeRef, sources: Syntax[] = []) {
    super(sources);
  }

}

export class TraitType extends TypeBase {

  public kind: TypeKind.TraitType = TypeKind.TraitType;

  private functionTypesByName = new FastStringMap<string, FunctionType>();

  constructor(
    public source: Syntax,
    public memberTypes: Iterable<[string, FunctionType]>,
    sources: Syntax[] = []
  ) {
    super(sources);
    for (const [name, type] of memberTypes) {
      this.functionTypesByName.set(name, type);
    }
  }

}

export class RecordType extends TypeBase {

  public kind: TypeKind.RecordType = TypeKind.RecordType;

  private memberTypes: RecordFieldType[] = [];
  private memberTypesByFieldName = new FastStringMap<string, RecordFieldType>();

  constructor(
    public source: Syntax | Type | number,
    iterable?: Iterable<RecordFieldType>,
    sources: Syntax[] = []
  ) {
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

  constructor(
    public elementTypes: TypeRef[] = [],
    sources: Syntax[] = []
  ) {
    super(sources);
  }

}

export class CallType extends TypeBase {

  public kind: TypeKind.CallType = TypeKind.CallType;

  constructor(
    public fnType: TypeRef,
    public argumentTypes: TypeRef[],
    sources: Syntax[] = [] 
  ) {
    super(sources);
  }

  public getArgumentTypes(): IterableIterator<TypeRef> {
    return this.argumentTypes[Symbol.iterator]();
  }

}

function isTypePotentiallyCallable(type: Type): boolean {
  return type.kind === TypeKind.FunctionType
      || type.kind === TypeKind.AnyType
      || (type.kind === TypeKind.UnionType && type.elementTypes.some(isTypePotentiallyCallable))
}

function* getAllUnionElementTypes(type: TypeRef): IterableIterator<TypeRef> {
  switch (type.kind) {
    case TypeKind.UnionType:
    {
      for (const elementType of type.getElementTypes()) {
        yield* getAllUnionElementTypes(elementType);
      }
      break;
    }
    default:
      yield type;
  }
}

export function prettyPrintType(typeRef: TypeRef): string {
  let out = ''
  let hasElementType = false;
  for (const elementType of getAllUnionElementTypes(typeRef)) {
    if (hasElementType) {
      out += ' | '
    }
    hasElementType = true;
    switch (elementType.kind) {
      case TypeKind.PlainRecordFieldType:
        out += '{ ' + elementType.name + ' } ';
        break;
      case TypeKind.CallType:
        out += 'return type of '
        out += prettyPrintType(elementType.fnType);
        out += '(';
        out += elementType.argumentTypes.map(prettyPrintType).join(', ')
        out += ')'
        break;
      case TypeKind.PrimType:
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
        out += 'fn (';
        out += elementType.paramTypes.map(prettyPrintType).join(', ');
        out += ')';
        break;
      }
      case TypeKind.IntersectType:
      {
        out += elementType.elementTypes.map(t => prettyPrintType(t)).join(' & ');
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
        if (elementType.sourceNodes.length > 0) {
          switch (elementType.sourceNodes[0].kind) {
            case SyntaxKind.BoltRecordDeclaration:
              out += elementType.sourceNodes[0].name.text;
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
      || isBoltFunctionDeclaration(node)
      || isBoltTypeDeclaration(node)
      || isBoltRecordDeclaration(node)
      || isBoltParameter(node)
      || isBoltRecordFieldValue(node)
      || isBoltRecordFieldPattern(node)
      || isBoltPattern(node)
      || isBoltTypeExpression(node)
      || isJSExpression(node)
      || isJSPattern(node)
      || isJSParameter(node)
}

export class TypeChecker {

  private dependencyGraph = new FastStringMap<string, Syntax[]>();

  private neverType = new NeverType();

  constructor(private resolver: SymbolResolver, private diagnostics: DiagnosticPrinter) {

  }

  private getNeverType() {
    return this.neverType;
  }

  private getPrimType(path: string): Type {
    const elements = path.split('::');
    const symbolPath = new SymbolPath(elements.slice(0,-1), true, elements[elements.length-1]);
    const resolvedSymbol = this.resolver.resolveGlobalSymbol(symbolPath, ScopeType.Type);
    if (resolvedSymbol === null) {
      this.diagnostics.add({
        message: E_BUILTIN_TYPE_MISSING,
        args: { name: path },
        severity: 'error',
      })
      return createRef(new AnyType);
    }
    return createRef(new PrimType(GLOBAL_SCOPE_MARKER + path, [...resolvedSymbol.declarations]));
  }

  public createTypeForValue(value: Value, source: Syntax): Type {
    if (typeof(value) === 'string') {
      return new PrimType('@String', [ source ]);
    } else if (typeof(value) === 'bigint') {
      return new PrimType('@int', [ source ]);
    } else if (typeof(value) === 'number') {
      return new PrimType('@f64', [ source ]);
    } else if (value instanceof Record) {
      const memberTypes = [];
      for (const [fieldName, fieldValue] of value.getFields()) {
        const recordFieldType = new PlainRecordFieldType(name, this.createTypeForValue(fieldValue));
         memberTypes.push(recordFieldType);
      }
      const recordType = new RecordType(nextRecordTypeId++, memberTypes);
      return recordType;
    } else {
      throw new Error(`Could not determine type of given value.`);
    }
  }

  private diagnoseTypeError(affectedType: Type, invalidType: Type): void {
    for (const source of affectedType.sourceNodes) {
      this.diagnostics.add({
        message: E_TYPE_MISMATCH,
        severity: 'error',
        args: { left: affectedType, right: invalidType },
        node: source,
        nested: invalidType.sourceNodes.map(node => {
          return {
            message: E_THIS_NODE_CAUSED_INVALID_TYPE,
            severity: 'info',
            node,
            args: { type: invalidType },
          }
        })
      });
    }
  }

  public registerSourceFile(sourceFile: SourceFile): void {
    for (const node of sourceFile.preorder()) {
      if (introducesType(node)) {
        node.type = createRef(new AnyType);
      }
      if (isBoltImplDeclaration(node)) {
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Type);
        assert(scope !== null);
        const traitSymbol = this.resolver.resolveSymbolPath(convertNodeToSymbolPath(node.traitTypeExpr), scope!)
        for (const traitDecl of traitSymbol!.declarations) {
          traitDecl.addImplDeclaration(node);
        }
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
      node.type.replaceWith(new IntersectType([node.type!.cloneRef(), derivedType], [ node ]));
      this.checkType(node.type!);

      if (!node.type!.failed) {
        for (const dependantNode of this.getParentsThatMightNeedUpdate(node)) {
          if (introducesType(dependantNode)) {
            nextQueue.add(dependantNode);
          }
        }
        for (const dependantNode of this.getNodesRequiringUpdate(node)) {
          assert(introducesType(dependantNode));
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
    return this.areTypesEquivalent(new TupleType, type);
  }

  /**
   * Narrows @param outter down to @param inner. If @param outer could not be narrowed, this function return null.
   * 
   * @param targetType The type that will be narrowed.
   * @param smallerType The type that serves as the outer bound of `smallerType`.
   */
  private deriveTypeUsingChildren(node: Syntax): Type {

    switch (node.kind) {

      case SyntaxKind.JSMemberExpression:
      {
        // TODO
        return createRef(new AnyType);
      }

      case SyntaxKind.JSLiteralExpression:
      {
        if (typeof(node.value) === 'string') {
          return this.getPrimType('String'); 
        } else if (typeof(node.value) === 'number') {
          return this.getPrimType('JSNum');
        } else {
          throw new Error(`I did not know how to derive a type for JavaScript value ${node.value}`)
        }
      }

      case SyntaxKind.JSReferenceExpression:
      {
        // TODO
        return createRef(new AnyType);
      }

      case SyntaxKind.JSCallExpression:
      {
        // TODO
        return createRef(new AnyType);
      }

      case SyntaxKind.BoltMatchExpression:
      {
        const exprTypes = [];
        for (const arm of node.arms) {
          this.markNodeAsRequiringUpdate(arm.body, node);
          exprTypes.push(arm.body.type!);
        }
        return createRef(new IntersectType(exprTypes, [ node ]));
      }

      case SyntaxKind.BoltParameter:
      {
        const resultTypes = [ node.bindings.type! ]
        this.markNodeAsRequiringUpdate(node.bindings, node);
        if (node.typeExpr !== null) {
          resultTypes.push(node.typeExpr.type!);
          this.markNodeAsRequiringUpdate(node.typeExpr, node);
        }
        if (node.defaultValue !== null) {
          resultTypes.push(node.defaultValue.type!)
          this.markNodeAsRequiringUpdate(node.defaultValue, node)
        }
        return createRef(new UnionType(resultTypes, [ node ]));
      }

      case SyntaxKind.BoltFunctionExpression:
      {
        for (const param of node.params) {
          this.markNodeAsRequiringUpdate(param, node);
        }
        const paramTypes = node.params.map(param => param.type!);
        let returnType;
        if (node.returnType === null) {
          returnType = createRef(new AnyType([ node ]));
        } else {
          returnType = node.returnType.type!;
          this.markNodeAsRequiringUpdate(node.returnType, node);
        }
        return createRef(new FunctionType(node.id, paramTypes, returnType, [ node ]));
      }

      case SyntaxKind.BoltRecordDeclaration:
      {
        if (node.members === null) {
          const symbolPath = getFullyQualifiedPathToNode(node);
          return createRef(new PrimType(symbolPath.encode(), [ node ]));
        } else {
          let memberTypes: RecordFieldType[] = []
          for (const member of node.members) {
            //assert(member instanceof PlainRecordFieldType);
            memberTypes.push(member.type! as RecordFieldType)
            this.markNodeAsRequiringUpdate(member, node);
          }
          return createRef(new RecordType(node, memberTypes));
        }
      }

      case SyntaxKind.BoltRecordDeclarationField:
      {
        const resultType = new PlainRecordFieldType(node.name.text, node.typeExpr.type!);
        this.markNodeAsRequiringUpdate(node.typeExpr, node)
        return resultType;
      }

      case SyntaxKind.BoltRecordPattern:
      {
        let memberTypes = []
        for (const member of node.fields) {
          //assert(member.type instanceof PlainRecordFieldType);
          memberTypes.push(member.type! as RecordFieldType);
          this.markNodeAsRequiringUpdate(member, node);
        }
        return createRef(new RecordType(node.name, memberTypes));
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
            nestedFieldType = node.pattern.type!;
            this.markNodeAsRequiringUpdate(node.pattern, node);
          }
          return createRef(new PlainRecordFieldType(node.name!.text, nestedFieldType));
        }
      }

      case SyntaxKind.BoltTypeAliasDeclaration:
      {
        // TODO
        return createRef(new AnyType);
      }

      //case SyntaxKind.BoltImplDeclaration:
      //{
      //  return new TraitType(
      //    node,
      //    node.elements
      //      .filter(isBoltFunctionDeclaration)
      //      .map(element => {
      //        this.markNodeAsRequiringUpdate(element, node);
      //        return [
      //          emitNode(element.name),
      //          element.type!.solved
      //        ] as [string, FunctionType];
      //      })
      //  );
      //}

      //case SyntaxKind.BoltTraitDeclaration:
      //{
      //  // TODO
      //  return new AnyType;
      //}

      case SyntaxKind.BoltVariableDeclaration:
      {
        let elementTypes = []
        if (node.value !== null) {
          elementTypes.push(node.value.type!);
          this.markNodeAsRequiringUpdate(node.value, node);
        }
        if (node.typeExpr !== null) {
          elementTypes.push(node.typeExpr.type!);
          this.markNodeAsRequiringUpdate(node.typeExpr, node);
        }
        return createRef(new UnionType(elementTypes));
      }

      case SyntaxKind.BoltExpressionPattern:
      {
        return this.deriveTypeUsingChildren(node.expression);
      }

      case SyntaxKind.BoltBindPattern:
      {
        // TODO
        return createRef(new AnyType);
      }

      case SyntaxKind.BoltConstantExpression:
      {
        return this.createTypeForValue(node.value, node);
      }

      case SyntaxKind.BoltFunctionTypeExpression:
      {
        const paramTypes = node.params.map(param => {
          this.markNodeAsRequiringUpdate(param, node);
          return param.type!;
        })
        let returnType = null;
        if (node.returnType === null) {
          returnType = createRef(new AnyType);
        } else {
          returnType = node.returnType.type!;
          this.markNodeAsRequiringUpdate(node.returnType!, node);
        }
        return createRef(new FunctionType(node.id, paramTypes, returnType, [ node ]));
      }

      case SyntaxKind.BoltMacroCall:
      {
        // TODO
        return createRef(new AnyType);
      }

      case SyntaxKind.BoltQuoteExpression:
      {
        return this.getPrimType('Lang::Bolt::Node');
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
        for (const memberType of this.getTypesForMember(node.path[0], node.path[0].text, node.expression.type!)) {
          unionType.addElement(memberType);
        }
        return createRef(unionType);
      }

      case SyntaxKind.BoltCallExpression:
      {
        let operandTypes = []
        for (const operand of node.operands) {
          operandTypes.push(operand.type!);
          this.markNodeAsRequiringUpdate(operand, node);
        }
        this.markNodeAsRequiringUpdate(node.operator, node);
        return createRef(new CallType(node.operator.type!, operandTypes, [ node ]));
      }

      case SyntaxKind.BoltReferenceExpression:
      {
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Variable);
        if (scope === null) {
          return createRef(new AnyType);
        }
        const symbolPath = convertNodeToSymbolPath(node.name);
        const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
        if (resolvedSym === null) {
          return createRef(new AnyType);
        }
        let elementTypes = [];
        for (const decl of resolvedSym.declarations) {
          elementTypes.push(decl.type!);
          this.markNodeAsRequiringUpdate(decl, node)
        }
        return createRef(new UnionType(elementTypes));
      }

      case SyntaxKind.BoltFunctionDeclaration:
      {
        let returnTypes: TypeRef[] = [];
        if (node.returnType !== null) {
          returnTypes.push(node.returnType.type!);
          this.markNodeAsRequiringUpdate(node.returnType, node);
        }
        if (node.body !== null) {
          for (const returnStmt of getAllReturnStatementsInFunctionBody(node.body)) {
            if (returnStmt.value !== null) {
              returnTypes.push(returnStmt.value.type!);
              this.markNodeAsRequiringUpdate(returnStmt.value, node);
            } else {
              returnTypes.push(createRef(new VoidType([ returnStmt ])));
            }
          }
        }
        let paramTypes = [];
        for (const param of node.params) {
          paramTypes.push(param.type!);
          this.markNodeAsRequiringUpdate(param, node);
        }
        return createRef(new FunctionType(node.id, paramTypes, createRef(new UnionType(returnTypes))));
      }

      case SyntaxKind.BoltReferenceTypeExpression:
      {
        if (node.name.modulePath.length === 0) {
          switch ((node.name.name as BoltIdentifier).text) {
            case 'never':
              return createRef(new NeverType);
            case 'any':
              return createRef(new AnyType);
            case 'int':
              return createRef(new PrimType('@int'));
            case 'String':
              return createRef(new PrimType('@String'));
          }
        }
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Type);
        assert(scope !== null);
        const symbolPath = convertNodeToSymbolPath(node.name);
        const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
        if (resolvedSym === null) {
          return createRef(new NeverType([ node ]));
        }
        let elementTypes = [];
        for (const decl of resolvedSym.declarations) {
          this.markNodeAsRequiringUpdate(decl, node);
          elementTypes.push(decl.type!);
        }
        return createRef(new UnionType(elementTypes, [ node ]));
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

  private areTypesEquivalent(a: Type, b: Type): boolean {

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
      return a.elementTypes.every(el => this.areTypesEquivalent(el, b));
    }
    if (b.kind === TypeKind.UnionType) {
      return b.elementTypes.some(el => this.areTypesEquivalent(el, a));
    }

    // To check equivalence, we have no choice but to resolve the return types and see if the returned type matches
    // the other size of the equivalence. The equivalence is anticommutative, so we have to repeat the checks for
    // each side of the equivalence.

    if (a.kind === TypeKind.CallType) {
      const resolvedType = this.resolveReturnType(a);
      return this.areTypesEquivalent(resolvedType, b);
    }
    if (b.kind === TypeKind.CallType) {
      const resolvedType = this.resolveReturnType(b);
      return this.areTypesEquivalent(a, resolvedType);
    }

    // The following cases cover types that have nominal typing as their semantics.
    // Checking equivalence between them should be the same as checking whether they originated
    // from the same node in the AST.

    if (a.kind === TypeKind.PrimType && b.kind === TypeKind.PrimType) {
      return a.name === b.name;
    }
    if (a.kind === TypeKind.FunctionType && b.kind === TypeKind.FunctionType) {
      return a.source === b.source;
    }
    if (a.kind === TypeKind.RecordType && b.kind === TypeKind.RecordType) {
      return a.source === b.source;
    }
    if (a.kind === TypeKind.TraitType && b.kind === TypeKind.TraitType) {
      return a.source === b.source;
    }

    // FIXME There are probably more cases that should be covered.
    return false;
    //throw new Error(`I did not know how to calculate the equivalence of ${TypeKind[a.kind]} and ${TypeKind[b.kind]}`)
  }

  private diagnoseFunctionCall(callType: CallType, fnType: FunctionType) {

    if (fnType.paramTypes.length > callType.argumentTypes.length) {

      if (!callType.failed) {

        let nested: Diagnostic[] = [];
        for (let i = callType.argumentTypes.length; i < fnType.paramTypes.length; i++) {
          nested.push({
            message: E_CANDIDATE_FUNCTION_REQUIRES_THIS_PARAMETER,
            severity: 'info',
            node: fnType.paramTypes[i].sourceNodes[0]
          });
        }
        this.diagnostics.add({
          message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
          severity: 'error',
          node: callType.sourceNodes[0],
          args: {
            expected: fnType.paramTypes.length,
            actual: callType.argumentTypes.length,
          },
          nested,
        });

        callType.markAsFailed();

      }

      return false;

    } else if (callType.argumentTypes.length > fnType.paramTypes.length) {

      if (!callType.failed) {

        const nested: Diagnostic[] = [];
        for (let i = fnType.paramTypes.length; i < callType.argumentTypes.length; i++) {
          nested.push({
            message: E_ARGUMENT_HAS_NO_CORRESPONDING_PARAMETER,
            severity: 'info',
            node: callType.argumentTypes[i].sourceNodes[0],
          });
        }
        this.diagnostics.add({
          message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
          severity: 'error',
          node: callType.sourceNodes[0],
          args: {
            expected: fnType.paramTypes.length,
            actual: callType.argumentTypes.length, 
          },
          nested,
        });

        callType.markAsFailed();

      }

      return false;

    } else {

      let hasErrors = false;

      const paramCount = fnType.paramTypes.length;

      for (let i = 0; i < paramCount; i++) {
        const argType = callType.argumentTypes[i];
        const paramType = fnType.paramTypes[i];
        if (!this.isTypeAssignableTo(argType, paramType)) {
          if (!argType.failed) {
            this.diagnostics.add({
              message: E_ARGUMENT_TYPE_NOT_ASSIGNABLE,
              severity: 'error',
              node: argType.sourceNodes[0],
              args: { argType, paramType },
              nested: [{
                message: E_PARAMETER_DECLARED_HERE,
                severity: 'info',
                node: fnType.paramTypes[i].sourceNodes[0],
              }]
            });
            argType.markAsFailed();
          }
          hasErrors = true;
        }
      }

      return !hasErrors;

    }

  }

  private *getReturnTypes(fnType: TypeRef, callType: TypeRef): IterableIterator<TypeRef> {

    switch (fnType.kind) {

      case TypeKind.NeverType:
        // If we requested the return type of a union containing a 'never'-type, then
        // we are not allowed to let any function match.
        break;

      case TypeKind.AnyType:
        // The return type of an 'any'-type (which includes all functions that have 'any' as return type)
        // is the 'any'-type. If we don't find a more specific match, this will be the type that is returned.
        yield fnType;
        break;

      case TypeKind.PrimType:
        // We can never call a primitive type, so indicate this error to the user.
        this.diagnostics.add({
          message: E_NOT_CALLABLE,
          severity: 'error',
          node: callType.sourceNodes[0],
        });
        break;

      case TypeKind.UnionType:
        for (const elementType of fnType.elementTypes) {
          yield* this.getReturnTypes(elementType, callType);
        }
        break;

      case TypeKind.FunctionType:
        if (this.diagnoseFunctionCall(callType, fnType)) {
          // If the argument types and parameter types didn't fail to match,
          // the function type is eligable to be 'called' by the given ReturnType.
          yield fnType.returnType;
        }
        break;

      case TypeKind.IntersectType:
        const returnTypes = [];
        for (const elementType of fnType.elementTypes) {
          for (const returnType of this.getReturnTypes(elementType, callType)) {
            returnTypes.push(returnType);
          }
        }
        yield createRef(new IntersectType(returnTypes));
        break;

      default:
        throw new Error(`Resolving the given type will not work.`)

    }

  }

  private mergeTypes(target: Type, source: Type): void {
    if ((target.kind === TypeKind.NeverType && source.kind === TypeKind.NeverType)
      || (target.kind === TypeKind.AnyType && source.kind === TypeKind.AnyType)) {
      pushAll(target.sourceNodes, source.sourceNodes);
    } else {
      throw new Error(`could not merge two types`);
    }
  }

  private removeNeverTypes(types: TypeRef[]) {
    for (let i = 0; i < types.length; i++) {
      if (types[i].kind === TypeKind.NeverType) {
        types.splice(i, 1);
      }
    }
  }

  private removeAnyTypes(types: TypeRef[]) {
    for (let i = 0; i < types.length; i++) {
      if (types[i].kind === TypeKind.AnyType) {
        types.splice(i, 1);
      }
    }
  }

  private mergeNeverTypes(types: TypeRef[]): Type | null {

    let firstNeverType = null;

    for (let i = 0; i < types.length; i++) {
      const type = types[i];
      if (type.kind === TypeKind.NeverType) {
        if (firstNeverType === null) {
          firstNeverType = type;
        } else {
          this.mergeTypes(firstNeverType, type);
        }
      }
    }

    return firstNeverType;

  }

  private mergeAnyTypes(types: TypeRef[]) {

    let firstAnyType = null;

    for (let i = 0; i < types.length; i++) {
      const type = types[i];
      if (type.kind === TypeKind.AnyType) {
        if (firstAnyType === null) {
          firstAnyType = type;
        } else {
          this.mergeTypes(firstAnyType, type);
          types.splice(i, 1);
        }
      }
    }

  }

  private checkType(type: TypeRef) {

    const resultTypes = [];

    switch (type.kind) {

      case TypeKind.PrimType:
        break;

      case TypeKind.FunctionType:
        break;

      case TypeKind.AnyType:
        break;

      case TypeKind.NeverType:
        this.diagnoseTypeError(type, null);
        break;

      case TypeKind.IntersectType:
      {
        for (const elementType of type.elementTypes) {
          this.checkType(elementType);
        }
        const neverType = this.mergeNeverTypes(type.elementTypes);
        if (neverType !== null) {
          this.diagnoseTypeError(type, neverType);
          type.replaceWith(neverType);
          break;
        }
        const anyType = this.mergeAnyTypes(type.elementTypes);
        let firstSpecialType = null;
        for (const elementType of type.elementTypes) {
          if (elementType.kind === TypeKind.PrimType || elementType.kind === TypeKind.FunctionType || elementType.kind === TypeKind.RecordType) {
            if (firstSpecialType === null) {
              firstSpecialType = elementType;
            } else {
              if (!this.areTypesEquivalent(firstSpecialType, elementType)) {
                this.diagnoseTypeError(firstSpecialType, elementType);
                this.mergeTypes(firstSpecialType, elementType);
                elementType.replaceWith(new NeverType(firstSpecialType.sourceNodes))
              }
            }
          }
        }
        if (anyType !== null && firstSpecialType !== null) {
          this.removeAnyTypes(type.elementTypes);
        }
        if (type.elementTypes.length === 1) {
          type.replaceWith(type.elementTypes[0]);
        }
        break;
      }

      case TypeKind.UnionType:
      {
        for (const elementType of type.elementTypes) {
          this.checkType(elementType);
        }
        this.removeNeverTypes(type.elementTypes);
        this.mergeAnyTypes(type.elementTypes);
        break;
      }

      case TypeKind.CallType:
      {
        this.checkType(type.fnType);
        for (const argType of type.getArgumentTypes()) {
          this.checkType(argType);
        }
        const returnTypes = [...this.getReturnTypes(type.fnType, type)];
        type.replaceWith(new UnionType(returnTypes, type.sourceNodes));
        break;
      }

      default:
        throw new Error(`I did not know how to check type ${TypeKind[type.kind]}`)

    }

  }

  private isTypeAssignableTo(source: Type, target: Type): boolean {

    // Functions are at the moment immutable and can never be assigned to one another.
    // FIXME Passing a lambda expression to a function parameter
    if (source.kind === TypeKind.FunctionType || target.kind === TypeKind.FunctionType) {
      return false;
    }

    if (target.kind === TypeKind.NeverType || source.kind === TypeKind.NeverType) {
      return false;
    }
    if (source.kind === TypeKind.AnyType || target.kind === TypeKind.AnyType) {
      return true;
    }

    if (source.kind === TypeKind.PrimType && target.kind === TypeKind.PrimType) {
      return source.name === target.name;
    }

    if (target.kind === TypeKind.IntersectType) {
      return target.elementTypes.every(t => this.isTypeAssignableTo(source, t));
    }
    if (source.kind === TypeKind.IntersectType) {
      return source.elementTypes.every(t => this.isTypeAssignableTo(t, target));
    }

    if (source.kind === TypeKind.UnionType) {
      return source.elementTypes.every(t => this.isTypeAssignableTo(t, target));
    }
    if (target.kind === TypeKind.UnionType) {
      return target.elementTypes.every(t => this.isTypeAssignableTo(source, t));
    }

    // FIXME cover more cases
    return false;
  }

}
