import { BoltImportDirective, Syntax, BoltParameter, BoltModulePath, BoltReferenceExpression, BoltReferenceTypeExpression, BoltSourceFile, BoltCallExpression, BoltReturnKeyword, BoltReturnStatement, SyntaxKind, NodeVisitor } from "./ast";
import { Program } from "./program";
import { DiagnosticPrinter, E_FILE_NOT_FOUND, E_TYPES_NOT_ASSIGNABLE, E_DECLARATION_NOT_FOUND, E_TYPE_DECLARATION_NOT_FOUND, E_MUST_RETURN_A_VALUE } from "./diagnostics";
import { getSymbolPathFromNode } from "./resolver"
import { inject } from "./di";
import { SymbolResolver, ScopeType } from "./resolver";
import { assert } from "./util";
import { emitNode } from "./emitter";
import { TypeChecker, Type } from "./types";
import { getReturnStatementsInFunctionBody } from "./common";

export class CheckInvalidFilePaths extends NodeVisitor {

    constructor(
        @inject private program: Program,
        @inject private diagnostics: DiagnosticPrinter,
    ) {
        super();
    }

    protected visitBoltImportDirective(node: BoltImportDirective) {
        const sourceFile = this.program.resolveToSourceFile(node.file.value, node);
        if (sourceFile === null) {
            this.diagnostics.add({
                severity: 'error',
                message: E_FILE_NOT_FOUND,
                args: { filename: node.file.value },
                node: node.file,
            });
        }
    }

}

export class CheckReference extends NodeVisitor {

    constructor(
        @inject private diagnostics: DiagnosticPrinter,
        @inject private resolver: SymbolResolver
    ) {
        super();
    }

    private checkBoltModulePath(node: BoltModulePath, symbolKind: ScopeType) {
        const scope = this.resolver.getScopeForNode(node, symbolKind);
        assert(scope !== null);
        const sym = this.resolver.resolveModulePath(node.elements.map(el => el.text), scope!);
        if (sym === null) {
            this.diagnostics.add({
                message: E_DECLARATION_NOT_FOUND,
                severity: 'error',
                args: { name: emitNode(node) },
                node,
            });
        }
    }

    protected visitBoltReferenceExpression(node: BoltReferenceExpression) {
        if (node.modulePath !== null) {
            this.checkBoltModulePath(node.modulePath, ScopeType.Variable);
        }
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Variable);
        assert(scope !== null);
        const resolvedSym = this.resolver.resolveSymbolPath(getSymbolPathFromNode(node), scope!);
        if (resolvedSym === null) {
            this.diagnostics.add({
                message: E_DECLARATION_NOT_FOUND,
                args: { name: emitNode(node.name) },
                severity: 'error',
                node: node.name,
            })
        }
    }

    protected visitBoltReferenceTypeExpression(node: BoltReferenceTypeExpression) {
        const scope = this.resolver.getScopeForNode(node, ScopeType.Type);
        assert(scope !== null);
        const resolvedSym = this.resolver.resolveSymbolPath(getSymbolPathFromNode(node), scope!);
        if (resolvedSym === null) {
            this.diagnostics.add({
                message: E_TYPE_DECLARATION_NOT_FOUND,
                args: { name: emitNode(node.path) },
                severity: 'error',
                node: node.path,
            })
        }
    }


}

export class CheckTypeAssignments extends NodeVisitor {

    constructor(
        @inject private diagnostics: DiagnosticPrinter,
        @inject private checker: TypeChecker,
    ) {
        super();
    }

    protected visitBoltReturnStatement(node: BoltReturnStatement) {

        const fnDecl = node.getParentOfKind(SyntaxKind.BoltFunctionDeclaration)!;

        if (node.value === null) {
          if (fnDecl.returnType !== null && this.checker.isVoid(fnDecl.returnType)) {
            this.diagnostics.add({
              message: E_MUST_RETURN_A_VALUE,
              node,
              severity: 'error',
            });
          }
        } else {
          for (const error of this.checker.getAssignmentErrors(fnDecl.returnType, node.value)) {
            this.diagnostics.add({
              message: E_MUST_RETURN_A_VALUE,
              node: node,
              severity: 'error',
            });
          }
        }

    }

    protected visitBoltParameter(node: BoltParameter) {
        if (node.defaultValue !== null) {
            for (const error of this.checker.getAssignmentErrors(node.bindings, node.defaultValue)) {
                this.diagnostics.add({
                    severity: 'error',
                    message: E_TYPES_NOT_ASSIGNABLE,
                    args: { node: error.node }
                });
            }
        }
    }

    protected visitBoltCallExpression(node: BoltCallExpression) {
        for (const fnDecl of this.checker.getCallableFunctions(node)) {
            for (const error of this.checker.getAssignmentErrors(fnDecl, node)) {
                this.diagnostics.add({
                    severity: 'error',
                    message: E_TYPES_NOT_ASSIGNABLE,
                    args: { node: error.node },
                });
            }
        }
    }

}
