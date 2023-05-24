
#include <exception>
#include <vector>

#include "llvm/Support/Casting.h"

#include "bolt/CST.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Diagnostics.hpp" 

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
    auto T = Tokens.get();
    if (T->getKind() != Kind) {
      throw UnexpectedTokenDiagnostic(File, T, std::vector<NodeKind> { Kind }); \
    }
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
        throw UnexpectedTokenDiagnostic(File, T0, std::vector { NodeKind::Identifier, NodeKind::StringLiteral, NodeKind::IntegerLiteral });
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
            goto after_scan;
          case NodeKind::Equals:
          case NodeKind::BlockStart:
          case NodeKind::LineFoldEnd:
          case NodeKind::EndOfFile:
            goto after_scan;
          default:
            break;
        }
      }
    }
after_scan:
    if (!HasConstraints) {
      return parseArrowTypeExpression();
    }
    Tokens.get();
    LParen* LParen = static_cast<class LParen*>(T0);
    std::vector<std::tuple<ConstraintExpression*, Comma*>> Constraints;
    RParen* RParen;
    RArrowAlt* RArrowAlt;
    for (;;) {
      ConstraintExpression* C;
      auto T0 = Tokens.peek();
      switch (T0->getKind()) {
        case NodeKind::RParen:
          Tokens.get();
          RParen = static_cast<class RParen*>(T0);
          RArrowAlt = expectToken<class RArrowAlt>();
          goto after_constraints;
        default:
          C = parseConstraintExpression();
          break;
      }
      Comma* Comma = nullptr;
      auto T1 = Tokens.get();
      switch (T1->getKind()) {
        case NodeKind::Comma:
          Constraints.push_back(std::make_tuple(C, static_cast<class Comma*>(T1)));
          continue;
        case NodeKind::RParen:
          RArrowAlt = static_cast<class RArrowAlt*>(T1);
          Constraints.push_back(std::make_tuple(C, nullptr));
          RArrowAlt = expectToken<class RArrowAlt>();
          goto after_constraints;
        default:
          throw UnexpectedTokenDiagnostic(File, T1, std::vector { NodeKind::Comma, NodeKind::RArrowAlt });
      }
    }
after_constraints:
    auto TE = parseArrowTypeExpression();
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
              throw UnexpectedTokenDiagnostic(File, T2, { NodeKind::Comma, NodeKind::RParen });
          }
        }
after_tuple_element:
        if (Elements.size() == 1) {
          return new NestedTypeExpression { LParen, std::get<0>(Elements.front()), RParen };
        }
        return new TupleTypeExpression { LParen, Elements, RParen };
      }
      case NodeKind::IdentifierAlt:
      {
        std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
        auto Name = expectToken<IdentifierAlt>();
        for (;;) {
          auto T1 = Tokens.peek();
          if (T1->getKind() != NodeKind::Dot) {
            break;
          }
          Tokens.get();
          ModulePath.push_back(std::make_tuple(static_cast<IdentifierAlt*>(Name), static_cast<Dot*>(T1)));
          Name = expectToken<IdentifierAlt>();
        }
        return new ReferenceTypeExpression(ModulePath, static_cast<IdentifierAlt*>(Name));
      }
      default:
        throw UnexpectedTokenDiagnostic(File, T0, std::vector { NodeKind::Identifier });
    }
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
    }
    if (!ParamTypes.empty()) {
      return new ArrowTypeExpression(ParamTypes, RetType);
    }
    return RetType;
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
          auto Dot = expectToken<class Dot>();
          ModulePath.push_back(std::make_tuple(static_cast<IdentifierAlt*>(T1), Dot));
        }
        auto T3 = Tokens.get();
        if (!llvm::isa<Symbol>(T3)) {
          throw UnexpectedTokenDiagnostic(File, T3, { NodeKind::Identifier, NodeKind::IdentifierAlt });
        }
        return new ReferenceExpression(ModulePath, static_cast<Symbol*>(T3));
      }
      case NodeKind::LParen:
      {
        Tokens.get();
        std::vector<std::tuple<Expression*, Comma*>> Elements;
        auto LParen = static_cast<class LParen*>(T0);
        RParen* RParen;
        for (;;) {
          auto T1 = Tokens.peek();
          if (llvm::isa<class RParen>(T1)) {
            Tokens.get();
            RParen = static_cast<class RParen*>(T1);
            break;
          }
          auto E = parseExpression();
          auto T2 = Tokens.get();
          switch (T2->getKind()) {
            case NodeKind::RParen:
              RParen = static_cast<class RParen*>(T2);
              Elements.push_back({ E, nullptr });
              goto finish;
            case NodeKind::Comma:
              Elements.push_back({ E, static_cast<class Comma*>(T2) });
              break;
            default:
              throw UnexpectedTokenDiagnostic(File, T2, { NodeKind::RParen, NodeKind::Comma });
          }
        }
finish:
        if (Elements.size() == 1 && !std::get<1>(Elements.front())) {
          return new NestedExpression(LParen, std::get<0>(Elements.front()), RParen);
        }
        return new TupleExpression { LParen, Elements, RParen };
      }
      case NodeKind::MatchKeyword:
      {
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
          BlockStart = expectToken<class BlockStart>();
        }
        std::vector<MatchCase*> Cases;
        for (;;) {
          auto T2 = Tokens.peek();
          if (llvm::isa<BlockEnd>(T2)) {
            Tokens.get();
            break;
          }
          auto Pattern = parsePattern();
          auto RArrowAlt = expectToken<class RArrowAlt>();
          auto Expression = parseExpression();
          expectToken<LineFoldEnd>();
          Cases.push_back(new MatchCase { Pattern, RArrowAlt, Expression });
        }
        return new MatchExpression(static_cast<MatchKeyword*>(T0), Value, BlockStart, Cases);
      }
      case NodeKind::IntegerLiteral:
      case NodeKind::StringLiteral:
        Tokens.get();
        return new ConstantExpression(static_cast<Literal*>(T0));
      default:
        throw UnexpectedTokenDiagnostic(File, T0, { NodeKind::MatchKeyword, NodeKind::Identifier, NodeKind::IdentifierAlt, NodeKind::IntegerLiteral, NodeKind::StringLiteral });
    }
  }

  Expression* Parser::parseMemberExpression() {
    auto E = parsePrimitiveExpression();
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
    std::vector<Expression*> Args;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() == NodeKind::LineFoldEnd || T1->getKind() == NodeKind::RParen || T1->getKind() == NodeKind::BlockStart || T1->getKind() == NodeKind::Comma || ExprOperators.isInfix(T1)) {
        break;
      }
      Args.push_back(parsePrimitiveExpression());
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
    for (auto Iter = Prefix.rbegin(); Iter != Prefix.rend(); Iter++) {
      E = new PrefixExpression(*Iter, E);
    }
    return E;
  }

  Expression* Parser::parseInfixOperatorAfterExpression(Expression* LHS, int MinPrecedence) {
    for (;;) {
      auto T0 = Tokens.peek();
      auto Info0 = ExprOperators.getInfix(T0);
      if (!Info0 || Info0->Precedence < MinPrecedence) {
        break;
      }
      Tokens.get();
      auto RHS = parseUnaryExpression();
      for (;;) {
        auto T1 = Tokens.peek();
        auto Info1 = ExprOperators.getInfix(T1);
        if (!Info1 || Info1->Precedence < Info0->Precedence && (Info1->Precedence > Info0->Precedence || Info1->isRightAssoc())) {
          break;
        }
        RHS = parseInfixOperatorAfterExpression(RHS, Info1->Precedence);
      }
      LHS = new InfixExpression(LHS, T0, RHS);
    }
    return LHS;
  }

  Expression* Parser::parseExpression() {
    return parseInfixOperatorAfterExpression(parseUnaryExpression(), 0);
  }

  ExpressionStatement* Parser::parseExpressionStatement() {
    auto E = parseExpression();
    BOLT_EXPECT_TOKEN(LineFoldEnd);
    return new ExpressionStatement(E);
  }

  ReturnStatement* Parser::parseReturnStatement() {
    auto T0 = static_cast<ReturnKeyword*>(expectToken(NodeKind::ReturnKeyword));
    Expression* Expression = nullptr;
    auto T1 = Tokens.peek();
    if (T1->getKind() != NodeKind::LineFoldEnd) {
      Expression = parseExpression();
    }
    BOLT_EXPECT_TOKEN(LineFoldEnd);
    return new ReturnStatement(static_cast<ReturnKeyword*>(T0), Expression);
  }

  IfStatement* Parser::parseIfStatement() {
    std::vector<IfStatementPart*> Parts;
    auto T0 = expectToken(NodeKind::IfKeyword);
    auto Test = parseExpression();
    auto T1 = static_cast<BlockStart*>(expectToken(NodeKind::BlockStart));
    std::vector<Node*> Then;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->getKind() == NodeKind::BlockEnd) {
        Tokens.get();
        break;
      }
      Then.push_back(parseLetBodyElement());
    }
    Parts.push_back(new IfStatementPart(T0, Test, T1, Then));
    BOLT_EXPECT_TOKEN(LineFoldEnd)
    auto T3 = Tokens.peek();
    if (T3->getKind() == NodeKind::ElseKeyword) {
      Tokens.get();
      auto T4 = static_cast<BlockStart*>(expectToken(NodeKind::BlockStart));
      std::vector<Node*> Else;
      for (;;) {
        auto T5 = Tokens.peek();
        if (T5->getKind() == NodeKind::BlockEnd) {
          Tokens.get();
          break;
        }
        Else.push_back(parseLetBodyElement());
      }
      Parts.push_back(new IfStatementPart(T3, nullptr, T4, Else));
      BOLT_EXPECT_TOKEN(LineFoldEnd)
    }
    return new IfStatement(Parts);
  }

  LetDeclaration* Parser::parseLetDeclaration() {

    PubKeyword* Pub = nullptr;
    LetKeyword* Let;
    MutKeyword* Mut = nullptr;
    auto T0 = Tokens.get();
    if (T0->getKind() == NodeKind::PubKeyword) {
      Pub = static_cast<PubKeyword*>(T0);
      T0 = Tokens.get();
    }
    if (T0->getKind() != NodeKind::LetKeyword) {
      throw UnexpectedTokenDiagnostic(File, T0, std::vector { NodeKind::LetKeyword });
    }
    Let = static_cast<LetKeyword*>(T0);
    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::MutKeyword) {
      Mut = static_cast<MutKeyword*>(T1);
      Tokens.get();
    }

    auto Patt = parsePattern();

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
          Params.push_back(new Parameter(parsePattern(), nullptr));
      }
    }

after_params:

    TypeAssert* TA = nullptr;
    if (T2->getKind() == NodeKind::Colon) {
      Tokens.get();
      auto TE = parseTypeExpression();
      TA = new TypeAssert(static_cast<Colon*>(T2), TE);
      T2 = Tokens.peek();
    }

    LetBody* Body;
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
          Elements.push_back(parseLetBodyElement());
        }
        Tokens.get();
        Body = new LetBlockBody(static_cast<BlockStart*>(T2), Elements);
        break;
      }
      case NodeKind::Equals:
        Tokens.get();
        Body = new LetExprBody(static_cast<Equals*>(T2), parseExpression());
        break;
      case NodeKind::LineFoldEnd:
        Body = nullptr;
        break;
      default:
        std::vector<NodeKind> Expected { NodeKind::BlockStart, NodeKind::LineFoldEnd, NodeKind::Equals };
        if (TA == nullptr) {
          // First tokens of TypeAssert
          Expected.push_back(NodeKind::Colon);
          // First tokens of Pattern
          Expected.push_back(NodeKind::Identifier);
        }
        throw UnexpectedTokenDiagnostic(File, T2, Expected);
    }

    BOLT_EXPECT_TOKEN(LineFoldEnd);

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
          goto after_seek;
        case NodeKind::RParen:
        case NodeKind::Comma:
        case NodeKind::RArrowAlt:
        case NodeKind::EndOfFile:
          goto after_seek;
        default:
          continue;
      }
    }
after_seek:
    if (HasTilde) {
      auto Left = parseArrowTypeExpression();
      auto Tilde = expectToken<class Tilde>();
      auto Right = parseArrowTypeExpression();
      return new EqualityConstraintExpression { Left, Tilde, Right };
    }
    auto Name = expectToken<IdentifierAlt>();
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
          throw UnexpectedTokenDiagnostic(File, T1, std::vector { NodeKind::RParen, NodeKind::RArrowAlt, NodeKind::Comma, NodeKind::Identifier });
      }
    }
after_vars:
    return new TypeclassConstraintExpression { Name, TEs };
  }

  VarTypeExpression* Parser::parseVarTypeExpression() {
    auto Name = expectToken<Identifier>();
    // TODO reject constructor symbols (starting with a capital letter)
    return new VarTypeExpression { Name };
  }

  InstanceDeclaration* Parser::parseInstanceDeclaration() {
    auto InstanceKeyword = expectToken<class InstanceKeyword>();
    auto Name = expectToken<IdentifierAlt>();
    std::vector<TypeExpression*> TypeExps;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->is<BlockStart>()) {
        break;
      }
      TypeExps.push_back(parseTypeExpression());
    }
    auto BlockStart = expectToken<class BlockStart>();
    std::vector<Node*> Elements;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->is<BlockEnd>()) {
        Tokens.get();
        break;
      }
      Elements.push_back(parseClassElement());
    }
    expectToken(NodeKind::LineFoldEnd);
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
    auto Name = expectToken<IdentifierAlt>();
    std::vector<VarTypeExpression*> TypeVars;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->getKind() == NodeKind::BlockStart) {
        break;
      }
      TypeVars.push_back(parseVarTypeExpression());
    }
    auto BlockStart = expectToken<class BlockStart>();
    std::vector<Node*> Elements;
    for (;;) {
      auto T2 = Tokens.peek();
      if (T2->is<BlockEnd>()) {
        Tokens.get();
        break;
      }
      Elements.push_back(parseClassElement());
    }
    expectToken(NodeKind::LineFoldEnd);
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
      Elements.push_back(parseSourceElement());
    }
    return new SourceFile(File, Elements);
  }

}

