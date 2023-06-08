
// FIXME writeExcerpt does not work well with the last line in a file

#include <sstream>
#include <cmath>

#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/Type.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"

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

  Diagnostic::Diagnostic(DiagnosticKind Kind):
    std::runtime_error("a compiler error occurred without being caught"), Kind(Kind) {}

  bool sourceLocLessThan(const Diagnostic* L, const Diagnostic* R) {
    auto N1 = L->getNode();
    auto N2 = R->getNode();
    if (N1 == nullptr && N2 == nullptr) {
      return false;
    }
    if (N1 == nullptr) {
      return true;
    }
    if (N2 == nullptr) {
      return false;
    }
    return N1->getStartLine() < N2->getStartLine() || N1->getStartColumn() < N2->getStartColumn();
  };

  void DiagnosticStore::sort() {
    std::sort(Diagnostics.begin(), Diagnostics.end(), sourceLocLessThan);
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
      case NodeKind::ReferenceTypeExpression:
        return "a type reference";
      case NodeKind::LetDeclaration:
        return "a let-declaration";
      case NodeKind::CallExpression:
        return "a call-expression";
      case NodeKind::InfixExpression:
        return "an infix-expression";
      case NodeKind::ReferenceExpression:
        return "a function or variable reference";
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
      case NodeKind::ListPattern:
        return "a list pattern";
      case NodeKind::TypeAssertAnnotation:
        return "an annotation for a type assertion";
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
        Out << describe(Y->ParamType) << " -> " << describe(Y->ReturnType);
        return Out.str();
      }
      case TypeKind::Con:
      {
        auto Y = static_cast<const TCon*>(Ty);
        return Y->DisplayName;
      }
      case TypeKind::App:
      {
        auto Y = static_cast<const TApp*>(Ty);
        return describe(Y->Op) + " " + describe(Y->Arg);
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
      case TypeKind::Nil:
        return "{}";
      case TypeKind::Absent:
        return "Abs";
      case TypeKind::Present:
      {
        auto Y = static_cast<const TPresent*>(Ty);
        return describe(Y->Ty);
      }
      case TypeKind::Field:
      {
        auto Y = static_cast<const TField*>(Ty);
        std::ostringstream out;
        out << "{ " << Y->Name << ": " << describe(Y->Ty);
        Ty = Y->RestTy;
        while (Ty->getKind() == TypeKind::Field) {
          auto Y = static_cast<const TField*>(Ty);
          out << "; " + Y->Name + ": " + describe(Y->Ty);
          Ty = Y->RestTy;
        }
        if (Ty->getKind() != TypeKind::Nil) {
          out << "; " + describe(Ty);
        }
        out << " }";
        return out.str();
      }
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

  DiagnosticStore::~DiagnosticStore() {
    // for (auto D: Diagnostics) {
    //   delete D;
    // }
  }

  ConsoleDiagnostics::ConsoleDiagnostics(std::ostream& Out):
    Out(Out) {}

  void ConsoleDiagnostics::setForegroundColor(Color C) {
    ActiveStyle.setForegroundColor(C);
    if (!EnableColors) {
      return;
    }
    writeForegroundANSI(C, Out);
  }

  void ConsoleDiagnostics::setBackgroundColor(Color C) {
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

  void ConsoleDiagnostics::applyStyles() {
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

  void ConsoleDiagnostics::setBold(bool Enable) {
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

  void ConsoleDiagnostics::setItalic(bool Enable) {
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

  void ConsoleDiagnostics::setUnderline(bool Enable) {
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

  void ConsoleDiagnostics::resetStyles() {
    ActiveStyle.reset();
    if (EnableColors) {
      Out << ANSI_RESET;
    }
  }

  void ConsoleDiagnostics::writeGutter(
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

 void ConsoleDiagnostics::writeExcerpt(
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

  void ConsoleDiagnostics::write(const std::string_view& S) {
    Out << S;
  }

  void ConsoleDiagnostics::write(char C) {
    Out << C;
  }

  void ConsoleDiagnostics::write(std::size_t I) {
    Out << I;
  }

  void ConsoleDiagnostics::writeBinding(const ByteString& Name) {
    write("'");
    write(Name);
    write("'");
  }

  void ConsoleDiagnostics::writeType(const Type* Ty) {
    TypePath Path;
    writeType(Ty, Path);
  }

  void ConsoleDiagnostics::writeType(const Type* Ty, const TypePath& Underline) {

    setForegroundColor(Color::Green);

    class TypePrinter : public ConstTypeVisitor {

      TypePath Path;
      ConsoleDiagnostics& W;
      const TypePath& Underline;

    public:

      TypePrinter(ConsoleDiagnostics& W, const TypePath& Underline):
        W(W), Underline(Underline) {}

      bool shouldUnderline() const {
        return !Underline.empty() && Path == Underline;
      }

      void enterType(const Type* Ty) override {
        if (shouldUnderline()) {
          W.setUnderline(true);
        }
      }

      void exitType(const Type* Ty) override {
        if (shouldUnderline()) {
          W.setUnderline(false);
        }
      }

      void visitAppType(const TApp *Ty) override {
        auto Y = static_cast<const TApp*>(Ty);
        Path.push_back(TypeIndex::forAppOpType());
        visit(Y->Op);
        Path.pop_back();
        W.write(" ");
        Path.push_back(TypeIndex::forAppArgType());
        visit(Y->Arg);
        Path.pop_back();
      }

      void visitVarType(const TVar* Ty) override {
        if (Ty->getVarKind() == VarKind::Rigid) {
          W.write(static_cast<const TVarRigid*>(Ty)->Name);
          return;
        }
        W.write("a");
        W.write(Ty->Id);
      }

      void visitConType(const TCon *Ty) override {
        W.write(Ty->DisplayName);
      }

      void visitArrowType(const TArrow* Ty) override {
        Path.push_back(TypeIndex::forArrowParamType());
        visit(Ty->ParamType);
        Path.pop_back();
        W.write(" -> ");
        Path.push_back(TypeIndex::forArrowReturnType());
        visit(Ty->ReturnType);
        Path.pop_back();
      }

      void visitTupleType(const TTuple *Ty) override {
        W.write("(");
        if (Ty->ElementTypes.size()) {
          auto Iter = Ty->ElementTypes.begin();
          Path.push_back(TypeIndex::forTupleElement(0));
          visit(*Iter++);
          Path.pop_back();
          std::size_t I = 1;
          while (Iter != Ty->ElementTypes.end()) {
            W.write(", ");
            Path.push_back(TypeIndex::forTupleElement(I++));
            visit(*Iter++);
            Path.pop_back();
          }
        }
        W.write(")");
      }

      void visitTupleIndexType(const TTupleIndex *Ty) override {
        Path.push_back(TypeIndex::forTupleIndexType());
        visit(Ty->Ty);
        Path.pop_back();
        W.write(".");
        W.write(Ty->I);
      }

      void visitNilType(const TNil *Ty) override {
        W.write("{}");
      }

      void visitAbsentType(const TAbsent *Ty) override {
        W.write("Abs");
      }

      void visitPresentType(const TPresent *Ty) override {
        Path.push_back(TypeIndex::forPresentType());
        visit(Ty->Ty);
        Path.pop_back();
      }

      void visitFieldType(const TField* Ty) override {
        W.write("{ ");
        W.write(Ty->Name);
        W.write(": ");
        Path.push_back(TypeIndex::forFieldType());
        visit(Ty->Ty);
        Path.pop_back();
        auto Ty2 = Ty->RestTy;
        Path.push_back(TypeIndex::forFieldRest());
        std::size_t I = 1;
        while (Ty2->getKind() == TypeKind::Field) {
          auto Y = static_cast<const TField*>(Ty2);
          W.write("; ");
          W.write(Y->Name);
          W.write(": ");
          Path.push_back(TypeIndex::forFieldType());
          visit(Y->Ty);
          Path.pop_back();
          Ty2 = Y->RestTy;
          Path.push_back(TypeIndex::forFieldRest());
          ++I;
        }
        if (Ty2->getKind() != TypeKind::Nil) {
          W.write("; ");
          visit(Ty2);
        }
        W.write(" }");
        for (auto K = 0; K < I; K++) {
          Path.pop_back();
        }
      }

    };

    TypePrinter P { *this, Underline };
    P.visit(Ty);

    resetStyles();
  }

  void ConsoleDiagnostics::writeType(std::size_t I) {
    setForegroundColor(Color::Green);
    write(I);
    resetStyles();
  }

  void ConsoleDiagnostics::writeNode(const Node* N) {
    auto Range = N->getRange();
    writeExcerpt(N->getSourceFile()->getTextFile(), Range, Range, Color::Red);
  }

  void ConsoleDiagnostics::writeLoc(const TextFile& File, const TextLoc& Loc) {
    setForegroundColor(Color::Yellow);
    write(File.getPath());
    write(":");
    write(Loc.Line);
    write(":");
    write(Loc.Column);
    write(":");
    resetStyles();
  }

  void ConsoleDiagnostics::writePrefix(const Diagnostic& D) {
    setForegroundColor(Color::Red);
    setBold(true);
    write("error: ");
    resetStyles();
  }

  void ConsoleDiagnostics::writeTypeclassName(const ByteString& Name) {
    setForegroundColor(Color::Magenta);
    write(Name);
    resetStyles();
  }

  void ConsoleDiagnostics::writeTypeclassSignature(const TypeclassSignature& Sig) {
    setForegroundColor(Color::Magenta);
    write(Sig.Id);
    for (auto TV: Sig.Params) {
      write(" ");
      write(describe(TV));
    }
    resetStyles();
  }

  void ConsoleDiagnostics::addDiagnostic(Diagnostic* D) {

    printDiagnostic(*D);

    // Since this DiagnosticEngine is expected to own the diagnostic, we simply
    // destroy the processed diagnostic so that there are no memory leaks.
    delete D;
  }

  void ConsoleDiagnostics::printDiagnostic(const Diagnostic& D) {

    switch (D.getKind()) {

      case DiagnosticKind::BindingNotFound:
      {
        auto E = static_cast<const BindingNotFoundDiagnostic&>(D);
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
        break;
      }

      case DiagnosticKind::UnexpectedToken:
      {
        auto E = static_cast<const UnexpectedTokenDiagnostic&>(D);
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
        break;
      }

      case DiagnosticKind::UnexpectedString:
      {
        auto E = static_cast<const UnexpectedStringDiagnostic&>(D);
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
        break;
      }

      case DiagnosticKind::UnificationError:
      {
        auto E = static_cast<const UnificationErrorDiagnostic&>(D);
        auto Left = E.OrigLeft->resolve(E.LeftPath);
        auto Right = E.OrigRight->resolve(E.RightPath);
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
        write(describe(E.Source->getKind()));
        write(":\n\n");
        // write(" - left type ");
        // writeType(E.OrigLeft, E.LeftPath);
        // write("\n");
        // write(" - right type ");
        // writeType(E.OrigRight, E.RightPath);
        // write("\n\n");
        writeNode(E.Source);
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
        break;
      }

      case DiagnosticKind::TypeclassMissing:
      {
        auto E = static_cast<const TypeclassMissingDiagnostic&>(D);
        writePrefix(E);
        write("the type class ");
        writeTypeclassSignature(E.Sig);
        write(" is missing from the declaration's type signature\n\n");
        writeNode(E.Decl);
        write("\n\n");
        break;
      }

      case DiagnosticKind::InstanceNotFound:
      {
        auto E = static_cast<const InstanceNotFoundDiagnostic&>(D);
        writePrefix(E);
        write("a type class instance ");
        writeTypeclassName(E.TypeclassName);
        write(" ");
        writeType(E.Ty);
        write(" was not found.\n\n");
        writeNode(E.Source);
        write("\n");
        break;
      }

      case DiagnosticKind::TupleIndexOutOfRange:
      {
        auto E = static_cast<const TupleIndexOutOfRangeDiagnostic&>(D);
        writePrefix(E);
        write("the index ");
        writeType(E.I);
        write(" is out of range for tuple ");
        writeType(E.Tuple);
        break;
      }

      case DiagnosticKind::InvalidTypeToTypeclass:
      {
        auto E = static_cast<const InvalidTypeToTypeclassDiagnostic&>(D);
        writePrefix(E);
        write("the type ");
        writeType(E.Actual);
        write(" was applied to type class names ");
        bool First = true;
        for (auto Class: E.Classes) {
          if (First) First = false;
          else write(", ");
          writeTypeclassName(Class);
        }
        write(" but this is invalid\n\n");
        break;
      }

      case DiagnosticKind::FieldNotFound:
      {
        auto E = static_cast<const FieldNotFoundDiagnostic&>(D);
        writePrefix(E);
        write("the field '");
        write(E.Name);
        write("' was required in one type but not found in another\n\n");
        writeNode(E.Source);
        write("\n");
        break;
      }

    }

  }

}
