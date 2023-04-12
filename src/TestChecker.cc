
#include "gtest/gtest.h"

#include "bolt/CST.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Checker.hpp"

using namespace bolt;

auto checkExpression(std::string Input) {
  ConsoleDiagnostics DS;
  TextFile T { "#<anonymous>", Input };
  VectorStream<std::string, Char> Chars { Input, EOF };
  Scanner S(T, Chars);
  Punctuator PT(S);
  Parser P(T, PT);
  auto SF = P.parseSourceFile();
  Checker C(DS);
  auto Solution = C.check(SF);
  return std::make_tuple(
    static_cast<ExpressionStatement*>(SF->Elements[0])->Expression,
    C,
    Solution
  );
}

TEST(CheckerTest, InfersIntFromIntegerLiteral) {
  auto [Expression, Checker, Solution] = checkExpression("1");
  ASSERT_EQ(Checker.getType(Expression, Solution), Checker.getIntType());
}

