
#include <stdio.h>

#include <iostream>
#include <fstream>
#include <algorithm>
#include <map>

#include "zen/config.hpp"
#include "zen/po.hpp"

#include "bolt/CST.hpp"
#include "bolt/CSTVisitor.hpp"
#include "bolt/ConsolePrinter.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Checker.hpp"
#include "bolt/Evaluator.hpp"

using namespace bolt;

/**
 * Status code that can be returned and should according to documentation
 * terminate xargs's looping.
 */
const constexpr int XARGS_STOP_LOOP = 255;

ByteString readFile(std::string Path) {

  std::ifstream File(Path);
  ByteString Out;

  File.seekg(0, std::ios::end);
  Out.reserve(File.tellg());
  File.seekg(0, std::ios::beg);

  Out.assign((std::istreambuf_iterator<char>(File)),
              std::istreambuf_iterator<char>());

  return Out;
}

namespace po = zen::po;

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
      po::command("eval", "Run sources")
        .pos_arg("file", po::some)
        .fallback())
    .parse_args(Argc, Argv)
    .unwrap();

  ZEN_ASSERT(Match.has_subcommand());

  auto [Name, Submatch] = Match.subcommand();

  auto IsVerify = Name == "verify";
  auto DirectDiagnostics = Match.has_flag("direct-diagnostics") && Match.get_flag<bool>("direct-diagnostics") && !IsVerify;
  auto AdditionalSyntax = Match.has_flag("additional-syntax") && Match.get_flag<bool>("additional-syntax");

  ConsolePrinter ThePrinter;
  ConsoleDiagnostics DE(ThePrinter);
  LanguageConfig Config;

  std::vector<SourceFile*> SourceFiles;

  for (auto Filename: Submatch->get_pos_args()) {

    auto Text = readFile(Filename);
    TextFile File { Filename, Text };
    VectorStream<ByteString, Char> Chars(Text, EOF);
    Scanner S(DE, File, Chars);
    Punctuator PT(S);
    Parser P(File, PT, DE);

    auto SF = P.parseSourceFile();
    if (SF == nullptr) {
      continue;
    }

    SF->setParents();

    SourceFiles.push_back(SF);
  }

  DiagnosticStore DS;
  Checker TheChecker { Config, DirectDiagnostics ? static_cast<DiagnosticEngine&>(DE) : static_cast<DiagnosticEngine&>(DS) };

  for (auto SF: SourceFiles) {
    TheChecker.check(SF);
  }

  if (IsVerify) {

    // TODO make this work with mulitple source files at once

    bool HasError = 0;

    struct AssertVisitor : public CSTVisitor<AssertVisitor> {
      Checker& C;
      DiagnosticEngine& DE;
      void visitExpression(Expression* N) {
        for (auto A: N->Annotations) {
          if (A->getKind() == NodeKind::TypeAssertAnnotation) {
            auto Left = C.getType(N);
            auto Right = static_cast<TypeAssertAnnotation*>(A)->getTypeExpression()->getType();
            std::cerr << "verify " << describe(Left) << " == " << describe(Right) << std::endl;
            if (*Left != *Right) {
              DE.add<UnificationErrorDiagnostic>(Left, Right, TypePath(), TypePath(), A);
            }
          }
        }
        visitEachChild(N);
      }
    };

    AssertVisitor V { {}, TheChecker, DE };
    for (auto SF: SourceFiles) {
      V.visit(SF);
    }

    struct ExpectDiagnosticVisitor : public CSTVisitor<ExpectDiagnosticVisitor> {
      std::multimap<std::size_t, unsigned> Expected;
      void visitExpressionAnnotation(ExpressionAnnotation* N) {
        if (N->getExpression()->is<CallExpression>()) {
          auto CE = static_cast<CallExpression*>(N->getExpression());
          if (CE->Function->is<ReferenceExpression>()) {
            auto RE = static_cast<ReferenceExpression*>(CE->Function);
            if (RE->getNameAsString() == "expect_diagnostic") {
              ZEN_ASSERT(CE->Args.size() == 1 && CE->Args[0]->is<LiteralExpression>());
              Expected.emplace(N->Parent->getStartLine(), static_cast<LiteralExpression*>(CE->Args[0])->getAsInt());
            }
          }
        }
      }
    };

    ExpectDiagnosticVisitor V1;
    for (auto SF: SourceFiles) {
      V1.visit(SF);
    }

    for (auto D: DS.Diagnostics) {
      auto N = D->getNode();
      if (N) {
        auto Line = N->getStartLine();
        auto Match = V1.Expected.find(Line);
        if (Match != V1.Expected.end() && Match->second == D->getCode()) {
          std::cerr << "skipped 1 diagnostic" << std::endl;
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

  if (Name == "eval") {
    Evaluator E;
    Env GlobalEnv;
    GlobalEnv.add("print", Value::binding([](auto Args) {
      ZEN_ASSERT(Args.size() == 1)
      std::cerr << Args[0].asString() << "\n";
      return Value::unit();
    }));
    for (auto SF: SourceFiles) {
      // TODO add a SourceFile-local env that inherits from GlobalEnv
      E.evaluate(SF, GlobalEnv);
    }
  }

  return 0;
}

