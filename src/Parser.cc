
// TODO check for memory leaks everywhere a nullptr is returned

#include <sys/wait.h>
#include <tuple>
#include <vector>

#include "zen/config.hpp"

#include "bolt/Common.hpp"
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

enum class Expected {
  PrimitiveExpression,
  Expression,
  IdentifierAlt,
  Identifier,
};

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

Parser::Parser(TextFile& File, DiagnosticEngine& DE):
  File(File), DE(DE) {
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


template<typename T>
T* Parser::expectToken(TokenStream& Tokens) {
  auto Tok = Tokens.get();
  if (Tok->getKind() != T::Kind) {
    DE.add<UnexpectedTokenDiagnostic>(File, Tok, std::vector<NodeKind> { T::Kind });
    return nullptr;
  }
  return static_cast<T*>(Tok);
}

template<typename T>
void unrefAll(std::vector<T>& Container) {
  for (auto Element: Container) {
    Element->unref();
  }
}

void Parser::cacheAnnotations(TokenStream& Tokens) {
  auto Start = Tokens.getAbsoluteOffset();
  auto Annotations = parseAnnotations(Tokens);
  auto End = Tokens.getAbsoluteOffset();
  CachedAnnotations = { End - Start, Annotations };
}

Token* Parser::peekTokenAfterAnnotations(TokenStream& Tokens) {
  auto Lookahead = Tokens.fork();
  cacheAnnotations(Lookahead);
  return Lookahead.peek();
}

Token* Parser::peekTokenAfterAnnotationsAndModifiers(TokenStream& Tokens) {
  auto Lookahead = Tokens.fork();
  cacheAnnotations(Lookahead);
  std::size_t I = 0;
  for (;;) {
    auto T0 = Lookahead.peek(I++);
    switch (T0->getKind()) {
      case NodeKind::PubKeyword:
      case NodeKind::MutKeyword:
        continue;
      default:
        return T0;
    }
  }
}

ListPattern* Parser::parseListPattern(TokenStream& Tokens) {

  auto LBracket = expectToken<class LBracket>(Tokens);
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

    auto P = parseWidePattern(Tokens);
    if (!P) {
      LBracket->unref();
      for (auto [Pattern, Comma]: Elements) {
          Pattern->unref();
          if (Comma) {
            Comma->unref();
          }
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
        Tokens.get(); // Error recovery
        DE.add<UnexpectedTokenDiagnostic>(File, T1, std::vector {
          NodeKind::Comma,
          NodeKind::RBracket
         });
    }
    
  }

finish:
  return new ListPattern { LBracket, Elements, RBracket };
}

std::vector<std::tuple<RecordPatternField*, Comma*>> Parser::parseRecordPatternFields(TokenStream& Tokens) {

  std::vector<std::tuple<RecordPatternField*, Comma*>> Fields;

  for (;;) {

    auto T0 = Tokens.peek();

    // If the next token on the stream is '}', then we are finished since this
    // loop always cleanly starts at the beginning of a list element.
    // The caller expects RBrace to be on the stream so we leave it there.
    if (T0->getKind() == NodeKind::RBrace) {
      break;
    }

    // We encountered the last element in a pattern: '..'. It may have an
    // expression associated with it, which we must parse.
    if (T0->getKind() == NodeKind::DotDot) {

      // Parse the '..'
      Tokens.get();
      auto DotDot = static_cast<class DotDot*>(T0);

      auto T1 = Tokens.peek();

      // If the '..' is immediately followed by '}' we can exit early.
      if (T1->getKind() == NodeKind::RBrace) {
        Fields.push_back(std::make_tuple(new RecordPatternField(DotDot), nullptr));
        break;
      }

      // We only get here if there's still something interesting on the stream.
      // Go parse it now.
      auto P = parseWidePattern(Tokens);

      if (P) {

        auto T2 = Tokens.peek();

        // We parsed the rest pattern but there's still something left on the
        // stream. That's an error.
        if (T2->getKind() != NodeKind::RBrace) {
          DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::RBrace });
          // We assume that this really was the last element on the stream.
          break;
        }

        // Everything OK; create the new field and stop looking for more elements.
        // The caller expects RBrace to be on the stream so we leave it there.
        Fields.push_back(std::make_tuple(new RecordPatternField(DotDot, P), nullptr));
        break;

      } else {

        // We failed to parse a pattern, so we'll act as if this was due to a small typo by the user.
        // parseWidePattern() should have already advanced the stream so we don't have to.
        continue;

      }
 
    }

    // If we get here we are NOT at a '..' pattern and also not at the end of
    // the contents of the braces.
    
    auto Name = expectToken<Identifier>(Tokens);

    if (!Name) {
      // Something else than an identifier on the stream. Since we can't guess
      // names, we ignore it and try again.
      continue;
    }

    Equals* Equals = nullptr;
    Pattern* Pattern = nullptr;

    auto T1 = Tokens.peek();

    // The pattern on the stream certainly is not a punned field pattern, so
    // assume that what follows will be '=' and an ordinary pattern.
    if (T1->getKind() == NodeKind::Equals) {

      Tokens.get();

      Equals = static_cast<class Equals*>(T1);
      Pattern = parseWidePattern(Tokens);

      // We failed to parse a pattern, so we'll act as if this was due to a small typo by the user.
      // parseWidePattern() should have already advanced the stream so we don't have to.
      if (!Pattern) {
        continue;
      }

    }

    auto Field = new RecordPatternField(Name, Equals, Pattern);

    auto T2 = Tokens.peek();

    // We break a second time here when encountering the same '}' because we
    // want to push the right delimiter in the list of fields. Encountering a
    // '}' means no delimiter was present.
    // The caller expects RBrace to be on the stream so we leave it there.
    if (T2->getKind() == NodeKind::RBrace) {
      Fields.push_back(std::make_tuple(Field, nullptr));
      break;
    }

    // If there's anything other than a '}' or ',' it's an error.
    if (T2->getKind() != NodeKind::Comma) {
      DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::RBrace, NodeKind::Comma });
      // We assume the user forgot to place their ',' and continue parsing fields as if the ',' was present.
      continue;
    }

    // The only case left is that our token is a ','. We consume it and add it
    // in the list of fields.
    Tokens.get();
    auto Comma = static_cast<class Comma*>(T2);
    Fields.push_back(std::make_tuple(Field, Comma));
  }

  return Fields;
}

Pattern* Parser::parsePrimitivePattern(TokenStream& Tokens, bool IsNarrow) {

  auto T0 = Tokens.peek();

  switch (T0->getKind()) {

    // Parse literals such as 1 or "foo"
    case NodeKind::StringLiteral:
    case NodeKind::IntegerLiteral:
      Tokens.get();
      return new LiteralPattern(static_cast<Literal*>(T0));

    // Parse a very basic pattern
    case NodeKind::Identifier:
      Tokens.get();
      return new BindPattern(static_cast<Identifier*>(T0));

    // Parse a record pattern with no name, such as { foo = 12, bar = True }
    case NodeKind::LBrace:
    {
      Tokens.get();
      auto LBrace = static_cast<class LBrace*>(T0);
      auto Fields = parseRecordPatternFields(Tokens);
      auto RBrace = static_cast<class RBrace*>(Tokens.get());
      return new RecordPattern(LBrace, Fields, RBrace);
    }

    // If we enounter an identifier of which the first letter is uppercase, then
    // we must be dealing with some named pattern. After all, module paths such
    // as Foo.Bar.bax don't make sense in BindPattern so those are excluded.
    case NodeKind::IdentifierAlt:
    {
      Tokens.get();
      auto Name = static_cast<IdentifierAlt*>(T0);
  
      // If 'spaces' aren't allowed (such as 'Foo a b'), then we must
      // immediately return with whatever we were allowed to parse. 
      if (IsNarrow) {
        return new NamedTuplePattern(Name, {});
      }

      auto T1 = Tokens.peek();

      // If we encounter '{', we almost certainly are dealing with a named
      // record pattern.  These are parsed in much the same way as above, except
      // that we return a named record pattern instead of an anonymous one.
      if (T1->getKind() == NodeKind::LBrace) {
        Tokens.get();
        auto LBrace = static_cast<class LBrace*>(T1);
        auto Fields = parseRecordPatternFields(Tokens);
        auto RBrace = static_cast<class RBrace*>(Tokens.get());
        return new NamedRecordPattern({}, Name, LBrace, Fields, RBrace);
      }

      // From here on we assume we are dealing with a named tuple pattern.

      std::vector<Pattern*> Patterns;

      for (;;) {

        auto T2 = Tokens.peek();

        // This check enables less 'primitive' patterns, such as a disjunctive
        // pattern, to be parsed.
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

        // Parse one element of the named tuple pattern, e.g. the 'a' in 'Foo a b'
        auto P = parseNarrowPattern(Tokens);

        // A simple check ensures that no nullptrs are pushed in the tuple element list.
        // parseNarrowPattern() returns a nullptr if it failed to parse a
        // pattern, but it should still advance the stream (even on error).
        if (P) {
          Patterns.push_back(P);
        }

      }
      return new NamedTuplePattern { Name, Patterns };
    }

    // Parse a list pattern such as [ first, second, third ]
    case NodeKind::LBracket:
      return parseListPattern(Tokens);

    // Parse something between parentheses; either a nested pattern or a tuple pattern.
    case NodeKind::LParen:
    {
      Tokens.get();
      auto LParen = static_cast<class LParen*>(T0);

      // The basic tactic is as follows: assume a tuple pattern, parse as many
      // tuple elements as possible, and in the case that there is only one then
      // return a NestedPattern
      
      std::vector<std::tuple<Pattern*, Comma*>> Elements;

      RParen* RParen;
      for (;;) {

        auto P = parseWidePattern(Tokens);

        if (!P) {
          // If the tuple element failed to parse, simply ignore it and try again.
          // parseNarrowPattern() returns a nullptr if it failed to parse a
          // pattern, but it should still advance the stream (even on error).
          continue;;
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
        }

      }

      // As promised, we reduce the tuple pattern to a simple nested pattern if
      // it only contains one element.
      if (Elements.size() == 1) {
        return new NestedPattern { LParen, std::get<0>(Elements.front()), RParen };
      }

      return new TuplePattern(LParen, Elements, RParen);
    }

    default:
      // We must consume at least one token. Since we haven't consumed a single
      // token yet, we do so now.
      Tokens.get();
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

Pattern* Parser::parseWidePattern(TokenStream& Tokens) {
  return parsePrimitivePattern(Tokens, false);
}

Pattern* Parser::parseNarrowPattern(TokenStream& Tokens) {
  return parsePrimitivePattern(Tokens, true);
}

TypeExpression* Parser::parseTypeExpression(TokenStream& Tokens) {
  return parseQualifiedTypeExpression(Tokens);
}

TypeExpression* Parser::parseQualifiedTypeExpression(TokenStream& Tokens) {

  bool HasConstraints = false;

  // First we check if there is a '=>', indicating that there are (class)
  // constraints that need to be parsed.
  auto T0 = Tokens.peek();
  if (isa<LParen>(T0)) {
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

  // Skip the logic below if we can see that there is are no constraint expressions.
  if (!HasConstraints) {
    return parseArrowTypeExpression(Tokens);
  }

  Tokens.get();
  LParen* LParen = static_cast<class LParen*>(T0);

  std::vector<std::tuple<ConstraintExpression*, Comma*>> Constraints;

  RParen* RParen;
  RArrowAlt* RArrowAlt;

  auto T1 = Tokens.peek();

  // Special case for when there are no actual constraints
  if (T1->getKind() == NodeKind::RParen) {
    Tokens.get();
    RParen = static_cast<class RParen*>(T1);
    goto after_constraints;
  }

  for (;;) {

    auto C = parseConstraintExpression(Tokens);

    if (!C) {
      // If we couldn't parse the constraint expression then we assume that
      // parseConstraintExpression() put the stream in a good position to
      // continue parsing other constraints.
      continue;
    }

    Comma* Comma = nullptr;

    auto T2 = Tokens.get();

    switch (T2->getKind()) {
      case NodeKind::Comma:
      {
        auto Comma = static_cast<class Comma*>(T2);
        Constraints.push_back(std::make_tuple(C, Comma));
        continue;
      }
      case NodeKind::RParen:
        RParen = static_cast<class RParen*>(T2);
        Constraints.push_back(std::make_tuple(C, nullptr));
        goto after_constraints;
      default:
        DE.add<UnexpectedTokenDiagnostic>(File, T2, std::vector { NodeKind::Comma, NodeKind::RArrowAlt });
    }
  }

after_constraints:

  RArrowAlt = expectToken<class RArrowAlt>(Tokens);
  if (!RArrowAlt) {
    LParen->unref();
    for (auto [CE, Comma]: Constraints) {
      CE->unref();
      if (Comma) {
          Comma->unref();
      }
    }
    RParen->unref();
    return nullptr;
  }

  auto TE = parseArrowTypeExpression(Tokens);
  if (!TE) {
    LParen->unref();
    for (auto [CE, Comma]: Constraints) {
      CE->unref();
      if (Comma) {
        Comma->unref();
      }
    }
    RParen->unref();
    RArrowAlt->unref();
    return nullptr;
  }

  return new QualifiedTypeExpression { Constraints, RArrowAlt, TE };
}

TypeExpression* Parser::parsePrimitiveTypeExpression(TokenStream& Tokens) {

  auto T0 = Tokens.peek();

  switch (T0->getKind()) {

    case NodeKind::Identifier:
        return parseVarTypeExpression(Tokens);

    case NodeKind::LBrace:
    {
      Tokens.get();
      auto LBrace = static_cast<class LBrace*>(T0);

      std::vector<std::tuple<RecordTypeExpressionField*, Comma*>> Fields;

      VBar* VBar = nullptr;
      TypeExpression* Rest = nullptr;

      for (;;) {

        auto T1 = Tokens.peek();

        if (T1->getKind() == NodeKind::RBrace) {
          break;
        }

        auto Name = expectToken<Identifier>(Tokens);
        if (Name == nullptr) {
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            if (Comma) {
              Comma->unref();
            }
          }
          return nullptr;
        }

        auto Colon = expectToken<class Colon>(Tokens);
        if (Colon == nullptr) {
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            if (Comma) {
              Comma->unref();
            }
          }
          Name->unref();
          return nullptr;
        }

        auto TE = parseTypeExpression(Tokens);
        if (TE == nullptr) {
          for (auto [Field, Comma]: Fields) {
            Field->unref();
            if (Comma) {
              Comma->unref();
            }
          }
          Name->unref();
          Colon->unref();
          return nullptr;
        }

        auto Field = new RecordTypeExpressionField(Name, Colon, TE);

        auto T3 = Tokens.peek();
        if (T3->getKind() == NodeKind::RBrace) {
          Fields.push_back(std::make_tuple(Field, nullptr));
          break;
        }

        if (T3->getKind() == NodeKind::VBar) {
          Tokens.get();
          Fields.push_back(std::make_tuple(Field, nullptr));
          VBar = static_cast<class VBar*>(T3);
          Rest = parseTypeExpression(Tokens);
          if (!Rest) {
            for (auto [Field, Comma]: Fields) {
              Field->unref();
              if (Comma) {
                Comma->unref();
              }
            }
            Field->unref();
            return nullptr;
          }
          auto T4 = Tokens.peek();
          if (T4->getKind() != NodeKind::RBrace) {
            for (auto [Field, Comma]: Fields) {
              Field->unref();
              if (Comma) {
                Comma->unref();
              }
            }
            Field->unref();
            Rest->unref();
            DE.add<UnexpectedTokenDiagnostic>(File, T4, std::vector { NodeKind::RBrace });
            return nullptr;
          }
          break;
        }

        if (T3->getKind() == NodeKind::Comma) {
          Tokens.get();
          auto Comma = static_cast<class Comma*>(T3);
          Fields.push_back(std::make_tuple(Field, Comma));
          continue;
        }

        DE.add<UnexpectedTokenDiagnostic>(File, T3, std::vector { NodeKind::RBrace, NodeKind::Comma, NodeKind::VBar });
        for (auto [Field, Comma]: Fields) {
          Field->unref();
          if (Comma) {
            Comma->unref();
          }
        }
        Field->unref();
        return nullptr;
      }
      auto RBrace = static_cast<class RBrace*>(Tokens.get());
      return new RecordTypeExpression(LBrace, Fields, VBar, Rest, RBrace);
    }

    case NodeKind::LParen:
    {
      Tokens.get();
      auto LParen = static_cast<class LParen*>(T0);

      std::vector<std::tuple<TypeExpression*, Comma*>> Elements;

      RParen* RParen;

      for (;;) {

        auto T1 = Tokens.peek();

        if (isa<class RParen>(T1)) {
          Tokens.get();
          RParen = static_cast<class RParen*>(T1);
          break;
        }

        auto TE = parseTypeExpression(Tokens);
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
              if (Comma) {
                Comma->unref();
              }
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
      return parseReferenceTypeExpression(Tokens);

    default:
      Tokens.get();
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::Identifier, NodeKind::IdentifierAlt, NodeKind::LParen });
      return nullptr;
  }

}

ReferenceTypeExpression* Parser::parseReferenceTypeExpression(TokenStream& Tokens) {

  std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;

  auto Name = expectToken<IdentifierAlt>(Tokens);
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

    Name = expectToken<IdentifierAlt>(Tokens);
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

TypeExpression* Parser::parseAppTypeExpression(TokenStream& Tokens) {

  auto OpTy = parsePrimitiveTypeExpression(Tokens);

  if (!OpTy) {
    return nullptr;
  }

  std::vector<TypeExpression*> ArgTys;

  for (;;) {
    auto T1 = Tokens.peek();
    auto Kind = T1->getKind();
    if (Kind == NodeKind::Comma
     || Kind == NodeKind::RArrow
      || Kind == NodeKind::Equals
       || Kind == NodeKind::BlockStart
        || Kind == NodeKind::LineFoldEnd
         || Kind == NodeKind::EndOfFile
          || Kind == NodeKind::RParen
           || Kind == NodeKind::RBracket
            || Kind == NodeKind::RBrace
             || Kind == NodeKind::VBar) {
      break;
    }
    auto TE = parsePrimitiveTypeExpression(Tokens);
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

TypeExpression* Parser::parseArrowTypeExpression(TokenStream& Tokens) {

  auto RetType = parseAppTypeExpression(Tokens);

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
    RetType = parseAppTypeExpression(Tokens);
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

MatchExpression* Parser::parseMatchExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  auto T0 = expectToken<MatchKeyword>(Tokens);
  if (!T0) {
    return nullptr;
  }

  Expression* Value;
  BlockStart* BlockStart;

  auto T1 = Tokens.peek();
  if (isa<class BlockStart>(T1)) {
    Value = nullptr;
    BlockStart = static_cast<class BlockStart*>(T1);
    Tokens.get();
  } else {
    Value = parseExpression(Tokens);
    if (!Value) {
      T0->unref();
      return nullptr;
    }
    BlockStart = expectToken<class BlockStart>(Tokens);
    if (!BlockStart) {
      T0->unref();
      Value->unref();
      return nullptr;
    }
  }

  std::vector<MatchCase*> Cases;

  for (;;) {
    auto T2 = Tokens.peek();
    if (isa<BlockEnd>(T2)) {
      Tokens.get()->unref();
      break;
    }
    auto Pattern = parseWidePattern(Tokens);
    if (!Pattern) {
      skipPastLineFoldEnd(Tokens);
      continue;
    }
    auto RArrowAlt = expectToken<class RArrowAlt>(Tokens);
    if (!RArrowAlt) {
      Pattern->unref();
      skipPastLineFoldEnd(Tokens);
      continue;
    }
    auto Expression = parseExpression(Tokens);
    if (!Expression) {
      Pattern->unref();
      RArrowAlt->unref();
      skipPastLineFoldEnd(Tokens);
      continue;
    }
    checkLineFoldEnd(Tokens);
    Cases.push_back(new MatchCase { Pattern, RArrowAlt, Expression });
  }

  return new MatchExpression {
    Annotations,
    static_cast<MatchKeyword*>(T0),
    Value,
    BlockStart,
    Cases
  };
}

RecordExpression* Parser::parseRecordExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  auto LBrace = expectToken<class LBrace>(Tokens);
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

      auto Name = expectToken<Identifier>(Tokens);
      if (!Name) {
        LBrace->unref();
        for (auto [Field, Comma]: Fields) {
          Field->unref();
          Comma->unref();
        }
        return nullptr;
      }

      auto Equals = expectToken<class Equals>(Tokens);
      if (!Equals) {
        LBrace->unref();
        for (auto [Field, Comma]: Fields) {
          Field->unref();
          Comma->unref();
        }
        Name->unref();
        return nullptr;
      }

      auto E = parseExpression(Tokens);
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
          if (Comma) {
            Comma->unref();
          }
        }
        Name->unref();
        Equals->unref();
        E->unref();
        return nullptr;
      }

    }

  }

  return new RecordExpression { Annotations, LBrace, Fields, RBrace };
}

FunctionExpression* Parser::parseFunctionExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  auto Backslash = expectToken<class Backslash>(Tokens);
  if (!Backslash) {
    return nullptr;
  }

  std::vector<Pattern*> Params;

  class RArrow* RArrow;

  for (;;) {

    auto T1 = Tokens.peek();

    if (isa<LineFoldEnd>(T1)) {
      DE.add<UnexpectedTokenDiagnostic>(File, T1, std::vector {
        NodeKind::RArrow,
      });
      Backslash->unref();
      unrefAll(Params);
      return nullptr;
    }

    if (isa<class RArrow>(T1)) {
      Tokens.get();
      RArrow = static_cast<class RArrow*>(T1);
      break;
    }

    auto P = parseNarrowPattern(Tokens);
    if (!P) {
      Backslash->unref();
      unrefAll(Params);
      return nullptr;
    }

    Params.push_back(P);
  }

  auto E = parseExpression(Tokens);
  if (!E) {
    Backslash->unref();
    unrefAll(Params);
    RArrow->unref();
    return nullptr;
  }

  return new FunctionExpression {
    Annotations,
    Backslash,
    Params,
    RArrow,
    E
  };
}

ReferenceExpression* Parser::parseReferenceExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;

  for (;;) {

    auto T1 = Tokens.peek(0);
    auto T2 = Tokens.peek(1);

    if (!isa<IdentifierAlt>(T1) || !isa<Dot>(T2)) {
      break;
    }

    Tokens.get();
    Tokens.get();

    ModulePath.push_back(std::make_tuple(static_cast<IdentifierAlt*>(T1), static_cast<class Dot*>(T2)));
  }

  auto T3 = Tokens.get();

  if (!isa<Identifier>(T3) && !isa<IdentifierAlt>(T3)) {
    for (auto [Name, Dot]: ModulePath) {
      Name->unref();
      Dot->unref();
    }
    DE.add<UnexpectedTokenDiagnostic>(File, T3, std::vector { NodeKind::Identifier, NodeKind::IdentifierAlt });
    return nullptr;
  }

  return new ReferenceExpression(Annotations, ModulePath, Symbol::from_raw_node(T3));
}

LiteralExpression* Parser::parseLiteralExpression(TokenStream& Tokens) {
  auto Annotations = parseAnnotations(Tokens);
  auto T0 = Tokens.get();
  if (!isa<Literal>(T0)) {
    DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::IntegerLiteral, NodeKind::StringLiteral });
    return nullptr;
  }
  return new LiteralExpression(Annotations, static_cast<Literal*>(T0));
}

Expression* Parser::parsePrimitiveExpression(TokenStream& Tokens) {

  auto T0 = peekTokenAfterAnnotations(Tokens);

  switch (T0->getKind()) {

    case NodeKind::Identifier:
    case NodeKind::IdentifierAlt:
      return parseReferenceExpression(Tokens);

    case NodeKind::LParen:
    {
      auto Annotations = parseAnnotations(Tokens);

      Tokens.get();
      auto LParen = static_cast<class LParen*>(T0);

      std::vector<std::tuple<Expression*, Comma*>> Elements;

      RParen* RParen;

      auto T1 = Tokens.peek();

      if (isa<class RParen>(T1)) {
        Tokens.get();
        RParen = static_cast<class RParen*>(T1);
        goto after_tuple_elements;
      }

      for (;;) {

        auto T1 = Tokens.peek();

        auto E = parseExpression(Tokens);
        if (!E) {
          LParen->unref();
          for (auto [E, Comma]: Elements) {
            E->unref();
            if (Comma) {
              Comma->unref();
            }
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
        return new NestedExpression(Annotations, LParen, std::get<0>(Elements.front()), RParen);
      }

      return new TupleExpression { Annotations, LParen, Elements, RParen };
    }

    case NodeKind::Backslash:
      return parseFunctionExpression(Tokens);

    case NodeKind::MatchKeyword:
      return parseMatchExpression(Tokens);
  
    case NodeKind::DoKeyword:
      return parseBlockExpression(Tokens);

    case NodeKind::IfKeyword:
      return parseIfExpression(Tokens);

    case NodeKind::ReturnKeyword:
      return parseReturnExpression(Tokens);

    case NodeKind::IntegerLiteral:
    case NodeKind::StringLiteral:
      return parseLiteralExpression(Tokens);

    case NodeKind::LBrace:
      return parseRecordExpression(Tokens);

    default:
      Tokens.get();
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector {
        NodeKind::MatchKeyword,
        NodeKind::DoKeyword,
        NodeKind::Identifier,
        NodeKind::IdentifierAlt,
        NodeKind::Backslash,
        NodeKind::LParen,
        NodeKind::LBrace,
        NodeKind::IntegerLiteral,
        NodeKind::StringLiteral
      });
      return nullptr;
  }

}

BlockExpression* Parser::parseBlockExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  auto DoKeyword = expectToken<class DoKeyword>(Tokens);
  if (!DoKeyword) {
    unrefAll(Annotations);
    return nullptr;
  }

  auto BlockStart = expectToken<class BlockStart>(Tokens);
  if (!BlockStart) {
    unrefAll(Annotations);
    DoKeyword->unref();
    return nullptr;
  }

  std::vector<Node*> Elements;

  for (;;) {

      auto T2 = Tokens.peek();

      if (T2->getKind() == NodeKind::BlockEnd) {
        Tokens.get()->unref();
        break;
      }

      auto Element = parseLetBodyElement(Tokens);
      if (Element == nullptr) {
        unrefAll(Annotations);
        DoKeyword->unref();
        BlockStart->unref();
        unrefAll(Elements);
        return nullptr;
      }

      Elements.push_back(Element);
  }

  return new BlockExpression {
    DoKeyword,
    BlockStart,
    Elements
  };
}

Expression* Parser::parseMemberExpression(TokenStream& Tokens) {
  auto E = parsePrimitiveExpression(Tokens);
  if (!E) {
    return nullptr;
  }
  for (;;) {
    auto T1 = Tokens.peek(0);
    auto T2 = Tokens.peek(1);
    if (!isa<Dot>(T1)) {
      break;
    }
    switch (T2->getKind()) {
      case NodeKind::IntegerLiteral:
      case NodeKind::Identifier:
      {
        Tokens.get();
        Tokens.get();
        auto Annotations = E->Annotations;
        E->Annotations = {};
        E = new MemberExpression { Annotations, E, static_cast<Dot*>(T1), T2 };
        break;
      }
      default:
        goto finish;
    }
  }
finish:
  return E;
}

Expression* Parser::parseCallExpression(TokenStream& Tokens) {
  auto Operator = parseMemberExpression(Tokens);
  if (!Operator) {
    return nullptr;
  }
  std::vector<Expression*> Args;
  for (;;) {
    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::LineFoldEnd
        || T1->getKind() == NodeKind::RParen
        || T1->getKind() == NodeKind::RBracket
        || T1->getKind() == NodeKind::RBrace
        || T1->getKind() == NodeKind::BlockStart
        || T1->getKind() == NodeKind::Comma
        || ExprOperators.isInfix(T1)) {
      break;
    }
    auto Arg = parseMemberExpression(Tokens);
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
  auto Annotations = Operator->Annotations;
  Operator->Annotations = {};
  return new CallExpression(Annotations, Operator, Args);
}

Expression* Parser::parseUnaryExpression(TokenStream& Tokens) {

  std::vector<Token*> Prefix;

  for (;;) {

    auto T0 = Tokens.peek();

    if (!ExprOperators.isPrefix(T0)) {
      break;
    }

    Tokens.get();

    Prefix.push_back(T0);
  }

  auto E = parseCallExpression(Tokens);
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

Expression* Parser::parseInfixOperatorAfterExpression(TokenStream& Tokens, Expression* Left, int MinPrecedence) {

  for (;;) {

    auto T0 = Tokens.peek();

    auto Info0 = ExprOperators.getInfix(T0);
    if (!Info0 || Info0->Precedence < MinPrecedence) {
      break;
    }

    Tokens.get();

    auto Right = parseUnaryExpression(Tokens);
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

      auto NewRight = parseInfixOperatorAfterExpression(Tokens, Right, Info1->Precedence);
      if (!NewRight) {
        Left->unref();
        T0->unref();
        Right->unref();
        return nullptr;
      }

      Right = NewRight;
    }

    Left = new InfixExpression(Left, Operator::from_raw_node(T0), Right);
  }

  return Left;
}

Expression* Parser::parseExpression(TokenStream& Tokens) {
  auto Left = parseUnaryExpression(Tokens);
  if (!Left) {
    return nullptr;
  }
  return parseInfixOperatorAfterExpression(Tokens, Left, 0);
}

Expression* Parser::parseExpressionStatement(TokenStream& Tokens) {
  auto E = parseExpression(Tokens);
  if (!E) {
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }
  checkLineFoldEnd(Tokens);
  return E;
}

ReturnExpression* Parser::parseReturnExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  auto ReturnKeyword = expectToken<class ReturnKeyword>(Tokens);
  if (!ReturnKeyword) {
    unrefAll(Annotations);
    return nullptr;
  }
  Expression* Expression;

  auto T1 = Tokens.peek();

  if (T1->getKind() == NodeKind::LineFoldEnd) {
    Tokens.get()->unref();
    Expression = nullptr;
  } else {
    Expression = parseExpression(Tokens);
    if (!Expression) {
      unrefAll(Annotations);
      ReturnKeyword->unref();
      return nullptr;
    }
  }

  return new ReturnExpression(Annotations, ReturnKeyword, Expression);
}

IfExpression* Parser::parseIfExpression(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  std::vector<IfExpressionPart*> Parts;

  auto IfKeyword = expectToken<class IfKeyword>(Tokens);
  if (!IfKeyword) {
    unrefAll(Annotations);
    return nullptr;
  }

  auto Test = parseExpression(Tokens);
  if (!Test) {
    IfKeyword->unref();
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto T1 = expectToken<BlockStart>(Tokens);
  if (!T1) {
    IfKeyword->unref();
    Test->unref();
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<Node*> Then;

  for (;;) {

    auto T2 = Tokens.peek();

    if (T2->getKind() == NodeKind::BlockEnd) {
      Tokens.get()->unref();
      break;
    }

    auto Element = parseLetBodyElement(Tokens);
    if (Element) {
      Then.push_back(Element);
    }

  }

  Tokens.get()->unref(); // Always a LineFoldEnd

  Parts.push_back(new IfExpressionPart(Annotations, IfKeyword, Test, T1, Then));

  for (;;) {

    auto T3 = peekTokenAfterAnnotationsAndModifiers(Tokens);

    if (T3->getKind() != NodeKind::ElseKeyword && T3->getKind() != NodeKind::ElifKeyword) {
      break;
    }

    auto Annotations = parseAnnotations(Tokens);

    Tokens.get();

    Expression* Test = nullptr;

    if (T3->getKind() == NodeKind::ElifKeyword) {
      Test = parseExpression(Tokens);
    }

    auto T4 = expectToken<BlockStart>(Tokens);
    if (!T4) {
      for (auto Part: Parts) {
        Part->unref();
      }
      return nullptr;
    }

    std::vector<Node*> Alt;

    for (;;) {

      auto T5 = Tokens.peek();

      if (T5->getKind() == NodeKind::BlockEnd) {
        Tokens.get()->unref();
        break;
      }

      auto Element = parseLetBodyElement(Tokens);
      if (Element) {
        Alt.push_back(Element);
      }

    }

    Tokens.get()->unref(); // Always a LineFoldEnd

    Parts.push_back(new IfExpressionPart(Annotations, T3, Test, T4, Alt));

    if (T3->getKind() == NodeKind::ElseKeyword) {
      break;
    }

  }

  return new IfExpression(Parts);
}

enum class FnMode {
  Prefix,
  Infix,
  Suffix,
  Wrapped,
  Named,
};

FunctionDeclaration* Parser::parseFunctionDeclaration(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);

  PubKeyword* Pub = nullptr;
  ForeignKeyword* Foreign = nullptr;
  FnKeyword* Fn;
  Operator Op;
  Symbol Sym;
  Pattern* Name;
  Parameter* Param;
  Parameter* Left;
  Parameter* Right;
  std::vector<Parameter*> Params;
  TypeAssert* TA = nullptr;
  LetBody* Body = nullptr;
  FnMode Mode;

  auto T0 = Tokens.get();
  if (T0->getKind() == NodeKind::PubKeyword) {
    Pub = static_cast<PubKeyword*>(T0);
    T0 = Tokens.get();
  }
  if (T0->getKind() == NodeKind::ForeignKeyword) {
    Foreign = static_cast<ForeignKeyword*>(T0);
    T0 = Tokens.get();
  }
  if (T0->getKind() != NodeKind::FnKeyword) {
    DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::FnKeyword });
    if (Pub) {
      Pub->unref();
    }
    if (Foreign) {
      Foreign->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }
  Fn = static_cast<FnKeyword*>(T0);

  auto T2 = Tokens.peek(0);
  auto T3 = Tokens.peek(1);
  auto T4 = Tokens.peek(2);
  if (isa<Operator>(T2)) {
    // Prefix function declaration
    Tokens.get();
    auto P1 = parseNarrowPattern(Tokens);
    Param = new Parameter(P1, nullptr);
    Op = Operator::from_raw_node(T2);
    Mode = FnMode::Prefix;
    goto after_params;
  } else if (isa<Operator>(T3) && (T4->getKind() == NodeKind::Colon || T4->getKind() == NodeKind::Equals || T4->getKind() == NodeKind::BlockStart || T4->getKind() == NodeKind::LineFoldEnd)) {
    // Sufffix function declaration
    auto P1 = parseNarrowPattern(Tokens);
    Param = new Parameter(P1, nullptr);
    Tokens.get();
    Op = Operator::from_raw_node(T3);
    Mode = FnMode::Suffix;
    goto after_params;
  } else if (T2->getKind() == NodeKind::LParen && isa<Operator>(T3) && T4->getKind() == NodeKind::RParen) {
    // Wrapped operator function declaration
    Tokens.get();
    Tokens.get();
    Tokens.get();
    Sym = new WrappedOperator(
      static_cast<class LParen*>(T2),
      Operator::from_raw_node(T3),
      static_cast<class RParen*>(T3)
    );
    Mode = FnMode::Wrapped;
  } else if (isa<Operator>(T3)) {
    // Infix function declaration
    auto P1 = parseNarrowPattern(Tokens);
    Left = new Parameter(P1, nullptr);
    Tokens.get();
    auto P2 = parseNarrowPattern(Tokens);
    Right = new Parameter(P2, nullptr);
    Op = Operator::from_raw_node(T3);
    Mode = FnMode::Infix;
    goto after_params;
  } else {
    // Variable declaration or named function declaration
    Mode = FnMode::Named;
    Name = parseNarrowPattern(Tokens);
    if (!Name) {
      if (Pub) {
        Pub->unref();
      }
      if (Foreign) {
        Foreign->unref();
      }
      Fn->unref();
      skipPastLineFoldEnd(Tokens);
      return nullptr;
    }
  }

  for (;;) {
    auto T5 = Tokens.peek();
    switch (T5->getKind()) {
      case NodeKind::LineFoldEnd:
      case NodeKind::BlockStart:
      case NodeKind::Equals:
      case NodeKind::Colon:
        goto after_params;
      default:
        auto P = parseNarrowPattern(Tokens);
        if (!P) {
          Tokens.get();
          P = new BindPattern(new Identifier("_"));
        }
        Params.push_back(new Parameter(P, nullptr));
    }
  }

after_params:

  auto T5 = Tokens.peek();

  if (T5->getKind() == NodeKind::Colon) {
    Tokens.get();
    auto TE = parseTypeExpression(Tokens);
    if (TE) {
      TA = new TypeAssert(static_cast<Colon*>(T5), TE);
    } else {
      skipPastLineFoldEnd(Tokens);
      goto finish;
    }
    T5 = Tokens.peek();
  }

  switch (T5->getKind()) {
    case NodeKind::BlockStart:
    {
      Tokens.get();
      std::vector<Node*> Elements;
      for (;;) {
        auto T6 = Tokens.peek();
        if (T6->getKind() == NodeKind::BlockEnd) {
          break;
        }
        auto Element = parseLetBodyElement(Tokens);
        if (Element) {
          Elements.push_back(Element);
        }
      }
      Tokens.get()->unref(); // Always a BlockEnd
      Body = new LetBlockBody(static_cast<BlockStart*>(T5), Elements);
      break;
    }
    case NodeKind::Equals:
    {
      Tokens.get();
      auto E = parseExpression(Tokens);
      if (!E) {
        skipPastLineFoldEnd(Tokens);
        goto finish;
      }
      Body = new LetExprBody(static_cast<Equals*>(T5), E);
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
      DE.add<UnexpectedTokenDiagnostic>(File, T5, Expected);
  }

  checkLineFoldEnd(Tokens);

finish:

  switch (Mode) {
    case FnMode::Prefix:
      return new PrefixFunctionDeclaration(
        Annotations,
        Pub,
        Foreign,
        Fn,
        Op,
        Param,
        TA,
        Body
      );
    case FnMode::Suffix:
      return new SuffixFunctionDeclaration(
        Annotations,
        Pub,
        Foreign,
        Fn,
        Param,
        Op,
        TA,
        Body
      );
    case FnMode::Infix:
      return new InfixFunctionDeclaration(
        Annotations,
        Pub,
        Foreign,
        Fn,
        Left,
        Op,
        Right,
        TA,
        Body
      );
    case FnMode::Wrapped:
      return new NamedFunctionDeclaration(
        Annotations,
        Pub,
        Foreign,
        Fn,
        Sym,
        Params,
        TA,
        Body
      );
    case FnMode::Named:
      return new NamedFunctionDeclaration(
        Annotations,
        Pub,
        Foreign,
        Fn,
        cast<BindPattern>(Name)->Name,
        Params,
        TA,
        Body
      );
  }
}


VariableDeclaration* Parser::parseVariableDeclaration(TokenStream& Tokens) {

  auto Annotations = parseAnnotations(Tokens);
  PubKeyword* Pub = nullptr;
  LetKeyword* Let;
  MutKeyword* Mut = nullptr;
  Operator Op;
  Symbol Sym;
  Pattern* Name;
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
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }
  Let = static_cast<LetKeyword*>(T0);
  auto T1 = Tokens.peek();
  if (T1->getKind() == NodeKind::MutKeyword) {
    Tokens.get();
    Mut = static_cast<MutKeyword*>(T1);
  }

  auto T2 = Tokens.peek(0);
  auto T3 = Tokens.peek(1);
  auto T4 = Tokens.peek(2);
  Name = parseNarrowPattern(Tokens);
  if (!Name) {
    if (Pub) {
      Pub->unref();
    }
    Let->unref();
    if (Mut) {
      Mut->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto T5 = Tokens.peek();

  if (T5->getKind() == NodeKind::Colon) {
    Tokens.get();
    auto TE = parseTypeExpression(Tokens);
    if (TE) {
      TA = new TypeAssert(static_cast<Colon*>(T5), TE);
    } else {
      skipPastLineFoldEnd(Tokens);
      goto finish;
    }
    T5 = Tokens.peek();
  }

  switch (T5->getKind()) {

    case NodeKind::BlockStart:
    {
      Tokens.get();

      std::vector<Node*> Elements;

      for (;;) {

        auto T6 = Tokens.peek();
        if (T6->getKind() == NodeKind::BlockEnd) {
          break;
        }

        auto Element = parseLetBodyElement(Tokens);
        if (Element) {
          Elements.push_back(Element);
        }

      }

      Tokens.get()->unref(); // Always a BlockEnd

      Body = new LetBlockBody(static_cast<BlockStart*>(T5), Elements);

      break;
    }

    case NodeKind::Equals:
    {
      Tokens.get();

      auto E = parseExpression(Tokens);
      if (!E) {
        skipPastLineFoldEnd(Tokens);
        goto finish;
      }

      Body = new LetExprBody(static_cast<Equals*>(T5), E);

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
      DE.add<UnexpectedTokenDiagnostic>(File, T5, Expected);
  }

  checkLineFoldEnd(Tokens);

finish:

  return new VariableDeclaration(
    Annotations,
    Pub,
    Let,
    Mut,
    Name,
    TA,
    Body
  );
}

Node* Parser::parseLetBodyElement(TokenStream& Tokens) {
  auto T0 = peekTokenAfterAnnotationsAndModifiers(Tokens);
  switch (T0->getKind()) {
    case NodeKind::LetKeyword:
      return parseVariableDeclaration(Tokens);
    case NodeKind::FnKeyword:
        return parseFunctionDeclaration(Tokens);
    default:
      return parseExpressionStatement(Tokens);
  }
}

ConstraintExpression* Parser::parseConstraintExpression(TokenStream& Tokens) {

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

    auto Left = parseArrowTypeExpression(Tokens);
    if (!Left) {
      return nullptr;
    }

    auto Tilde = expectToken<class Tilde>(Tokens);
    if (!Tilde) {
      Left->unref();
      return nullptr;
    }

    auto Right = parseArrowTypeExpression(Tokens);
    if (!Right) {
      Left->unref();
      Tilde->unref();
      return nullptr;
    }

    return new EqualityConstraintExpression { Left, Tilde, Right };
  }

  auto Name = expectToken<IdentifierAlt>(Tokens);
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

VarTypeExpression* Parser::parseVarTypeExpression(TokenStream& Tokens) {

  auto Name = expectToken<Identifier>(Tokens);
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

InstanceDeclaration* Parser::parseInstanceDeclaration(TokenStream& Tokens) {

  auto InstanceKeyword = expectToken<class InstanceKeyword>(Tokens);
  if (!InstanceKeyword) {
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto Name = expectToken<IdentifierAlt>(Tokens);
  if (!Name) {
    InstanceKeyword->unref();
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<TypeExpression*> TypeExps;

  for (;;) {

    auto T1 = Tokens.peek();
    if (isa<BlockStart>(T1)) { 
      break;
    }

    auto TE = parseTypeExpression(Tokens);
    if (!TE) {
      InstanceKeyword->unref();
      Name->unref();
      for (auto TE: TypeExps) {
        TE->unref();
      }
      skipPastLineFoldEnd(Tokens);
      return nullptr;
    }

    TypeExps.push_back(TE);
  }

  auto BlockStart = expectToken<class BlockStart>(Tokens);
  if (!BlockStart) { 
    InstanceKeyword->unref();
    Name->unref();
    for (auto TE: TypeExps) {
      TE->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<Node*> Elements;

  for (;;) {

    auto T2 = Tokens.peek();

    if (isa<BlockEnd>(T2)) {
      Tokens.get()->unref();
      break;
    }

    auto Element = parseClassElement(Tokens);
    if (Element) {
      Elements.push_back(Element);
    }

  }

  checkLineFoldEnd(Tokens);

  return new InstanceDeclaration(
    InstanceKeyword,
    Name,
    TypeExps,
    BlockStart,
    Elements
  );
}

ClassDeclaration* Parser::parseClassDeclaration(TokenStream& Tokens) {

  PubKeyword* PubKeyword = nullptr;

  auto T0 = Tokens.peek();
  if (T0->getKind() == NodeKind::PubKeyword) {
    Tokens.get();
    PubKeyword = static_cast<class PubKeyword*>(T0);
  }

  auto ClassKeyword = expectToken<class ClassKeyword>(Tokens);
  if (!ClassKeyword) {
    if (PubKeyword) {
      PubKeyword->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto Name = expectToken<IdentifierAlt>(Tokens);
  if (!Name) {
    if (PubKeyword) {
      PubKeyword->unref();
    }
    ClassKeyword->unref();
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<VarTypeExpression*> TypeVars;

  for (;;) {

    auto T2 = Tokens.peek();
    if (T2->getKind() == NodeKind::BlockStart) {
      break;
    }

    auto TE = parseVarTypeExpression(Tokens);
    if (!TE) {
      if (PubKeyword) {
        PubKeyword->unref();
      }
      ClassKeyword->unref();
      for (auto TV: TypeVars) {
        TV->unref();
      }
      skipPastLineFoldEnd(Tokens);
      return nullptr;
    }

    TypeVars.push_back(TE);
  }

  auto BlockStart = expectToken<class BlockStart>(Tokens);
  if (!BlockStart) {
    if (PubKeyword) {
      PubKeyword->unref();
    }
    ClassKeyword->unref();
    for (auto TV: TypeVars) {
      TV->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<Node*> Elements;

  for (;;) {

    auto T2 = Tokens.peek();

    if (isa<BlockEnd>(T2)) {
      Tokens.get()->unref();
      break;
    }

    auto Element = parseClassElement(Tokens);
    if (Element) {
      Elements.push_back(Element);
    } else {
      skipPastLineFoldEnd(Tokens);
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

std::vector<RecordDeclarationField*> Parser::parseRecordDeclarationFields(TokenStream& Tokens) {

  std::vector<RecordDeclarationField*> Fields;

  for (;;) {

    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::BlockEnd) {
      Tokens.get()->unref();
      break;
    }

    auto Name = expectToken<Identifier>(Tokens);
    if (!Name) {
      skipPastLineFoldEnd(Tokens);
      continue;
    }

    auto Colon = expectToken<class Colon>(Tokens);
    if (!Colon) {
      Name->unref();
      skipPastLineFoldEnd(Tokens);
      continue;
    }

    auto TE = parseTypeExpression(Tokens);
    if (!TE) {
      Name->unref();
      Colon->unref();
      skipPastLineFoldEnd(Tokens);
      continue;
    }

    checkLineFoldEnd(Tokens);

    Fields.push_back(new RecordDeclarationField { Name, Colon, TE });
  }

  return Fields;
}

RecordDeclaration* Parser::parseRecordDeclaration(TokenStream& Tokens) {

  PubKeyword* Pub = nullptr;

  auto T0 = Tokens.peek();
  if (T0->getKind() == NodeKind::MutKeyword) {
    Tokens.get();
    Pub = static_cast<PubKeyword*>(T0);
  }

  auto Struct = expectToken<StructKeyword>(Tokens);
  if (!Struct) {
    if (Pub) {
      Pub->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto Name = expectToken<IdentifierAlt>(Tokens);
  if (!Name) {
    if (Pub) {
      Pub->unref();
    }
    Struct->unref();
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<VarTypeExpression*> Vars;
  for (;;) {
    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::BlockStart) {
      break;
    }
    auto Var = parseVarTypeExpression(Tokens);
    if (Var) {
      Vars.push_back(Var);
    } else {
      Tokens.get();
    }
  }

  auto BS = expectToken<BlockStart>(Tokens);
  if (!BS) {
    if (Pub) {
      Pub->unref();
    }
    Struct->unref();
    Name->unref();
    for (auto V: Vars) {
      V->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto Fields = parseRecordDeclarationFields(Tokens);

  Tokens.get()->unref(); // Always a LineFoldEnd

  return new RecordDeclaration { Pub, Struct, Name, Vars, BS, Fields };
}

VariantDeclaration* Parser::parseVariantDeclaration(TokenStream& Tokens) {

  PubKeyword* Pub = nullptr;

  auto T0 = Tokens.peek();
  if (T0->getKind() == NodeKind::MutKeyword) {
    Tokens.get();
    Pub = static_cast<PubKeyword*>(T0);
  }

  auto Enum = expectToken<EnumKeyword>(Tokens);
  if (!Enum) {
    if (Pub) {
      Pub->unref();
    }
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  auto Name = expectToken<IdentifierAlt>(Tokens);
  if (!Name) {
    if (Pub) {
      Pub->unref();
    }
    Enum->unref();
    skipPastLineFoldEnd(Tokens);
    return nullptr;
  }

  std::vector<VarTypeExpression*> TVs;

  for (;;) {

    auto T0 = Tokens.peek();
    if (T0->getKind() == NodeKind::BlockStart) {
      break;
    }

    auto Var = parseVarTypeExpression(Tokens);
    if (Var) {
      TVs.push_back(Var);
    }

  }

  auto BS = expectToken<BlockStart>(Tokens);
  if (!BS) {
    if (Pub) {
      Pub->unref();
    }
    Enum->unref();
    Name->unref();
    skipPastLineFoldEnd(Tokens);
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

    auto Name = expectToken<IdentifierAlt>(Tokens);
    if (!Name) {
      skipPastLineFoldEnd(Tokens);
      continue;
    }

    auto T1 = Tokens.peek();
    if (T1->getKind() == NodeKind::BlockStart) {
      Tokens.get();
      auto BS = static_cast<BlockStart*>(T1);
      auto Fields = parseRecordDeclarationFields(Tokens);
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

        auto TE = parsePrimitiveTypeExpression(Tokens);
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

  checkLineFoldEnd(Tokens);

  return new VariantDeclaration { Pub, Enum, Name, TVs, BS, Members };
}

Node* Parser::parseClassElement(TokenStream& Tokens) {

  auto T0 = Tokens.peek();

  switch (T0->getKind()) {

    case NodeKind::LetKeyword:
      return parseVariableDeclaration(Tokens);

    case NodeKind::TypeKeyword:
      // TODO

    default:
      DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector<NodeKind> { NodeKind::LetKeyword, NodeKind::TypeKeyword });
      skipPastLineFoldEnd(Tokens);
      return nullptr;
  }
}

Node* Parser::parseSourceElement(TokenStream& Tokens) {

  auto T0 = peekTokenAfterAnnotationsAndModifiers(Tokens);

  switch (T0->getKind()) {
    case NodeKind::LetKeyword:
      return parseVariableDeclaration(Tokens);

    case NodeKind::FnKeyword:
        return parseFunctionDeclaration(Tokens);

    case NodeKind::ClassKeyword:
      return parseClassDeclaration(Tokens);

    case NodeKind::InstanceKeyword:
      return parseInstanceDeclaration(Tokens);

    case NodeKind::StructKeyword:
      return parseRecordDeclaration(Tokens);

    case NodeKind::EnumKeyword:
      return parseVariantDeclaration(Tokens);

    default:
      return parseExpressionStatement(Tokens);
  }
}

SourceFile* Parser::parseSourceFile(TokenStream& Tokens) {

  std::vector<Node*> Elements;

  for (;;) {

    auto T0 = Tokens.peek();
    if (isa<EndOfFile>(T0)) {
      break;
    }

    auto Element = parseSourceElement(Tokens);
    if (Element) {
      Elements.push_back(Element);
    }

  }

  return new SourceFile(File, Elements);
}

std::vector<Annotation*> Parser::parseAnnotations(TokenStream& Tokens) {

  // Small trick to avoid parsing annotations multiple times if a lookahead
  // function was used. The tested variable will be populated in functions such
  // as peekTokenAfterAnnotations()
  if (CachedAnnotations) {
    auto [Count, Keep] = *CachedAnnotations;
    Tokens.skip(Count);
    CachedAnnotations = {};
    return Keep;
  }

  std::vector<Annotation*> Annotations;

  for (;;) {

    auto T0 = Tokens.peek();
    if (T0->getKind() != NodeKind::At) {
      break;
    }

    auto At = static_cast<class At*>(T0);
    Tokens.get();

    auto T1 = Tokens.peek();
    switch (T1->getKind()) {
      case NodeKind::Colon:
      {
        auto Colon = static_cast<class Colon*>(T1);
        Tokens.get();
        auto TE = parsePrimitiveTypeExpression(Tokens);
        if (!TE) {
          // TODO
          continue;
        }
        Annotations.push_back(new TypeAssertAnnotation { At, Colon, TE });
        continue;
      }
      default:
      {
        // auto Name = static_cast<Identifier*>(T1);
        // Tokens.get();
        auto E = parseExpression(Tokens);
        if (!E) {
          At->unref();
          skipPastLineFoldEnd(Tokens);
          continue;
        }
        checkLineFoldEnd(Tokens);
        Annotations.push_back(new ExpressionAnnotation { At, E });
        continue;
      }
      // default:
      //   DE.add<UnexpectedTokenDiagnostic>(File, T1, std::vector { NodeKind::Colon, NodeKind::Identifier });
      //   At->unref();
      //   skipToLineFoldEnd();
      //   break;
    }

next_annotation:;
  }

  return Annotations;
}

void Parser::skipToRBrace(TokenStream& Tokens) {

  unsigned ParenLevel = 0;
  unsigned BracketLevel = 0;
  unsigned BraceLevel = 0;
  unsigned BlockLevel = 0;

  for (;;) {

    auto T0 = Tokens.peek();

    switch (T0->getKind()) {

      case NodeKind::EndOfFile:
        return;

      case NodeKind::LineFoldEnd:
        Tokens.get()->unref();
        if (BlockLevel == 0 && ParenLevel == 0 && BracketLevel == 0 && BlockLevel == 0) {
          return;
        }
        break;

      case NodeKind::BlockStart:
        Tokens.get()->unref();
        BlockLevel++;
        break;

      case NodeKind::BlockEnd:
        Tokens.get()->unref();
        BlockLevel--;
        break;

      case NodeKind::LParen:
        Tokens.get()->unref();
        ParenLevel++;
        break;

      case NodeKind::LBracket:
        Tokens.get()->unref();
        BracketLevel++;
        break;

      case NodeKind::LBrace:
        Tokens.get()->unref();
        BraceLevel++;
        break;

      case NodeKind::RParen:
        Tokens.get()->unref();
        ParenLevel--;
        break;

      case NodeKind::RBracket:
        Tokens.get()->unref();
        BracketLevel--;
        break;

      case NodeKind::RBrace:
        if (BlockLevel == 0 && ParenLevel == 0 && BracketLevel == 0 && BlockLevel == 0) {
          return;
        }
        Tokens.get()->unref();
        BraceLevel--;
        break;

      default:
        Tokens.get()->unref();
        break;
        
    }

  }

}

void Parser::skipPastLineFoldEnd(TokenStream& Tokens) {

  unsigned ParenLevel = 0;
  unsigned BracketLevel = 0;
  unsigned BraceLevel = 0;
  unsigned BlockLevel = 0;

  for (;;) {

    auto T0 = Tokens.get();

    switch (T0->getKind()) {

      case NodeKind::EndOfFile:
        return;

      case NodeKind::LineFoldEnd:
        T0->unref();
        if (BlockLevel == 0 && ParenLevel == 0 && BracketLevel == 0 && BlockLevel == 0) {
          return;
        }
        break;

      case NodeKind::BlockStart:
        T0->unref();
        BlockLevel++;
        break;

      case NodeKind::BlockEnd:
        T0->unref();
        BlockLevel--;
        break;

      case NodeKind::LParen:
        T0->unref();
        ParenLevel++;
        break;

      case NodeKind::LBracket:
        T0->unref();
        BracketLevel++;
        break;

      case NodeKind::LBrace:
        T0->unref();
        BraceLevel++;
        break;

      case NodeKind::RParen:
        T0->unref();
        ParenLevel--;
        break;

      case NodeKind::RBracket:
        T0->unref();
        BracketLevel--;
        break;

      case NodeKind::RBrace:
        T0->unref();
        BraceLevel--;
        break;

      default:
        T0->unref();
        break;
    }

  }

}

void Parser::checkLineFoldEnd(TokenStream& Tokens) {

  auto T0 = Tokens.peek();

  if (T0->getKind() == NodeKind::LineFoldEnd) {
    Tokens.get()->unref();
  } else {
    DE.add<UnexpectedTokenDiagnostic>(File, T0, std::vector { NodeKind::LineFoldEnd });
    skipPastLineFoldEnd(Tokens);
  }

}

}

