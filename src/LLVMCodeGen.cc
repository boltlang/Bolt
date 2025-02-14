
#include <cmath>
#include <memory>

#include "llvm/IR/Module.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/Type.h"
#include "llvm/IR/Function.h"

#include "bolt/CST.hpp"
#include "bolt/Type.hpp"
#include "bolt/Checker.hpp"

#include "LLVMCodeGen.hpp"

namespace bolt {

LLVMCodeGen::LLVMCodeGen(llvm::LLVMContext& TheContext, Checker& TheChecker):
  TheContext(TheContext), TheChecker(TheChecker) {
    IntBitWidth = 64;
    IntType = llvm::Type::getIntNTy(TheContext, IntBitWidth);
    BoolType = llvm::Type::getInt1Ty(TheContext);
    UnitType = llvm::StructType::get(TheContext);
    StringType = llvm::PointerType::getUnqual(llvm::Type::getInt8Ty(TheContext));
    Types.emplace("Int", IntType);
    Types.emplace("Bool", BoolType);
    Types.emplace("String", BoolType);
  }

llvm::Value* LLVMCodeGen::generateExpression(Expression* E, llvm::BasicBlock* BB) {

  switch (E->getKind()) {

    case NodeKind::LiteralExpression:
    {
      auto Lit = static_cast<LiteralExpression*>(E);
      switch (Lit->Token->getKind()) {
        case NodeKind::IntegerLiteral:
        {
          auto V = static_cast<IntegerLiteral*>(Lit->Token)->V;
          ZEN_ASSERT(V < std::pow(2, IntBitWidth));
          return llvm::ConstantInt::get(TheContext, llvm::APInt(IntBitWidth, V));
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

    case NodeKind::ReturnExpression:
      {
        auto Return = static_cast<ReturnExpression*>(E);
        std::optional<llvm::Value*> Value;
        if (Return->hasExpression()) {
          auto Value = generateExpression(Return->getExpression(), BB);
          Builder->CreateRet(Value);
        } else {
          Builder->CreateRetVoid();
        }
        return llvm::ConstantStruct::get(UnitType, {});
      }

    default:
      ZEN_UNREACHABLE

  }

}

llvm::Type* LLVMCodeGen::generateType(Type* Ty) {

  std::vector<Type*> ParamTypes;
  while (Ty->getKind() == TypeKind::Fun) {
    auto Fun = static_cast<TFun*>(Ty);
    ParamTypes.push_back(Fun->getLeft());
    Ty = Fun->getRight();
  }

  switch (Ty->getKind()) {

    case TypeKind::Con:
    {
      auto Con = static_cast<TCon*>(Ty);
      auto Match = Types.find(ByteString { Con->getName() });
      ZEN_ASSERT(Match != Types.end());
      return Match->second;
    }

    default:
      ZEN_UNREACHABLE

  }
}

void LLVMCodeGen::generateFunctionDeclaration(FunctionDeclaration* Decl, llvm::BasicBlock* BB) {
  auto Ty = generateType(TheChecker.getTypeOfNode(Decl));
  
}

void LLVMCodeGen::generateElement(Node* N, llvm::BasicBlock* BB) {

  if (isa<Expression>(N)) {
    auto Expr = static_cast<Expression*>(N);
    generateExpression(Expr, BB);
    return;
  }

  switch (N->getKind()) {

    case NodeKind::NamedFunctionDeclaration:
    case NodeKind::PrefixFunctionDeclaration:
    case NodeKind::InfixFunctionDeclaration:
    case NodeKind::SuffixFunctionDeclaration:
      return generateFunctionDeclaration(static_cast<FunctionDeclaration*>(N), BB);

    default:
      ZEN_UNREACHABLE

  }

}

std::unique_ptr<llvm::Module> LLVMCodeGen::generate(SourceFile* SF) {
  auto TheModule = std::make_unique<llvm::Module>(SF->File.getPath(), TheContext);
  auto MainType = llvm::FunctionType::get(IntType, std::vector<llvm::Type*> { IntType }, false);
  auto Main = llvm::Function::Create(MainType, llvm::Function::ExternalLinkage, "main", TheModule.get());
  for (auto Element: SF->Elements) {
    generateElement(Element, &Main->getEntryBlock());
  }
  return TheModule;
}

}
