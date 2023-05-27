
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

  class Parser {

    TextFile& File;
    DiagnosticEngine& DE;

    Stream<Token*>& Tokens;

    OperatorTable ExprOperators;

    Token* peekFirstTokenAfterModifiers();

    Token* expectToken(NodeKind Ty);

    template<typename T>
    T* expectToken() {
      return static_cast<T*>(expectToken(getNodeType<T>()));
    }

    Expression* parseInfixOperatorAfterExpression(Expression* LHS, int MinPrecedence);

    MatchExpression* parseMatchExpression();
    Expression* parseMemberExpression();
    Expression* parsePrimitiveExpression();

    ConstraintExpression* parseConstraintExpression();

    TypeExpression* parsePrimitiveTypeExpression();
    TypeExpression* parseQualifiedTypeExpression();
    TypeExpression* parseArrowTypeExpression();
    VarTypeExpression* parseVarTypeExpression();
    ReferenceTypeExpression* parseReferenceTypeExpression();

    void checkLineFoldEnd();
    void skipToLineFoldEnd();

  public:

    Parser(TextFile& File, Stream<Token*>& S, DiagnosticEngine& DE);

    TypeExpression* parseTypeExpression();

    Pattern* parsePattern();

    Parameter* parseParam();

    ReferenceExpression* parseReferenceExpression();

    Expression* parseUnaryExpression();

    Expression* parseExpression();

    Expression* parseCallExpression();

    IfStatement* parseIfStatement();

    ReturnStatement* parseReturnStatement();

    ExpressionStatement* parseExpressionStatement();

    Node* parseLetBodyElement();

    LetDeclaration* parseLetDeclaration();

    Node* parseClassElement();

    ClassDeclaration* parseClassDeclaration();

    InstanceDeclaration* parseInstanceDeclaration();

    RecordDeclaration* parseRecordDeclaration();

    Node* parseSourceElement();

    SourceFile* parseSourceFile();

  };

}

