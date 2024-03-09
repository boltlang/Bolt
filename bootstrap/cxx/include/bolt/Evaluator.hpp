
#pragma once

#include <unordered_map>
#include <functional>

#include "bolt/ByteString.hpp"
#include "bolt/CST.hpp"

namespace bolt {

enum class ValueKind {
  Empty,
  String,
  Integer,
  Tuple,
  SourceFunction,
  NativeFunction,
};

class Value {

  using NativeFunction = std::function<Value(std::vector<Value>)>;

  using Tuple = std::vector<Value>;

  ValueKind Kind;

  union {
    ByteString S;
    Integer I;
    FunctionDeclaration* D;
    NativeFunction F;
    Tuple T;
  };

public:

  Value():
    Kind(ValueKind::Empty) {}

  Value(ByteString S):
    Kind(ValueKind::String), S(S) {}

  Value(Integer I):
    Kind(ValueKind::Integer), I(I) {}

  Value(FunctionDeclaration* D):
    Kind(ValueKind::SourceFunction), D(D) {}

  Value(NativeFunction F):
    Kind(ValueKind::NativeFunction), F(F) {}

  Value(std::vector<Value> T):
    Kind(ValueKind::Tuple), T(T) {}

  Value(const Value& V):
    Kind(V.Kind) {
      switch (Kind) {
        case ValueKind::String:
          new (&S) ByteString(V.S);
          break;
        case ValueKind::Integer:
          new (&I) Integer(V.I);
          break;
        case ValueKind::Tuple:
          new (&I) Tuple(V.T);
          break;
        case ValueKind::SourceFunction:
          new (&D) FunctionDeclaration*(V.D);
          break;
        case ValueKind::NativeFunction:
          new (&F) NativeFunction(V.F);
          break;
        case ValueKind::Empty:
          break;
      }
    }

  Value& operator=(const Value& Other) noexcept {
    Kind = Other.Kind;
    switch (Kind) {
      case ValueKind::String:
        new (&S) ByteString(Other.S);
        break;
      case ValueKind::Integer:
        new (&I) Integer(Other.I);
        break;
      case ValueKind::Tuple:
        new (&I) Tuple(Other.T);
        break;
      case ValueKind::SourceFunction:
        new (&D) FunctionDeclaration*(Other.D);
        break;
      case ValueKind::NativeFunction:
        new (&F) NativeFunction(Other.F);
        break;
      case ValueKind::Empty:
        break;
    }
    return *this;
  }

  // Add move constructor and move assignment methods

  inline ValueKind getKind() const noexcept {
    return Kind;
  }

  inline ByteString& asString() {
    ZEN_ASSERT(Kind == ValueKind::String);
    return S;
  }

  inline FunctionDeclaration* getDeclaration() {
    ZEN_ASSERT(Kind == ValueKind::SourceFunction);
    return D;
  }

  inline NativeFunction getBinding() {
    ZEN_ASSERT(Kind == ValueKind::NativeFunction);
    return F;
  }

  static Value binding(NativeFunction F) {
    return Value(F);
  }

  static Value unit() {
    return Value(Tuple {});
  }

  ~Value() {
    switch (Kind) {
      case ValueKind::String:
        S.~ByteString();
        break;
      case ValueKind::Integer:
        I.~Integer();
        break;
      case ValueKind::Tuple:
        T.~Tuple();
        break;
      case ValueKind::SourceFunction:
        break;
      case ValueKind::NativeFunction:
        F.~NativeFunction();
        break;
      case ValueKind::Empty:
        break;
    }
  }

};

class Env {

  std::unordered_map<ByteString, Value> Bindings;

public:

  void add(const ByteString& Name, Value V) {
    Bindings.emplace(Name, V);
  }

  Value& lookup(const ByteString& Name) {
    auto Match = Bindings.find(Name);
    ZEN_ASSERT(Match != Bindings.end());
    return Match->second;
  }

};

class Evaluator {

public:

  void assignPattern(Pattern* P, Value& V, Env& E);

  Value apply(Value Op, std::vector<Value> Args);

  Value evaluateExpression(Expression* N, Env& E);

  void evaluate(Node* N, Env& E);

};

}
