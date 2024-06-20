
#pragma once

#include <cstdlib>
#include <concepts>
#include <string>
#include <utility>

#include "bolt/Common.hpp"

namespace bolt {

template<typename T>
concept ErrorLike = requires (T a) {
  { message(a) } -> std::convertible_to<std::string>;
};

template<typename T>
struct Left {
  T value;
};

template<typename T>
struct Right {
  T value;
};

template<typename L, typename R>
class Either {

  bool _is_left;

  union {
    L _left;
    R _right;
  };

public:

  template<typename L1>
  Either(const Left<L1>& left):
    _is_left(true), _left(left.value) {}

  template<typename R1>
  Either(const Right<R1>& right):
    _is_left(false), _right(right.value) {}

  template<typename L1>
  Either(Left<L1>&& left):
    _is_left(true), _left(std::move(left.value)) {}

  template<typename R2>
  Either(Right<R2>&& right):
    _is_left(false), _right(std::move(right.value)) {}

  Either(const Either& other):
    _is_left(_is_left) {
      if (other._is_left) {
        new (&_left)L(other._left);
      } else {
        new (&_right)L(other._right);
      }
    }

  Either(Either&& other):
    _is_left(std::move(other._is_left)) {
      if (_is_left) {
        new (&_left)L(std::move(other._left));
      } else {
        new (&_right)L(std::move(other._right));
      }
    }

  bool is_left() const {
    return _is_left;
  }

  auto left() const {
    return _left;
  }

  auto right() const {
    return _right;
  }

  R&& unwrap() requires ErrorLike<L> {
    if (_is_left) {
      auto desc = message(_left);
      ZEN_PANIC("trying to unwrap a result containing an error: %s", desc.c_str());
    }
    return std::move(_right);
  }

  ~Either() {
    if (_is_left) {
      _left.~L();
    } else {
      _right.~R();
    }
  }

};

// template<typename L>
// auto left(const L& value) {
//   return Left<L> { value };
// }

template<typename L>
auto left(L&& value) {
  return Left<L> { std::move(value) };
}

// template<typename R>
// auto right(const R& value) {
//   return Right<R> { value };
// }

template<typename R>
auto right(R&& value) {
  return Right<R> { std::move(value) };
}

}
