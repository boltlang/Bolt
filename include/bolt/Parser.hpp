
#pragma once

#include <unordered_map>
#include <optional>

#include "bolt/CST.hpp"
#include "bolt/Stream.hpp"

namespace bolt {

class DiagnosticEngine;
class Scanner;

enum OperatorFlags {
  OperatorFlags_Prefix = 1,
  OperatorFlags_Suffix = 2,
  OperatorFlags_InfixL = 4,
  OperatorFlags_InfixR = 8,
};

struct OperatorInfo {

  int Precedence;
  unsigned Flags;

  inline bool isPrefix() const noexcept {
    return Flags & OperatorFlags_Prefix;
  }

  inline bool isSuffix() const noexcept {
    return Flags & OperatorFlags_Suffix;
  }

  inline bool isInfix() const noexcept {
    return Flags & (OperatorFlags_InfixL | OperatorFlags_InfixR);
  }

  inline bool isRightAssoc() const noexcept {
    return Flags & OperatorFlags_InfixR;
  }

};

class OperatorTable {

  std::unordered_map<std::string, OperatorInfo> Mapping;

public:

  void add(std::string Name, unsigned Flags, int Precedence);

  std::optional<OperatorInfo> getInfix(Token* T);

  bool isInfix(Token* T);
  bool isPrefix(Token* T);
  bool isSuffix(Token* T);

};

class TokenStream {

  std::vector<Token*>& Buffer;
  std::size_t Offset;

public:

  TokenStream(
    std::vector<Token*>& Buffer,
    std::size_t Offset = 0
  ): Buffer(Buffer), Offset(Offset) {}

  std::size_t getAbsoluteOffset() const {
    return Offset;
  }

  Token* peek(std::size_t I = 0) {
    auto RealOffset = Offset + I;
    if (RealOffset >= Buffer.size()) {
      return Buffer.back();
    }
    return Buffer[RealOffset];
  }

  TokenStream fork() {
    return TokenStream { Buffer, Offset };
  }

  void skip(std::size_t Count) {
    Offset = std::min(Buffer.size()-1, Offset + Count);
  }

  Token* get() {
    auto Tok = Buffer[Offset];
    if (Offset+1 < Buffer.size()) {
      ++Offset;
    }
    return Tok;
  }

};

class Parser {

  TextFile& File;
  DiagnosticEngine& DE;

  OperatorTable ExprOperators;

  std::optional<std::pair<std::size_t, std::vector<Annotation*>>> CachedAnnotations;

  void cacheAnnotations(TokenStream& Tokens);

  Token* peekTokenAfterAnnotations(TokenStream& Tokens);
  Token* peekTokenAfterAnnotationsAndModifiers(TokenStream& Tokens);

  std::vector<RecordDeclarationField*> parseRecordDeclarationFields(TokenStream& Tokens);
  std::vector<std::tuple<RecordPatternField*, Comma*>> parseRecordPatternFields(TokenStream& Tokens);

  template<typename T>
  T* expectToken(TokenStream& Tokens);

  Expression* parseInfixOperatorAfterExpression(TokenStream& Tokens, Expression* LHS, int MinPrecedence);

  MatchExpression* parseMatchExpression(TokenStream& Tokens);
  Expression* parseMemberExpression(TokenStream& Tokens);
  RecordExpression* parseRecordExpression(TokenStream& Tokens);
  Expression* parsePrimitiveExpression(TokenStream& Tokens);

  ConstraintExpression* parseConstraintExpression(TokenStream& Tokens);

  TypeExpression* parseAppTypeExpression(TokenStream& Tokens);
  TypeExpression* parsePrimitiveTypeExpression(TokenStream& Tokens);
  TypeExpression* parseQualifiedTypeExpression(TokenStream& Tokens);
  TypeExpression* parseArrowTypeExpression(TokenStream& Tokens);
  VarTypeExpression* parseVarTypeExpression(TokenStream& Tokens);
  ReferenceTypeExpression* parseReferenceTypeExpression(TokenStream& Tokens);

  std::vector<Annotation*> parseAnnotations(TokenStream& Tokens);

  void checkLineFoldEnd(TokenStream& Tokens);
  void skipPastLineFoldEnd(TokenStream& Tokens);
  void skipToRBrace(TokenStream& Tokens);

public:

  Parser(TextFile& File, DiagnosticEngine& DE);

  TypeExpression* parseTypeExpression(TokenStream& Tokens);

  ListPattern* parseListPattern(TokenStream& Tokens);
  Pattern* parsePrimitivePattern(TokenStream& Tokkens, bool IsNarrow);
  Pattern* parseWidePattern(TokenStream& Tokens);
  Pattern* parseNarrowPattern(TokenStream& Tokens);

  Parameter* parseParam(TokenStream& Tokens);

  LiteralExpression* parseLiteralExpression(TokenStream& Tokens);
  FunctionExpression* parseFunctionExpression(TokenStream& Tokens);
  ReferenceExpression* parseReferenceExpression(TokenStream& Tokens);
  Expression* parseUnaryExpression(TokenStream& Tokens);
  Expression* parseExpression(TokenStream& Tokens);
  BlockExpression* parseBlockExpression(TokenStream& Tokens);
  Expression* parseCallExpression(TokenStream& Tokens);
  IfExpression* parseIfExpression(TokenStream& Tokens);

  ReturnExpression* parseReturnExpression(TokenStream& Tokens);

  Expression* parseExpressionStatement(TokenStream& Tokens);

  Node* parseLetBodyElement(TokenStream& Tokens);

  FunctionDeclaration* parseFunctionDeclaration(TokenStream& Tokens);
  VariableDeclaration* parseVariableDeclaration(TokenStream& Tokens);

  Node* parseClassElement(TokenStream& Tokens);

  ClassDeclaration* parseClassDeclaration(TokenStream& Tokens);

  InstanceDeclaration* parseInstanceDeclaration(TokenStream& Tokens);

  RecordDeclaration* parseRecordDeclaration(TokenStream& Tokens);

  VariantDeclaration* parseVariantDeclaration(TokenStream& Tokens);

  Node* parseSourceElement(TokenStream& Tokens);

  SourceFile* parseSourceFile(TokenStream& Tokens);

};

}

