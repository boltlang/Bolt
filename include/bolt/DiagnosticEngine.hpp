
#pragma once

#include <utility>
#include <vector>
#include <iostream>

#include "bolt/ByteString.hpp"

namespace bolt {

  class Diagnostic;
  class TypeclassSignature;
  class Type;
  class Node;

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

    ~DiagnosticStore();

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

  /**
   * Prints any diagnostic message that was added to it to the console.
   */
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

    /**
     * Assumes the diagnostic is to be owned by this ConsoleDiagnostics.
     */
    void addDiagnostic(Diagnostic* Diagnostic);

    void printDiagnostic(const Diagnostic& D);

  };

}
