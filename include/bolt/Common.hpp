
#pragma once

#include <concepts>
#include <cstdlib>

#include "zen/config.hpp"

namespace bolt {

class LanguageConfig {

  enum ConfigFlags {
    ConfigFlags_TypeVarsRequireForall = 1 << 0,
  };

  unsigned Flags = 0;

public:

  void setTypeVarsRequireForall(bool Enable) {
    if (Enable) {
      Flags |= ConfigFlags_TypeVarsRequireForall;
    } else {
      Flags |= ~ConfigFlags_TypeVarsRequireForall;
    }
  }

  bool typeVarsRequireForall() const noexcept {
    return Flags & ConfigFlags_TypeVarsRequireForall;
  }

  bool hasImmediateDiagnostics() const noexcept {
    // TODO make this a configuration flag
    return true;
  }

};

template<typename T>
concept HoldsKind = requires (T a) {
  { a.getKind() } -> std::convertible_to<decltype(T::Kind)>;
};

template<typename D, typename T>
bool isa(const T* value) {
  ZEN_ASSERT(value != nullptr);
  return D::classof(value);
}

template<HoldsKind D, typename T>
bool isa(const T* value) {
  ZEN_ASSERT(value != nullptr);
  return D::Kind == value->getKind();
}

template<typename D, typename B>
D* cast(B* base) {
  ZEN_ASSERT(isa<D>(base));
  return static_cast<D*>(base);
}

template<typename D, typename B>
const D* cast(const B* base) {
  ZEN_ASSERT(isa<D>(base));
  return static_cast<const D*>(base);
}

}
