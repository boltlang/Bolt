
#pragma once

#include "bolt/CST.hpp"

namespace bolt {

  class Scanner;

  class Parser {

    Stream<Token*>& Tokens;

    Token* peekFirstTokenAfterModifiers();

  public:

    Parser(Stream<Token*>& S);

    QualifiedName* parseQualifiedName();

    TypeExpression* parseTypeExpression();

    Pattern* parsePattern();

    Param* parseParam();

    ReferenceExpression* parseReferenceExpression();

    Expression* parseExpression();

    ExpressionStatement* parseExpressionStatement();

    LetBodyElement* parseLetBodyElement();

    LetDeclaration* parseLetDeclaration();

    SourceElement* parseSourceElement();

    SourceFile* parseSourceFile();

  };

}

