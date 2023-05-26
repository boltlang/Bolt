
// TODO check for memory leaks everywhere a nullptr is returned

#include <exception>
#include <vector>

#include "llvm/Support/Casting.h"

#include "bolt/CST.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Diagnostics.hpp" 
#include "bolt/DiagnosticEngine.hpp"

namespace bolt {

  std::optional<OperatorInfo> OperatorTable::getInfix(Token* T) {
    auto Match = Mapping.find(T->getText());
    if (Match == Mapping.end() || !Match->second.isInfix()) {
      return {};
    }
    return Match->second;
  }

  bool OperatorTable::isInfix(Token* T) {
    auto Match = Mapping.find(T->getText());
    return Match != Mapping.end() && Match->second.isInfix();
  }

  bool OperatorTable::isPrefix(Token* T) {
    auto Match = Mapping.find(T->getText());
    return Match != Mapping.end() && Match->second.isPrefix();
  }

  bool OperatorTable::isSuffix(Token* T) {
    auto Match = Mapping.find(T->getText());
    return Match != Mapping.end() && Match->second.isSuffix();
  }

  void OperatorTable::add(std::string Name, unsigned Flags, int Precedence) { 
    Mapping.emplace(Name, OperatorInfo { Precedence, Flags });
  }

  Parser::Parser(TextFile& File, Stream<Token*>& S, DiagnosticEngine& DE):
    File(File), Tokens(S), DE(DE) {
      ExprOperators.add("**", OperatorFlags_InfixR, 10);
      ExprOperators.add("*", OperatorFlags_InfixL, 5);
      ExprOperators.add("/", OperatorFlags_InfixL, 5);
      ExprOperators.add("+", OperatorFlags_InfixL, 4);
      ExprOperators.add("-", OperatorFlags_InfixL, 4);
      ExprOperators.add("<", OperatorFlags_InfixL, 3);
      ExprOperators.add(">", OperatorFlags_InfixL, 3);
      ExprOperators.add("<=", OperatorFlags_InfixL, 3);
      ExprOperators.add(">=", OperatorFlags_InfixL, 3);
      ExprOperators.add("==", OperatorFlags_InfixL, 3);
      ExprOperators.add("!=", OperatorFlags_InfixL, 3);
      ExprOperators.add(":", OperatorFlags_InfixL, 2);
      ExprOperators.add("<|>", OperatorFlags_InfixL, 1);
      ExprOperators.add("$", OperatorFlags_InfixR, 0);
    }

  Token* Parser::peekFirstTokenAfterModifiers() {
    std::size_t I = 0;
    for (;;) {
      auto T0 = Tokens.peek(I++);
      switch (T0->getKind()) {
        case NodeKind::PubKeyword:
        case NodeKind::MutKeyword:
          continue;
        default:
          return T0;
      }
    }
  }

#define BOLT_EXPECT_TOKEN(name) \
  { \
    auto __Token = Tokens.get(); \
    if (!llvm::isa<name>(__Token)) { \
      throw UnexpectedTokenDiagnostic(File, __Token, std::vector<NodeKind> { NodeKind::name }); \
    } \
  }

  Token* Parser::expectToken(NodeKind Kind) {
    auto T = Tokens.peek();
    if (T->getKind() != Kind) {
      DE.add<UnexpectedTokenDiagnostic>(File, T, std::vector<NodeKind> { Kind });
      return nullptr;
    }
    Tokens.get();
    return T;
  }

  Pattern* Parser::parsePattern() {
    auto T0 = Tokens.peek();
    switch (T0->getKind()) {
      case NodeKind::StringLiteral:
      case NodeKind::IntegerLiteral:
        Tokens.get();
        return new LiteralPattern(static_cast<Literal*>(T0));
      case NodeKind::Identifier:
        Tokens.get();
        return new BindPattern(static_cast<Identifier*>(T0));
      default:
        Tokens.get();
        DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::Identifier, NodeKind::StringLiteral, NodeKind::IntegerLiteral });
        return nullptr;
    }
  }

  TypeExpression* Parser::parseTypeExpression() {
    return parseQualifiedTypeExpression();
  }

  TypeExpression* Parser::parseQualifiedTypeExpression() {
    bool HasConstraints = false;
    auto T0 = Tokens.peek();
    if (llvm::isa<LParen>(T0)) {
      std::size_t I = 1;
      for (;;) {
        auto T0 = Tokens.peek(I++);
        switch (T0->getKind()) {
          case NodeKind::RArrowAlt:
            HasConstraints = true;
            goto after_lookahead;
          case NodeKind::Equals:
          case NodeKind::BlockStart:
          case NodeKind::LineFoldEnd:
          case NodeKind::EndOfFile:
            goto after_lookahead;
          default:
            break;
        }
      }
    }
after_lookahead:
    if (!HasConstraints) {
      return parseArrowTypeExpression();
    }
    Tokens.get();
    LParen* LParen = static_cast<class LParen*>(T0);
    std::vector<std::tuple<ConstraintExpression*, Comma*>> Constraints;
    RParen* RParen;
    RArrowAlt* RArrowAlt;
    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::RParen) {
      Tokens.get();
      RParen = static_cast<class RParen*>(T1);
      goto after_constraints;
    }
    for (;;) {
      auto C = parseConstraintExpression();
      Comma* Comma = nullptr;
      auto T2 = Tokens.get();
      switch (T2->getKind()) {
        case NodeKind::Comma:
        {
          auto Comma = static_cast<class Comma*>(T2);
          if (C) {
            Constraints.push_back(std::make_tuple(C, Comma));
          } else {
            Comma->unref();
          }
          continue;
        }
        case NodeKind::RParen:
          RParen = static_cast<class RParen*>(T2);
          if (C) {
            Constraints.push_back(std::make_tuple(C, nullptr));
          }
          goto after_constraints;
        default:
          DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::Comma, NodeKind::RArrowAlt });
          return nullptr;
      }
    }
after_constraints:
    RArrowAlt = expectToken<class RArrowAlt>();
    if (!RArrowAlt) {
      LParen->unref();
      for (auto [CE, Comma]: Constraints) {
        CE->unref();
      }
      RParen->unref();
      return nullptr;
    }
    auto TE = parseArrowTypeExpression();
    if (!TE) {
      LParen->unref();
      for (auto [CE, Comma]: Constraints) {
        CE->unref();
      }
      RParen->unref();
      RArrowAlt->unref();
      return nullptr;
    }
    return new QualifiedTypeExpression(Constraints, RArrowAlt, TE);
  }

  TypeExpression* Parser::parsePrimitiveTypeExpression() {
    auto T0 = Tokens.peek();
    switch (T0->getKind()) {
      case NodeKind::Identifier:
          return parseVarTypeExpression();
      case NodeKind::LParen:
      {
        Tokens.get();
        auto LParen = static_cast<class LParen*>(T0);
        std::vector<std::tuple<TypeExpression*, Comma*>> Elements;
        RParen* RParen;
        for (;;) {
          auto T1 = Tokens.peek();
          if (llvm::isa<class RParen>(T1)) {
            Tokens.get();
            RParen = static_cast<class RParen*>(T1);
            break;
          }
          auto TE = parseTypeExpression();
          if (!TE) {
            LParen->unref();
            for (auto [TE, Comma]: Elements) {
              TE->unref();
              Comma->unref();
            }
            return nullptr;
          }
          auto T2 = Tokens.get();
          switch (T2->getKind()) {
            case NodeKind::RParen:
              RParen = static_cast<class RParen*>(T1);
              Elements.push_back({ TE, nullptr });
              goto after_tuple_element;
            case NodeKind::Comma:
              Elements.push_back({ TE, static_cast<Comma*>(T2) });
              continue;
            default:
              DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::Comma, NodeKind::RParen });
              LParen->unref();
              for (auto [TE, Comma]: Elements) {
                TE->unref();
                Comma->unref();
              }
              return nullptr;
          }
        }
after_tuple_element:
        if (Elements.size() == 1) {
          return new NestedTypeExpression { LParen, std::get<0>(Elements.front()), RParen };
        }
        return new TupleTypeExpression { LParen, Elements, RParen };
      }
      case NodeKind::IdentifierAlt:
        return parseReferenceTypeExpression();
      default:
        Tokens.get();
        DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::Identifier, NodeKind::IdentifierAlt, NodeKind::LParen });
        return nullptr;
    }
  }

  ReferenceTypeExpression* Parser::parseReferenceTypeExpression() {
    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
    auto Name = expectToken<IdentifierAlt>();
    if (!Name) {
      return nullptr;
    }
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() != NodeKind::Dot) {
        break;
      }
      Tokens.get();
      ModulePath.push_back(std::make_tuple(static_cast<IdentifierAlt*>(Name), static_cast<Dot*>(T1)));
      Name = expectToken<IdentifierAlt>();
      if (!Name) {
        for (auto [Name, Dot]: ModulePath) {
          Name->unref();
          Dot->unref();
        }
        return nullptr;
      }
    }
    return new ReferenceTypeExpression(ModulePath, static_cast<IdentifierAlt*>(Name));
  }

  TypeExpression* Parser::parseArrowTypeExpression() {
    auto RetType = parsePrimitiveTypeExpression();
    std::vector<TypeExpression*> ParamTypes;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() != NodeKind::RArrow) {
        break;
      }
      Tokens.get();
      ParamTypes.push_back(RetType);
      RetType = parsePrimitiveTypeExpression();
      if (!RetType) {
        for (auto ParamType: ParamTypes) {
          ParamType->unref();
        }
        return nullptr;
      }
    }
    if (!ParamTypes.empty()) {
      return new ArrowTypeExpression(ParamTypes, RetType);
    }
    return RetType;
  }

  MatchExpression* Parser::parseMatchExpression() {
    auto T0 = expectToken<MatchKeyword>();
    if (!T0) {
      return nullptr;
    }
    Tokens.get();
    auto T1 = Tokens.peek();
    Expression* Value;
    BlockStart* BlockStart;
    if (llvm::isa<class BlockStart>(T1)) {
      Value = nullptr;
      BlockStart = static_cast<class BlockStart*>(T1);
      Tokens.get();
    } else {
      Value = parseExpression();
      if (!Value) {
        T0->unref();
        return nullptr;
      }
      BlockStart = expectToken<class BlockStart>();
      if (!BlockStart) {
        T0->unref();
        Value->unref();
        return nullptr;
      }
    }
    std::vector<MatchCase*> Cases;
    for (;;) {
      auto T2 = Tokens.peek();
      if (llvm::isa<BlockEnd>(T2)) {
        Tokens.get();
        break;
      }
      auto Pattern = parsePattern();
      if (!Pattern) {
        T0->unref();
        Value->unref();
        BlockStart->unref();
        for (auto Case: Cases) {
          Case->unref();
        }
        skipToLineFoldEnd();
        continue;
      }
      auto RArrowAlt = expectToken<class RArrowAlt>();
      if (!RArrowAlt) {
        T0->unref();
        Value->unref();
        BlockStart->unref();
        for (auto Case: Cases) {
          Case->unref();
        }
        Pattern->unref();
        skipToLineFoldEnd();
        continue;
      }
      auto Expression = parseExpression();
      if (!Expression) {
        T0->unref();
        Value->unref();
        BlockStart->unref();
        for (auto Case: Cases) {
          Case->unref();
        }
        Pattern->unref();
        RArrowAlt->unref();
        skipToLineFoldEnd();
        continue;
      }
      checkLineFoldEnd();
      Cases.push_back(new MatchCase { Pattern, RArrowAlt, Expression });
    }
    return new MatchExpression(static_cast<MatchKeyword*>(T0), Value, BlockStart, Cases);
  }

  Expression* Parser::parsePrimitiveExpression() {
    auto T0 = Tokens.peek();
    switch (T0->getKind()) {
      case NodeKind::Identifier:
      case NodeKind::IdentifierAlt:
      {
        std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
        for (;;) {
          auto T1 = Tokens.peek(0);
          auto T2 = Tokens.peek(1);
          if (!llvm::isa<IdentifierAlt>(T1) || !llvm::isa<Dot>(T2)) {
            break;
          }
          Tokens.get();
          Tokens.get();
          ModulePath.push_back(std::make_tuple(static_cast<IdentifierAlt*>(T1), static_cast<class Dot*>(T2)));
        }
        auto T3 = Tokens.get();
        if (!llvm::isa<Symbol>(T3)) {
          for (auto [Name, Dot]: ModulePath) {
            Name->unref();
            Dot->unref();
          }
          DE.add<UnexpectedTokenDiagnostic>(File, T3, std::vector { NodeKind::Identifier, NodeKind::IdentifierAlt });
          return nullptr;
        }
        return new ReferenceExpression(ModulePath, static_cast<Symbol*>(T3));
      }
      case NodeKind::LParen:
      {
        Tokens.get();
        std::vector<std::tuple<Expression*, Comma*>> Elements;
        auto LParen = static_cast<class LParen*>(T0);
        RParen* RParen;
        auto T1 = Tokens.peek();
        if (llvm::isa<class RParen>(T1)) {
          Tokens.get();
          RParen = static_cast<class RParen*>(T1);
          goto after_tuple_elements;
        }
        for (;;) {
          auto T1 = Tokens.peek();
          auto E = parseExpression();
          if (!E) {
            LParen->unref();
            for (auto [E, Comma]: Elements) {
              E->unref();
              Comma->unref();
            }
            return nullptr;
          }
          auto T2 = Tokens.get();
          switch (T2->getKind()) {
            case NodeKind::RParen:
              RParen = static_cast<class RParen*>(T2);
              Elements.push_back({ E, nullptr });
              goto after_tuple_elements;
            case NodeKind::Comma:
              Elements.push_back({ E, static_cast<class Comma*>(T2) });
              break;
            default:
              DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::RParen, NodeKind::Comma });
              LParen->unref();
              for (auto [E, Comma]: Elements) {
                E->unref();
                Comma->unref();
              }
              return nullptr;
            case NodeKind::LineFoldEnd:
            case NodeKind::BlockStart:
            case NodeKind::EndOfFile:
              // Can recover from this one
              RParen = nullptr;
              DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::RParen, NodeKind::Comma });
              goto after_tuple_elements;
          }
        }
after_tuple_elements:
        if (Elements.size() == 1 && !std::get<1>(Elements.front())) {
          return new NestedExpression(LParen, std::get<0>(Elements.front()), RParen);
        }
        return new TupleExpression { LParen, Elements, RParen };
      }
      case NodeKind::MatchKeyword:
        return parseMatchExpression();
      case NodeKind::IntegerLiteral:
      case NodeKind::StringLiteral:
        Tokens.get();
        return new ConstantExpression(static_cast<Literal*>(T0));
      default:
        Tokens.get();
        DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::MatchKeyword, NodeKind::Identifier, NodeKind::IdentifierAlt, NodeKind::LParen, NodeKind::IntegerLiteral, NodeKind::StringLiteral });
        return nullptr;
    }
  }

  Expression* Parser::parseMemberExpression() {
    auto E = parsePrimitiveExpression();
    if (!E) {
      return nullptr;
    }
    for (;;) {
      auto T1 = Tokens.peek(0);
      auto T2 = Tokens.peek(1);
      if (!llvm::isa<Dot>(T1)) {
        break;
      }
      switch (T2->getKind()) {
        case NodeKind::IntegerLiteral:
        case NodeKind::Identifier:
          Tokens.get();
          Tokens.get();
          E = new MemberExpression { E, static_cast<Dot*>(T1), T2 };
          break;
        default:
          goto finish;
      }
    }
finish:
    return E;
  }

  Expression* Parser::parseCallExpression() {
    auto Operator = parseMemberExpression();
    if (!Operator) {
      return nullptr;
    }
    std::vector<Expression*> Args;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() == NodeKind::LineFoldEnd || T1->getKind() == NodeKind::RParen || T1->getKind() == NodeKind::BlockStart || T1->getKind() == NodeKind::Comma || ExprOperators.isInfix(T1)) {
        break;
      }
      auto Arg = parsePrimitiveExpression();
      if (!Arg) {
        Operator->unref();
        for (auto Arg: Args) {
          Arg->unref();
        }
        return nullptr;
      }
      Args.push_back(Arg);
    }
    if (Args.empty()) {
      return Operator;
    }
    return new CallExpression(Operator, Args);
  }

  Expression* Parser::parseUnaryExpression() {
    std::vector<Token*> Prefix;
    for (;;) {
      auto T0 = Tokens.peek();
      if (!ExprOperators.isPrefix(T0)) {
        break;
      }
      Tokens.get();
      Prefix.push_back(T0);
    }
    auto E = parseCallExpression();
    if (!E) {
      for (auto Tok: Prefix) {
        Tok->unref();
      }
      return nullptr;
    }
    for (auto Iter = Prefix.rbegin(); Iter != Prefix.rend(); Iter++) {
      E = new PrefixExpression(*Iter, E);
    }
    return E;
  }

  Expression* Parser::parseInfixOperatorAfterExpression(Expression* Left, int MinPrecedence) {
    for (;;) {
      auto T0 = Tokens.peek();
      auto Info0 = ExprOperators.getInfix(T0);
      if (!Info0 || Info0->Precedence < MinPrecedence) {
        break;
      }
      Tokens.get();
      auto Right = parseUnaryExpression();
      if (!Right) {
        Left->unref();
        T0->unref();
        return nullptr;
      }
      for (;;) {
        auto T1 = Tokens.peek();
        auto Info1 = ExprOperators.getInfix(T1);
        if (!Info1 || Info1->Precedence < Info0->Precedence && (Info1->Precedence > Info0->Precedence || Info1->isRightAssoc())) {
          break;
        }
        auto NewRight = parseInfixOperatorAfterExpression(Right, Info1->Precedence);
        if (!NewRight) {
          Left->unref();
          T0->unref();
          Right->unref();
          return nullptr;
        }
        Right = NewRight;
      }
      Left = new InfixExpression(Left, T0, Right);
    }
    return Left;
  }

  Expression* Parser::parseExpression() {
    auto Left = parseUnaryExpression();
    if (!Left) {
      return nullptr;
    }
    return parseInfixOperatorAfterExpression(Left, 0);
  }

  ExpressionStatement* Parser::parseExpressionStatement() {
    auto E = parseExpression();
    if (!E) {
      skipToLineFoldEnd();
      return nullptr;
    }
    checkLineFoldEnd();
    return new ExpressionStatement(E);
  }

  ReturnStatement* Parser::parseReturnStatement() {
    auto ReturnKeyword = expectToken<class ReturnKeyword>();
    if (!ReturnKeyword) {
      return nullptr;
    }
    Expression* Expression;
    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::LineFoldEnd) {
      Tokens.get()->unref();
      Expression = nullptr;
    } else {
      Expression = parseExpression();
      if (!Expression) {
        ReturnKeyword->unref();
        skipToLineFoldEnd();
        return nullptr;
      }
      checkLineFoldEnd();
    }
    return new ReturnStatement(ReturnKeyword, Expression);
  }

  IfStatement* Parser::parseIfStatement() {
    std::vector<IfStatementPart*> Parts;
    auto IfKeyword = expectToken<class IfKeyword>();
    auto Test = parseExpression();
    if (!Test) {
      IfKeyword->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    auto T1 = expectToken<BlockStart>();
    if (!T1) {
      IfKeyword->unref();
      Test->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<Node*> Then;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->getKind() == NodeKind::BlockEnd) {
        Tokens.get();
        break;
      }
      auto Element = parseLetBodyElement();
      if (Element) {
        Then.push_back(Element);
      }
    }
    Tokens.get(); // Always a LineFoldEnd
    Parts.push_back(new IfStatementPart(IfKeyword, Test, T1, Then));
    auto T3 = Tokens.peek();
    if (T3->getKind() == NodeKind::ElseKeyword) {
      Tokens.get();
      auto T4 = expectToken<BlockStart>();
      if (!T4) {
        for (auto Part: Parts) {
          Part->unref();
        }
        return nullptr;
      }
      std::vector<Node*> Else;
      for (;;) {
        auto T5 = Tokens.peek();
        if (T5->getKind() == NodeKind::BlockEnd) {
          Tokens.get();
          break;
        }
        auto Element = parseLetBodyElement();
        if (Element) {
          Else.push_back(Element);
        }
      }
      Tokens.get(); // Always a LineFoldEnd
      Parts.push_back(new IfStatementPart(T3, nullptr, T4, Else));
    }
    return new IfStatement(Parts);
  }

  LetDeclaration* Parser::parseLetDeclaration() {

    PubKeyword* Pub = nullptr;
    LetKeyword* Let;
    MutKeyword* Mut = nullptr;
    TypeAssert* TA = nullptr;
    LetBody* Body = nullptr;

    auto T0 = Tokens.get();
    if (T0->getKind() == NodeKind::PubKeyword) {
      Pub = static_cast<PubKeyword*>(T0);
      T0 = Tokens.get();
    }
    if (T0->getKind() != NodeKind::LetKeyword) {
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::LetKeyword });
      if (Pub) {
        Pub->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    Let = static_cast<LetKeyword*>(T0);
    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::MutKeyword) {
      Mut = static_cast<MutKeyword*>(T1);
      Tokens.get();
    }

    auto Patt = parsePattern();
    if (!Patt) {
      if (Pub) {
        Pub->unref();
      }
      Let->unref();
      if (Mut) {
        Mut->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }

    std::vector<Parameter*> Params;
    Token* T2;
    for (;;) {
      T2 = Tokens.peek();
      switch (T2->getKind()) {
        case NodeKind::LineFoldEnd:
        case NodeKind::BlockStart:
        case NodeKind::Equals:
        case NodeKind::Colon:
          goto after_params;
        default:
          auto P = parsePattern();
          if (P == nullptr) {
            P = new BindPattern(new Identifier("_"));
          }
          Params.push_back(new Parameter(P, nullptr));
      }
    }

after_params:

    if (T2->getKind() == NodeKind::Colon) {
      Tokens.get();
      auto TE = parseTypeExpression();
      if (TE) {
        TA = new TypeAssert(static_cast<Colon*>(T2), TE);
      } else {
        skipToLineFoldEnd();
        goto finish;
      }
      T2 = Tokens.peek();
    }

    switch (T2->getKind()) {
      case NodeKind::BlockStart:
      {
        Tokens.get();
        std::vector<Node*> Elements;
        for (;;) {
          auto T3 = Tokens.peek();
          if (T3->getKind() == NodeKind::BlockEnd) {
            break;
          }
          auto Element = parseLetBodyElement();
          if (Element) {
            Elements.push_back(Element);
          }
        }
        Tokens.get();
        Body = new LetBlockBody(static_cast<BlockStart*>(T2), Elements);
        break;
      }
      case NodeKind::Equals:
      {
        Tokens.get();
        auto E = parseExpression();
        if (E == nullptr) {
          skipToLineFoldEnd();
          goto finish;
        }
        if (E) {
          Body = new LetExprBody(static_cast<Equals*>(T2), E);
        }
        break;
      }
      case NodeKind::LineFoldEnd:
        break;
      default:
        std::vector<NodeKind> Expected { NodeKind::BlockStart, NodeKind::LineFoldEnd, NodeKind::Equals };
        if (TA == nullptr) {
          // First tokens of TypeAssert
          Expected.push_back(NodeKind::Colon);
          // First tokens of Pattern
          Expected.push_back(NodeKind::Identifier);
        }
        DE.add<UnexpectedTokenDiagnostic>(File, T2, Expected);
    }

after_body:

    checkLineFoldEnd();

finish:
    return new LetDeclaration(
      Pub,
      Let,
      Mut,
      Patt,
      Params,
      TA,
      Body
    );
  }

  Node* Parser::parseLetBodyElement() {
    auto T0 = peekFirstTokenAfterModifiers();
    switch (T0->getKind()) {
      case NodeKind::LetKeyword:
        return parseLetDeclaration();
      case NodeKind::ReturnKeyword:
        return parseReturnStatement();
      case NodeKind::IfKeyword:
        return parseIfStatement();
      default:
        return parseExpressionStatement();
    }
  }

  ConstraintExpression* Parser::parseConstraintExpression() {
    bool HasTilde = false;
    for (std::size_t I = 0; ; I++) {
      auto Tok = Tokens.peek(I);
      switch (Tok->getKind()) {
        case NodeKind::Tilde:
          HasTilde = true;
          goto after_lookahead;
        case NodeKind::RParen:
        case NodeKind::Comma:
        case NodeKind::RArrowAlt:
        case NodeKind::EndOfFile:
          goto after_lookahead;
        default:
          continue;
      }
    }
after_lookahead:
    if (HasTilde) {
      auto Left = parseArrowTypeExpression();
      if (!Left) {
        return nullptr;
      }
      auto Tilde = expectToken<class Tilde>();
      if (!Tilde) {
        Left->unref();
        return nullptr;
      }
      auto Right = parseArrowTypeExpression();
      if (!Right) {
        Left->unref();
        Tilde->unref();
        return nullptr;
      }
      return new EqualityConstraintExpression { Left, Tilde, Right };
    }
    auto Name = expectToken<IdentifierAlt>();
    if (!Name) {
      return nullptr;
    }
    std::vector<VarTypeExpression*> TEs;
    for (;;) {
      auto T1 = Tokens.peek();
      switch (T1->getKind()) {
        case NodeKind::RParen:
        case NodeKind::RArrowAlt:
        case NodeKind::Comma:
          goto after_vars;
        case NodeKind::Identifier:
          Tokens.get();
          TEs.push_back(new VarTypeExpression { static_cast<Identifier*>(T1) });
          break;
        default:
          DE.add<UnexpectedTokenDiagnostic>(File, T1, std::vector { NodeKind::RParen, NodeKind::RArrowAlt, NodeKind::Comma, NodeKind::Identifier });
          Name->unref();
          return nullptr;
      }
    }
after_vars:
    return new TypeclassConstraintExpression { Name, TEs };
  }

  VarTypeExpression* Parser::parseVarTypeExpression() {
    auto Name = expectToken<Identifier>();
    if (!Name) {
      return nullptr;
    }
    for (auto Ch: Name->Text) {
      if (!std::islower(Ch)) {
        // TODO
        // DE.add<TypeVarMustContainLowercaseLettersDiagnostic>(Name);
        Name->unref();
        return nullptr;
      }
    }
    return new VarTypeExpression { Name };
  }

  InstanceDeclaration* Parser::parseInstanceDeclaration() {
    auto InstanceKeyword = expectToken<class InstanceKeyword>();
    if (!InstanceKeyword) {
      skipToLineFoldEnd();
      return nullptr;
    }
    auto Name = expectToken<IdentifierAlt>();
    if (!Name) {
      InstanceKeyword->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<TypeExpression*> TypeExps;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->is<BlockStart>()) {
        break;
      }
      auto TE = parseTypeExpression();
      if (!TE) {
        InstanceKeyword->unref();
        Name->unref();
        for (auto TE: TypeExps) {
          TE->unref();
        }
        skipToLineFoldEnd();
        return nullptr;
      }
      TypeExps.push_back(TE);
    }
    auto BlockStart = expectToken<class BlockStart>();
    if (!BlockStart) { 
      InstanceKeyword->unref();
      Name->unref();
      for (auto TE: TypeExps) {
        TE->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<Node*> Elements;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->is<BlockEnd>()) {
        Tokens.get();
        break;
      }
      auto Element = parseClassElement();
      if (Element) {
        Elements.push_back(Element);
      }
    }
    checkLineFoldEnd();
    return new InstanceDeclaration(
      InstanceKeyword,
      Name,
      TypeExps,
      BlockStart,
      Elements
    );
  }

  ClassDeclaration* Parser::parseClassDeclaration() {
    PubKeyword* PubKeyword = nullptr;
    auto T0 = Tokens.peek();
    if (T0->getKind() == NodeKind::PubKeyword) {
      Tokens.get();
      PubKeyword = static_cast<class PubKeyword*>(T0);
    }
    auto ClassKeyword = expectToken<class ClassKeyword>();
    if (!ClassKeyword) {
      if (PubKeyword) {
        PubKeyword->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    auto Name = expectToken<IdentifierAlt>();
    if (!Name) {
      if (PubKeyword) {
        PubKeyword->unref();
      }
      ClassKeyword->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<VarTypeExpression*> TypeVars;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->getKind() == NodeKind::BlockStart) {
        break;
      }
      auto TE = parseVarTypeExpression();
      if (!TE) {
        if (PubKeyword) {
          PubKeyword->unref();
        }
        ClassKeyword->unref();
        for (auto TV: TypeVars) {
          TV->unref();
        }
        skipToLineFoldEnd();
        return nullptr;
      }
      TypeVars.push_back(TE);
    }
    auto BlockStart = expectToken<class BlockStart>();
    if (!BlockStart) {
      if (PubKeyword) {
        PubKeyword->unref();
      }
      ClassKeyword->unref();
      for (auto TV: TypeVars) {
        TV->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<Node*> Elements;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->is<BlockEnd>()) {
        Tokens.get();
        break;
      }
      auto Element = parseClassElement();
      if (Element) {
        Elements.push_back(Element);
      }
    }
    Tokens.get(); // Always a LineFoldEnd
    return new ClassDeclaration(
      PubKeyword,
      ClassKeyword,
      Name,
      TypeVars,
      BlockStart,
      Elements
    );
  }

  Node* Parser::parseClassElement() {
    auto T0 = Tokens.peek();
    switch (T0->getKind()) {
      case NodeKind::LetKeyword:
        return parseLetDeclaration();
      case NodeKind::TypeKeyword:
        // TODO
      default:
        throw UnexpectedTokenDiagnostic(File, T0, std::vector<NodeKind> { NodeKind::LetKeyword, NodeKind::TypeKeyword });
    }
  }

  Node* Parser::parseSourceElement() {
    auto T0 = peekFirstTokenAfterModifiers();
    switch (T0->getKind()) {
      case NodeKind::LetKeyword:
        return parseLetDeclaration();
      case NodeKind::IfKeyword:
        return parseIfStatement();
      case NodeKind::ClassKeyword:
        return parseClassDeclaration();
      case NodeKind::InstanceKeyword:
        return parseInstanceDeclaration();
      default:
        return parseExpressionStatement();
    }
  }

  SourceFile* Parser::parseSourceFile() {
    std::vector<Node*> Elements;
    for (;;) {
      auto T0 = Tokens.peek();
      if (T0->is<EndOfFile>()) {
        break;
      }
      auto Element = parseSourceElement();
      if (Element) {
        Elements.push_back(Element);
      }
    }
    return new SourceFile(File, Elements);
  }

  void Parser::skipToLineFoldEnd() {
   unsigned Level = 0;
    for (;;) {
      auto T0 = Tokens.get();
      switch (T0->getKind()) {
        case NodeKind::EndOfFile:
          return;
        case  NodeKind::LineFoldEnd:
          T0->unref();
          if (Level == 0) {
            return;
          }
          break;
        case NodeKind::BlockStart:
          T0->unref();
          Level++;
          break;
        case NodeKind::BlockEnd:
          T0->unref();
          Level--;
          break;
        default:
          T0->unref();
          break;
      }
    }
  }

  void Parser::checkLineFoldEnd() {
    auto T0 = Tokens.get();
    if (T0->getKind() != NodeKind::LineFoldEnd) {
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::LineFoldEnd });
      skipToLineFoldEnd();
    }
  }

}

