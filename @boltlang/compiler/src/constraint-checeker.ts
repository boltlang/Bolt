
import { BoltCallExpression, BoltExpression, BoltFunctionDeclaration, BoltFunctionExpression, BoltIdentifier, BoltPattern, BoltTypeExpression, isBoltFunctionDeclaration, isBoltRecordDeclaration, isBoltTypeAliasDeclaration, isBoltVariableDeclaration, kindToString, SourceFile, Syntax, SyntaxKind } from "./ast";
import { getAllReturnStatementsInFunctionBody, getSymbolText } from "./common";
import { DiagnosticPrinter, E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL, E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL, E_TYPE_MISMATCH } from "./diagnostics";
import { Value } from "./evaluator";
import { convertNodeToSymbolPath, ScopeType, SymbolResolver } from "./resolver";
import { assert, FastStringMap, MapLike, prettyPrintTag } from "./util";

class NodeWithContext<N = Syntax> {

  constructor(
    public node: N,
    public context: CFGNode,
  ) {

  }

}

class Environment {

  private boundVariables = new FastStringMap<string, NodeWithContext>();

  constructor(public parentEnv: Environment | null = null) {

  }

  public set(name: string, node: NodeWithContext): void {
    this.boundVariables.set(name, node);
  }

  public lookup(name: string) {
    let currEnv: Environment | null = this;
    do {
      if (currEnv.boundVariables.has(name)){ 
        return currEnv.boundVariables.get(name);
      }
      currEnv = currEnv.parentEnv;
    } while (currEnv !== null);
    throw new Error(`Could not find '${name}' in this environment or any of its parents`);
  }

}

enum TypeKind {
  Prim,
}

type Type
  = PrimType

class PrimType {

  public kind: TypeKind.Prim = TypeKind.Prim;

  constructor(public name: string) {

  }

  [prettyPrintTag]() {
    return this.name;
  }

}

type Condition
  = ExprCondition
  | NotCondition
  | InContextCondition
  | AlwaysTrueCondition

class ExprCondition {

  constructor(public expression: BoltExpression) {

  }

}

class NotCondition {

  constructor(public conditionToNegate: Condition) {

  }

}

class AlwaysTrueCondition {

}

class ConditionGroup {

  private conditions: Condition[] = [];

  public addCondition(condition: Condition): void {
    this.conditions.push(condition);
  }

  public getAllConditions(): Iterable<Condition> {
    return this.conditions;
  }

  public removeCondition(condition: Condition): void {
    for (let i = 0; i < this.conditions.length; i++) {
      if (condition === this.conditions[i]) {
        this.conditions.splice(i, 1);
        break;
      }
    }
  }

  public isEmpty(): boolean {
    return this.conditions.length === 0;
  }

}

class InContextCondition {

  constructor(public node: CFGNode) {

  }

}

class HasValueAssertion {

  constructor(public value: Value) {

  }

}

type Assertion
  = HasValueAssertion
  | ConditionalAssertion
  | NegatedAssertion

class NegatedAssertion {

  constructor(public propertyToNegate: Assertion) {

  }

}

class ConditionalAssertion {

  constructor(
    public condition: ConditionGroup,
    public assertion: Assertion,
  ) {

  }

}

class PropertyGroup {

  private properties: Assertion[] = []

  // public addCondition(condition: Condition): void {
  //   this.assumptions.push(condition);
  // }

  public addProperty(property: Assertion) {
    this.properties.push(property);
  }

  public getAllProperties(): Iterable<Assertion> {
    return this.properties[Symbol.iterator]();
  }

  [Symbol.iterator]() {
    return this.properties[Symbol.iterator]();
  }

}

export type NodeProperties = FastStringMap<number, PropertyGroup>;

class CFGNode {


  constructor(
    public id: number,
    public sourceNode: Syntax,
    public prevNode: CFGNode | null = null,
  ) {

  }

  // public addPropertyToNode(node: Syntax, property: Property) {
  //   if (!this.properties.has(node.id)) {
  //     this.properties.set(node.id, new PropertyGroup);
  //   }
  //   this.properties.get(node.id).addProperty(property);
  // }

  // public getPropertiesOfNode(node: Syntax): PropertyGroup {
  //   if (!this.properties.has(node.id)) {
  //     this.properties.set(node.id, new PropertyGroup);
  //   }
  //   return this.properties.get(node.id);
  // }

  public isStaticContext() {
    let currContext: CFGNode | null = this;
    do {
      if (currContext.assumptions.length > 0) {
        return false;
      }
      currContext = currContext.prevNode;
    } while (currContext !== null);
    return true;
  }

}

const BUILTIN_TYPES = [
  'String',
  'int',
  'f32',
  'f64',
  'bool'
]

const BUILTIN_OPERATORS: MapLike<Function> = {

  '=='(lhs: number, rhs: number) {
    return lhs === rhs;
  }

}

function hasBuiltinOperator(name: string) {
  return BUILTIN_OPERATORS[name] !== undefined;
}

function getBuiltinOperator(name: string) {
  assert(BUILTIN_OPERATORS[name] !== undefined);
  return BUILTIN_OPERATORS[name];
}

export class TypeChecker {

  private nextSimulationContextId = 1;
  private contextById = new FastStringMap<number, CFGNode>();

  constructor(private resolver: SymbolResolver, private diagnostics: DiagnosticPrinter) {
    
  }

  private getContextById(id: number): CFGNode {
    return this.contextById.get(id);
  }

  public registerSourceFile(sourceFile: SourceFile) {

  }

  public checkNode(node: Syntax) {

    switch (node.kind) {

      case SyntaxKind.BoltSourceFile:
        const env = new Environment();
        let context = this.createCFGNode(node);
        for (const element of node.elements) {
          context = this.simulateEvaluation(element, context, env);
        }
        this.checkTypeAssertions(node);
        break;

      default:
        throw new Error(`Could not check node of kind ${kindToString(node.kind)}`)

    }

  }

  private *getStaticPropertiesOfNode(node: Syntax): Iterable<Assertion> {
    for (const [context, property] of this.getAllPropertiesOfNode(node)) {
      yield property;
    }
  }

  private inferTypeFromStaticProperties(node: Syntax): Type {

    for (const property of this.getStaticPropertiesOfNode(node)) {
      if (property instanceof HasValueAssertion) {
        if (typeof(property.value) === 'bigint') {
          return new PrimType('int');
        } else if (typeof(property.value) === 'string') {
          return new PrimType('String');
        } else {
          throw new Error(`Could not infer the type of an unknown value.`);
        }
      }
    }

    throw new Error(`Could not deduce the type of ${kindToString(node.kind)} because not enough properties were available.`);
  }

  private isTypeAssignableTo(sourceType: Type, targetType: Type) {

    if (sourceType.kind === TypeKind.Prim && targetType.kind === TypeKind.Prim) {
      return sourceType.name === targetType.name;
    }

    throw new Error(`Could not deduce whether unknown types ${sourceType} and ${targetType} are assignable.`);
  }

  private checkTypeAssignment(value: BoltExpression, typeExpr: BoltTypeExpression) {

    switch (typeExpr.kind) {

      case SyntaxKind.BoltReferenceTypeExpression:
      {
        if (typeExpr.name.isAbsolute === false
            && typeExpr.name.modulePath.length === 0
            && BUILTIN_TYPES.indexOf((typeExpr.name.name as BoltIdentifier).text) !== -1) {
          const valueType = this.inferTypeFromStaticProperties(value);
          const typeExprType = new PrimType((typeExpr.name.name as BoltIdentifier).text);
          if (!this.isTypeAssignableTo(valueType, typeExprType)) {
            this.diagnostics.add({
              node: value,
              message: E_TYPE_MISMATCH,
              severity: 'error',
              args: {
                left: typeExprType,
                right: valueType,
              }
            });
          }
          break;
        }
        const scope = this.resolver.getScopeForNode(typeExpr, ScopeType.Type);
        assert(scope !== null);
        const resolvedSymbol = this.resolver.resolveSymbolPath(convertNodeToSymbolPath(typeExpr), scope!);
        if (resolvedSymbol === null) {
          // TODO add diagnostic
          break;
        }
        for (const declNode of resolvedSymbol.declarations) {
          if (isBoltTypeAliasDeclaration(declNode)) {
            // TODO
          } else if (isBoltRecordDeclaration(declNode)) {
            // TODO
          }
        }
        break;
      }

      default:
        throw new Error(`Could not check type assignment because ${kindToString(typeExpr.kind)} was not handled.`)

    }

  }

  public getTypeFromTypeExpr(typeExpr: BoltTypeExpression) {
    
  }

  private checkTypeAssertions(node: Syntax): void {

    switch (node.kind) {

      case SyntaxKind.BoltModule:
      case SyntaxKind.BoltSourceFile:
        for (const element of node.elements) {
          this.checkTypeAssertions(element);
        }
        break;

      case SyntaxKind.BoltVariableDeclaration:
        if (node.typeExpr !== null && node.value !== null) {
          this.checkTypeAssignment(node.value, node.typeExpr);
        }
        break;

      case SyntaxKind.BoltFunctionDeclaration:
        if (node.returnTypeExpr !== null && node.body !== null) {
          for (const returnStmt of getAllReturnStatementsInFunctionBody(node.body)) {
            if (returnStmt.value !== null) {
              this.checkTypeAssignment(returnStmt.value, node.returnTypeExpr);
            } else {
              // TODO
            }
          }
        }
        break;

    }

  }

  /**
   * Resolves the given expression to a function declaration or function
   * expression that matches the given signature.
   */
  private resolveToCallable(expr: BoltExpression, args: BoltExpression[]): BoltFunctionDeclaration | BoltFunctionExpression | null {

    // TODO actually match the signature for overloads

    switch (expr.kind) {

      case SyntaxKind.BoltFunctionExpression:
        return expr;

      case SyntaxKind.BoltReferenceExpression:
      {
        const scope = this.resolver.getScopeForNode(expr, ScopeType.Variable);
        assert(scope !== null);
        const resolvedSymbol = this.resolver.resolveSymbolPath(convertNodeToSymbolPath(expr), scope!);
        if (resolvedSymbol === null) {
          return null;
        }
        for (const declNode of resolvedSymbol.declarations) {
          if (isBoltVariableDeclaration(declNode)) {
            if (declNode.value !== null) {
              const resolved = this.resolveToCallable(declNode.value, args);
              if (resolved !== null) {
                return resolved;
              }
            } else {
              // TODO add diagnostic
            }
          } else if (isBoltFunctionDeclaration(declNode)) {
            return declNode;
          } else {
            throw new Error(`While resolving to a function: a reference expression resolved to an unhandled declaration ${kindToString(declNode.kind)}`);
          }
        }
      }

      default:
        throw new Error(`While resolving to a function: the expression ${kindToString(expr.kind)} was not recognised.`)

    }

  }

  private createCFGNode(sourceNode: Syntax, prevContext: CFGNode | null = null) {
    const context = new CFGNode(this.nextSimulationContextId++, sourceNode, prevContext);
    this.contextById.set(context.id, context);
    return context;
  }

  private bindExpressionToPattern(
    pattern: BoltPattern,
    expr: BoltExpression,
    exprContext: CFGNode,
    env: Environment
  ) {

    switch (pattern.kind) {

      case SyntaxKind.BoltBindPattern:
        env.set(pattern.name.text, new NodeWithContext(expr, exprContext));
        break;

      default:
        throw new Error(`While binding an expression to a pattern: unhandled pattern of kind ${kindToString(pattern.kind)}`);

    }
  }

  private *getAllPropertiesOfNode(node: Syntax): IterableIterator<[CFGNode, Assertion]> {
    if (node.properties === undefined) {
      return;
    }
    for (const [contextId, propertyGroup] of node.properties) {
      const context = this.getContextById(contextId);
      for (const property of propertyGroup.getAllProperties()) {
        yield [context, property];
      }
    }
  }

  private getAllPropertiesOfNodeInContext(node: Syntax, expectedContext: CFGNode): PropertyGroup {
    if (node.properties === undefined) {
      node.properties = new FastStringMap<number, PropertyGroup>();
    }
    if (!node.properties.has(expectedContext.id)) {
      node.properties.set(expectedContext.id, new PropertyGroup());
    }
    return node.properties.get(expectedContext.id);
  }

  private addPropertyToNodeInContext(node: Syntax, property: Assertion, context: CFGNode) {
    if (node.properties === undefined) {
      node.properties = new FastStringMap<number, PropertyGroup>();
    }
    if (!node.properties.has(context.id)) {
      node.properties.set(context.id, new PropertyGroup());
    }
    node.properties.get(context.id).addProperty(property);
  }

  private evaluateExpression(expr: BoltExpression, env: Environment): Value | null {

    switch (expr.kind) {

      case SyntaxKind.BoltConstantExpression:
        return expr.value;

      case SyntaxKind.BoltCallExpression:
        const fn = this.evaluateExpression(expr.operator, env);
        const args = expr.operands.map(op => this.evaluateExpression(op, env));
        return fn(...args);

      case SyntaxKind.BoltReferenceExpression:
        const text = getSymbolText(expr.name.name);
        if (!expr.name.isAbsolute
            && expr.name.modulePath.length === 0
            && hasBuiltinOperator(text)) {
          return getBuiltinOperator(text);
        } else {
          const resolved = env.lookup(text);
          return this.evaluateExpression(resolved.node, env);
        }

      default:
        throw new Error(`Could not evaluate expression of unhandled kind ${kindToString(expr.kind)}`);

    }

  }

  private doesAssumptionHold(assumption: Condition, env: Environment): boolean {
    if (assumption instanceof ExprCondition) {
      return this.evaluateExpression(assumption.expression, env) === true;
    } else if (assumption instanceof NotCondition) {
      return !this.doesAssumptionHold(assumption.conditionToNegate, env);
    } else if (assumption instanceof ConditionGroup) {
      for (const childCond of assumption.getAllConditions()) {
        if (!this.doesAssumptionHold(childCond, env)) {
          return false;
        }
      }
      return true;
    } else {
      throw new Error(`Unrecognised assumption ${assumption}`);
    }
  }

  private reduceAssumptions(assumptions: ConditionGroup, env: Environment) {
    for (const assumption of [...assumptions.getAllConditions()]) {
      if (this.doesAssumptionHold(assumption, env)) {
        assumptions.removeCondition(assumption);
      }
    }
  }

  private addPropertiesOfNodeToNode(srcNode: Syntax, srcContext: CFGNode, dstNode: Syntax, dstContext: CFGNode) {
     for (const property of this.getAllPropertiesOfNodeInContext(srcNode, srcContext)) {
       this.addPropertyToNodeInContext(dstNode, property, dstContext);
     }
  }

  private simulateCallExpression(expr: BoltCallExpression, context: CFGNode, env: Environment): CFGNode {

    const fnDecl = this.resolveToCallable(expr.operator, expr.operands);

    if (fnDecl === null) {
      // TODO add diagnostic
      return context;
    }

    switch (fnDecl.kind) {

      case SyntaxKind.BoltFunctionDeclaration:
      {

        if (expr.operands.length < fnDecl.params.length) {
          this.diagnostics.add({
            message: E_TOO_FEW_ARGUMENTS_FOR_FUNCTION_CALL,
            severity: 'error',
            node: expr,
          })
        } else if (expr.operands.length > fnDecl.params.length) {
          this.diagnostics.add({
            message: E_TOO_MANY_ARGUMENTS_FOR_FUNCTION_CALL,
            severity: 'error',
            node: expr,
          })
        } else {
          const fnEnv = new Environment(env);
          let fnContext = context;
          if (fnDecl.body !== null) {
            for (let i = 0; i < fnDecl.params.length; i++) {
              const param = fnDecl.params[i];
              const arg = expr.operands[i];
              this.bindExpressionToPattern(param.bindings, arg, context, fnEnv);
            }
            for (const element of fnDecl.body) {
              fnContext = this.simulateEvaluation(element, fnContext, fnEnv);
            }
            for (const returnStmt of getAllReturnStatementsInFunctionBody(fnDecl.body)) {
              if (returnStmt.value !== null) {
                this.addPropertiesOfNodeToNode(returnStmt.value, fnContext, expr, context);
              } else {
                // TODO handle case where return expression is not set
              }
            }
          }
          return context;
        }
      }

      default:
        throw new Error(`Could not simulate application because the node ${kindToString(fnDecl.kind)} was not recognised.`)

    }

  }

  private mergeProperties(a: BoltExpression, b: BoltExpression, context: CFGNode) {

  }

  private setExpressionToTrue(expr: BoltExpression, context: CFGNode, negated: boolean) {

    switch (expr.kind) {

      case SyntaxKind.BoltCallExpression:
      {
        if (expr.operator.kind === SyntaxKind.BoltReferenceExpression) {
          const text = getSymbolText(expr.operator.name.name);
          switch (text) {

            case '==':
              this.mergeProperties(expr.operands[0], expr.operands[1], context);
              break;

            default:
              throw new Error(`Could not assume the given expression to be true: unsupported operator`)

          }
        }
      }

    }

  }

  private setConditionToTrue(condition: Condition, context: CFGNode, negated: boolean = true): void {
    if (condition instanceof ExprCondition) {
      this.setExpressionToTrue(condition.expression, context, negated);
    } else if (condition instanceof NotCondition) { 
      this.setConditionToTrue(condition.conditionToNegate, context, !negated);
    } else {
      throw new Error(`Could not convert unhandled conditon to a property.`);
    }
  }

  private simulateEvaluation(node: Syntax, context: CFGNode, env: Environment): CFGNode {

    switch (node.kind) {

      case SyntaxKind.BoltReferenceExpression:
        assert(!node.name.isAbsolute);
        assert(node.name.modulePath.length === 0);
        const nodeWithContext = env.lookup(getSymbolText(node.name.name));
        this.addPropertiesOfNodeToNode(nodeWithContext.node, nodeWithContext.context, node, context);
        return context;

      case SyntaxKind.BoltConstantExpression:
        this.addPropertyToNodeInContext(node, new HasValueAssertion(node.value), context);
        return context;

      case SyntaxKind.BoltRecordDeclaration:
      case SyntaxKind.BoltFunctionDeclaration:
        return context;

      case SyntaxKind.BoltCallExpression:
        return this.simulateCallExpression(node, context, env);

      case SyntaxKind.BoltCaseStatement:
      {
        let failedConditions = [];

        for (const caseNode of node.cases) {

          // Create a new context that will be used to evaluate the
          // if-statement's body. The context will hold additional properties
          // about this if-statement, as well as the properties of the parent context.
          const caseContext = this.createCFGNode(caseNode, context);

          // If a previous condition failed, we know that whatever expression
          // that was used to test that condition must evaluate to false. We
          // add this information to the newly created context.
          for (const failedCondition of failedConditions) {
            this.setConditionToTrue(failedCondition, caseContext);
          }

          // We are going to evaluate the body of this test case, which assumes
          // that the test expression associated with this if-statement
          // evaluated to true. We add these assumptions to the context that
          // will be used to evaluate the statements.
          if (caseNode.test !== null) {
            const condition = new ExprCondition(caseNode.test);
            this.setConditionToTrue(condition, caseContext);
            failedConditions.push(new NotCondition(condition))
          }

          // Now iterate over the if-statement's body with the new context.
          for (const element of caseNode.body) {
            this.simulateEvaluation(element, context, env);
          }

        }
        return context;
      }

      case SyntaxKind.BoltReturnStatement:
        if (node.value !== null) {
          this.simulateEvaluation(node.value, context, env);
        } else {
          // TODO handle the case where return is empty
        }
        break;

      case SyntaxKind.BoltExpressionStatement:
        return this.simulateEvaluation(node.expression, context, env);

      case SyntaxKind.BoltVariableDeclaration:
        if (node.value !== null) {
          this.bindExpressionToPattern(node.bindings, node.value, context, env);
          return this.simulateEvaluation(node.value, context, env);
        } else {
          this.addPropertyToNodeInContext(node, new IsUnsafeProperty(), context);
          return context;
        }

      default:
        throw new Error(`Could not simulate evaluation of node of kind ${kindToString(node.kind)}`);

    }

  }

}
