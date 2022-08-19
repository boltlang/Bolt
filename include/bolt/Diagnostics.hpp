
#pragma once

#include <vector>
#include <stdexcept>

#include "bolt/String.hpp"
#include "bolt/CST.hpp"

namespace bolt {

  class Diagnostic : std::runtime_error {
  public:
    Diagnostic();
  };

  class UnexpectedTokenDiagnostic : public Diagnostic {
  public:

    Token* Actual;
    std::vector<NodeType> Expected;

    inline UnexpectedTokenDiagnostic(Token* Actual, std::vector<NodeType> Expected):
      Actual(Actual), Expected(Expected) {}

  };

  class UnexpectedStringDiagnostic : public Diagnostic {
  public:

    TextLoc Location;
    String Actual;

    inline UnexpectedStringDiagnostic(TextLoc Location, String Actual):
      Location(Location), Actual(Actual) {}

  };

}
