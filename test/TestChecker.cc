
#include "gtest/gtest.h"

#include "bolt/CST.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Checker.hpp"

using namespace bolt;

auto checkSourceFile(std::string Input) {
  DiagnosticStore DS;
  TextFile T { "#<anonymous>", Input };
  VectorStream<std::string, Char> Chars { Input, EOF };
  Scanner S { DS, T, Chars };
  Punctuator PT(S);
  Parser P { T, PT, DS };
  LanguageConfig Config;
  auto SF = P.parseSourceFile();
  SF->setParents();
  Checker C { DS };
  C.run(SF);
  return std::make_tuple(SF, C, DS);
}

auto checkExpression(std::string Input) {
  auto [SF, C, DS] = checkSourceFile(Input);
  return std::make_tuple(
    static_cast<ExpressionStatement*>(SF->Elements[0])->Expression,
    C,
    DS
  );
}

TEST(CheckerTest, InfersIntFromIntegerLiteral) {
  auto [Expr, Checker, DS] = checkExpression("1");
  ASSERT_EQ(DS.countDiagnostics(), 0);
  ASSERT_EQ(*Checker.getTypeOfNode(Expr), *Checker.getIntType());
}

TEST(CheckerTest, TestIllegalTypingVariable) {
  auto [SF, C, DS] = checkSourceFile("let a: Int = \"foo\"");
  ASSERT_EQ(DS.countDiagnostics(), 1);
  auto D1 = DS.Diagnostics[0];
  ASSERT_EQ(D1->getKind(), DiagnosticKind::TypeMismatchError);
  auto Diag = static_cast<TypeMismatchError*>(D1);
  // TODO these types have to be sorted first
  ASSERT_EQ(Diag->Left, C.getIntType());
  ASSERT_EQ(Diag->Right, C.getStringType());
}

