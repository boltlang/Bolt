
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

std::string describe(NodeType Type) {
  switch (Type) {
    case NodeType::Identifier:
      return "an identifier";
    case NodeType::CustomOperator:
      return "an operator";
    case NodeType::IntegerLiteral:
      return "an integer literal";
    case NodeType::EndOfFile:
      return "end-of-file";
    case NodeType::BlockStart:
      return "the start of a new indented block";
    case NodeType::BlockEnd:
      return "the end of the current indented block";
    case NodeType::LineFoldEnd:
      return "the end of the current line-fold";
    case NodeType::LParen:
      return "'('";
    case NodeType::RParen:
      return "')'";
    case NodeType::LBrace:
      return "'['";
    case NodeType::RBrace:
      return "']'";
    case NodeType::LBracket:
      return "'{'";
    case NodeType::RBracket:
      return "'}'";
    case NodeType::Colon:
      return "':'";
    case NodeType::Equals:
      return "'='";
    case NodeType::StringLiteral:
      return "a string literal";
    case NodeType::Dot:
      return "'.'";
    case NodeType::PubKeyword:
      return "'pub'";
    case NodeType::LetKeyword:
      return "'let'";
    case NodeType::MutKeyword:
      return "'mut'";
    case NodeType::ReturnKeyword:
      return "'return'";
    case NodeType::TypeKeyword:
      return "'type'";
    default:
      ZEN_UNREACHABLE
  }
}


int main(int argc, const char* argv[]) {

  if (argc < 2) {
    fprintf(stderr, "Not enough arguments provided.\n");
    return 1;
  }

  auto Text = readFile(argv[1]);
  VectorStream<String> Chars(Text, EOF);
  Scanner S(Chars);
  Punctuator PT(S);
  Parser P(PT);

  SourceFile* SF; 

#ifdef NDEBUG
  try {
    SF = P.parseSourceFile();
  } catch (UnexpectedTokenDiagnostic& E) {
    std::cerr << "<unknown.bolt>:" << E.Actual->getStartLine() << ":" << E.Actual->getStartColumn() << ": expected ";
    switch (E.Expected.size()) {
      case 0:
        std::cerr << "nothing";
        break;
      case 1:
        std::cerr << describe(E.Expected[0]);
        break;
      default:
        auto Iter = E.Expected.begin();
        std::cerr << describe(*Iter++);
        NodeType Prev;
        while (Iter != E.Expected.end()) {
          std::cerr << ", " << describe(Prev);
          Prev = *Iter++;
        }
        std::cerr << " or " << describe(Prev);
        break;
    }
    std::cerr << " but instead got '" << E.Actual->getText() << "'\n";
  }
#else
  SF = P.parseSourceFile();
#endif

  Checker TheChecker;
  TheChecker.check(SF);

  return 0;
}

