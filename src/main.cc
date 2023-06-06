
#include <stdio.h>

#include <iostream>
#include <fstream>
#include <algorithm>

#include "zen/config.hpp"
#include "zen/po.hpp"

#include "bolt/CST.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Checker.hpp"
#include "bolt/Evaluator.hpp"

using namespace bolt;

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
    .flag(po::flag<bool>("direct-diagnostics", "Immediately print diagnostics without sorting them first")) // TODO support default values in zen::po
    .subcommand(
      po::command("check", "Check sources for programming mistakes")
        .pos_arg("file", po::some))
    .subcommand(
      po::command("eval", "Run sources")
        .pos_arg("file", po::some)
        .fallback())
    .parse_args(Argc, Argv)
    .unwrap();

  ZEN_ASSERT(Match.has_subcommand());

  auto DirectDiagnostics = Match.has_flag("direct-diagnostics") && Match.get_flag<bool>("direct-diagnostics");

  auto [Name, Submatch] = Match.subcommand();

  ConsoleDiagnostics DE;
  LanguageConfig Config;

  std::vector<SourceFile*> SourceFiles;

  for (auto Filename: Submatch->get_pos_args()) {

    auto Text = readFile(Filename);
    TextFile File { Filename, Text };
    VectorStream<ByteString, Char> Chars(Text, EOF);
    Scanner S(File, Chars);
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

  auto lessThan = [](const Diagnostic* L, const Diagnostic* R) {
    auto N1 = L->getNode();
    auto N2 = R->getNode();
    if (N1 == nullptr && N2 == nullptr) {
      return false;
    }
    if (N1 == nullptr) {
      return true;
    }
    if (N2 == nullptr) {
      return false;
    }
    return N1->getStartLine() < N2->getStartLine() || N1->getStartColumn() < N2->getStartColumn();
  };
  std::sort(DS.Diagnostics.begin(), DS.Diagnostics.end(), lessThan);

  for (auto D: DS.Diagnostics) {
    DE.addDiagnostic(D);
  }

  if (DE.hasError()) {
    return 1;
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

