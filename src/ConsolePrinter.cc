
// FIXME writeExcerpt does not work well with the last line in a file

#include <functional>
#include <cmath>

#include "bolt/CST.hpp"
#include "bolt/Type.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/ConsolePrinter.hpp"

#define ANSI_RESET "\u001b[0m"
#define ANSI_BOLD "\u001b[1m"
#define ANSI_ITALIC "\u001b[3m"
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
    case NodeKind::Assignment:
      return "an assignment such as := or +=";
    case NodeKind::ExpressionAnnotation:
      return "a user-defined annotation";
    case NodeKind::TypeAssertAnnotation:
      return "a built-in annotation for a type assertion";
    case NodeKind::TypeclassConstraintExpression:
      return "a type class constraint";
    case NodeKind::EqualityConstraintExpression:
      return "an equality constraint";
    case NodeKind::QualifiedTypeExpression:
      return "a type expression with some constraints";
    case NodeKind::ReferenceTypeExpression:
      return "a reference to another type";
    case NodeKind::ArrowTypeExpression:
      return "a function type signature";
    case NodeKind::AppTypeExpression:
      return "an application of one type to another";
    case NodeKind::VarTypeExpression:
      return "a rigid variable";
    case NodeKind::NestedTypeExpression:
      return "a type expression wrapped in '(' and ')'";
    case NodeKind::TupleTypeExpression:
      return "a tuple type expression";
    case NodeKind::BindPattern:
      return "a variable binder";
    case NodeKind::NamedTuplePattern:
      return "a pattern for a variant member";
    case NodeKind::TuplePattern:
      return "a pattern for a tuple";
    case NodeKind::ListPattern:
      return "a pattern for a list";
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
    case NodeKind::At:
      return "'@'";
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
    case NodeKind::ForeignKeyword:
      return "'foreign'";
    case NodeKind::MutKeyword:
      return "'mut'";
    case NodeKind::MatchKeyword:
      return "'match'";
    case NodeKind::ReturnKeyword:
      return "'return'";
    case NodeKind::TypeKeyword:
      return "'type'";
    case NodeKind::IfKeyword:
      return "'if'";
    case NodeKind::ElifKeyword:
      return "'elif'";
    case NodeKind::ElseKeyword:
      return "'else'";
    case NodeKind::StructKeyword:
      return "'struct'";
    case NodeKind::EnumKeyword:
      return "'enum'";
    case NodeKind::ClassKeyword:
      return "'class'";
    case NodeKind::InstanceKeyword:
      return "'instance'";
    case NodeKind::PrefixFunctionDeclaration:
    case NodeKind::InfixFunctionDeclaration:
    case NodeKind::SuffixFunctionDeclaration:
    case NodeKind::NamedFunctionDeclaration:
      return "a let-declaration";
    case NodeKind::VariableDeclaration:
      return "a let-declaration";
    case NodeKind::CallExpression:
      return "a call-expression";
    case NodeKind::InfixExpression:
      return "an infix-expression";
    case NodeKind::ReferenceExpression:
      return "a reference to a function or variable";
    case NodeKind::MatchExpression:
      return "a match-expression";
    case NodeKind::LiteralExpression:
      return "a literal expression";
    case NodeKind::MemberExpression:
      return "an accessor of a member";
    case NodeKind::IfStatement:
      return "an if-statement";
    case NodeKind::IfStatementPart:
      return "a branch of an if-statement";
    case NodeKind::VariantDeclaration:
      return "a variant";
    case NodeKind::MatchCase:
      return "a match-arm";
    case NodeKind::LetExprBody:
      return "the body of a let-declaration";
    default:
      ZEN_UNREACHABLE
  }
}

static std::string describe(Token* T) {
  switch (T->getKind()) {
    case NodeKind::LineFoldEnd:
    case NodeKind::BlockStart:
    case NodeKind::BlockEnd:
    case NodeKind::EndOfFile:
      return describe(T->getKind());
    default:
      return "'" + T->getText() + "'";
  }
}

void writeForegroundANSI(Color C, std::ostream& Out) {
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

void writeBackgroundANSI(Color C, std::ostream& Out) {
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

ConsolePrinter::ConsolePrinter(std::ostream& Out):
  Out(Out) {}

void ConsolePrinter::setForegroundColor(Color C) {
  ActiveStyle.setForegroundColor(C);
  if (!EnableColors) {
    return;
  }
  writeForegroundANSI(C, Out);
}

void ConsolePrinter::setBackgroundColor(Color C) {
  ActiveStyle.setBackgroundColor(C);
  if (!EnableColors) {
    return;
  }
  if (C == Color::None) {
    Out << ANSI_RESET;
    applyStyles();
  }
  writeBackgroundANSI(C, Out);
}

void ConsolePrinter::applyStyles() {
  if (ActiveStyle.isBold()) {
    Out << ANSI_BOLD;
  }
  if (ActiveStyle.isUnderline()) {
    Out << ANSI_UNDERLINE;
  }
  if (ActiveStyle.isItalic()) {
    Out << ANSI_ITALIC;
  }
  if (ActiveStyle.hasBackgroundColor()) {
    setBackgroundColor(ActiveStyle.getBackgroundColor());
  }
  if (ActiveStyle.hasForegroundColor()) {
    setForegroundColor(ActiveStyle.getForegroundColor());
  }
}

void ConsolePrinter::setBold(bool Enable) {
  ActiveStyle.setBold(Enable);
  if (!EnableColors) {
    return;
  }
  if (Enable) {
    Out << ANSI_BOLD;
  } else {
    Out << ANSI_RESET;
    applyStyles();
  }
}

void ConsolePrinter::setItalic(bool Enable) {
  ActiveStyle.setItalic(Enable);
  if (!EnableColors) {
    return;
  }
  if (Enable) {
    Out << ANSI_ITALIC;
  } else {
    Out << ANSI_RESET;
    applyStyles();
  }
}

void ConsolePrinter::setUnderline(bool Enable) {
  ActiveStyle.setItalic(Enable);
  if (!EnableColors) {
    return;
  }
  if (Enable) {
    Out << ANSI_UNDERLINE;
  } else {
    Out << ANSI_RESET;
    applyStyles();
  }
}

void ConsolePrinter::resetStyles() {
  ActiveStyle.reset();
  if (EnableColors) {
    Out << ANSI_RESET;
  }
}

void ConsolePrinter::writeGutter(
  std::size_t GutterWidth,
  std::string Text
) {
  ZEN_ASSERT(Text.size() <= GutterWidth);
  auto LeadingSpaces = GutterWidth - Text.size();
  Out << "  ";
  setForegroundColor(Color::Black);
  setBackgroundColor(Color::White);
  for (std::size_t i = 0; i < LeadingSpaces; i++) {
    Out << ' ';
  }
  Out << Text;
  resetStyles();
  Out << " ";
}

void ConsolePrinter::writeHighlight(
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
  if (start_column == end_column) {
    Out << "↖";
  } else {
    for (std::size_t i = start_column; i < end_column; i++) {
      Out << '~';
    }
  }
  resetStyles();
  Out << '\n';
}

void ConsolePrinter::writeExcerpt(
  const TextFile& File,
  TextRange ToPrint,
  TextRange ToHighlight,
  Color HighlightColor
) {

  auto LineCount = File.getLineCount();
  auto Text = File.getText();
  auto StartPos = ToPrint.Start;
  auto EndPos = ToPrint.End;
  auto StartLine = StartPos.Line-1 > ExcerptLinesPre ? StartPos.Line - ExcerptLinesPre : 1;
  auto StartOffset = File.getStartOffsetOfLine(StartLine);
  auto EndLine = std::min(LineCount, EndPos.Line + ExcerptLinesPost);
  auto EndOffset = File.getEndOffsetOfLine(EndLine);
  auto GutterWidth = std::max<std::size_t>(2, countDigits(EndLine+1));
  auto HighlightStart = ToHighlight.Start;
  auto HighlightEnd = ToHighlight.End;
  auto HighlightRange = TextRange { HighlightStart, HighlightEnd };

  std::size_t CurrColumn = 1;
  std::size_t CurrLine = StartLine;
  bool AtBlankLine = true;
  for (std::size_t I = StartOffset; I < EndOffset; I++) {
    auto C = Text[I];
    if (AtBlankLine) {
      writeGutter(GutterWidth, std::to_string(CurrLine));
    }
    if (C == '\n') {
      Out << C;
      writeHighlight(GutterWidth, HighlightRange, HighlightColor, CurrLine, CurrColumn);
      CurrLine++;
      CurrColumn = 1;
      AtBlankLine = true;
    } else {
      AtBlankLine = false;
      Out << C;
      CurrColumn++;
    }
  }

}

void ConsolePrinter::write(const std::string_view& S) {
  Out << S;
}

void ConsolePrinter::write(char C) {
  Out << C;
}

void ConsolePrinter::write(std::size_t I) {
  Out << I;
}

void ConsolePrinter::writeBinding(const ByteString& Name) {
  write("'");
  write(Name);
  write("'");
}

void ConsolePrinter::writeNode(const Node* N) {
  auto Range = N->getRange();
  writeExcerpt(N->getSourceFile()->getTextFile(), Range, Range, Color::Red);
}

void ConsolePrinter::writeLoc(const TextFile& File, const TextLoc& Loc) {
  setForegroundColor(Color::Yellow);
  write(File.getPath());
  write(":");
  write(Loc.Line);
  write(":");
  write(Loc.Column);
  write(":");
  resetStyles();
}

void ConsolePrinter::writePrefix(const Diagnostic& D) {
  setForegroundColor(Color::Red);
  setBold(true);
  write("error: ");
  resetStyles();
}

void ConsolePrinter::writeType(Type* Ty) {
  std::function<void(Type*)> visit = [&](auto Ty) {
    switch (Ty->getKind()) {
      case TypeKind::Var:
      {
        auto T = static_cast<TVar*>(Ty);
        // FIXME
        write("α");
        break;
      }
      case TypeKind::Con:
      {
        auto T = static_cast<TCon*>(Ty);
        write(T->getName());
        break;
      }
      case TypeKind::Fun:
      {
        auto T = static_cast<TFun*>(Ty);
        visit(T->getLeft());
        write(" -> ");
        visit(T->getRight());
        break;
      }
      case TypeKind::App:
      {
        auto T = static_cast<TApp*>(Ty);
        visit(T->getLeft());
        write(" ");
        visit(T->getRight());
        break;
      }
    }
  };
  setForegroundColor(Color::Green);
  visit(Ty);
  resetStyles();
}

void ConsolePrinter::writeDiagnostic(const Diagnostic& D) {

  switch (D.getKind()) {

    case DiagnosticKind::BindingNotFound:
    {
      auto& E = static_cast<const BindingNotFoundDiagnostic&>(D);
      writePrefix(E);
      write("binding ");
      writeBinding(E.Name);
      write(" was not found\n\n");
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
      auto& E = static_cast<const UnexpectedTokenDiagnostic&>(D);
      writePrefix(E);
      writeLoc(E.File, E.Actual->getStartLoc());
      write(" expected ");
      switch (E.Expected.size()) {
        case 0:
          write("nothing");
          break;
        case 1:
          write(describe(E.Expected[0]));
          break;
        default:
          auto Iter = E.Expected.begin();
          Out << describe(*Iter++);
          NodeKind Prev = *Iter++;
          while (Iter != E.Expected.end()) {
            write(", ");
            write(describe(Prev));
            Prev = *Iter++;
          }
          write(" or ");
          write(describe(Prev));
          break;
      }
      write(" but instead got ");
      write(describe(E.Actual));
      write("\n\n");
      writeExcerpt(E.File, E.Actual->getRange(), E.Actual->getRange(), Color::Red);
      write("\n");
      return;
    }

    case DiagnosticKind::UnexpectedString:
    {
      auto& E = static_cast<const UnexpectedStringDiagnostic&>(D);
      writePrefix(E);
      writeLoc(E.File, E.Location);
      write(" unexpected '");
      for (auto Chr: E.Actual) {
        switch (Chr) {
          case '\\':
            write("\\\\");
            break;
          case '\'':
            write("\\'");
            break;
          default:
            write(Chr);
            break;
        }
      }
      write("'\n\n");
      TextRange Range { E.Location, E.Location + E.Actual };
      writeExcerpt(E.File, Range, Range, Color::Red);
      write("\n");
      return;
    }

    case DiagnosticKind::TypeMismatchError:
    {
      auto& E = static_cast<const TypeMismatchError&>(D);
      // auto Left = E.OrigLeft->resolve(E.LeftPath);
      // auto Right = E.OrigRight->resolve(E.RightPath);
      auto Left = E.Left;
      auto Right = E.Right;
      auto S = E.getNode();
      writePrefix(E);
      write("the types ");
      writeType(Left);
      write(" and ");
      writeType(Right);
      write(" failed to match\n\n");
      setForegroundColor(Color::Yellow);
      setBold(true);
      write("  info: ");
      resetStyles();
      write("due to an equality constraint on ");
      write(describe(S->getKind()));
      write(":\n\n");
      // write(" - left type ");
      // writeType(E.OrigLeft, E.LeftPath);
      // write("\n");
      // write(" - right type ");
      // writeType(E.OrigRight, E.RightPath);
      // write("\n\n");
      writeNode(S);
      write("\n");
      // if (E.Left != E.OrigLeft) {
      //   setForegroundColor(Color::Yellow);
      //   setBold(true);
      //   write("  info: ");
      //   resetStyles();
      //   write("the type ");
      //   writeType(E.Left);
      //   write(" occurs in the full type ");
      //   writeType(E.OrigLeft);
      //   write("\n\n");
      // }
      // if (E.Right != E.OrigRight) {
      //   setForegroundColor(Color::Yellow);
      //   setBold(true);
      //   write("  info: ");
      //   resetStyles();
      //   write("the type ");
      //   writeType(E.Right);
      //   write(" occurs in the full type ");
      //   writeType(E.OrigRight);
      //   write("\n\n");
      // }
      return;
    }

  }

  ZEN_UNREACHABLE

}

}
