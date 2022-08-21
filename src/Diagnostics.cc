
#include <sstream>

#include "zen/config.hpp"

#include "bolt/Diagnostics.hpp"
#include "bolt/Checker.hpp"

#define ANSI_RESET "\u001b[0m"
#define ANSI_BOLD "\u001b[1m"
#define ANSI_UNDERLINE "\u001b[4m"
#define ANSI_REVERSED "\u001b[7m"

#define ANSI_FG_BLACK "\u001b[30m"
#define ANSI_FG_RED "\u001b[31m"
#define ANSI_FG_GREEN "\u001b[32m"
#define ANSI_FG_YELLOW "\u001b[33m"
#define ANSI_FG_BLUE "\u001b[34m"
#define ANSI_FG_CYAN "\u001b[35m"
#define ANSI_FG_MAGENTA "\u001b[36m"
#define ANSI_FG_WHITE "\u001b[37m"

#define ANSI_BG_BLACK "\u001b[40m"
#define ANSI_BG_RED "\u001b[41m"
#define ANSI_BG_GREEN "\u001b[42m"
#define ANSI_BG_YELLOW "\u001b[43m"
#define ANSI_BG_BLUE "\u001b[44m"
#define ANSI_BG_CYAN "\u001b[45m"
#define ANSI_BG_MAGENTA "\u001b[46m"
#define ANSI_BG_WHITE "\u001b[47m"

namespace bolt {

  Diagnostic::Diagnostic(DiagnosticKind Kind):
    std::runtime_error("a compiler error occurred without being caught"), Kind(Kind) {}

  static std::string describe(NodeType Type) {
    switch (Type) {
      case NodeType::Identifier:
        return "an identifier";
      case NodeType::CustomOperator:
        return "an operator";
      case NodeType::IntegerLiteral:
        return "an integer literal";
      case NodeType::EndOfFile:
        return "end-of-file";
      case NodeType::BlockStart:
        return "the start of a new indented block";
      case NodeType::BlockEnd:
        return "the end of the current indented block";
      case NodeType::LineFoldEnd:
        return "the end of the current line-fold";
      case NodeType::LParen:
        return "'('";
      case NodeType::RParen:
        return "')'";
      case NodeType::LBrace:
        return "'['";
      case NodeType::RBrace:
        return "']'";
      case NodeType::LBracket:
        return "'{'";
      case NodeType::RBracket:
        return "'}'";
      case NodeType::Colon:
        return "':'";
      case NodeType::Equals:
        return "'='";
      case NodeType::StringLiteral:
        return "a string literal";
      case NodeType::Dot:
        return "'.'";
      case NodeType::PubKeyword:
        return "'pub'";
      case NodeType::LetKeyword:
        return "'let'";
      case NodeType::MutKeyword:
        return "'mut'";
      case NodeType::ReturnKeyword:
        return "'return'";
      case NodeType::TypeKeyword:
        return "'type'";
      default:
        ZEN_UNREACHABLE
    }
  }

  static std::string describe(const Type* Ty) {
    switch (Ty->getKind()) {
      case TypeKind::Any:
        return "any";
      case TypeKind::Var:
        return "a" + std::to_string(static_cast<const TVar*>(Ty)->Id);
      case TypeKind::Arrow:
      {
        auto Y = static_cast<const TArrow*>(Ty);
        std::ostringstream Out;
        Out << "(";
        bool First = true;
        for (auto PT: Y->ParamTypes) {
          if (First) First = false;
          else Out << ", ";
          Out << describe(PT);
        }
        Out << ") -> " << describe(Y->ReturnType);
        return Out.str();
      }
      case TypeKind::Con:
      {
        auto Y = static_cast<const TCon*>(Ty);
        std::ostringstream Out;
        if (!Y->DisplayName.empty()) {
          Out << Y->DisplayName;
        } else {
          Out << "C" << Y->Id;
        }
        for (auto Arg: Y->Args) {
          Out << " " << describe(Arg);
        }
        return Out.str();
      }
    }
  }


  ConsoleDiagnostics::ConsoleDiagnostics(std::ostream& Out):
    Out(Out) {}

  void ConsoleDiagnostics::addDiagnostic(const Diagnostic& D) {

    switch (D.getKind()) {

      case DiagnosticKind::BindingNotFound:
      {
        auto E = static_cast<const BindingNotFoundDiagnostic&>(D);
        Out << ANSI_BOLD ANSI_FG_RED "error: " ANSI_RESET "binding '" << E.Name << "' was not found\n";
        //if (E.Initiator != nullptr) {
        //  writeExcerpt(E.Initiator->getRange());
        //}
        break;
      }

      case DiagnosticKind::UnexpectedToken:
      {
        auto E = static_cast<const UnexpectedTokenDiagnostic&>(D);
        Out << "<unknown.bolt>:" << E.Actual->getStartLine() << ":" << E.Actual->getStartColumn() << ": expected ";
        switch (E.Expected.size()) {
          case 0:
            Out << "nothing";
            break;
          case 1:
            Out << describe(E.Expected[0]);
            break;
          default:
            auto Iter = E.Expected.begin();
            Out << describe(*Iter++);
            NodeType Prev;
            while (Iter != E.Expected.end()) {
              Out << ", " << describe(Prev);
              Prev = *Iter++;
            }
            Out << " or " << describe(Prev);
            break;
        }
        Out << " but instead got '" << E.Actual->getText() << "'\n";
        break;
      }

      case DiagnosticKind::UnexpectedString:
      {
        auto E = static_cast<const UnexpectedStringDiagnostic&>(D);
        Out << "<unknown.bolt>:" << E.Location.Line << ":" << E.Location.Column << ": unexpected '";
        for (auto Chr: E.Actual) {
          switch (Chr) {
            case '\\':
              Out << "\\\\";
              break;
            case '\'':
              Out << "\\'";
              break;
            default:
              Out << Chr;
              break;
          }
        }
        break;
      }

      case DiagnosticKind::UnificationError:
      {
        auto E = static_cast<const UnificationErrorDiagnostic&>(D);
        Out << ANSI_FG_RED << ANSI_BOLD << "error: " << ANSI_RESET << "the types " << ANSI_FG_GREEN << describe(E.Left) << ANSI_RESET
            << " and " << ANSI_FG_GREEN << describe(E.Right) << ANSI_RESET << " failed to match\n";
        break;
      }

    }

  }

}
