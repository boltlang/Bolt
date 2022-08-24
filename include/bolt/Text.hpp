
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
    String Text;

    std::vector<size_t> LineOffsets;

  public:

    TextFile(ByteString Path, String Text);

    size_t getLine(size_t Offset);
    size_t getColumn(size_t Offset);
    size_t getStartOffset(size_t Line);

    size_t getLineCount() const;

    ByteString getPath() const;

    String getText() const;

  };

}

