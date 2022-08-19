#ifndef BOLT_TEXT_HPP
#define BOLT_TEXT_HPP

#include <stddef.h>

#include <string>

namespace bolt {

  class TextLoc {
  public:

    size_t Line = 1;
    size_t Column = 1;

    void advance(const std::string& Text) {
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

}

#endif // of #ifndef BOLT_TEXT_HPP
