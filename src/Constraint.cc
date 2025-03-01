
#include "bolt/Constraint.hpp"

namespace bolt {

std::string Constraint::toString() const {
  switch (Kind) {
    case ConstraintKind::TypesEqual:
      return static_cast<const CTypesEqual*>(this)->toString();
  }
}

Constraint* substitute(const Constraint* C, const TVSub& Sub) {
  switch (C->getKind()) {
    case ConstraintKind::TypesEqual:
    {
      auto TE = static_cast<const CTypesEqual*>(C);
      return new CTypesEqual(
        substitute(TE->getLeft(), Sub),
        substitute(TE->getRight(), Sub),
        TE->getOrigin()
      );
    }
  }
}

std::string CTypesEqual::toString() const {
  return A->toString() + " ~ " + B->toString();
}

}
