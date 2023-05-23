
// FIXME writeExcerpt does not work well with the last line in a file

#include <sstream>
#include <cmath>

#include "bolt/CST.hpp"
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

  template<typename T>
  T countDigits(T number) {
    if (number == 0) {
      return 1;
    }
    return std::ceil(std::log10(number+1));
  }

  Diagnostic::Diagnostic(DiagnosticKind Kind):
    std::runtime_error("a compiler error occurred without being caught"), Kind(Kind) {}

  static std::string describe(NodeKind Type) {
    switch (Type) {
      case NodeKind::Identifier:
        return "an identifier starting with a lowercase letter";
      case NodeKind::IdentifierAlt:
        return "an identifier starting with a capital letter";
      case NodeKind::CustomOperator:
        return "an operator";
      case NodeKind::IntegerLiteral:
        return "an integer literal";
      case NodeKind::EndOfFile:
        return "end-of-file";
      case NodeKind::BlockStart:
        return "the start of a new indented block";
      case NodeKind::BlockEnd:
        return "the end of the current indented block";
      case NodeKind::LineFoldEnd:
        return "the end of the current line-fold";
      case NodeKind::LParen:
        return "'('";
      case NodeKind::RParen:
        return "')'";
      case NodeKind::LBrace:
        return "'['";
      case NodeKind::RBrace:
        return "']'";
      case NodeKind::LBracket:
        return "'{'";
      case NodeKind::RBracket:
        return "'}'";
      case NodeKind::Colon:
        return "':'";
      case NodeKind::Comma:
        return "','";
      case NodeKind::Equals:
        return "'='";
      case NodeKind::StringLiteral:
        return "a string literal";
      case NodeKind::Dot:
        return "'.'";
      case NodeKind::DotDot:
        return "'..'";
      case NodeKind::Tilde:
        return "'~'";
      case NodeKind::RArrow:
        return "'->'";
      case NodeKind::RArrowAlt:
        return "'=>'";
      case NodeKind::PubKeyword:
        return "'pub'";
      case NodeKind::LetKeyword:
        return "'let'";
      case NodeKind::MutKeyword:
        return "'mut'";
      case NodeKind::MatchKeyword:
        return "'match'";
      case NodeKind::ReturnKeyword:
        return "'return'";
      case NodeKind::TypeKeyword:
        return "'type'";
      default:
        ZEN_UNREACHABLE
    }
  }

  std::string describe(const Type* Ty) {
    switch (Ty->getKind()) {
      case TypeKind::Var:
      {
        auto TV = static_cast<const TVar*>(Ty);
        if (TV->getVarKind() == VarKind::Rigid) {
          return static_cast<const TVarRigid*>(TV)->Name;
        }
        return "a" + std::to_string(TV->Id);
      }
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
      case TypeKind::Tuple:
      {
        std::ostringstream Out;
        auto Y = static_cast<const TTuple*>(Ty);
        Out << "(";
        if (Y->ElementTypes.size()) {
          auto Iter = Y->ElementTypes.begin();
          Out << describe(*Iter++);
          while (Iter != Y->ElementTypes.end()) {
            Out << ", " << describe(*Iter++);
          }
        }
        Out << ")";
        return Out.str();
      }
      case TypeKind::TupleIndex:
      {
        auto Y = static_cast<const TTupleIndex*>(Ty);
        return describe(Y->Ty) + "." + std::to_string(Y->I);
      }
    }
  }

  ConsoleDiagnostics::ConsoleDiagnostics(std::ostream& Out):
    Out(Out) {}

  void ConsoleDiagnostics::setForegroundColor(Color C) {
    if (EnableColors) {
      switch (C) {
        case Color::None:
          break;
        case Color::Black:
          Out << ANSI_FG_BLACK;
          break;
        case Color::White:
          Out << ANSI_FG_WHITE;
          break;
        case Color::Red:
          Out << ANSI_FG_RED;
          break;
        case Color::Yellow:
          Out << ANSI_FG_YELLOW;
          break;
        case Color::Green:
          Out << ANSI_FG_GREEN;
          break;
        case Color::Blue:
          Out << ANSI_FG_BLUE;
          break;
        case Color::Cyan:
          Out << ANSI_FG_CYAN;
          break;
        case Color::Magenta:
          Out << ANSI_FG_MAGENTA;
          break;
      }
    }
  }


  void ConsoleDiagnostics::setBackgroundColor(Color C) {
    if (EnableColors) {
      switch (C) {
        case Color::None:
          break;
        case Color::Black:
          Out << ANSI_BG_BLACK;
          break;
        case Color::White:
          Out << ANSI_BG_WHITE;
          break;
        case Color::Red:
          Out << ANSI_BG_RED;
          break;
        case Color::Yellow:
          Out << ANSI_BG_YELLOW;
          break;
        case Color::Green:
          Out << ANSI_BG_GREEN;
          break;
        case Color::Blue:
          Out << ANSI_BG_BLUE;
          break;
        case Color::Cyan:
          Out << ANSI_BG_CYAN;
          break;
        case Color::Magenta:
          Out << ANSI_BG_MAGENTA;
          break;
      }
    }
  }

  void ConsoleDiagnostics::setBold(bool Enable) {
    if (Enable) {
      Out << ANSI_BOLD;
    }
  }

  void ConsoleDiagnostics::setItalic(bool Enable) {
    if (Enable) {
      // TODO
    }
  }

  void ConsoleDiagnostics::setUnderline(bool Enable) {
    if (Enable) {
      Out << ANSI_UNDERLINE;
    }
  }

  void ConsoleDiagnostics::resetStyles() {
    if (EnableColors) {
      Out << ANSI_RESET;
    }
  }

  void ConsoleDiagnostics::writeGutter(
    std::size_t GutterWidth,
    std::size_t Line
  ) {
    auto LineNumberDigitCount = countDigits(Line);
    auto LeadingSpaces = GutterWidth - LineNumberDigitCount;
    Out << "  ";
    setForegroundColor(Color::Black);
    setBackgroundColor(Color::White);
    for (std::size_t i = 0; i < LeadingSpaces; i++) {
      Out << ' ';
    }
    Out << Line;
    resetStyles();
    Out << " ";
  }

  void ConsoleDiagnostics::writeHighlight(
    std::size_t GutterWidth,
    TextRange Range,
    Color HighlightColor,
    std::size_t Line,
    std::size_t LineLength
  ) {
    if (Line < Range.Start.Line || Range.End.Line < Line) {
      return;
    }
    Out << "  ";
    setBackgroundColor(Color::White);
    for (std::size_t i = 0; i < GutterWidth; i++) {
      Out << ' ';
    }
    resetStyles();
    Out << ' ';
    std::size_t start_column = Range.Start.Line == Line ? Range.Start.Column : 1;
    std::size_t end_column = Range.End.Line == Line ? Range.End.Column : LineLength+1;
    for (std::size_t i = 1; i < start_column; i++) {
      Out << ' ';
    }
    setForegroundColor(HighlightColor);
    for (std::size_t i = start_column; i < end_column; i++) {
      Out << '~';
    }
    resetStyles();
    Out << '\n';
  }

 void ConsoleDiagnostics::writeExcerpt(
    TextFile& File,
    TextRange ToPrint,
    TextRange ToHighlight,
    Color HighlightColor
  ) {

    auto Text = File.getText();
    auto StartPos = ToPrint.Start;
    auto EndPos = ToPrint.End;
    auto StartLine = StartPos.Line-1 > ExcerptLinesPre ? StartPos.Line - ExcerptLinesPost : 1;
    auto StartOffset = File.getStartOffset(StartLine);
    auto EndLine = std::min(File.getLineCount(), EndPos.Line + ExcerptLinesPost);
    auto EndOffset = File.getStartOffset(EndLine+1);
    auto GutterWidth = std::max<std::size_t>(2, countDigits(EndLine+1));
    auto HighlightStart = ToHighlight.Start;
    auto HighlightEnd = ToHighlight.End;
    auto HighlightRange = TextRange { HighlightStart, HighlightEnd };

    std::size_t CurrColumn = 1;
    std::size_t CurrLine = StartLine;
    writeGutter(GutterWidth, CurrLine);
    for (std::size_t i = StartOffset; i < EndOffset; i++) {
      auto C = Text[i];
      Out << C;
      if (C == '\n') {
        writeHighlight(GutterWidth, HighlightRange, HighlightColor, CurrLine, CurrColumn);
        if (CurrLine == EndLine && C == '\n') {
          break;
        }
        CurrLine++;
        writeGutter(GutterWidth, CurrLine);
        CurrColumn = 1;
      } else {
        CurrColumn++;
      }
    }

  }

  void ConsoleDiagnostics::addDiagnostic(const Diagnostic& D) {

    switch (D.getKind()) {

      case DiagnosticKind::BindingNotFound:
      {
        auto E = static_cast<const BindingNotFoundDiagnostic&>(D);
        Out << ANSI_BOLD ANSI_FG_RED "error: " ANSI_RESET "binding '" << E.Name << "' was not found\n\n";
        if (E.Initiator != nullptr) {
          auto Range = E.Initiator->getRange();
          //std::cerr << Range.Start.Line << ":" << Range.Start.Column << "-" << Range.End.Line << ":" << Range.End.Column << "\n";
          writeExcerpt(E.Initiator->getSourceFile()->getTextFile(), Range, Range, Color::Red);
          Out << "\n";
        }
        return;
      }

      case DiagnosticKind::UnexpectedToken:
      {
        auto E = static_cast<const UnexpectedTokenDiagnostic&>(D);
        setForegroundColor(Color::Red);
        setBold(true);
        Out << "error: ";
        resetStyles();
        setForegroundColor(Color::Yellow);
        Out << E.File.getPath() << ":" << E.Actual->getStartLine() << ":" << E.Actual->getStartColumn() << ":";
        resetStyles();
        Out << " expected ";
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
            NodeKind Prev = *Iter++;
            while (Iter != E.Expected.end()) {
              Out << ", " << describe(Prev);
              Prev = *Iter++;
            }
            Out << " or " << describe(Prev);
            break;
        }
        Out << " but instead got '" << E.Actual->getText() << "'\n\n";
        writeExcerpt(E.File, E.Actual->getRange(), E.Actual->getRange(), Color::Red);
        Out << "\n";
        return;
      }

      case DiagnosticKind::UnexpectedString:
      {
        auto E = static_cast<const UnexpectedStringDiagnostic&>(D);
        setForegroundColor(Color::Red);
        setBold(true);
        Out << "error: ";
        resetStyles();
        Out << E.File.getPath() << ":" << E.Location.Line << ":" << E.Location.Column << ": unexpected '";
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
        Out << "'\n\n";
        TextRange Range { E.Location, E.Location + E.Actual };
        writeExcerpt(E.File, Range, Range, Color::Red);
        Out << "\n";
        return;
      }

      case DiagnosticKind::UnificationError:
      {
        auto E = static_cast<const UnificationErrorDiagnostic&>(D);
        setForegroundColor(Color::Red);
        setBold(true);
        Out << "error: ";
        resetStyles();
        auto Left = E.Left->resolve(E.LeftPath);
        auto Right = E.Right->resolve(E.RightPath);
        Out << "the types " << ANSI_FG_GREEN << describe(Left) << ANSI_RESET
            << " and " << ANSI_FG_GREEN << describe(Right) << ANSI_RESET << " failed to match\n\n";
        if (E.Source) {
          auto Range = E.Source->getRange();
          writeExcerpt(E.Source->getSourceFile()->getTextFile(), Range, Range, Color::Red);
          Out << "\n";
        }
        if (!E.LeftPath.empty()) {
          setForegroundColor(Color::Yellow);
          setBold(true);
          Out << "  info: ";
          resetStyles();
          Out << "type " << ANSI_FG_GREEN << describe(Left) << ANSI_RESET << " occurs in the full type " << ANSI_FG_GREEN << describe(E.Left) << ANSI_RESET << "\n\n";
        }
        if (!E.RightPath.empty()) {
          setForegroundColor(Color::Yellow);
          setBold(true);
          Out << "  info: ";
          resetStyles();
          Out << "type " << ANSI_FG_GREEN << describe(Right) << ANSI_RESET << " occurs in the full type " << ANSI_FG_GREEN << describe(E.Right) << ANSI_RESET << "\n\n";
        }
        return;
      }

      case DiagnosticKind::TypeclassMissing:
      {
        auto E = static_cast<const TypeclassMissingDiagnostic&>(D);
        setForegroundColor(Color::Red);
        setBold(true);
        Out << "error: ";
        resetStyles();
        Out << "the type class " << ANSI_FG_YELLOW << E.Sig.Id;
        for (auto TV: E.Sig.Params) {
          Out << " " << describe(TV);
        }
        Out << ANSI_RESET << " is missing from the declaration's type signature\n\n";
        auto Range = E.Decl->getRange();
        writeExcerpt(E.Decl->getSourceFile()->getTextFile(), Range, Range, Color::Yellow);
        Out << "\n\n";
        return;
      }

      case DiagnosticKind::InstanceNotFound:
      {
        auto E = static_cast<const InstanceNotFoundDiagnostic&>(D);
        setForegroundColor(Color::Red);
        setBold(true);
        Out << "error: ";
        resetStyles();
        Out << "a type class instance " << ANSI_FG_YELLOW << E.TypeclassName << " " << describe(E.Ty) << ANSI_RESET " was not found.\n\n";
        auto Range = E.Source->getRange();
        //std::cerr << Range.Start.Line << ":" << Range.Start.Column << "-" << Range.End.Line << ":" << Range.End.Column << "\n";
        writeExcerpt(E.Source->getSourceFile()->getTextFile(), Range, Range, Color::Red);
        Out << "\n";
        return;
      }

      case DiagnosticKind::ClassNotFound:
      {
        auto E = static_cast<const ClassNotFoundDiagnostic&>(D);
        setForegroundColor(Color::Red);
        setBold(true);
        Out << "error: ";
        resetStyles();
        Out << "the type class " << ANSI_FG_YELLOW << E.Name << ANSI_RESET " was not found.\n\n";
        return;
      }

    }

    ZEN_UNREACHABLE
  }

}
