
#pragma once

#include <cstdint>
#include <string>
#include <deque>
#include <stack>

#include "bolt/String.hpp"
#include "bolt/Stream.hpp"
#include "bolt/CST.hpp"

namespace bolt {

  class Token;

  class Scanner : public BufferedStream<Token*> {

    TextFile& File;

    Stream<Char>& Chars;

    TextLoc CurrLoc;

    inline TextLoc getCurrentLoc() const {
      return CurrLoc;
    }

    inline Char getChar() {
      auto Chr = Chars.get();
      if (Chr == '\n') {
        CurrLoc.Line += 1;
        CurrLoc.Column = 1;
      } else {
        CurrLoc.Column += 1;
      }
      return Chr;
    }

    inline Char peekChar(std::size_t Offset = 0) {
      return Chars.peek(Offset);
    }

  protected:

    Token* read() override;

  public:

    Scanner(TextFile& File, Stream<Char>& Chars);

  };

  enum class FrameType {
    Block,
    LineFold,
  };

  class Punctuator : public BufferedStream<Token*> {

    Stream<Token*>& Tokens;

    std::stack<FrameType> Frames;
    std::stack<TextLoc> Locations;

  protected:

    virtual Token* read() override;

  public:

    Punctuator(Stream<Token*>& Tokens);

  };

}
