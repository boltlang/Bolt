
#include <unordered_map>

#include "zen/config.hpp"

#include "llvm/Support/Casting.h"

#include "bolt/Text.hpp"
#include "bolt/Integer.hpp"
#include "bolt/CST.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/Scanner.hpp"

namespace bolt {

  static inline bool isWhiteSpace(Char Chr) {
    switch (Chr) {
      case ' ':
      case '\n':
      case '\r':
      case '\t':
        return true;
      default:
        return false;
    }
  }

  static inline bool isOperatorPart(Char Chr) {
    switch (Chr) { 
      case '+':
      case '-':
      case '*':
      case '/':
      case '^':
      case '&':
      case '|':
      case '%':
      case '$':
      case '!':
      case '?':
      case '>':
      case '<':
      case '=':
        return true;
      default:
        return false;
    }
  }

  static bool isIdentifierPart(Char Chr) {
    return (Chr >= 65 && Chr <= 90) // Uppercase letter
        || (Chr >= 96 && Chr <= 122) // Lowercase letter
        || (Chr >= 48 && Chr <= 57) // Digit
        || Chr == '_';
  }

  static int toDigit(Char Chr) {
    ZEN_ASSERT(Chr >= 48 && Chr <= 57);
    return Chr - 48;
  }

  std::unordered_map<ByteString, NodeKind> Keywords = {
    { "pub", NodeKind::PubKeyword },
    { "let", NodeKind::LetKeyword },
    { "mut", NodeKind::MutKeyword },
    { "return", NodeKind::ReturnKeyword },
    { "type", NodeKind::TypeKeyword },
    { "mod", NodeKind::ModKeyword },
    { "if", NodeKind::IfKeyword },
    { "else", NodeKind::ElseKeyword },
    { "elif", NodeKind::ElifKeyword },
    { "match", NodeKind::MatchKeyword },
    { "class", NodeKind::ClassKeyword },
    { "instance", NodeKind::InstanceKeyword },
    { "struct", NodeKind::StructKeyword },
  };

  Scanner::Scanner(TextFile& File, Stream<Char>& Chars):
    File(File), Chars(Chars) {}

  Token* Scanner::read() {

    TextLoc StartLoc;
    Char C0;

    for (;;) {
      StartLoc = getCurrentLoc();
      C0 = getChar();
      if (isWhiteSpace(C0)) {
        continue;
      }
      if (C0 == '#') {
        for (;;) {
          C0 = getChar();
          if (C0 == '\n' || C0 == EOF) {
            break;
          }
        }
        continue;
      }
      break;
    }

    switch (C0) {

      case static_cast<Char>(EOF):
        return new EndOfFile(StartLoc);

      case '0':
      case '1':
      case '2':
      case '3':
      case '4':
      case '5':
      case '6':
      case '7':
      case '8':
      case '9':
      {
        Integer I = toDigit(C0);
        for (;;) {
          auto C1 = peekChar();
          switch (C1) {
            case '0':
            case '1':
            case '2':
            case '3':
            case '4':
            case '5':
            case '6':
            case '7':
            case '8':
            case '9':
              getChar();
              I = I * 10 + toDigit(C1);
              break;
            default:
              goto digit_finish;
          }
        }
digit_finish:
        return new IntegerLiteral(I, StartLoc);
      }

      case 'A':
      case 'B':
      case 'C':
      case 'D':
      case 'E':
      case 'F':
      case 'G':
      case 'H':
      case 'I':
      case 'J':
      case 'K':
      case 'L':
      case 'M':
      case 'N':
      case 'O':
      case 'P':
      case 'Q':
      case 'R':
      case 'S':
      case 'T':
      case 'U':
      case 'V':
      case 'W':
      case 'X':
      case 'Y':
      case 'Z':
      {
        ByteString Text { static_cast<char>(C0) };
        for (;;) {
          auto C1 = peekChar();
          if (!isIdentifierPart(C1)) {
            break;
          }
          Text.push_back(C1);
          getChar();
        }
        return new IdentifierAlt(Text, StartLoc);
      }

      case 'a':
      case 'b':
      case 'c':
      case 'd':
      case 'e':
      case 'f':
      case 'g':
      case 'h':
      case 'i':
      case 'j':
      case 'k':
      case 'l':
      case 'm':
      case 'n':
      case 'o':
      case 'p':
      case 'q':
      case 'r':
      case 's':
      case 't':
      case 'u':
      case 'v':
      case 'w':
      case 'x':
      case 'y':
      case 'z':
      case '_':
      {
        ByteString Text { static_cast<char>(C0) };
        for (;;) {
          auto C1 = peekChar();
          if (!isIdentifierPart(C1)) {
            break;
          }
          Text.push_back(C1);
          getChar();
        }
        auto Match = Keywords.find(Text);
        if (Match != Keywords.end()) {
          switch (Match->second) {
            case NodeKind::PubKeyword:
              return new PubKeyword(StartLoc);
            case NodeKind::LetKeyword:
              return new LetKeyword(StartLoc);
            case NodeKind::MutKeyword:
              return new MutKeyword(StartLoc);
            case NodeKind::TypeKeyword:
              return new TypeKeyword(StartLoc);
            case NodeKind::ReturnKeyword:
              return new ReturnKeyword(StartLoc);
            case NodeKind::IfKeyword:
              return new IfKeyword(StartLoc);
            case NodeKind::ElifKeyword:
              return new ElifKeyword(StartLoc);
            case NodeKind::ElseKeyword:
              return new ElseKeyword(StartLoc);
            case NodeKind::MatchKeyword:
              return new MatchKeyword(StartLoc);
            case NodeKind::ClassKeyword:
              return new ClassKeyword(StartLoc);
            case NodeKind::InstanceKeyword:
              return new InstanceKeyword(StartLoc);
            case NodeKind::StructKeyword:
              return new StructKeyword(StartLoc);
            default:
              ZEN_UNREACHABLE
          }
        }
        return new Identifier(Text, StartLoc);
      }

      case '"':
      {
        ByteString Text;
        bool Escaping = false;
        for (;;) {
          auto Loc = getCurrentLoc();
          auto C1 = getChar();
          if (Escaping) {
            switch (C1) {
              case 'a': Text.push_back('\a'); break;
              case 'b': Text.push_back('\b'); break;
              case 'f': Text.push_back('\f'); break;
              case 'n': Text.push_back('\n'); break;
              case 'r': Text.push_back('\r'); break;
              case 't': Text.push_back('\t'); break;
              case 'v': Text.push_back('\v'); break;
              case '0': Text.push_back('\0'); break;
              case '\'': Text.push_back('\''); break;
              case '"': Text.push_back('"'); break;
              default:
                throw UnexpectedStringDiagnostic(File, Loc, String { static_cast<char>(C1) });
            }
            Escaping = false;
          } else {
            switch (C1) {
              case '"':
                goto after_string_contents;
              case '\\':
                Escaping = true;
                break;
              default:
                Text.push_back(C1);
                break;
            }
          }
        }
after_string_contents:
        return new StringLiteral(Text, StartLoc);
      }

      case '.':
      {
        auto C1 = peekChar();
        if (C1 == '.') {
          getChar();
          auto C2 = peekChar();
          if (C2 == '.') {
            throw UnexpectedStringDiagnostic(File, getCurrentLoc(), String { static_cast<char>(C2) });
          }
          return new DotDot(StartLoc);
        }
        return new Dot(StartLoc);
      }

      case '+':
      case '-':
      case '*':
      case '/':
      case '^':
      case '&':
      case '|':
      case '%':
      case '$':
      case '!':
      case '?':
      case '>':
      case '<':
      case '=':
      {
        ByteString Text { static_cast<char>(C0) };
        for (;;) {
          auto C1 = peekChar();
          if (!isOperatorPart(C1)) {
            break;
          }
          Text.push_back(static_cast<char>(C1));
          getChar();
        }
        if (Text == "->") {
          return new RArrow(StartLoc);
        } else if (Text == "=>") {
          return new RArrowAlt(StartLoc);
        } else if (Text == "=") {
          return new Equals(StartLoc);
        } else if (Text.back() == '=' && Text[Text.size()-2] != '=') {
          return new Assignment(Text.substr(0, Text.size()-1), StartLoc);
        }
        return new CustomOperator(Text, StartLoc);
      }
      

#define BOLT_SIMPLE_TOKEN(ch, name) case ch: return new name(StartLoc);

    BOLT_SIMPLE_TOKEN(',', Comma)
    BOLT_SIMPLE_TOKEN(':', Colon)
    BOLT_SIMPLE_TOKEN('(', LParen)
    BOLT_SIMPLE_TOKEN(')', RParen)
    BOLT_SIMPLE_TOKEN('[', LBracket)
    BOLT_SIMPLE_TOKEN(']', RBracket)
    BOLT_SIMPLE_TOKEN('{', LBrace)
    BOLT_SIMPLE_TOKEN('}', RBrace)
    BOLT_SIMPLE_TOKEN('~', Tilde)

    default:
      throw UnexpectedStringDiagnostic(File, StartLoc, String { static_cast<char>(C0) });

    }

  }

  Punctuator::Punctuator(Stream<Token*>& Tokens):
    Tokens(Tokens) {
      Frames.push(FrameType::Block);
      Locations.push(TextLoc { 0, 0 });
    }

  Token* Punctuator::read() {

    auto T0 = Tokens.peek();

    if (llvm::isa<EndOfFile>(T0)) {
      if (Frames.size() == 1) {
        return T0;
      }
      auto Frame = Frames.top();
      Frames.pop();
      switch (Frame) {
        case FrameType::Block:
          return new BlockEnd(T0->getStartLoc());
        case FrameType::LineFold:
          return new LineFoldEnd(T0->getStartLoc());
      }
    }

    auto RefLoc = Locations.top();
    switch (Frames.top()) {
      case FrameType::LineFold:
      {
        if (T0->getStartLine() > RefLoc.Line
          && T0->getStartColumn() <= RefLoc.Column) {
            Frames.pop();
            Locations.pop();
            return new LineFoldEnd(T0->getStartLoc());
        }
        if (llvm::isa<Dot>(T0)) {
          auto T1 = Tokens.peek(1);
          if (T1->getStartLine() > T0->getEndLine()) {
              Tokens.get();
              Frames.push(FrameType::Block);
              return new BlockStart(T0->getStartLoc());
          }
        }
        return Tokens.get();
      }
      case FrameType::Block:
      {
        if (T0->getStartColumn() <= RefLoc.Column) {
          Frames.pop();
          return new BlockEnd(T0->getStartLoc());
        }

        Frames.push(FrameType::LineFold);
        Locations.push(T0->getStartLoc());

        return Tokens.get();
      }
    }


  }

}
