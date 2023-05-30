
#pragma once

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

  };

}
