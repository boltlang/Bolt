
#pragma once

#include <vector>
#include <memory>

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
    TupleIndexOutOfRange,
    InvalidTypeToTypeclass,
    FieldNotFound,
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

    virtual unsigned getCode() const noexcept = 0;

  };

  class UnexpectedStringDiagnostic : public Diagnostic {
  public:

    TextFile& File;
    TextLoc Location;
    String Actual;

    inline UnexpectedStringDiagnostic(TextFile& File, TextLoc Location, String Actual):
      Diagnostic(DiagnosticKind::UnexpectedString), File(File), Location(Location), Actual(Actual) {}

    unsigned getCode() const noexcept override {
      return 1001;
    }

  };

  class UnexpectedTokenDiagnostic : public Diagnostic {
  public:

    TextFile& File;
    Token* Actual;
    std::vector<NodeKind> Expected;

    inline UnexpectedTokenDiagnostic(TextFile& File, Token* Actual, std::vector<NodeKind> Expected):
      Diagnostic(DiagnosticKind::UnexpectedToken), File(File), Actual(Actual), Expected(Expected) {}

    unsigned getCode() const noexcept override {
      return 1101;
    }

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

    unsigned getCode() const noexcept override {
      return 2005;
    }

  };

  class UnificationErrorDiagnostic : public Diagnostic {
  public:

    Type* OrigLeft;
    Type* OrigRight;
    TypePath LeftPath;
    TypePath RightPath;
    Node* Source;

    inline UnificationErrorDiagnostic(Type* OrigLeft, Type* OrigRight, TypePath LeftPath, TypePath RightPath, Node* Source):
      Diagnostic(DiagnosticKind::UnificationError), OrigLeft(OrigLeft), OrigRight(OrigRight), LeftPath(LeftPath), RightPath(RightPath), Source(Source) {}

    inline Type* getLeft() const {
      return OrigLeft->resolve(LeftPath);
    }

    inline Type* getRight() const {
      return OrigRight->resolve(RightPath);
    }

    inline Node* getNode() const override {
      return Source;
    }

    unsigned getCode() const noexcept override {
      return 2010;
    }

  };

  class TypeclassMissingDiagnostic : public Diagnostic {
  public:

    TypeclassSignature Sig;
    Node* Decl;

    inline TypeclassMissingDiagnostic(TypeclassSignature Sig, Node* Decl):
      Diagnostic(DiagnosticKind::TypeclassMissing), Sig(Sig), Decl(Decl) {}

    inline Node* getNode() const override {
      return Decl;
    }

    unsigned getCode() const noexcept override {
      return 2201;
    }

  };

  class InstanceNotFoundDiagnostic : public Diagnostic {
  public:

    ByteString TypeclassName;
    Type* Ty;
    Node* Source;

    inline InstanceNotFoundDiagnostic(ByteString TypeclassName, Type* Ty, Node* Source):
      Diagnostic(DiagnosticKind::InstanceNotFound), TypeclassName(TypeclassName), Ty(Ty), Source(Source) {}

    inline Node* getNode() const override {
      return Source;
    }

    unsigned getCode() const noexcept override {
      return 2251;
    }

  };

  class TupleIndexOutOfRangeDiagnostic : public Diagnostic {
  public:

    TTuple* Tuple;
    std::size_t I;

    inline TupleIndexOutOfRangeDiagnostic(TTuple* Tuple, std::size_t I):
      Diagnostic(DiagnosticKind::TupleIndexOutOfRange), Tuple(Tuple), I(I) {}

    unsigned getCode() const noexcept override {
      return 2015;
    }

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

    unsigned getCode() const noexcept override {
      return 2060;
    }

  };

  class FieldNotFoundDiagnostic : public Diagnostic {
  public:

    ByteString Name;
    Type* Ty;
    TypePath Path;
    Node* Source;

    inline FieldNotFoundDiagnostic(ByteString Name, Type* Ty, TypePath Path, Node* Source):
      Diagnostic(DiagnosticKind::FieldNotFound), Name(Name), Ty(Ty), Path(Path), Source(Source) {}

    unsigned getCode() const noexcept override {
      return 2017;
    }

  };

}
