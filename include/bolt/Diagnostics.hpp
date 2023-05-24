
#pragma once

#include <vector>
#include <stdexcept>
#include <memory>
#include <iostream>

#include "bolt/ByteString.hpp"
#include "bolt/String.hpp"
#include "bolt/CST.hpp"
#include "bolt/Type.hpp"

namespace bolt {

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

    virtual Node* getNode() const {
      return nullptr;
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

    inline Node* getNode() const override {
      return Initiator;
    }

  };

  class UnificationErrorDiagnostic : public Diagnostic {
  public:

    Type* Left;
    Type* Right;
    TypePath LeftPath;
    TypePath RightPath;
    Node* Source;

    inline UnificationErrorDiagnostic(Type* Left, Type* Right, TypePath LeftPath, TypePath RightPath, Node* Source):
      Diagnostic(DiagnosticKind::UnificationError), Left(Left), Right(Right), LeftPath(LeftPath), RightPath(RightPath), Source(Source) {}

    inline Node* getNode() const override {
      return Source;
    }

  };

  class TypeclassMissingDiagnostic : public Diagnostic {
  public:

    TypeclassSignature Sig;
    LetDeclaration* Decl;

    inline TypeclassMissingDiagnostic(TypeclassSignature Sig, LetDeclaration* Decl):
      Diagnostic(DiagnosticKind::TypeclassMissing), Sig(Sig), Decl(Decl) {}

    inline Node* getNode() const override {
      return Decl;
    }

  };

  class InstanceNotFoundDiagnostic : public Diagnostic {
  public:

    ByteString TypeclassName;
    TCon* Ty;
    Node* Source;

    inline InstanceNotFoundDiagnostic(ByteString TypeclassName, TCon* Ty, Node* Source):
      Diagnostic(DiagnosticKind::InstanceNotFound), TypeclassName(TypeclassName), Ty(Ty), Source(Source) {}

    inline Node* getNode() const override {
      return Source;
    }

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
    std::vector<TypeclassId> Classes;
    Node* Source;

    inline InvalidTypeToTypeclassDiagnostic(Type* Actual, std::vector<TypeclassId> Classes, Node* Source):
      Diagnostic(DiagnosticKind::InvalidTypeToTypeclass), Actual(Actual), Classes(Classes), Source(Source) {}

    inline Node* getNode() const override {
      return Source;
    }

  };

  class DiagnosticEngine {
  public:

    virtual void addDiagnostic(Diagnostic* Diagnostic) = 0;

    template<typename D, typename ...Ts>
    void add(Ts&&... Args) {
      addDiagnostic(new D { std::forward<Ts>(Args)... });
    }

    virtual ~DiagnosticEngine() {}

  };

  /**
   * Keeps diagnostics alive in-memory until a seperate procedure processes them.
   */
  class DiagnosticStore : public DiagnosticEngine {
  public:

    std::vector<Diagnostic*> Diagnostics;

    void addDiagnostic(Diagnostic* Diagnostic) {
      Diagnostics.push_back(Diagnostic);
    }

    void clear() {
      Diagnostics.clear();
    }

    ~DiagnosticStore() {
      for (auto D: Diagnostics) {
        delete D;
      }
    }

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
      const TextFile& File,
      TextRange ToPrint,
      TextRange ToHighlight,
      Color HighlightColor
    );

    void writeNode(const Node* N);

    void writePrefix(const Diagnostic& D);
    void writeBinding(const ByteString& Name);
    void writeType(std::size_t I);
    void writeType(const Type* Ty);
    void writeLoc(const TextFile& File, const TextLoc& Loc);
    void writeTypeclassName(const ByteString& Name);
    void writeTypeclassSignature(const TypeclassSignature& Sig);

    void write(const std::string_view& S);
    void write(std::size_t N);

  public:

    unsigned ExcerptLinesPre = 2;
    unsigned ExcerptLinesPost = 2;
    std::size_t MaxTypeSubsitutionCount = 0;
    bool PrintFilePosition = true;
    bool PrintExcerpts = true;
    bool EnableColors = true;

    ConsoleDiagnostics(std::ostream& Out = std::cerr);

    void addDiagnostic(Diagnostic* Diagnostic) override;

  };

}
