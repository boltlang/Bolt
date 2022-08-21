
#include <stdio.h>

#include <iostream>
#include <fstream>

#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Checker.hpp"

using namespace bolt;

String readFile(std::string Path) {

  std::ifstream File(Path);
  String Out;

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

  auto Text = readFile(argv[1]);
  VectorStream<String> Chars(Text, EOF);
  Scanner S(Chars);
  Punctuator PT(S);
  Parser P(PT);

  SourceFile* SF; 

#ifdef NDEBUG
  try {
    SF = P.parseSourceFile();
  } catch (Diagnostic& D) {
    DE.addDiagnostic(D);
  }
#else
  SF = P.parseSourceFile();
#endif

  Checker TheChecker { DE };
  TheChecker.check(SF);

  return 0;
}

