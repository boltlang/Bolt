
#include <cmath>
#include <memory>

#include "bolt/CST.hpp"
#include "bolt/CSTVisitor.hpp"

#include "LLVMCodeGen.hpp"

namespace bolt {

LLVMCodeGen::LLVMCodeGen(llvm::LLVMContext* TheContext):
  TheContext(TheContext) {}

llvm::Value* LLVMCodeGen::generateExpression(Expression* E) {

  switch (E->getKind()) {

    case NodeKind::LiteralExpression:
    {
      auto Lit = static_cast<LiteralExpression*>(E);
      switch (Lit->Token->getKind()) {
        case NodeKind::IntegerLiteral:
        {
          auto V = static_cast<IntegerLiteral*>(Lit->Token)->V;
          // TODO assert that V fits in the APInt
          return llvm::ConstantInt::get(*TheContext, llvm::APInt(32, V));
        }
        case NodeKind::StringLiteral:
        {
          auto Str = static_cast<StringLiteral*>(Lit->Token)->Text;
          return Builder->CreateGlobalStringPtr(llvm::StringRef(Str));
        }
        default:
          ZEN_UNREACHABLE
      }
    }

    default:
      ZEN_UNREACHABLE

  }

}

void LLVMCodeGen::generateElement(Node* N) {
  switch (N->getKind()) {
    case NodeKind::ExpressionStatement:
    {
      auto Stmt = static_cast<ExpressionStatement*>(N);
      generateExpression(Stmt->Expression);
    }
    default:
      ZEN_UNREACHABLE
  }
}

void LLVMCodeGen::generate(SourceFile* SF) {
  Module = std::make_unique<llvm::Module>(SF->File.getPath(), *TheContext);
  for (auto Element: SF->Elements) {
    generateElement(Element);
  }
}

}
