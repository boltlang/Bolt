
#include "gtest/gtest.h"

#include "bolt/Text.hpp"

using namespace bolt;

TEST(TextFileTest, ReportsCorrectLine) {
  TextFile T1 { "foo.txt", "bar\nbaz\nbax\n" };
  ASSERT_EQ(T1.getLine(0), 1);
  ASSERT_EQ(T1.getLine(1), 1);
  ASSERT_EQ(T1.getLine(2), 1);
  ASSERT_EQ(T1.getLine(3), 1);
  ASSERT_EQ(T1.getLine(4), 2);
  ASSERT_EQ(T1.getLine(5), 2);
  ASSERT_EQ(T1.getLine(6), 2);
  ASSERT_EQ(T1.getLine(7), 2);
  ASSERT_EQ(T1.getLine(8), 3);
  ASSERT_EQ(T1.getLine(9), 3);
  ASSERT_EQ(T1.getLine(10), 3);
  ASSERT_EQ(T1.getLine(11), 3);
}

TEST(TextFileTest, ReportsCorrectStartOffset) {
  TextFile T1 { "foo.txt", "bar\nbaz\nbax\n" };
  ASSERT_EQ(T1.getStartOffset(1), 0);
  ASSERT_EQ(T1.getStartOffset(2), 4);
  ASSERT_EQ(T1.getStartOffset(3), 8);
  ASSERT_EQ(T1.getStartOffset(4), 12);
  ASSERT_EQ(T1.getStartOffset(5), 12);
}

TEST(TextFileTest, ReportsCorrectColumn) {
  TextFile T1 { "foo.txt", "bar\nbaz\nbax\n" };
  ASSERT_EQ(T1.getColumn(0), 1);
  ASSERT_EQ(T1.getColumn(1), 2);
  ASSERT_EQ(T1.getColumn(2), 3);
  ASSERT_EQ(T1.getColumn(3), 4);
  ASSERT_EQ(T1.getColumn(4), 1);
  ASSERT_EQ(T1.getColumn(5), 2);
  ASSERT_EQ(T1.getColumn(6), 3);
  ASSERT_EQ(T1.getColumn(7), 4);
  ASSERT_EQ(T1.getColumn(8), 1);
  ASSERT_EQ(T1.getColumn(9), 2);
  ASSERT_EQ(T1.getColumn(10), 3);
  ASSERT_EQ(T1.getColumn(11), 4);
}
