
#pragma once

#include <cstdlib>
#include <memory>

#include "llvm/IR/IRBuilder.h"

#include "bolt/ByteString.hpp"

namespace bolt {

class Checker;
class Type;
class Node;
class SourceFile;
class Expression;
class FunctionDeclaration;;

class LLVMCodeGen {

  llvm::LLVMContext& TheContext;

  Checker& TheChecker;

  std::unordered_map<ByteString, llvm::Type*> Types;

  unsigned int IntBitWidth;
  llvm::Type* IntType;
  llvm::Type* BoolType;
  llvm::StructType* UnitType;

  llvm::PointerType* StringType;

  std::unique_ptr<llvm::IRBuilder<>> Builder;

public:

  LLVMCodeGen(
    llvm::LLVMContext& TheContext,
    Checker& TheChecker
  );

  llvm::Value* generateExpression(Expression* Expr, llvm::BasicBlock* BB);

  llvm::Type* generateType(Type* Ty);

  void generateFunctionDeclaration(FunctionDeclaration* Decl, llvm::BasicBlock* BB);

  void generateElement(Node* Element, llvm::BasicBlock* BB);

  std::unique_ptr<llvm::Module> generate(SourceFile* SF);

};

}
