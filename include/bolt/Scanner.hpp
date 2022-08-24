
#pragma once

#include <cstdint>
#include <string>
#include <deque>
#include <stack>

#include "bolt/Text.hpp"
#include "bolt/String.hpp"

namespace bolt {

  class Token;

  template<typename T>
  class Stream {
  public:

    virtual T get() = 0; 
    virtual T peek(std::size_t Offset = 0) = 0;

    virtual ~Stream() {}

  };

  template<typename ContainerT, typename T = typename ContainerT::value_type>
  class VectorStream : public Stream<T> {
  public:

    using value_type = T;

    ContainerT& Data;
    value_type Sentry;
    std::size_t Offset;

    VectorStream(ContainerT& Data, value_type Sentry, std::size_t Offset = 0):
      Data(Data), Sentry(Sentry), Offset(Offset) {}

    value_type get() override {
      return Offset < Data.size() ? Data[Offset++] : Sentry;
    }

    value_type peek(std::size_t Offset2) override {
      auto I = Offset + Offset2;
      return I < Data.size() ? Data[I] : Sentry;
    }

  };

  template<typename T>
  class BufferedStream : public Stream<T> {

    std::deque<T> Buffer;

  protected:

    virtual T read() = 0;

  public:

    using value_type = T;

    value_type get() override {
      if (Buffer.empty()) {
        return read();
      } else {
        auto Keep = Buffer.front();
        Buffer.pop_front();
        return Keep;
      }
    }

    value_type peek(std::size_t Offset = 0) override {
      while (Buffer.size() <= Offset) {
        Buffer.push_back(read());
      }
      return Buffer[Offset];
    }

  };

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
