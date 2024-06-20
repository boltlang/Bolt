
#pragma once

#include <memory>

#include "llvm/IR/IRBuilder.h"

namespace bolt {

class Node;
class SourceFile;
class Expression;

class LLVMCodeGen {

  llvm::LLVMContext* TheContext;

  std::unique_ptr<llvm::IRBuilder<>> Builder;

  std::unique_ptr<llvm::Module> Module;

public:

  LLVMCodeGen(llvm::LLVMContext* TheContext);

  llvm::Value* generateExpression(Expression* E);

  void generateElement(Node* Element);
  void generate(SourceFile* SF);

};

}
