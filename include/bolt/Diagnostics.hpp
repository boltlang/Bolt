
#pragma once

#include <vector>
#include <stdexcept>
#include <memory>
#include <iostream>

#include "bolt/ByteString.hpp"
#include "bolt/String.hpp"
#include "bolt/CST.hpp"

namespace bolt {

  class Type;

  enum class DiagnosticKind : unsigned char {
    UnexpectedToken,
    UnexpectedString,
    BindingNotFound,
    UnificationError,
  };

  class Diagnostic : std::runtime_error {

    const DiagnosticKind Kind;

  protected:

    Diagnostic(DiagnosticKind Kind);
  
  public:

    DiagnosticKind getKind() const noexcept {
      return Kind;
    }

  };

  class UnexpectedTokenDiagnostic : public Diagnostic {
  public:

    TextFile& File;
    Token* Actual;
    std::vector<NodeType> Expected;

    inline UnexpectedTokenDiagnostic(TextFile& File, Token* Actual, std::vector<NodeType> Expected):
      Diagnostic(DiagnosticKind::UnexpectedToken), File(File), Actual(Actual), Expected(Expected) {}

  };

  class UnexpectedStringDiagnostic : public Diagnostic {
  public:

    TextFile& File;
    TextLoc Location;
    String Actual;

    inline UnexpectedStringDiagnostic(TextFile& File, TextLoc Location, String Actual):
      Diagnostic(DiagnosticKind::UnexpectedString), File(File), Location(Location), Actual(Actual) {}

  };

  class BindingNotFoundDiagnostic : public Diagnostic {
  public:

    ByteString Name;
    Node* Initiator;

    inline BindingNotFoundDiagnostic(ByteString Name, Node* Initiator):
      Diagnostic(DiagnosticKind::BindingNotFound), Name(Name), Initiator(Initiator) {}

  };

  class UnificationErrorDiagnostic : public Diagnostic {
  public:
    
    Type* Left;
    Type* Right;
    Node* Source;

    inline UnificationErrorDiagnostic(Type* Left, Type* Right, Node* Source):
      Diagnostic(DiagnosticKind::UnificationError), Left(Left), Right(Right), Source(Source) {}

  };

  class DiagnosticEngine {
  protected:

  public:

    virtual void addDiagnostic(const Diagnostic& Diagnostic) = 0;

    template<typename D, typename ...Ts>
    void add(Ts&&... Args) {
      D Diag { std::forward<Ts>(Args)... };
      addDiagnostic(Diag);
    }

    virtual ~DiagnosticEngine() {}

  };

  enum class Color {
    None,
    Black,
    White,
    Red,
    Yellow,
    Green,
    Blue,
    Cyan,
    Magenta,
  };

  class ConsoleDiagnostics : public DiagnosticEngine {

    std::ostream& Out;

    void setForegroundColor(Color C);
    void setBackgroundColor(Color C);
    void setBold(bool Enable);
    void setItalic(bool Enable);
    void setUnderline(bool Enable);
    void resetStyles();

    void writeGutter(
      std::size_t GutterWidth,
      std::size_t Line
    );

    void writeHighlight(
      std::size_t GutterWidth,
      TextRange Range,
      Color HighlightColor,
      std::size_t Line,
      std::size_t LineLength
    );

    void writeExcerpt(
      TextFile& File,
      TextRange ToPrint,
      TextRange ToHighlight,
      Color HighlightColor
    );

  public:

    unsigned ExcerptLinesPre = 2;
    unsigned ExcerptLinesPost = 2;
    std::size_t MaxTypeSubsitutionCount = 0;
    bool PrintFilePosition = true;
    bool PrintExcerpts = true;
    bool EnableColors = true;

    void addDiagnostic(const Diagnostic& Diagnostic) override;

    ConsoleDiagnostics(std::ostream& Out = std::cerr);

  };
}
