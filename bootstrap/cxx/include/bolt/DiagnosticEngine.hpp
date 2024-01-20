
#pragma once

#include <utility>
#include <vector>
#include <iostream>

namespace bolt {

  class ConsolePrinter;
  class Diagnostic;
  class TypeclassSignature;
  class Type;
  class Node;

  class DiagnosticEngine {
  protected:

    bool HasError = false;

    virtual void addDiagnostic(Diagnostic* Diagnostic) = 0;

  public:

    bool FailOnError = false;

    inline bool hasError() const noexcept {
      return HasError;
    }

    template<typename D, typename ...Ts>
    void add(Ts&&... Args) {
      // if (FailOnError) {
      //   ZEN_PANIC("An error diagnostic caused the program to abort.");
      // }
      HasError = true;
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

    void sort();

    std::size_t countDiagnostics() const noexcept {
      return Diagnostics.size();
    }

    ~DiagnosticStore();

  };

  class ConsoleDiagnostics : public DiagnosticEngine {

    ConsolePrinter& ThePrinter;

  protected:

    void addDiagnostic(Diagnostic* Diagnostic) override;

  public:

    ConsoleDiagnostics(ConsolePrinter& ThePrinter);

  };

}
