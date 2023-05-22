
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
  class TCon;
  class TVar;
  class TTuple;

  using TypeclassId = ByteString;

  struct TypeclassSignature {

    using TypeclassId = ByteString;
    TypeclassId Id;
    std::vector<TVar*> Params;

    bool operator<(const TypeclassSignature& Other) const;
    bool operator==(const TypeclassSignature& Other) const;

  };

  enum class DiagnosticKind : unsigned char {
    UnexpectedToken,
    UnexpectedString,
    BindingNotFound,
    UnificationError,
    TypeclassMissing,
    InstanceNotFound,
    ClassNotFound,
    TupleIndexOutOfRange,
    InvalidTypeToTypeclass,
  };

  class Diagnostic : std::runtime_error {

    const DiagnosticKind Kind;

  protected:

    Diagnostic(DiagnosticKind Kind);
  
  public:

    inline DiagnosticKind getKind() const noexcept {
      return Kind;
    }

  };

  class UnexpectedTokenDiagnostic : public Diagnostic {
  public:

    TextFile& File;
    Token* Actual;
    std::vector<NodeKind> Expected;

    inline UnexpectedTokenDiagnostic(TextFile& File, Token* Actual, std::vector<NodeKind> Expected):
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

  class TypeclassMissingDiagnostic : public Diagnostic {
  public:

    TypeclassSignature Sig;
    LetDeclaration* Decl;

    inline TypeclassMissingDiagnostic(TypeclassSignature Sig, LetDeclaration* Decl):
      Diagnostic(DiagnosticKind::TypeclassMissing), Sig(Sig), Decl(Decl) {}

  };

  class InstanceNotFoundDiagnostic : public Diagnostic {
  public:

    ByteString TypeclassName;
    TCon* Ty;
    Node* Source;

    inline InstanceNotFoundDiagnostic(ByteString TypeclassName, TCon* Ty, Node* Source):
      Diagnostic(DiagnosticKind::InstanceNotFound), TypeclassName(TypeclassName), Ty(Ty), Source(Source) {}

  };

  class ClassNotFoundDiagnostic : public Diagnostic {
  public:

    ByteString Name;

    inline ClassNotFoundDiagnostic(ByteString Name):
      Diagnostic(DiagnosticKind::ClassNotFound), Name(Name) {}

  };

  class TupleIndexOutOfRangeDiagnostic : public Diagnostic {
  public:

    TTuple* Tuple;
    std::size_t I;

    inline TupleIndexOutOfRangeDiagnostic(TTuple* Tuple, std::size_t I):
      Diagnostic(DiagnosticKind::TupleIndexOutOfRange), Tuple(Tuple), I(I) {}

  };

  class InvalidTypeToTypeclassDiagnostic : public Diagnostic {
  public:

    Type* Actual;

    inline InvalidTypeToTypeclassDiagnostic(Type* Actual):
      Diagnostic(DiagnosticKind::InvalidTypeToTypeclass) {}

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
