
#include "bolt/Constraint.hpp"

namespace bolt {

std::string Constraint::toString() const {
  switch (Kind) {
    case ConstraintKind::TypesEqual:
      return static_cast<const CTypesEqual*>(this)->toString();
  }
}

std::string CTypesEqual::toString() const {
  return A->toString() + " ~ " + B->toString();
}

}
