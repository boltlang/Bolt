
#include <cwchar>
#include <iostream>
#include <fstream>
#include <algorithm>
#include <map>

#include "llvm/IR/Module.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Path.h"

#include "zen/po.hpp"
#include "zen/fs/io.hpp"

#include "bolt/CST.hpp"
#include "bolt/CSTVisitor.hpp"
#include "bolt/ConsolePrinter.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Checker.hpp"
#include "bolt/Evaluator.hpp"
#include "bolt/Program.hpp"

#include "LLVMCodeGen.hpp"

using namespace bolt;

/**
 * Status code that can be returned and should according to documentation
 * terminate xargs's looping.
 */
const constexpr int XARGS_STOP_LOOP = 255;

namespace po = zen::po;

auto getAllTokens(Stream<Token*>& S) {
  std::vector<Token*> Tokens;
  for (;;) {
    auto Tok = S.get();
    Tokens.push_back(Tok);
    if (Tok->getKind() == NodeKind::EndOfFile)  {
      break;
    }
  }
  return Tokens;
}

int main(int Argc, const char* Argv[]) {

  auto Match = po::program("bolt", "The offical compiler for the Bolt programming language")
    .flag(po::flag<bool>("additional-syntax", "Enable additional Bolt syntax for asserting compiler state"))
    .flag(po::flag<bool>("direct-diagnostics", "Immediately print diagnostics without sorting them first")) // TODO support default values in zen::po
    .subcommand(
      po::command("check", "Check sources for programming mistakes")
        .pos_arg("file", po::some))
    .subcommand(
      po::command("verify", "Verify integrity of the compiler on selected file(s)")
        .pos_arg("file", po::some))
    .subcommand(
      po::command("build", "Build sources into a library or executable")
        .pos_arg("file", po::some))
    .subcommand(
      po::command("eval", "Run sources")
        .pos_arg("file", po::some))
    .parse_args(Argc, Argv)
    .unwrap();

  ZEN_ASSERT(Match.has_subcommand());

  auto [Name, Submatch] = Match.subcommand();

  auto IsVerify = Name == "verify";
  auto DirectDiagnostics = Match.has_flag("direct-diagnostics") && Match.get_flag<bool>("direct-diagnostics") && !IsVerify;
  auto AdditionalSyntax = Match.has_flag("additional-syntax") && Match.get_flag<bool>("additional-syntax");

  ConsolePrinter ThePrinter;
  ConsoleDiagnostics DE(ThePrinter);
  DiagnosticStore DS;
  LanguageConfig Config;

  Program Prog { DirectDiagnostics ? static_cast<DiagnosticEngine&>(DE) : DS, Config };

  for (auto Filename: Submatch->get_pos_args()) {

    auto ReadResult = zen::fs::read_file(Filename);
    if (!ReadResult) {
      DE.add<OpenFileFailedDiagnostic>(Filename, ReadResult.left());
      continue;
    }
    ByteString Text { ReadResult->c_str(), ReadResult->size() };
    TextFile File { Filename, Text };
    VectorStream<ByteString, Char> Chars { Text, EOF };
    Scanner TheScanner(DE, File, Chars);
    Punctuator ThePunctuator(TheScanner);
    auto Buffer = getAllTokens(ThePunctuator);
    Parser TheParser(File, DE);
    TokenStream Tokens { Buffer };

    auto SF = TheParser.parseSourceFile(Tokens);
    if (SF == nullptr) {
      continue;
    }

    SF->setParents();

    Prog.addSourceFile(Filename, SF);
  }

  Prog.check();

  if (IsVerify) {

    // TODO make this work with mulitple source files at once

    bool HasError = 0;

    struct AssertVisitor : public CSTVisitor<AssertVisitor> {

      Checker& C;
      DiagnosticEngine& DE;

      void visitExpression(Expression* N) {
        for (auto A: N->Annotations) {
          if (A->getKind() == NodeKind::TypeAssertAnnotation) {
            auto TA = static_cast<TypeAssertAnnotation*>(A);
            auto Left = C.getTypeOfNode(N);
            auto Right = TA->getTypeExpression()->getType();
            std::cerr << "verify " << Left->toString() << " == " << Right->toString() << std::endl;
            if (*Left != *Right) {
              DE.add<TypeMismatchError>(Left, Right, TA->getTypeExpression());
            }
          }
        }
        visitEachChild(N);
      }

    };

    for (auto SF: Prog.getSourceFiles()) {
      AssertVisitor V { {}, Prog.getTypeChecker(SF), DE };
      V.visit(SF);
    }

    struct ExpectDiagnosticVisitor : public CSTVisitor<ExpectDiagnosticVisitor> {

      std::multimap<std::size_t, unsigned> Expected;

      void visitExpressionAnnotation(ExpressionAnnotation* N) {
        if (isa<CallExpression>(N->getExpression())) {
          auto CE = static_cast<CallExpression*>(N->getExpression());
          if (isa<ReferenceExpression>(CE->Function)) {
            auto RE = static_cast<ReferenceExpression*>(CE->Function);
            if (RE->getNameAsString() == "expect_diagnostic") {
              ZEN_ASSERT(CE->Args.size() == 1 && isa<LiteralExpression>(CE->Args[0]));
              Expected.emplace(N->Parent->getStartLine(), static_cast<LiteralExpression*>(CE->Args[0])->getAsInt());
            }
          }
        }
      }

    };

    ExpectDiagnosticVisitor V1;
    for (auto SF: Prog.getSourceFiles()) {
      V1.visit(SF);
    }

    for (auto D: DS.Diagnostics) {
      auto N = D->getNode();
      if (N) {
        auto Line = N->getStartLine();
        auto Match = V1.Expected.find(Line);
        if (Match != V1.Expected.end() && Match->second == D->getCode()) {
          std::cerr << "caught 1 diagnostic" << std::endl;
          continue;
        }
      }
      // Whenever D did not succeed to match we have to print the diagnostic error
      ThePrinter.writeDiagnostic(*D);
      HasError = true;
    }

    if (HasError) {
      return XARGS_STOP_LOOP;
    }

  } else {

    DS.sort();
    for (auto D: DS.Diagnostics) {
      ThePrinter.writeDiagnostic(*D);
    }

  }

  if (DE.hasError()) {
    return 255;
  }

  if (Name == "build") {

    // auto HostABI = "x86_64";
    // auto TripleStr = "x86_64-pc-linux-gnu";

    // std::string Error;
    // auto Target = llvm::TargetRegistry::lookupTarget(TripleStr, Error);
    // if (!Target) {
    //   error("failed to create codegen target: {}\n", Error);
    //   return 255;
    // }

    llvm::LLVMContext TheContext;
    for (auto SF: Prog.getSourceFiles()) {

      LLVMCodeGen CG { TheContext, Prog.getTypeChecker(SF) };
      auto Module = CG.generate(SF);

      auto SourcePath = SF->getFilePath();

      auto IRPath = SourcePath.parent_path() / (SourcePath.stem().string() + ".ll");

      std::cerr << IRPath << "\n";

      // std::error_code EC;
      // llvm::raw_fd_ostream OS { IRPath, EC };
      // Module->print(OS, nullptr);
    }

  } else if (Name == "eval") {
    Evaluator E;
    Env GlobalEnv;
    GlobalEnv.add("print", Value::binding([](auto Args) {
      ZEN_ASSERT(Args.size() == 1)
      std::cerr << Args[0].asString() << "\n";
      return Value::unit();
    }));
    for (auto SF: Prog.getSourceFiles()) {
      // TODO add a SourceFile-local env that inherits from GlobalEnv
      E.evaluate(SF, GlobalEnv);
    }

  }

  return 0;
}

