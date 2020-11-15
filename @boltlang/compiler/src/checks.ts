import { BoltImportDirective, Syntax, BoltReferenceExpression, BoltReferenceTypeExpression, SyntaxKind, Visitor, BoltSyntax, BoltIdentifier } from "./ast";
import { Program } from "./program";
import { DiagnosticPrinter, E_FILE_NOT_FOUND, E_DECLARATION_NOT_FOUND, E_TYPE_DECLARATION_NOT_FOUND} from "./diagnostics";
import { convertNodeToSymbolPath } from "./resolver"
import { inject } from "./ioc";
import { SymbolResolver, ScopeType } from "./resolver";
import { assert, every } from "./util";
import { emitNode } from "./emitter";
import { TypeChecker } from "./checker";
import { getSymbolText } from "./common";

export class CheckInvalidFilePaths extends Visitor {

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
                severity: 'fatal',
                message: E_FILE_NOT_FOUND,
                args: { filename: node.file.value },
                node: node.file,
            });
        }
    }

}

export class CheckReferences extends Visitor {

    constructor(
        @inject private diagnostics: DiagnosticPrinter,
        @inject private resolver: SymbolResolver,
        @inject private checker: TypeChecker
    ) {
        super();
    }

    private checkBoltModulePath(node: BoltSyntax, elements: BoltIdentifier[]) {

        let modScope = this.resolver.getScopeForNode(node, ScopeType.Module);
        assert(modScope !== null);
        let foundModule = false;
        let partiallyMatchingModules = [];

        // We will keep looping until we are at the topmost module of
        // the package corresponding to `node`.
        while (true) {

          let failedToFindScope = false;
          let currScope = modScope;

          // Go through each of the parent names in normal order, resolving to the module
          // that declared the name, and mark when we failed to look up the inner module.
          for (const name of elements) {
            const sym = currScope!.getLocalSymbol(name.text);;
            if (sym === null) {
              failedToFindScope = true;
              partiallyMatchingModules.push(((currScope!.source) as NodeScopeSource).node);
              break;
            }
            assert(every(sym.declarations.values(), decl => decl.kind === SyntaxKind.BoltModule));
            currScope = sym.scope;
          }

          // If the previous loop did not fail, that means we found a module.
          if (!failedToFindScope) {
            foundModule = true;
            break;
          }

          // We continue the outer loop by going up one scope.
          const nextScope = modScope!.getNextScope();

          // If we are here and there are no scopes left to search in, then no scope had the given module.
          if (nextScope === null) {
              break;
          }

          modScope = nextScope;

        }
 
        if (!foundModule) {
            this.diagnostics.add({
                message: E_DECLARATION_NOT_FOUND,
                severity: 'error',
                args: { name: emitNode(node) },
                node,
            });
            // TODO add informational diagnostics about the modules that provided a partial match
        }

    }

    protected visitBoltReferenceExpression(node: BoltReferenceExpression) {
        this.checkBoltModulePath(node.name, node.name.modulePath);
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Variable);
        assert(scope !== null);
        const resolvedSym = this.resolver.resolveSymbolPath(convertNodeToSymbolPath(node), scope!);
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
        const name = getSymbolText(node.name.name);
        if (node.name.modulePath.length === 0 && this.checker.isBuiltinType(name)) {
            return;
        }
        const scope = this.resolver.getScopeSurroundingNode(node, ScopeType.Type);
        assert(scope !== null);
        const symbolPath = convertNodeToSymbolPath(node.name);
        const resolvedSym = this.resolver.resolveSymbolPath(symbolPath, scope!);
        if (resolvedSym === null) {
            this.diagnostics.add({
                message: E_TYPE_DECLARATION_NOT_FOUND,
                args: { name: emitNode(node.name) },
                severity: 'error',
                node: node.name,
            })
        }
    }


}

export class CheckTypeAssignments extends Visitor {

    constructor(@inject private diagnostics: DiagnosticPrinter) {
        super();
    }

    protected visitSyntax(node: Syntax) {
        for (const error of node.errors) {
            this.diagnostics.add({ node, ...error });
        }
    }

}

//export class CheckTypeAssignments extends NodeVisitor {
//
//    constructor(
//        @inject private diagnostics: DiagnosticPrinter,
//        @inject private checker: TypeChecker,
//    ) {
//        super();
//    }
//
//    protected visitBoltReturnStatement(node: BoltReturnStatement) {
//
//        const fnDecl = node.getParentOfKind(SyntaxKind.BoltFunctionDeclaration)!;
//
//        if ((this.checker.isVoidType(node) && !this.checker.isVoidType(fnDecl)) {
//            this.diagnostics.add({
//              message: E_MUST_RETURN_A_VALUE,
//            node: node.value !== null ? node.value : node,
//              severity: 'error',
//            });
//        } else if (!this.checker.isVoidType(node) && this.checker.isVoidType(fnDecl)) {
//            this.diagnostics.add({
//                message: E_MAY_NOT_RETURN_A_VALUE,
//                node: node.value !== null ? node.value : node,
//                severity: 'error',
//            })
//        } else {
//            for (const error of this.checker.checkAssignment(fnDecl, node)) {
//                this.diagnostics.add({
//                  message: E_TYPES_NOT_ASSIGNABLE,
//                  node: error.node,
//                  severity: 'error',
//                });
//            }
//        }
//
//    }
//
//    protected visitBoltParameter(node: BoltParameter) {
//        if (node.defaultValue !== null) {
//            for (const error of this.checker.checkAssignment(node.bindings, node.defaultValue)) {
//                this.diagnostics.add({
//                    severity: 'error',
//                    message: E_TYPES_NOT_ASSIGNABLE,
//                    node: error.node,
//                });
//            }
//        }
//    }
//
//    protected visitBoltCallExpression(node: BoltCallExpression) {
//        for (const fnDecl of this.checker.getCallableFunctions(node)) {
//            for (const error of this.checker.checkAssignment(fnDecl, node)) {
//                this.diagnostics.add({
//                    severity: 'error',
//                    message: E_TYPES_NOT_ASSIGNABLE,
//                    node: error.node,
//                });
//            }
//        }
//    }
//
//}
