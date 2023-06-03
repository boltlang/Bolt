
#include <stdio.h>

#include <iostream>
#include <fstream>
#include <algorithm>

#include "zen/config.hpp"

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

int main(int argc, const char* argv[]) {

  if (argc < 2) {
    fprintf(stderr, "Not enough arguments provided.\n");
    return 1;
  }

  ConsoleDiagnostics DE;
  LanguageConfig Config;

  auto Text = readFile(argv[1]);
  TextFile File { argv[1], Text };
  VectorStream<ByteString, Char> Chars(Text, EOF);
  Scanner S(File, Chars);
  Punctuator PT(S);
  Parser P(File, PT, DE);

  auto SF = P.parseSourceFile();
  if (SF == nullptr) {
    return 1;
  }

  SF->setParents();

  DiagnosticStore DS;
  Checker TheChecker { Config, DE }; // TODO set this to DS in production
  TheChecker.check(SF);

  auto LT = [](const Diagnostic* L, const Diagnostic* R) {
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
  std::sort(DS.Diagnostics.begin(), DS.Diagnostics.end(), LT);

  for (auto D: DS.Diagnostics) {
    DE.printDiagnostic(*D);
  }

  if (DE.hasError()) {
    return 1;
  }

  Evaluator E;
  Env TheEnv;
  TheEnv.add("print", Value::binding([](auto Args) {
    ZEN_ASSERT(Args.size() == 1)
    std::cerr << Args[0].asString() << "\n";
    return Value::unit();
  }));
  E.evaluate(SF, TheEnv);

  return 0;
}

