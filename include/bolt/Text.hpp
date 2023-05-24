
#pragma once

#include <stddef.h>

#include <vector>
#include <string>

#include "bolt/ByteString.hpp"
#include "bolt/String.hpp"

namespace bolt {

  class TextLoc {
  public:

    size_t Line = 1;
    size_t Column = 1;

    inline void advance(const String& Text) {
      for (auto Chr: Text) {
        if (Chr == '\n') {
          Line++;
          Column = 1;
        } else {
          Column++;
        }
      }
    }

    inline TextLoc operator+(const String& Text) const {
      TextLoc Out { Line, Column };
      Out.advance(Text);
      return Out;
    }

  };

  struct TextRange {
    TextLoc Start;
    TextLoc End;
  };

  class TextFile {

    ByteString Path;
    ByteString Text;

    std::vector<size_t> LineOffsets;

  public:

    TextFile(ByteString Path, ByteString Text);

    size_t getLine(size_t Offset) const;
    size_t getColumn(size_t Offset) const;
    size_t getStartOffset(size_t Line) const;

    size_t getLineCount() const;

    ByteString getPath() const;

    ByteString getText() const;

  };

}

