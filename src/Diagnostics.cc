
#include "bolt/Diagnostics.hpp"

namespace bolt {

  Diagnostic::Diagnostic():
    std::runtime_error("a compiler error occurred without being caught") {}

}
