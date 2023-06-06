
#pragma once

#include <utility>
#include <vector>
#include <iostream>

#include "bolt/ByteString.hpp"
#include "bolt/CST.hpp"
#include "bolt/Type.hpp"

namespace bolt {

  class Diagnostic;
  class TypeclassSignature;
  class Type;
  class Node;

  class DiagnosticEngine {

    bool HasError = false;

  public:

    inline bool hasError() const noexcept {
      return HasError;
    }

    virtual void addDiagnostic(Diagnostic* Diagnostic) = 0;

    template<typename D, typename ...Ts>
    void add(Ts&&... Args) {
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

    std::size_t countDiagnostics() const noexcept {
      return Diagnostics.size();
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

  enum StyleFlags : unsigned {
    StyleFlags_None = 0,
    StyleFlags_Bold = 1 << 0,
    StyleFlags_Underline = 1 << 1,
    StyleFlags_Italic = 1 << 2,
  };

  class Style {

    unsigned Flags = StyleFlags_None;

    Color FgColor = Color::None;
    Color BgColor = Color::None;

  public:

    Color getForegroundColor() const noexcept {
      return FgColor;
    }

    Color getBackgroundColor() const noexcept {
      return BgColor;
    }

    void setForegroundColor(Color NewColor) noexcept {
      FgColor = NewColor;
    }

    void setBackgroundColor(Color NewColor) noexcept {
      BgColor = NewColor;
    }

    bool hasForegroundColor() const noexcept {
      return FgColor != Color::None;
    }

    bool hasBackgroundColor() const noexcept {
      return BgColor != Color::None;
    }

    void clearForegroundColor() noexcept {
      FgColor = Color::None;
    }

    void clearBackgroundColor() noexcept {
      BgColor = Color::None;
    }

    bool isUnderline() const noexcept {
      return Flags & StyleFlags_Underline;
    }

    bool isItalic() const noexcept {
      return Flags & StyleFlags_Italic;
    }

    bool isBold() const noexcept {
      return Flags & StyleFlags_Bold;
    }

    void setUnderline(bool Enable) noexcept {
      if (Enable) {
        Flags |= StyleFlags_Underline;
      } else {
        Flags &= ~StyleFlags_Underline;
      }
    }

    void setItalic(bool Enable) noexcept {
      if (Enable) {
        Flags |= StyleFlags_Italic;
      } else {
        Flags &= ~StyleFlags_Italic;
      }
    }

    void setBold(bool Enable) noexcept {
      if (Enable) {
        Flags |= StyleFlags_Bold;
      } else {
        Flags &= ~StyleFlags_Bold;
      }
    }

    void reset() noexcept {
      FgColor = Color::None;
      BgColor = Color::None;
      Flags = 0;
    }

  };

  /**
   * Prints any diagnostic message that was added to it to the console.
   */
  class ConsoleDiagnostics : public DiagnosticEngine {

    std::ostream& Out;

    Style ActiveStyle;

    void setForegroundColor(Color C);
    void setBackgroundColor(Color C);
    void applyStyles();

    void setBold(bool Enable);
    void setItalic(bool Enable);
    void setUnderline(bool Enable);
    void resetStyles();

    void writeGutter(
      std::size_t GutterWidth,
      std::string Text
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
    void writeType(const Type* Ty, const TypePath& Underline);
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
