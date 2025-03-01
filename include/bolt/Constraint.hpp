
#pragma once

#include "bolt/Type.hpp"

namespace bolt {

class Node;

enum class ConstraintKind {
  TypesEqual,
};

class Constraint {

  ConstraintKind Kind;

protected:

  Constraint(ConstraintKind Kind):
    Kind(Kind) {}

public:

  inline ConstraintKind getKind() const {
    return Kind;
  }

  std::string toString() const;

};

class CTypesEqual : public Constraint {

  Type* A;
  Type* B;
  Node* Origin;

public:

  CTypesEqual(Type* A, Type* B, Node* Origin):
    Constraint(ConstraintKind::TypesEqual), A(A), B(B), Origin(Origin) {}

  Type* getLeft() const {
    return A;
  }

  Type* getRight() const {
    return B;
  }

  Node* getOrigin() const {
    return Origin;
  }

  std::string toString() const;

};

}
