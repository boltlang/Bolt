
#pragma once

#include <stddef.h>

#include <vector>
#include <string>

#include "bolt/ByteString.hpp"

namespace bolt {

  class TextLoc {
  public:

    size_t Line = 1;
    size_t Column = 1;

    inline void advance(const std::string& Text) {
      for (auto Chr: Text) {
        if (Chr == '\n') {
          Line++;
          Column = 1;
        } else {
          Column++;
        }
      }
    }

  };

  class TextRange {
  public:
    TextLoc Start;
    TextLoc End;
  };

  class TextFile {

    ByteString Path;
    ByteString Text;

    std::vector<size_t> LineOffsets;

  public:

    TextFile(ByteString Path, ByteString Text);

    size_t getLine(size_t Offset);
    size_t getColumn(size_t Offset);
    size_t getStartOffset(size_t Line);

    ByteString getPath() const;

    ByteString getText() const;

  };

}

