
// TODO check for memory leaks everywhere a nullptr is returned

#include <exception>
#include <vector>

#include "llvm/Support/Casting.h"

#include "bolt/CST.hpp"
#include "bolt/Scanner.hpp"
#include "bolt/Parser.hpp"
#include "bolt/Diagnostics.hpp" 
#include "bolt/DiagnosticEngine.hpp"

// ## Some rules
//
// 1. Only Tokens.get() if you are certain the token is valid. If not, you
//    should first Tokens.peek() and only call Tokens.get() if all checks
//    succeeded.
//
// 2. Do not consume a token when emitting an error. It is up to
//    skipToLineFoldEnd() to skip the actual tokens. Because that function skips
//    over blocks, it is important it knows when a block started.
//
// 3. Always unref() whatever CST node that has been allocated on error. That
//    includes tokens. This avoids memory leaks. And yes, they matter when the
//    compiler is permanently on such as in a language server.
//
// 5. Always unref() a LineFoldEnd or BlockEnd obtained via Tokens.get(), since
//    it will never be stored somewhere
//
// 6. Maintain the invariant that a wrong parse will never advance the input stream.

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

  Token* Parser::expectToken(NodeKind Kind) {
    auto T = Tokens.peek();
    if (T->getKind() != Kind) {
      DE.add<UnexpectedTokenDiagnostic>(File, T, std::vector<NodeKind> { Kind });
      return nullptr;
    }
    Tokens.get();
    return T;
  }

  ListPattern* Parser::parseListPattern() {
    auto LBracket = expectToken<class LBracket>();
    if (!LBracket) {
      return nullptr;
    }
    std::vector<std::tuple<Pattern*, Comma*>> Elements;
    RBracket* RBracket;
    auto T0 = Tokens.peek();
    if (T0->getKind() == NodeKind::RBracket) {
      Tokens.get();
      RBracket = static_cast<class RBracket*>(T0);
      goto finish;
    }
    for (;;) {
      auto P = parseWidePattern();
      if (!P) {
        LBracket->unref();
        for (auto [Element, Separator]: Elements) {
          Element->unref();
          Separator->unref();
        }
        return nullptr;
      }
      auto T1 = Tokens.peek();
      switch (T1->getKind()) {
        case NodeKind::Comma:
          Tokens.get();
          Elements.push_back(std::make_tuple(P, static_cast<Comma*>(T1)));
          break;
        case NodeKind::RBracket:
          Tokens.get();
          Elements.push_back(std::make_tuple(P, nullptr));
          RBracket = static_cast<class RBracket*>(T1);
          goto finish;
        default:
          DE.add<UnexpectedTokenDiagnostic>(File, T1, std::vector { NodeKind::Comma, NodeKind::RBracket });
      }
    }
finish:
    return new ListPattern { LBracket, Elements, RBracket };
  }

  Pattern* Parser::parsePrimitivePattern(bool IsNarrow) {
    auto T0 = Tokens.peek();
    switch (T0->getKind()) {
      case NodeKind::StringLiteral:
      case NodeKind::IntegerLiteral:
        Tokens.get();
        return new LiteralPattern(static_cast<Literal*>(T0));
      case NodeKind::Identifier:
        Tokens.get();
        return new BindPattern(static_cast<Identifier*>(T0));
      case NodeKind::IdentifierAlt:
      {
        Tokens.get();
        auto Name = static_cast<IdentifierAlt*>(T0);
        if (IsNarrow) {
          return new NamedPattern(Name, {});
        }
        std::vector<Pattern*> Patterns;
        for (;;) {
          auto T2 = Tokens.peek();
          if (T2->getKind() == NodeKind::RParen
              || T2->getKind() == NodeKind::RBracket
              || T2->getKind() == NodeKind::RBrace
              || T2->getKind() == NodeKind::Comma
              || T2->getKind() == NodeKind::Colon
              || T2->getKind() == NodeKind::Equals
              || T2->getKind() == NodeKind::BlockStart
              || T2->getKind() == NodeKind::RArrowAlt) {
            break;
          }
          auto P = parseNarrowPattern();
          if (!P) {
            Name->unref();
            for (auto P: Patterns) {
              P->unref();
            }
            return nullptr;
          }
          Patterns.push_back(P);
        }
        return new NamedPattern { Name, Patterns };
      }
      case NodeKind::LBracket:
        return parseListPattern();
      case NodeKind::LParen:
      {
        Tokens.get();
        auto LParen = static_cast<class LParen*>(T0);
        std::vector<std::tuple<Pattern*, Comma*>> Elements;
        RParen* RParen;
        for (;;) {
          auto P = parseWidePattern();
          if (!P) {
            LParen->unref();
            for (auto [P, Comma]: Elements) {
              P->unref();
              Comma->unref();
            }
            // TODO maybe skip to next comma?
            return nullptr;
          }
          auto T1 = Tokens.peek();
          if (T1->getKind() == NodeKind::Comma) {
            Tokens.get();
            Elements.push_back(std::make_tuple(P, static_cast<Comma*>(T1)));
          } else if (T1->getKind() == NodeKind::RParen) {
            Tokens.get();
            RParen = static_cast<class RParen*>(T1);
            Elements.push_back(std::make_tuple(P, nullptr));
            break;
          } else {
            DE.add<UnexpectedTokenDiagnostic>(File, T1, std::vector { NodeKind::Comma, NodeKind::RParen });
            LParen->unref();
            for (auto [P, Comma]: Elements) {
              P->unref();
              Comma->unref();
            }
            // TODO maybe skip to next comma?
            return nullptr;

          }
        }
        if (Elements.size() == 1) {
          return new NestedPattern { LParen, std::get<0>(Elements.front()), RParen };
        }
        return new TuplePattern(LParen, Elements, RParen);
      }
      default:
        // Tokens.get();
        DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector {
          NodeKind::Identifier,
          NodeKind::IdentifierAlt,
          NodeKind::StringLiteral,
          NodeKind::IntegerLiteral,
          NodeKind::LParen,
          NodeKind::LBracket
        });
        return nullptr;
    }
  }

  Pattern* Parser::parseWidePattern() {
    return parsePrimitivePattern(false);
  }

  Pattern* Parser::parseNarrowPattern() {
    return parsePrimitivePattern(true);
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
        // Tokens.get();
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

  TypeExpression* Parser::parseAppTypeExpression() {
    auto OpTy = parsePrimitiveTypeExpression();
    if (!OpTy) {
      return nullptr;
    }
    std::vector<TypeExpression*> ArgTys;
    for (;;) {
      auto T1 = Tokens.peek();
      auto Kind = T1->getKind();
      if (Kind == NodeKind::RArrow || Kind == NodeKind::Equals || Kind == NodeKind::BlockStart || Kind == NodeKind::LineFoldEnd || Kind == NodeKind::EndOfFile || Kind == NodeKind::RParen) {
        break;
      }
      auto TE = parsePrimitiveTypeExpression();
      if (!TE) {
        OpTy->unref();
        for (auto Arg: ArgTys) {
          Arg->unref();
        }
        return nullptr;
      }
      ArgTys.push_back(TE);
    }
    if (ArgTys.empty()) {
      return OpTy;
    }
    return new AppTypeExpression { OpTy, ArgTys };
  }

  TypeExpression* Parser::parseArrowTypeExpression() {
    auto RetType = parseAppTypeExpression();
    if (RetType == nullptr) {
      return nullptr;
    }
    std::vector<TypeExpression*> ParamTypes;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() != NodeKind::RArrow) {
        break;
      }
      Tokens.get();
      ParamTypes.push_back(RetType);
      RetType = parseAppTypeExpression();
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
        Tokens.get()->unref();
        break;
      }
      auto Pattern = parseWidePattern();
      if (!Pattern) {
        skipToLineFoldEnd();
        continue;
      }
      auto RArrowAlt = expectToken<class RArrowAlt>();
      if (!RArrowAlt) {
        Pattern->unref();
        skipToLineFoldEnd();
        continue;
      }
      auto Expression = parseExpression();
      if (!Expression) {
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

  RecordExpression* Parser::parseRecordExpression() {
    auto LBrace = expectToken<class LBrace>();
    if (!LBrace) {
      return nullptr;
    }
    RBrace* RBrace;
    auto T1 = Tokens.peek();
    std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields;
    if (T1->getKind() == NodeKind::RBrace) {
      Tokens.get();
      RBrace = static_cast<class RBrace*>(T1);
    } else {
      for (;;) {
        auto Name = expectToken<Identifier>();
        if (!Name) {
          LBrace->unref();
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            Comma->unref();
          }
          return nullptr;
        }
        auto Equals = expectToken<class Equals>();
        if (!Equals) {
          LBrace->unref();
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            Comma->unref();
          }
          Name->unref();
          return nullptr;
        }
        auto E = parseExpression();
        if (!E) {
          LBrace->unref();
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            Comma->unref();
          }
          Name->unref();
          Equals->unref();
          return nullptr;
        }
        auto T2 = Tokens.peek();
        if (T2->getKind() == NodeKind::Comma) {
          Tokens.get();
          Fields.push_back(std::make_tuple(new RecordExpressionField { Name, Equals, E }, static_cast<Comma*>(T2)));
        } else if (T2->getKind() == NodeKind::RBrace) {
          Tokens.get();
          RBrace = static_cast<class RBrace*>(T2);
          Fields.push_back(std::make_tuple(new RecordExpressionField { Name, Equals, E }, nullptr));
          break;
        } else {
          DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::Comma, NodeKind::RBrace });
          LBrace->unref();
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            Comma->unref();
          }
          Name->unref();
          Equals->unref();
          E->unref();
          return nullptr;
        }
      }
    }
    return new RecordExpression { LBrace, Fields, RBrace };
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
      case NodeKind::LBrace:
        return parseRecordExpression();
      default:
        // Tokens.get();
        DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::MatchKeyword, NodeKind::Identifier, NodeKind::IdentifierAlt, NodeKind::LParen, NodeKind::LBrace, NodeKind::IntegerLiteral, NodeKind::StringLiteral });
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
      if (T1->getKind() == NodeKind::LineFoldEnd
          || T1->getKind() == NodeKind::RParen
          || T1->getKind() == NodeKind::RBrace
          || T1->getKind() == NodeKind::BlockStart
          || T1->getKind() == NodeKind::Comma
          || ExprOperators.isInfix(T1)) {
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
        Tokens.get()->unref();
        break;
      }
      auto Element = parseLetBodyElement();
      if (Element) {
        Then.push_back(Element);
      }
    }
    Tokens.get()->unref(); // Always a LineFoldEnd
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
          Tokens.get()->unref();
          break;
        }
        auto Element = parseLetBodyElement();
        if (Element) {
          Else.push_back(Element);
        }
      }
      Tokens.get()->unref(); // Always a LineFoldEnd
      Parts.push_back(new IfStatementPart(T3, nullptr, T4, Else));
    }
    return new IfStatement(Parts);
  }

VariableDeclaration* Parser::parseVariableDeclaration() {

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

    auto P = parseWidePattern();
    if (!P) {
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

    auto T2 = Tokens.peek();
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
        Tokens.get()->unref(); // Always a BlockEnd
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

    checkLineFoldEnd();

finish:

    return new VariableDeclaration { Pub, Let, Mut, P, TA, Body };
  }

  FunctionDeclaration* Parser::parseFunctionDeclaration() {

    PubKeyword* Pub = nullptr;
    FnKeyword* Fn;
    TypeAssert* TA = nullptr;
    LetBody* Body = nullptr;

    auto T0 = Tokens.get();
    if (T0->getKind() == NodeKind::PubKeyword) {
      Pub = static_cast<PubKeyword*>(T0);
      T0 = Tokens.get();
    }
    if (T0->getKind() != NodeKind::FnKeyword) {
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::FnKeyword });
      if (Pub) {
        Pub->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    Fn = static_cast<FnKeyword*>(T0);

    auto Name = expectToken<Identifier>();
    if (!Name) {
      if (Pub) {
        Pub->unref();
      }
      Fn->unref();
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
          auto P = parseNarrowPattern();
          if (!P) {
            Tokens.get();
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
        Tokens.get()->unref(); // Always a BlockEnd
        Body = new LetBlockBody(static_cast<BlockStart*>(T2), Elements);
        break;
      }
      case NodeKind::Equals:
      {
        Tokens.get();
        auto E = parseExpression();
        if (!E) {
          skipToLineFoldEnd();
          goto finish;
        }
        Body = new LetExprBody(static_cast<Equals*>(T2), E);
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

    checkLineFoldEnd();

finish:

    return new FunctionDeclaration(
      Pub,
      Fn,
      Name,
      Params,
      TA,
      Body
    );
  }

  Node* Parser::parseLetBodyElement() {
    auto T0 = peekFirstTokenAfterModifiers();
    switch (T0->getKind()) {
      case NodeKind::LetKeyword:
        return parseVariableDeclaration();
      case NodeKind::FnKeyword:
        return parseFunctionDeclaration();
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
        Tokens.get()->unref();
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
        Tokens.get()->unref();
        break;
      }
      auto Element = parseClassElement();
      if (Element) {
        Elements.push_back(Element);
      }
    }
    Tokens.get()->unref(); // Always a LineFoldEnd
    return new ClassDeclaration(
      PubKeyword,
      ClassKeyword,
      Name,
      TypeVars,
      BlockStart,
      Elements
    );
  }

  std::vector<RecordDeclarationField*> Parser::parseRecordFields() {
    std::vector<RecordDeclarationField*> Fields;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() == NodeKind::BlockEnd) {
        Tokens.get()->unref();
        break;
      }
      auto Name = expectToken<Identifier>();
      if (!Name) {
        skipToLineFoldEnd();
        continue;
      }
      auto Colon = expectToken<class Colon>();
      if (!Colon) {
        Name->unref();
        skipToLineFoldEnd();
        continue;
      }
      auto TE = parseTypeExpression();
      if (!TE) {
        Name->unref();
        Colon->unref();
        skipToLineFoldEnd();
        continue;
      }
      checkLineFoldEnd();
      Fields.push_back(new RecordDeclarationField { Name, Colon, TE });
    }
    return Fields;
  }

  RecordDeclaration* Parser::parseRecordDeclaration() {
    auto T0 = Tokens.peek();
    PubKeyword* Pub = nullptr;
    if (T0->getKind() == NodeKind::MutKeyword) {
      Tokens.get();
      Pub = static_cast<PubKeyword*>(T0);
    }
    auto Struct = expectToken<StructKeyword>();
    if (!Struct) {
      if (Pub) {
        Pub->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    auto Name = expectToken<IdentifierAlt>();
    if (!Name) {
      if (Pub) {
        Pub->unref();
      }
      Struct->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<VarTypeExpression*> Vars;
    for (;;) {
      auto T1 = Tokens.peek();
      if (T1->getKind() == NodeKind::BlockStart) {
        break;
      }
      auto Var = parseVarTypeExpression();
      if (Var) {
        Vars.push_back(Var);
      }
    }
    auto BS = expectToken<BlockStart>();
    if (!BS) {
      if (Pub) {
        Pub->unref();
      }
      Struct->unref();
      Name->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    auto Fields = parseRecordFields();
    Tokens.get()->unref(); // Always a LineFoldEnd
    return new RecordDeclaration { Pub, Struct, Name, Vars, BS, Fields };
  }

  VariantDeclaration* Parser::parseVariantDeclaration() {
    auto T0 = Tokens.peek();
    PubKeyword* Pub = nullptr;
    if (T0->getKind() == NodeKind::MutKeyword) {
      Tokens.get();
      Pub = static_cast<PubKeyword*>(T0);
    }
    auto Enum = expectToken<EnumKeyword>();
    if (!Enum) {
      if (Pub) {
        Pub->unref();
      }
      skipToLineFoldEnd();
      return nullptr;
    }
    auto Name = expectToken<IdentifierAlt>();
    if (!Name) {
      if (Pub) {
        Pub->unref();
      }
      Enum->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<VarTypeExpression*> TVs;
    for (;;) {
      auto T0 = Tokens.peek();
      if (T0->getKind() == NodeKind::BlockStart) {
        break;
      }
      auto Var = parseVarTypeExpression();
      if (Var) {
        TVs.push_back(Var);
      }
    }
    auto BS = expectToken<BlockStart>();
    if (!BS) {
      if (Pub) {
        Pub->unref();
      }
      Enum->unref();
      Name->unref();
      skipToLineFoldEnd();
      return nullptr;
    }
    std::vector<VariantDeclarationMember*> Members;
    for (;;) {
next_member:
      auto T0 = Tokens.peek();
      if (T0->getKind() == NodeKind::BlockEnd) {
        Tokens.get()->unref();
        break;
      }
      auto Name = expectToken<IdentifierAlt>();
      if (!Name) {
        skipToLineFoldEnd();
        continue;
      }
      auto T1 = Tokens.peek();
      if (T1->getKind() == NodeKind::BlockStart) {
        Tokens.get();
        auto BS = static_cast<BlockStart*>(T1);
        auto Fields = parseRecordFields();
        // TODO continue; on error in Fields
        Members.push_back(new RecordVariantDeclarationMember { Name, BS, Fields });
      } else {
        std::vector<TypeExpression*> Elements;
        for (;;) {
          auto T2 = Tokens.peek();
          if (T2->getKind() == NodeKind::LineFoldEnd) {
            Tokens.get()->unref();
            break;
          }
          auto TE = parsePrimitiveTypeExpression();
          if (!TE) {
            Name->unref();
            for (auto El: Elements) {
              El->unref();
            }
            goto next_member;
          }
          Elements.push_back(TE);
        }
        Members.push_back(new TupleVariantDeclarationMember { Name, Elements });
      }
    }
    checkLineFoldEnd();
    return new VariantDeclaration { Pub, Enum, Name, TVs, BS, Members };
  }

  Node* Parser::parseClassElement() {
    auto T0 = Tokens.peek();
    switch (T0->getKind()) {
      case NodeKind::FnKeyword:
        return parseFunctionDeclaration();
      case NodeKind::TypeKeyword:
        // TODO
      default:
        DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector<NodeKind> { NodeKind::FnKeyword, NodeKind::TypeKeyword });
        skipToLineFoldEnd();
        return nullptr;
    }
  }

  Node* Parser::parseSourceElement() {
    auto T0 = peekFirstTokenAfterModifiers();
    switch (T0->getKind()) {
      case NodeKind::LetKeyword:
        return parseVariableDeclaration();
      case NodeKind::FnKeyword:
        return parseFunctionDeclaration();
      case NodeKind::IfKeyword:
        return parseIfStatement();
      case NodeKind::ClassKeyword:
        return parseClassDeclaration();
      case NodeKind::InstanceKeyword:
        return parseInstanceDeclaration();
      case NodeKind::StructKeyword:
        return parseRecordDeclaration();
      case NodeKind::EnumKeyword:
        return parseVariantDeclaration();
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
    auto T0 = Tokens.peek();
    if (T0->getKind() == NodeKind::LineFoldEnd) {
      Tokens.get()->unref();
    } else {
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::LineFoldEnd });
      skipToLineFoldEnd();
    }
  }

}

