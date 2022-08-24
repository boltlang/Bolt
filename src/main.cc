
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

  auto Text = readFile(argv[1]);
  TextFile File { argv[1], Text };
  VectorStream<ByteString, Char> Chars(Text, EOF);
  Scanner S(File, Chars);
  Punctuator PT(S);
  Parser P(File, PT);

  SourceFile* SF; 

  try {
    SF = P.parseSourceFile();
  } catch (Diagnostic& D) {
    DE.addDiagnostic(D);
    return 1;
  }

  SF->setParents();

  Checker TheChecker { DE };
  TheChecker.check(SF);

  return 0;
}

