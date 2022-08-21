
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

    Token* Actual;
    std::vector<NodeType> Expected;

    inline UnexpectedTokenDiagnostic(Token* Actual, std::vector<NodeType> Expected):
      Diagnostic(DiagnosticKind::UnexpectedToken), Actual(Actual), Expected(Expected) {}

  };

  class UnexpectedStringDiagnostic : public Diagnostic {
  public:

    TextLoc Location;
    String Actual;

    inline UnexpectedStringDiagnostic(TextLoc Location, String Actual):
      Diagnostic(DiagnosticKind::UnexpectedString), Location(Location), Actual(Actual) {}

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

    inline UnificationErrorDiagnostic(Type* Left, Type* Right):
      Diagnostic(DiagnosticKind::UnificationError), Left(Left), Right(Right) {}

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

  class ConsoleDiagnostics : public DiagnosticEngine {

    std::ostream& Out;

  public:

    void addDiagnostic(const Diagnostic& Diagnostic) override;

    ConsoleDiagnostics(std::ostream& Out = std::cerr);

  };
}
