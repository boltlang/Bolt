
#pragma once

#include <cwchar>
#include <vector>

#include "bolt/ByteString.hpp"
#include "bolt/String.hpp"
#include "bolt/CST.hpp"
#include "bolt/Type.hpp"

namespace bolt {

enum class DiagnosticKind : unsigned char {
  BindingNotFound,
  // FieldNotFound,
  // InstanceNotFound,
  // InvalidTypeToTypeclass,
  // NotATuple,
  // TupleIndexOutOfRange,
  // TypeclassMissing,
  UnexpectedString,
  UnexpectedToken,
  TypeMismatchError,
};

class Diagnostic {

  const DiagnosticKind Kind;

protected:

  Diagnostic(DiagnosticKind Kind);

public:

  inline DiagnosticKind getKind() const {
    return Kind;
  }

  virtual Node* getNode() const {
    return nullptr;
  }

  virtual unsigned getCode() const = 0;

  virtual ~Diagnostic() {}

};

class UnexpectedStringDiagnostic : public Diagnostic {
public:

  TextFile& File;
  TextLoc Location;
  String Actual;

  inline UnexpectedStringDiagnostic(TextFile& File, TextLoc Location, String Actual):
    Diagnostic(DiagnosticKind::UnexpectedString), File(File), Location(Location), Actual(Actual) {}

  unsigned getCode() const override {
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

  unsigned getCode() const override {
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

  unsigned getCode() const override {
    return 2005;
  }

};

class TypeMismatchError : public Diagnostic {
public:

  Type* Left;
  Type* Right;
  Node* N;

  inline TypeMismatchError(Type* Left, Type* Right, Node* N):
    Diagnostic(DiagnosticKind::TypeMismatchError), Left(Left), Right(Right), N(N) {}

  inline Node* getNode() const override {
    return N;
  }

  unsigned getCode() const override {
    return 3001;
  }

};

}
