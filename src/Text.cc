
#include "zen/config.hpp"

#include "bolt/Text.hpp"
#include "bolt/ByteString.hpp"

namespace bolt {

  TextFile::TextFile(ByteString Path, ByteString Text):
    Path(Path), Text(Text) {
      LineOffsets.push_back(0);
      for (size_t I = 0; I < Text.size(); I++) {
        auto Chr = Text[I];
        if (Chr == '\n') {
          LineOffsets.push_back(I+1);
        }
      }
      LineOffsets.push_back(Text.size());
    }

  size_t TextFile::getStartOffset(size_t Line) {
    return LineOffsets[Line-1];
  }

  size_t TextFile::getLine(size_t Offset) {
    ZEN_ASSERT(Offset < Text.size());
    for (size_t I = 0; I < LineOffsets.size(); ++I) {
      if (LineOffsets[I] > Offset) {
        return I;
      }
    }
    ZEN_UNREACHABLE
  }

  size_t TextFile::getColumn(size_t Offset) {
    auto Line = getLine(Offset);
    auto StartOffset = getStartOffset(Line);
    return Offset - StartOffset + 1 ;
  }

  ByteString TextFile::getPath() const {
    return Path;
  }

  ByteString TextFile::getText() const {
    return Text;
  }

}
