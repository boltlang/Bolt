
#pragma once

#include "bolt/CST.hpp"
#include "bolt/CSTVisitor.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Program.hpp"

namespace bolt {

class CheckBindingsVisitor : public CSTVisitor<CheckBindingsVisitor> {

  DiagnosticEngine& DE;

protected:

  void visitReferenceExpression(ReferenceExpression* Expr) {
    if (!Expr->getScope()->lookup(Expr->getSymbolPath())) {
      DE.add<BindingNotFoundDiagnostic>(Expr->getNameAsString(), Expr->Name);
    }
  }

public:

  CheckBindingsVisitor(DiagnosticEngine& DE):
    DE(DE) {}

};

class CheckBindingsPass {

  Program& P;

public:

  CheckBindingsPass(Program& P):
    P(P) {}

  void applySourceFile(SourceFile* SF) {
    CheckBindingsVisitor V { P.getDiagnostics() };
    V.visit(SF);
  }

};

}

